use std::{
    collections::HashSet,
    path::{Component, Path as FsPath, PathBuf},
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use uuid::Uuid;

use crate::codex_app_server::{CodexThreadRequest, CodexTurnRequest, CodexTurnResponse};
use crate::execution_queue::{ExecutionJob, ExecutionJobRequest, ExecutionJobStatus};
use crate::remote_computer_runner::{
    RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    remote_computer_runner_for_config,
};
use crate::shell_runner::{shell_command, shell_runner};
use crate::{
    AppError, AppState, Approval, Artifact, CreateRemoteComputerJobAssignment,
    CreateRemoteComputerLease, Environment, RemoteComputer, RemoteComputerJobAssignment,
    RemoteComputerLease, ToolCall, new_audit_log, record_remote_computer_job_assignment_event,
};

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const REMOTE_CODEX_FINAL_BEGIN: &str = "__MANDOFORGE_CODEX_FINAL_BEGIN__";
const REMOTE_CODEX_FINAL_END: &str = "__MANDOFORGE_CODEX_FINAL_END__";

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexRequest {
    task: String,
    #[serde(default = "default_sandbox")]
    sandbox_mode: String,
    #[serde(default)]
    execution_strategy: Option<String>,
    #[serde(default)]
    poll_attempts: Option<u32>,
    #[serde(default)]
    poll_interval_ms: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AgentCliRequest {
    profile: String,
    task: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[allow(dead_code)]
fn default_sandbox() -> String {
    "workspace-write".to_string()
}

#[async_trait]
pub(crate) trait ExecutionWorker: Send + Sync {
    fn mode(&self) -> &'static str;

    async fn execute_approved_tool(
        &self,
        state: &AppState,
        approval: &Approval,
    ) -> Result<ExecutionWorkerOutcome, AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionWorkerOutcome {
    Completed,
    Queued,
}

pub(crate) struct InlineExecutionWorker;

#[async_trait]
impl ExecutionWorker for InlineExecutionWorker {
    fn mode(&self) -> &'static str {
        "inline"
    }

    async fn execute_approved_tool(
        &self,
        state: &AppState,
        approval: &Approval,
    ) -> Result<ExecutionWorkerOutcome, AppError> {
        let Some(job) = enqueue_approved_job(state, approval).await? else {
            return Ok(ExecutionWorkerOutcome::Completed);
        };
        run_execution_job(state, job.id, "inline").await?;
        Ok(ExecutionWorkerOutcome::Completed)
    }
}

pub(crate) struct QueueBackedExecutionWorker;

#[async_trait]
impl ExecutionWorker for QueueBackedExecutionWorker {
    fn mode(&self) -> &'static str {
        "queue"
    }

    async fn execute_approved_tool(
        &self,
        state: &AppState,
        approval: &Approval,
    ) -> Result<ExecutionWorkerOutcome, AppError> {
        let Some(job) = enqueue_approved_job(state, approval).await? else {
            return Ok(ExecutionWorkerOutcome::Completed);
        };
        state
            .append_event(
                "system",
                Some(job.id),
                approval.session_id,
                "execution.queued",
                json!({"execution_job_id": job.id, "approval_id": approval.id, "tool_call_id": job.tool_call_id, "tool": job.tool_name}),
            )
            .await?;
        Ok(ExecutionWorkerOutcome::Queued)
    }
}

async fn enqueue_approved_job(
    state: &AppState,
    approval: &Approval,
) -> Result<Option<ExecutionJob>, AppError> {
    let Some(tool_call_id) = approval.tool_call_id else {
        return Ok(None);
    };
    let tool_call = state.get_tool_call(tool_call_id).await?;
    Ok(Some(
        state
            .execution_queue
            .enqueue(ExecutionJobRequest {
                session_id: approval.session_id,
                approval_id: approval.id,
                tool_call_id,
                tool_name: tool_call.tool_name,
                max_attempts: None,
            })
            .await?,
    ))
}

pub(crate) async fn run_execution_job(
    state: &AppState,
    job_id: Uuid,
    worker_id: &str,
) -> Result<ExecutionJob, AppError> {
    let job = state.execution_queue.start(job_id, worker_id).await?;
    let remote_environment_contract =
        match remote_computer_environment_contract_for_job(state, &job).await {
            Ok(contract) => contract,
            Err(error) => {
                let error_message = error.message.clone();
                state
                    .append_event(
                        "worker",
                        Some(job.id),
                        job.session_id,
                        "environment.remote_computer_contract_invalid",
                        json!({
                            "execution_job_id": job.id,
                            "approval_id": job.approval_id,
                            "tool_call_id": job.tool_call_id,
                            "tool": job.tool_name,
                            "error": error_message
                        }),
                    )
                    .await?;
                return retry_or_fail_started_execution_job(
                    state,
                    &job,
                    None,
                    error,
                    json!({"stage": "environment_contract"}),
                )
                .await;
            }
        };
    let approval = match state.get_approval(job.approval_id).await {
        Ok(approval) => approval,
        Err(error) => {
            return retry_or_fail_started_execution_job(
                state,
                &job,
                None,
                error,
                json!({"stage": "approval_load"}),
            )
            .await;
        }
    };
    if approval.status != "approved" {
        return retry_or_fail_started_execution_job(
            state,
            &job,
            None,
            AppError::bad_request("execution job approval is not approved"),
            json!({"stage": "approval_status", "approval_status": approval.status}),
        )
        .await;
    }
    let tool_call = match state.get_tool_call(job.tool_call_id).await {
        Ok(tool_call) => tool_call,
        Err(error) => {
            return retry_or_fail_started_execution_job(
                state,
                &job,
                None,
                error,
                json!({"stage": "tool_call_load"}),
            )
            .await;
        }
    };
    let active_assignment = match active_remote_computer_assignment_for_job(state, &job).await {
        Ok(assignment) => assignment,
        Err(error) => {
            return retry_or_fail_started_execution_job(
                state,
                &job,
                None,
                error,
                json!({"stage": "remote_computer_assignment_load"}),
            )
            .await;
        }
    };
    let remote_computer_assignment = match active_assignment {
        Some(assignment) => {
            if let Err(error) = validate_active_remote_computer_assignment_for_job(
                state,
                &job,
                &assignment,
                remote_environment_contract.as_ref(),
            )
            .await
            {
                return retry_or_fail_started_execution_job(
                    state,
                    &job,
                    Some(&assignment),
                    error,
                    json!({
                        "stage": "remote_computer_assignment_validation",
                        "environment_id": remote_environment_contract
                            .as_ref()
                            .map(|contract| contract.environment_id),
                        "environment_contract": remote_environment_contract
                            .as_ref()
                            .map(RemoteComputerEnvironmentContract::evidence)
                    }),
                )
                .await;
            }
            Some(assignment)
        }
        None => {
            match auto_assign_remote_computer_for_job(
                state,
                &job,
                worker_id,
                remote_environment_contract.as_ref(),
            )
            .await
            {
                Ok(assignment) => assignment,
                Err(error) => {
                    return retry_or_fail_started_execution_job(
                        state,
                        &job,
                        None,
                        error,
                        json!({
                            "stage": "remote_computer_auto_assignment",
                            "environment_id": remote_environment_contract
                                .as_ref()
                                .map(|contract| contract.environment_id),
                            "environment_contract": remote_environment_contract
                                .as_ref()
                                .map(RemoteComputerEnvironmentContract::evidence)
                        }),
                    )
                    .await;
                }
            }
        }
    };
    if remote_environment_contract.is_some() && remote_computer_assignment.is_none() {
        let error = AppError::bad_request(
            "remote_computer environment has no claimable Remote Computer lease or warm-pool resource",
        );
        return retry_or_fail_started_execution_job(
            state,
            &job,
            None,
            error,
            json!({
                "stage": "remote_computer_assignment_missing",
                "environment_id": remote_environment_contract
                    .as_ref()
                    .map(|contract| contract.environment_id),
                "environment_contract": remote_environment_contract
                    .as_ref()
                    .map(RemoteComputerEnvironmentContract::evidence)
            }),
        )
        .await;
    }
    if let Some(assignment) = remote_computer_assignment.as_ref() {
        if let Err(error) = record_remote_computer_execution_handoff_acknowledged(
            state, &job, worker_id, assignment,
        )
        .await
        {
            return retry_or_fail_started_execution_job(
                state,
                &job,
                Some(assignment),
                error,
                json!({"stage": "remote_computer_handoff_acknowledge"}),
            )
            .await;
        }
        if let Err(error) =
            append_remote_computer_execution_transport_plan(state, &job, worker_id, assignment)
                .await
        {
            return retry_or_fail_started_execution_job(
                state,
                &job,
                Some(assignment),
                error,
                json!({"stage": "remote_computer_execution_transport_plan"}),
            )
            .await;
        }
    }
    if remote_environment_contract.is_some() && !remote_computer_pod_execution_requested() {
        let error = AppError::bad_request(
            "remote_computer environment requires enabled Remote Computer execution transport",
        );
        return retry_or_fail_started_execution_job(
            state,
            &job,
            remote_computer_assignment.as_ref(),
            error,
            json!({
                "stage": "remote_computer_execution_transport_disabled",
                "environment_id": remote_environment_contract
                    .as_ref()
                    .map(|contract| contract.environment_id),
                "environment_contract": remote_environment_contract
                    .as_ref()
                    .map(RemoteComputerEnvironmentContract::evidence)
            }),
        )
        .await;
    }
    let result = match tool_call.tool_name.as_str() {
        "file.write"
            if remote_computer_assignment.is_some()
                && remote_computer_pod_execution_requested() =>
        {
            execute_approved_remote_computer_file_write(
                state,
                &approval,
                &tool_call,
                remote_computer_assignment
                    .as_ref()
                    .expect("checked assignment"),
            )
            .await
        }
        "file.write" => execute_approved_file_write(state, &approval, &tool_call).await,
        "shell.exec"
            if remote_computer_assignment.is_some()
                && remote_computer_pod_execution_requested() =>
        {
            execute_approved_remote_computer_shell(
                state,
                &approval,
                &tool_call,
                remote_computer_assignment
                    .as_ref()
                    .expect("checked assignment"),
            )
            .await
        }
        "shell.exec" => execute_approved_shell(state, &approval, &tool_call).await,
        "codex.exec"
            if remote_computer_assignment.is_some()
                && remote_computer_pod_execution_requested() =>
        {
            execute_approved_remote_computer_codex(
                state,
                &approval,
                &tool_call,
                remote_computer_assignment
                    .as_ref()
                    .expect("checked assignment"),
            )
            .await
        }
        "codex.exec" => execute_approved_codex(state, &approval, &tool_call).await,
        "agent_cli.exec"
            if remote_computer_assignment.is_some()
                && remote_computer_pod_execution_requested() =>
        {
            execute_approved_remote_computer_agent_cli(
                state,
                &approval,
                &tool_call,
                remote_computer_assignment
                    .as_ref()
                    .expect("checked assignment"),
            )
            .await
        }
        "agent_cli.exec" => execute_approved_agent_cli(state, &approval, &tool_call).await,
        _ => {
            state
                .update_tool_call_status(
                    tool_call.id,
                    "completed",
                    Some(json!({"approval": "approved"})),
                    None,
                )
                .await?;
            Ok(())
        }
    };
    if result.is_ok() {
        finalize_remote_computer_assignment_for_job(
            state,
            &job,
            remote_computer_assignment.as_ref(),
            "completed",
            "remote_computer.execution_handoff_completed",
            json!({"execution_job_status": "completed"}),
        )
        .await?;
        state.execution_queue.complete(job.id).await
    } else {
        let error = result.expect_err("checked error");
        retry_or_fail_started_execution_job(
            state,
            &job,
            remote_computer_assignment.as_ref(),
            error,
            json!({"stage": "tool_execution"}),
        )
        .await
    }
}

async fn retry_or_fail_started_execution_job(
    state: &AppState,
    job: &ExecutionJob,
    assignment: Option<&RemoteComputerJobAssignment>,
    error: AppError,
    details: Value,
) -> Result<ExecutionJob, AppError> {
    let error_message = error.message.clone();
    let updated = state
        .execution_queue
        .retry_or_fail(job.id, &error_message)
        .await?;
    let queued = updated.status == ExecutionJobStatus::Queued;
    let assignment_status = if queued { "released" } else { "failed" };
    let assignment_event = if queued {
        "remote_computer.execution_handoff_released"
    } else {
        "remote_computer.execution_handoff_failed"
    };
    finalize_remote_computer_assignment_for_job(
        state,
        job,
        assignment,
        assignment_status,
        assignment_event,
        merge_json_object(
            json!({
                "execution_job_status": updated.status.clone(),
                "attempt_count": updated.attempt_count,
                "max_attempts": updated.max_attempts,
                "last_error": updated.last_error.clone(),
            }),
            details.clone(),
        ),
    )
    .await?;
    state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            if queued {
                "execution.retry_queued"
            } else {
                "execution.failed"
            },
            merge_json_object(
                json!({
                    "execution_job_id": job.id,
                    "approval_id": job.approval_id,
                    "tool_call_id": job.tool_call_id,
                    "tool": job.tool_name,
                    "assignment_id": assignment.map(|assignment| assignment.id),
                    "remote_computer_id": assignment.map(|assignment| assignment.remote_computer_id),
                    "lease_id": assignment.map(|assignment| assignment.lease_id),
                    "attempt_count": updated.attempt_count,
                    "max_attempts": updated.max_attempts,
                    "last_error": updated.last_error.clone(),
                }),
                details,
            ),
        )
        .await?;
    if queued { Ok(updated) } else { Err(error) }
}

fn merge_json_object(mut base: Value, extra: Value) -> Value {
    if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    base
}

async fn record_remote_computer_execution_handoff_acknowledged(
    state: &AppState,
    job: &ExecutionJob,
    worker_id: &str,
    assignment: &RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let execution_enabled = remote_computer_pod_execution_requested();
    let handoff_mode = if execution_enabled {
        "assigned-pod-execution"
    } else {
        "control-plane-only"
    };
    let details = json!({
        "assignment_id": assignment.id,
        "execution_job_id": job.id,
        "approval_id": job.approval_id,
        "tool_call_id": job.tool_call_id,
        "tool": job.tool_name,
        "remote_computer_id": assignment.remote_computer_id,
        "lease_id": assignment.lease_id,
        "worker_id": worker_id,
        "execution_enabled": execution_enabled,
        "handoff_mode": handoff_mode
    });
    state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            "remote_computer.execution_handoff_acknowledged",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(job.session_id),
            "worker",
            Some(job.id),
            "remote_computer.execution_handoff_acknowledged",
            "remote_computer_job_assignment",
            Some(assignment.id),
            details,
        ))
        .await?;
    Ok(())
}

fn remote_computer_pod_execution_requested() -> bool {
    let transport_mode = std::env::var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let execution_enabled = env_flag("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
    execution_enabled && matches!(transport_mode.as_str(), "kubernetes" | "k8s")
}

async fn append_remote_computer_execution_transport_plan(
    state: &AppState,
    job: &ExecutionJob,
    worker_id: &str,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let transport_mode = std::env::var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "reserved".to_string());
    let requested_execution_enabled = env_flag("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
    let execution_enabled = remote_computer_pod_execution_requested();
    let handoff_mode = if execution_enabled {
        "assigned-pod-execution"
    } else {
        "control-plane-only"
    };
    let pod_exec_api_path = remote_computer.pod_name.as_ref().map(|pod_name| {
        format!(
            "/api/v1/namespaces/{}/pods/{}/exec",
            remote_computer.namespace, pod_name
        )
    });
    let normalized_transport_mode = transport_mode.trim().to_ascii_lowercase();
    let transport_status = if execution_enabled {
        "execution_enabled"
    } else if matches!(normalized_transport_mode.as_str(), "kubernetes" | "k8s") {
        "blocked"
    } else {
        "reserved"
    };
    let details = json!({
        "assignment_id": assignment.id,
        "execution_job_id": job.id,
        "approval_id": job.approval_id,
        "tool_call_id": job.tool_call_id,
        "tool": job.tool_name,
        "remote_computer_id": assignment.remote_computer_id,
        "lease_id": assignment.lease_id,
        "worker_id": worker_id,
        "transport_mode": transport_mode,
        "transport_status": transport_status,
        "namespace": remote_computer.namespace,
        "pod_name": remote_computer.pod_name,
        "pod_exec_api_path": pod_exec_api_path,
        "requested_execution_enabled": requested_execution_enabled,
        "execution_enabled": execution_enabled,
        "handoff_mode": handoff_mode
    });
    state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            "remote_computer.execution_transport_planned",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(job.session_id),
            "worker",
            Some(job.id),
            "remote_computer.execution_transport_planned",
            "remote_computer_job_assignment",
            Some(assignment.id),
            details,
        ))
        .await?;
    Ok(())
}

async fn active_remote_computer_assignment_for_job(
    state: &AppState,
    job: &ExecutionJob,
) -> Result<Option<RemoteComputerJobAssignment>, AppError> {
    Ok(state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .find(|assignment| {
            assignment.execution_job_id == job.id && assignment.status == "assigned"
        }))
}

async fn validate_active_remote_computer_assignment_for_job(
    state: &AppState,
    job: &ExecutionJob,
    assignment: &RemoteComputerJobAssignment,
    environment_contract: Option<&RemoteComputerEnvironmentContract>,
) -> Result<(), AppError> {
    let Some(contract) = environment_contract else {
        return Ok(());
    };
    if assignment.execution_job_id != job.id {
        return Err(AppError::bad_request(
            "active Remote Computer assignment does not belong to the execution job",
        ));
    }
    if assignment.session_id != job.session_id {
        return Err(AppError::bad_request(
            "active Remote Computer assignment session does not match the execution job",
        ));
    }
    let lease = state
        .list_remote_computer_leases()
        .await?
        .into_iter()
        .find(|lease| lease.id == assignment.lease_id)
        .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
    if lease.remote_computer_id != assignment.remote_computer_id {
        return Err(AppError::bad_request(
            "active Remote Computer assignment lease does not match the assigned computer",
        ));
    }
    if lease.status != "leased" {
        return Err(AppError::bad_request(
            "active Remote Computer assignment lease is not leased",
        ));
    }
    if lease
        .lease_expires_at
        .as_ref()
        .is_some_and(|lease_expires_at| lease_expires_at <= &Utc::now())
    {
        return Err(AppError::bad_request(
            "active Remote Computer assignment lease has expired",
        ));
    }
    if lease
        .session_id
        .as_ref()
        .is_some_and(|session_id| *session_id != job.session_id)
    {
        return Err(AppError::bad_request(
            "active Remote Computer assignment lease session does not match the execution job",
        ));
    }
    if !contract.matches_lease(&lease) {
        return Err(AppError::bad_request(
            "active Remote Computer assignment lease does not match the remote_computer environment contract",
        ));
    }
    let computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    if !contract.matches_computer(&computer) {
        return Err(AppError::bad_request(
            "active Remote Computer assignment does not match the remote_computer environment contract",
        ));
    }
    Ok(())
}

async fn auto_assign_remote_computer_for_job(
    state: &AppState,
    job: &ExecutionJob,
    worker_id: &str,
    environment_contract: Option<&RemoteComputerEnvironmentContract>,
) -> Result<Option<RemoteComputerJobAssignment>, AppError> {
    if environment_contract.is_none() {
        return Ok(None);
    }
    let contract = environment_contract.expect("checked remote computer contract");
    let assigned_lease_ids: HashSet<_> = state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .filter(|assignment| assignment.status == "assigned")
        .map(|assignment| assignment.lease_id)
        .collect();
    let computers = state.list_remote_computers().await?;
    let lease = if let Some(lease) = state
        .list_remote_computer_leases()
        .await?
        .into_iter()
        .filter(|lease| {
            lease.status == "leased"
                && !assigned_lease_ids.contains(&lease.id)
                && lease
                    .lease_expires_at
                    .is_none_or(|lease_expires_at| lease_expires_at > Utc::now())
                && lease
                    .session_id
                    .is_none_or(|session_id| session_id == job.session_id)
                && computers
                    .iter()
                    .find(|computer| computer.id == lease.remote_computer_id)
                    .is_some_and(|computer| contract.matches_computer(computer))
                && contract.matches_lease(lease)
        })
        .max_by_key(|lease| {
            (
                lease.session_id == Some(job.session_id),
                lease.heartbeat_at.unwrap_or(lease.created_at),
            )
        }) {
        lease
    } else {
        match claim_remote_computer_warm_pool_lease_for_job(state, job, worker_id, contract).await?
        {
            Some(lease) => lease,
            None => return Ok(None),
        }
    };
    let assignment = state
        .create_remote_computer_job_assignment(
            job.id,
            job.session_id,
            CreateRemoteComputerJobAssignment {
                lease_id: lease.id,
                assigned_by: Some(worker_id.to_string()),
                metadata: Some(json!({
                    "handoff_mode": "environment-worker-lease",
                    "source": "run_execution_job",
                    "environment_contract": contract.evidence()
                })),
            },
        )
        .await?;
    record_remote_computer_job_assignment_event(
        state,
        &assignment,
        job,
        "remote_computer.execution_handoff_assigned",
    )
    .await?;
    Ok(Some(assignment))
}

async fn claim_remote_computer_warm_pool_lease_for_job(
    state: &AppState,
    job: &ExecutionJob,
    worker_id: &str,
    contract: &RemoteComputerEnvironmentContract,
) -> Result<Option<crate::RemoteComputerLease>, AppError> {
    let leased_remote_computer_ids: HashSet<_> = state
        .list_remote_computer_leases()
        .await?
        .into_iter()
        .filter(|lease| lease.status == "leased")
        .map(|lease| lease.remote_computer_id)
        .collect();
    let Some(computer) = state
        .list_remote_computers()
        .await?
        .into_iter()
        .filter(|computer| {
            computer.status == "available"
                && !leased_remote_computer_ids.contains(&computer.id)
                && contract.matches_computer(computer)
                && computer
                    .metadata
                    .get("warm_pool")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .max_by_key(|computer| computer.updated_at)
    else {
        return Ok(None);
    };
    let lease = state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(job.session_id),
                worker_id: Some(worker_id.to_string()),
                lease_seconds: Some(900),
                metadata: Some(json!({
                    "handoff_mode": "environment-warm-pool-lease",
                    "source": "run_execution_job",
                    "execution_job_id": job.id,
                    "tool_call_id": job.tool_call_id,
                    "environment_contract": contract.evidence()
                })),
            },
        )
        .await?;
    let details = json!({
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "session_id": lease.session_id,
        "worker_id": lease.worker_id,
        "status": lease.status,
        "lease_expires_at": lease.lease_expires_at,
        "source": "auto-warm-pool-lease",
        "environment_contract": contract.evidence(),
        "execution_job_id": job.id,
        "tool_call_id": job.tool_call_id,
    });
    state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            "remote_computer.warm_pool_claimed",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(job.session_id),
            "worker",
            Some(job.id),
            "remote_computer.warm_pool_claimed",
            "remote_computer_lease",
            Some(lease.id),
            details,
        ))
        .await?;
    Ok(Some(lease))
}

async fn finalize_remote_computer_assignment_for_job(
    state: &AppState,
    job: &ExecutionJob,
    assignment: Option<&crate::RemoteComputerJobAssignment>,
    status: &str,
    event_type: &str,
    metadata: Value,
) -> Result<(), AppError> {
    let Some(assignment) = assignment else {
        return Ok(());
    };
    let updated = state
        .update_remote_computer_job_assignment_status(assignment.id, status, metadata)
        .await?;
    record_remote_computer_job_assignment_event(state, &updated, job, event_type).await
}

#[derive(Debug, Clone)]
struct RemoteComputerEnvironmentContract {
    environment_id: Uuid,
    environment_name: String,
    pool: Option<String>,
    profile: Option<String>,
    namespace: Option<String>,
    remote_computer_id: Option<Uuid>,
    metadata_selector: Vec<(String, String)>,
}

async fn remote_computer_environment_contract_for_job(
    state: &AppState,
    job: &ExecutionJob,
) -> Result<Option<RemoteComputerEnvironmentContract>, AppError> {
    let session = state.get_session(job.session_id).await?;
    let Some(environment_id) = session.environment_id else {
        return Ok(None);
    };
    let environment = state.get_environment(environment_id).await?;
    if environment.environment_type != "remote_computer" {
        return Ok(None);
    }
    RemoteComputerEnvironmentContract::from_environment(&environment).map(Some)
}

impl RemoteComputerEnvironmentContract {
    fn from_environment(environment: &Environment) -> Result<Self, AppError> {
        if environment.status != "enabled" || environment.release_state != "active" {
            return Err(AppError::bad_request(format!(
                "remote_computer environment {} is not active and enabled",
                environment.id
            )));
        }
        let profile = &environment.remote_computer_profile;
        let remote_computer_id = optional_uuid_from_json(profile, "remote_computer_id")?;
        let metadata_selector = profile
            .get("metadata_selector")
            .and_then(Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(Self {
            environment_id: environment.id,
            environment_name: environment.name.clone(),
            pool: optional_string_from_json(profile, "pool"),
            profile: optional_string_from_json(profile, "profile"),
            namespace: optional_string_from_json(profile, "namespace"),
            remote_computer_id,
            metadata_selector,
        })
    }

    fn matches_computer(&self, computer: &RemoteComputer) -> bool {
        if self
            .remote_computer_id
            .is_some_and(|remote_computer_id| computer.id != remote_computer_id)
        {
            return false;
        }
        if self
            .profile
            .as_deref()
            .is_some_and(|profile| computer.profile != profile)
        {
            return false;
        }
        if self
            .namespace
            .as_deref()
            .is_some_and(|namespace| computer.namespace != namespace)
        {
            return false;
        }
        if self.pool.as_deref().is_some_and(|pool| {
            computer
                .metadata
                .get("pool")
                .and_then(Value::as_str)
                .map(|value| value != pool)
                .unwrap_or(true)
        }) {
            return false;
        }
        self.metadata_selector.iter().all(|(key, expected)| {
            computer
                .metadata
                .get(key)
                .and_then(Value::as_str)
                .map(|value| value == expected)
                .unwrap_or(false)
        })
    }

    fn matches_lease(&self, lease: &RemoteComputerLease) -> bool {
        lease
            .metadata
            .get("environment_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_none_or(|environment_id| environment_id == self.environment_id)
    }

    fn evidence(&self) -> Value {
        json!({
            "environment_id": self.environment_id,
            "environment_name": self.environment_name,
            "environment_type": "remote_computer",
            "pool": self.pool,
            "profile": self.profile,
            "namespace": self.namespace,
            "remote_computer_id": self.remote_computer_id,
            "metadata_selector": self.metadata_selector.iter().map(|(key, value)| json!({"key": key, "value": value})).collect::<Vec<_>>()
        })
    }
}

fn optional_string_from_json(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn optional_uuid_from_json(value: &Value, key: &str) -> Result<Option<Uuid>, AppError> {
    let Some(raw) = optional_string_from_json(value, key) else {
        return Ok(None);
    };
    Uuid::parse_str(&raw)
        .map(Some)
        .map_err(|_| AppError::bad_request(format!("{key} must be a valid UUID")))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

fn host_shell_execution_allowed() -> bool {
    env_flag("MANDOFORGE_ALLOW_HOST_SHELL_EXEC")
}

async fn execute_approved_file_write(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let relative_path = tool_call
        .args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("diagnostics.md");
    let content = tool_call
        .args
        .get("content")
        .or_else(|| tool_call.args.get("markdown"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let workspace = session_workspace(state, approval.session_id).await?;
    let output_path = safe_workspace_path(&workspace, relative_path)?;
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&output_path, content).await?;

    let result = json!({
        "approval": "approved",
        "path": relative_path,
        "bytes": content.len(),
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: approval.session_id,
        artifact_type: "file".to_string(),
        name: FsPath::new(relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(relative_path)
            .to_string(),
        path: Some(relative_path.to_string()),
        content: json!({"text": content}),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
            "system",
            Some(artifact.id),
            approval.session_id,
            "artifact.created",
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "path": relative_path, "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_remote_computer_file_write(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let relative_path = tool_call
        .args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("file.write requires path"))?;
    let content = tool_call
        .args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("file.write requires content"))?;
    validate_workspace_relative_path(relative_path)?;
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let pod_name = remote_computer
        .pod_name
        .clone()
        .ok_or_else(|| AppError::bad_request("Remote computer has no pod_name for Pod exec"))?;
    let command = remote_file_write_command(relative_path, content);
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let response = runner
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_exec".to_string()),
                remote_computer_id: Some(remote_computer.id),
                session_id: Some(approval.session_id),
                pod_name: Some(pod_name.clone()),
                metadata: Some(json!({"command": command, "tool_call_id": tool_call.id})),
            },
        )
        .await;
    let exec_result = response.exec_result.clone().ok_or_else(|| {
        AppError::bad_request(format!(
            "Remote Computer file.write did not return output: {}",
            response.message
        ))
    })?;
    if response.status != "exec_ok" || !response.execution_enabled {
        return Err(AppError::bad_request(response.message));
    }
    let status = exec_result.get("status").cloned().unwrap_or(Value::Null);
    if !kubernetes_exec_status_succeeded(&status) {
        return Err(AppError::bad_request(format!(
            "Remote Computer file.write failed with status {status}"
        )));
    }
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(
        exec_result
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let stderr = truncate_output(
        exec_result
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let result = json!({
        "approval": "approved",
        "path": relative_path,
        "runner": "remote_computer_pod_exec",
        "remote_computer_id": remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": remote_computer.namespace,
        "pod_name": pod_name,
        "status": status,
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated || exec_result
            .get("stdout_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated || exec_result
            .get("stderr_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: approval.session_id,
        artifact_type: "file".to_string(),
        name: relative_path.to_string(),
        path: Some(relative_path.to_string()),
        content: json!({
            "text": content,
            "runner": "remote_computer_pod_exec",
            "remote_computer_id": remote_computer.id,
            "assignment_id": assignment.id,
        }),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
            "worker",
            Some(tool_call.id),
            approval.session_id,
            "remote_computer.execution_transport_completed",
            json!({
                "tool_call_id": tool_call.id,
                "assignment_id": assignment.id,
                "remote_computer_id": remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": pod_name,
                "tool": tool_call.tool_name,
                "path": relative_path,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "status": status,
                "execution_enabled": true,
            }),
        )
        .await?;
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .append_event(
            "system",
            Some(artifact.id),
            approval.session_id,
            "artifact.created",
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type, "runner": "remote_computer_pod_exec"}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "path": relative_path,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "pod_name": pod_name,
                "status": status,
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_shell(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    if !host_shell_execution_allowed() {
        return Err(AppError::bad_request(
            "host shell.exec is disabled; use Remote Computer execution or set MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1",
        ));
    }
    let command = tool_call
        .args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("shell.exec requires command"))?;
    let workspace = session_workspace(state, approval.session_id).await?;
    let runner = shell_runner();
    let mut process = shell_command(&runner, &workspace, command);
    let output = tokio::time::timeout(Duration::from_secs(30), process.output())
        .await
        .map_err(|_| AppError::bad_request("shell.exec timed out"))??;
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout), limit);
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr), limit);

    let result = json!({
        "approval": "approved",
        "command": command,
        "runner": runner,
        "workspace": workspace.display().to_string(),
        "exit_code": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated,
    });
    if !output.status.success() {
        let error_payload = json!({
            "error": "shell.exec exited unsuccessfully",
            "content": result
        });
        state
            .append_event(
                "tool",
                Some(tool_call.id),
                approval.session_id,
                "tool.error",
                json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": error_payload}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(approval.session_id),
                "tool",
                Some(tool_call.id),
                "tool.failed",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": tool_call.tool_name, "command": command, "runner": runner, "exit_code": output.status.code(), "resumed_after_approval": true}),
            ))
            .await?;
        return Err(AppError::bad_request(format!(
            "shell.exec exited unsuccessfully: {:?}",
            output.status.code()
        )));
    }
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "command": command, "runner": runner, "exit_code": output.status.code(), "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_remote_computer_shell(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let command = tool_call
        .args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("shell.exec requires command"))?;
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let pod_name = remote_computer
        .pod_name
        .clone()
        .ok_or_else(|| AppError::bad_request("Remote computer has no pod_name for Pod exec"))?;
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let response = runner
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_exec".to_string()),
                remote_computer_id: Some(remote_computer.id),
                session_id: Some(approval.session_id),
                pod_name: Some(pod_name.clone()),
                metadata: Some(json!({"command": command, "tool_call_id": tool_call.id})),
            },
        )
        .await;
    let exec_result = response.exec_result.clone().ok_or_else(|| {
        AppError::bad_request(format!(
            "Remote Computer Pod exec did not return output: {}",
            response.message
        ))
    })?;
    if response.status != "exec_ok" || !response.execution_enabled {
        return Err(AppError::bad_request(response.message));
    }
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(
        exec_result
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let stderr = truncate_output(
        exec_result
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let status = exec_result.get("status").cloned().unwrap_or(Value::Null);
    let result = json!({
        "approval": "approved",
        "command": command,
        "runner": "remote_computer_pod_exec",
        "remote_computer_id": remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": remote_computer.namespace,
        "pod_name": pod_name,
        "status": status,
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated || exec_result
            .get("stdout_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated || exec_result
            .get("stderr_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
    state
        .append_event(
            "worker",
            Some(tool_call.id),
            approval.session_id,
            "remote_computer.execution_transport_completed",
            json!({
                "tool_call_id": tool_call.id,
                "assignment_id": assignment.id,
                "remote_computer_id": remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": pod_name,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "status": status,
                "execution_enabled": true,
            }),
        )
        .await?;
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "command": command,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "pod_name": pod_name,
                "status": status,
                "stdout_chars": stdout.text.chars().count(),
                "stderr_chars": stderr.text.chars().count(),
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_remote_computer_codex(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let request: CodexRequest = serde_json::from_value(tool_call.args.clone())?;
    if request.sandbox_mode != "read-only" && request.sandbox_mode != "workspace-write" {
        return Err(AppError::bad_request(
            "codex sandbox mode requires approval",
        ));
    }
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let pod_name = remote_computer
        .pod_name
        .clone()
        .ok_or_else(|| AppError::bad_request("Remote computer has no pod_name for Pod exec"))?;
    let command = remote_codex_exec_command(&request);
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "codex.task.started",
            json!({
                "task": &request.task,
                "sandbox_mode": &request.sandbox_mode,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
                "namespace": remote_computer.namespace,
                "pod_name": pod_name,
            }),
        )
        .await?;
    let response = runner
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_exec".to_string()),
                remote_computer_id: Some(remote_computer.id),
                session_id: Some(approval.session_id),
                pod_name: Some(pod_name.clone()),
                metadata: Some(json!({"command": command, "tool_call_id": tool_call.id})),
            },
        )
        .await;
    let exec_result = response.exec_result.clone().ok_or_else(|| {
        AppError::bad_request(format!(
            "Remote Computer Codex exec did not return output: {}",
            response.message
        ))
    })?;
    if response.status != "exec_ok" || !response.execution_enabled {
        return Err(AppError::bad_request(response.message));
    }

    let stdout_full = exec_result
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let stderr_full = exec_result
        .get("stderr")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let remote_output = split_remote_codex_output(stdout_full);
    for event in parse_codex_jsonl(&remote_output.jsonl_stdout) {
        state
            .append_event(
                "tool",
                Some(tool_call.id),
                approval.session_id,
                "codex.event",
                json!({"codex_event_type": codex_jsonl_event_type(&event), "event": event, "runner": "remote_computer_pod_exec"}),
            )
            .await?;
    }

    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&remote_output.jsonl_stdout, limit);
    let stderr = truncate_output(stderr_full, limit);
    let final_output = truncate_output(&remote_output.final_message, limit);
    let status = exec_result.get("status").cloned().unwrap_or(Value::Null);
    let stdout_truncated = stdout.truncated
        || exec_result
            .get("stdout_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let stderr_truncated = stderr.truncated
        || exec_result
            .get("stderr_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    if !remote_output.final_message.trim().is_empty() {
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id: approval.session_id,
            artifact_type: "markdown".to_string(),
            name: "codex-final-message.md".to_string(),
            path: Some("codex-final-message.md".to_string()),
            content: json!({
                "markdown": final_output.text.clone(),
                "markdown_bytes": final_output.original_bytes,
                "markdown_truncated": final_output.truncated,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
            }),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                approval.session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type, "runner": "remote_computer_pod_exec"}),
            )
            .await?;
    }

    let event_type = if kubernetes_exec_status_succeeded(&status) {
        "codex.task.completed"
    } else {
        "codex.task.failed"
    };
    state
        .append_event(
            "worker",
            Some(tool_call.id),
            approval.session_id,
            "remote_computer.execution_transport_completed",
            json!({
                "tool_call_id": tool_call.id,
                "assignment_id": assignment.id,
                "remote_computer_id": remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": pod_name,
                "tool": tool_call.tool_name,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "status": status,
                "execution_enabled": true,
            }),
        )
        .await?;
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            event_type,
            json!({
                "status": status,
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout_truncated,
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr_truncated,
                "final_message": final_output.text,
                "final_message_bytes": final_output.original_bytes,
                "final_message_truncated": final_output.truncated,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
            }),
        )
        .await?;
    let result = json!({
        "runner": "remote_computer_pod_exec",
        "remote_computer_id": remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": remote_computer.namespace,
        "pod_name": pod_name,
        "status": status,
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout_truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr_truncated,
        "final_message": final_output.text,
        "final_message_bytes": final_output.original_bytes,
        "final_message_truncated": final_output.truncated,
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "pod_name": pod_name,
                "status": status,
                "stdout_chars": stdout.text.chars().count(),
                "stderr_chars": stderr.text.chars().count(),
                "final_message_chars": final_output.text.chars().count(),
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_remote_computer_agent_cli(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let request: AgentCliRequest = serde_json::from_value(tool_call.args.clone())?;
    let profile = normalize_agent_cli_profile(&request.profile)?;
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == assignment.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let pod_name = remote_computer
        .pod_name
        .clone()
        .ok_or_else(|| AppError::bad_request("Remote computer has no pod_name for Pod exec"))?;
    let profile_config = agent_cli_profile_config(state, &profile).await?;
    enforce_bound_agent_cli_profile(state, approval.session_id, &profile).await?;
    let profile_source = match profile_config.source {
        AgentCliProfileConfigSource::Managed => "managed",
        AgentCliProfileConfigSource::Environment => "environment",
    };
    let command = remote_agent_cli_exec_command(&request, &profile_config)?;
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "agent_cli.task.started",
            json!({
                "profile": profile,
                "profile_source": profile_source,
                "task": &request.task,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
                "namespace": remote_computer.namespace,
                "pod_name": pod_name,
            }),
        )
        .await?;
    let response = runner
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_exec".to_string()),
                remote_computer_id: Some(remote_computer.id),
                session_id: Some(approval.session_id),
                pod_name: Some(pod_name.clone()),
                metadata: Some(
                    json!({"command": command, "tool_call_id": tool_call.id, "profile": profile, "profile_source": profile_source}),
                ),
            },
        )
        .await;
    let exec_result = response.exec_result.clone().ok_or_else(|| {
        AppError::bad_request(format!(
            "Remote Computer agent CLI exec did not return output: {}",
            response.message
        ))
    })?;
    if response.status != "exec_ok" || !response.execution_enabled {
        return Err(AppError::bad_request(response.message));
    }

    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(
        exec_result
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let stderr = truncate_output(
        exec_result
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        limit,
    );
    let status = exec_result.get("status").cloned().unwrap_or(Value::Null);
    let event_type = if kubernetes_exec_status_succeeded(&status) {
        "agent_cli.task.completed"
    } else {
        "agent_cli.task.failed"
    };
    state
        .append_event(
            "worker",
            Some(tool_call.id),
            approval.session_id,
            "remote_computer.execution_transport_completed",
            json!({
                "tool_call_id": tool_call.id,
                "assignment_id": assignment.id,
                "remote_computer_id": remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": pod_name,
                "tool": tool_call.tool_name,
                "profile": profile,
                "profile_source": profile_source,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "status": status,
                "execution_enabled": true,
            }),
        )
        .await?;
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            event_type,
            json!({
                "profile": profile,
                "profile_source": profile_source,
                "status": status,
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout.truncated || exec_result
                    .get("stdout_truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr.truncated || exec_result
                    .get("stderr_truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
            }),
        )
        .await?;
    let result = json!({
        "runner": "remote_computer_pod_exec",
        "profile": profile,
        "remote_computer_id": remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": remote_computer.namespace,
        "pod_name": pod_name,
        "status": status,
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated || exec_result
            .get("stdout_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated || exec_result
            .get("stderr_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "profile": profile,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": remote_computer.id,
                "assignment_id": assignment.id,
                "pod_name": pod_name,
                "status": status,
                "stdout_chars": stdout.text.chars().count(),
                "stderr_chars": stderr.text.chars().count(),
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_codex(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let request: CodexRequest = serde_json::from_value(tool_call.args.clone())?;
    match run_codex(state, approval.session_id, request).await {
        Ok(result) => {
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.result",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.completed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "resumed_after_approval": true}),
                ))
                .await?;
            Ok(())
        }
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "error": error_payload, "resumed_after_approval": true}),
                ))
                .await?;
            Err(error)
        }
    }
}

async fn execute_approved_agent_cli(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let request: AgentCliRequest = serde_json::from_value(tool_call.args.clone())?;
    match run_agent_cli(state, approval.session_id, request).await {
        Ok(result) => {
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.result",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.completed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "profile": result.get("profile"), "resumed_after_approval": true}),
                ))
                .await?;
            Ok(())
        }
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "error": error_payload, "resumed_after_approval": true}),
                ))
                .await?;
            Err(error)
        }
    }
}

async fn session_workspace(state: &AppState, session_id: Uuid) -> Result<PathBuf, AppError> {
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    Ok(workspace)
}

fn safe_workspace_path(workspace: &FsPath, relative_path: &str) -> Result<PathBuf, AppError> {
    validate_workspace_relative_path(relative_path)?;
    Ok(workspace.join(FsPath::new(relative_path)))
}

fn validate_workspace_relative_path(relative_path: &str) -> Result<(), AppError> {
    let path = FsPath::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(AppError::bad_request(
            "file.write path must stay inside the session workspace",
        ));
    }
    Ok(())
}

#[allow(dead_code)]
async fn run_codex(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
) -> Result<Value, AppError> {
    if request.sandbox_mode != "read-only" && request.sandbox_mode != "workspace-write" {
        return Err(AppError::bad_request(
            "codex sandbox mode requires approval",
        ));
    }
    match codex_execution_strategy(&request)? {
        CodexExecutionStrategy::Cli => run_codex_cli(state, session_id, request).await,
        CodexExecutionStrategy::AppServer => run_codex_app_server(state, session_id, request).await,
        CodexExecutionStrategy::Auto => {
            if state.codex_app_server_config.is_none() {
                return run_codex_cli(state, session_id, request).await;
            }
            let fallback_request = request.clone();
            match run_codex_app_server(state, session_id, request).await {
                Ok(result) => Ok(result),
                Err(error) => {
                    state
                        .append_event(
                            "tool",
                            None,
                            session_id,
                            "codex.task.fallback",
                            json!({
                                "from": "app-server",
                                "to": "cli",
                                "reason": error.message,
                            }),
                        )
                        .await?;
                    run_codex_cli(state, session_id, fallback_request).await
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexExecutionStrategy {
    Auto,
    Cli,
    AppServer,
}

fn codex_execution_strategy(request: &CodexRequest) -> Result<CodexExecutionStrategy, AppError> {
    let env_strategy = std::env::var("MANDOFORGE_CODEX_EXECUTION_STRATEGY").ok();
    let raw = request
        .execution_strategy
        .as_deref()
        .or(env_strategy.as_deref())
        .unwrap_or("auto")
        .trim()
        .to_ascii_lowercase();
    match raw.as_str() {
        "auto" => Ok(CodexExecutionStrategy::Auto),
        "cli" | "codex-cli" => Ok(CodexExecutionStrategy::Cli),
        "app-server" | "app_server" | "codex-app-server" => Ok(CodexExecutionStrategy::AppServer),
        other => Err(AppError::bad_request(format!(
            "unsupported Codex execution strategy: {other}"
        ))),
    }
}

struct RemoteCodexOutput {
    jsonl_stdout: String,
    final_message: String,
}

fn remote_codex_exec_command(request: &CodexRequest) -> String {
    let final_path = "/workspace/.mandoforge/codex-final-message.md";
    format!(
        "set -u\nmkdir -p /workspace/.mandoforge\ncd /workspace\ncodex exec --sandbox {} --json --output-last-message {} --cd /workspace {}\ncode=$?\nprintf '\\n{}\\n'\ncat {} 2>/dev/null || true\nprintf '\\n{}\\n'\nexit $code",
        shell_single_quote(&request.sandbox_mode),
        shell_single_quote(final_path),
        shell_single_quote(&request.task),
        REMOTE_CODEX_FINAL_BEGIN,
        shell_single_quote(final_path),
        REMOTE_CODEX_FINAL_END
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn remote_agent_cli_exec_command(
    request: &AgentCliRequest,
    config: &AgentCliProfileConfig,
) -> Result<String, AppError> {
    let profile = normalize_agent_cli_profile(&request.profile)?;
    if config.source == AgentCliProfileConfigSource::Managed {
        let mut command = String::new();
        command.push_str("set -eu\ncd /workspace\n");
        for (key, value) in &config.env {
            command.push_str(&format!("export {}={}\n", key, shell_single_quote(value)));
        }
        command.push_str("set --\n");
        for arg in &config.args {
            command.push_str(&format!("set -- \"$@\" {}\n", shell_single_quote(arg)));
        }
        for arg in &request.args {
            command.push_str(&format!("set -- \"$@\" {}\n", shell_single_quote(arg)));
        }
        command.push_str(&format!(
            "MANDOFORGE_AGENT_CLI_PROFILE={} MANDOFORGE_AGENT_TASK={} {} \"$@\" {}\n",
            shell_single_quote(&profile),
            shell_single_quote(&request.task),
            shell_single_quote(&config.command),
            shell_single_quote(&request.task)
        ));
        return Ok(command);
    }

    let mut command = String::new();
    command.push_str("set -eu\ncd /workspace\nagent_cli_profile=");
    command.push_str(&shell_single_quote(&profile));
    command.push('\n');
    command.push_str(
        "allowed=\",${MANDOFORGE_AGENT_CLI_ALLOWED_PROFILES:-},\"\n\
case \"$allowed\" in *\",$agent_cli_profile,\"*) ;; *) echo \"agent CLI profile is not allowlisted: $agent_cli_profile\" >&2; exit 64 ;; esac\n\
command_var=\"MANDOFORGE_AGENT_CLI_$(printf '%s' \"$agent_cli_profile\" | tr '[:lower:]-' '[:upper:]_')_COMMAND\"\n\
args_var=\"MANDOFORGE_AGENT_CLI_$(printf '%s' \"$agent_cli_profile\" | tr '[:lower:]-' '[:upper:]_')_ARGS\"\n\
agent_command=\"$(eval \"printf '%s' \\\"\\${$command_var:-}\\\"\")\"\n\
agent_args=\"$(eval \"printf '%s' \\\"\\${$args_var:-}\\\"\")\"\n\
if [ -z \"$agent_command\" ]; then echo \"agent CLI profile $agent_cli_profile is missing $command_var\" >&2; exit 64; fi\n",
    );
    command.push_str("set --\n");
    command.push_str("if [ -n \"$agent_args\" ]; then\n  # Profile args intentionally use simple whitespace splitting; wrap complex CLIs in a shim.\n  set -- $agent_args\nfi\n");
    for arg in &request.args {
        command.push_str(&format!("set -- \"$@\" {}\n", shell_single_quote(arg)));
    }
    command.push_str(&format!(
        "MANDOFORGE_AGENT_CLI_PROFILE=\"$agent_cli_profile\" MANDOFORGE_AGENT_TASK={} \"$agent_command\" \"$@\" {}\n",
        shell_single_quote(&request.task),
        shell_single_quote(&request.task)
    ));
    Ok(command)
}

fn remote_file_write_command(relative_path: &str, content: &str) -> String {
    let delimiter = heredoc_delimiter(content);
    format!(
        "set -eu\ncd /workspace\nmkdir -p -- \"$(dirname -- {})\"\ncat > {} <<'{}'\n{}\n{}\nprintf 'wrote file %s\\n' {}",
        shell_single_quote(relative_path),
        shell_single_quote(relative_path),
        delimiter,
        content,
        delimiter,
        shell_single_quote(relative_path)
    )
}

fn heredoc_delimiter(content: &str) -> String {
    for index in 0..1000 {
        let delimiter = format!("MANDOFORGE_FILE_WRITE_EOF_{index}");
        if !content.lines().any(|line| line == delimiter) {
            return delimiter;
        }
    }
    format!("MANDOFORGE_FILE_WRITE_EOF_{}", Uuid::new_v4().simple())
}

fn split_remote_codex_output(stdout: &str) -> RemoteCodexOutput {
    let Some((jsonl_stdout, rest)) = stdout.split_once(REMOTE_CODEX_FINAL_BEGIN) else {
        return RemoteCodexOutput {
            jsonl_stdout: stdout.to_string(),
            final_message: String::new(),
        };
    };
    let final_message = rest
        .split_once(REMOTE_CODEX_FINAL_END)
        .map(|(message, _)| message)
        .unwrap_or(rest)
        .trim_matches('\n')
        .to_string();
    RemoteCodexOutput {
        jsonl_stdout: jsonl_stdout.trim_end_matches('\n').to_string(),
        final_message,
    }
}

fn kubernetes_exec_status_succeeded(status: &Value) -> bool {
    if status.is_null() {
        return true;
    }
    let status_text = status
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let exit_code = status.get("exitCode").and_then(Value::as_i64).unwrap_or(0);
    status_text.eq_ignore_ascii_case("success") && exit_code == 0
}

async fn run_codex_app_server(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
) -> Result<Value, AppError> {
    let config = state
        .codex_app_server_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("Codex App Server is not configured"))?;
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    state
        .append_event(
            "tool",
            None,
            session_id,
            "codex.task.started",
            json!({
                "task": &request.task,
                "sandbox_mode": &request.sandbox_mode,
                "workspace": workspace,
                "runner": "app-server",
            }),
        )
        .await?;

    let thread_request = CodexThreadRequest {
        metadata: json!({
            "session_id": session_id,
            "workspace": workspace,
            "sandbox_mode": &request.sandbox_mode,
            "source": "approved_codex_exec",
        }),
    };
    let thread = state
        .codex_app_server_client
        .create_thread(config, thread_request.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "thread.create",
            Some(thread.thread_id.clone()),
            None,
            None,
            serde_json::to_value(&thread_request)?,
            serde_json::to_value(&thread)?,
        )
        .await?;

    let turn_request = CodexTurnRequest {
        message: request.task.clone(),
        metadata: json!({
            "session_id": session_id,
            "workspace": workspace,
            "sandbox_mode": &request.sandbox_mode,
            "source": "approved_codex_exec",
        }),
    };
    let turn = state
        .codex_app_server_client
        .create_turn(config, &thread.thread_id, turn_request.clone())
        .await?;
    let turn_run = state
        .record_codex_app_server_run(
            "turn.create",
            Some(thread.thread_id.clone()),
            Some(turn.turn_id.clone()),
            None,
            serde_json::to_value(&turn_request)?,
            serde_json::to_value(&turn)?,
        )
        .await?;
    let poll_result = poll_codex_app_server_turn_for_worker(
        state,
        config,
        session_id,
        turn_run.id,
        turn.clone(),
        request.poll_attempts,
        request.poll_interval_ms,
    )
    .await?;
    let final_turn = poll_result.turn;
    let final_status = final_turn
        .status
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let event_type =
        if poll_result.terminal && codex_app_server_turn_status_succeeded(&final_status) {
            "codex.task.completed"
        } else {
            "codex.task.failed"
        };

    state
        .append_event(
            "tool",
            None,
            session_id,
            event_type,
            json!({
                "runner": "app-server",
                "thread_id": thread.thread_id,
                "turn_id": final_turn.turn_id,
                "status": final_status,
                "terminal": poll_result.terminal,
                "poll_attempts": poll_result.attempts,
                "result": final_turn.result,
                "fallback_used": false,
            }),
        )
        .await?;

    if event_type == "codex.task.failed" {
        return Err(AppError::bad_request(format!(
            "Codex App Server turn ended with status {final_status}"
        )));
    }

    Ok(json!({
        "runner": "app-server",
        "thread_id": thread.thread_id,
        "turn_id": final_turn.turn_id,
        "status": final_status,
        "terminal": poll_result.terminal,
        "poll_attempts": poll_result.attempts,
        "result": final_turn.result,
        "fallback_used": false,
    }))
}

struct CodexAppServerWorkerPollResult {
    turn: CodexTurnResponse,
    attempts: u32,
    terminal: bool,
}

async fn poll_codex_app_server_turn_for_worker(
    state: &AppState,
    config: &crate::codex_app_server::CodexAppServerConfig,
    session_id: Uuid,
    run_id: Uuid,
    initial_turn: CodexTurnResponse,
    requested_attempts: Option<u32>,
    requested_interval_ms: Option<u64>,
) -> Result<CodexAppServerWorkerPollResult, AppError> {
    let mut turn = initial_turn;
    let mut status = turn.status.clone().unwrap_or_else(|| "unknown".to_string());
    if codex_app_server_turn_status_is_terminal(&status) {
        return Ok(CodexAppServerWorkerPollResult {
            turn,
            attempts: 0,
            terminal: true,
        });
    }
    let max_attempts = requested_attempts
        .or_else(codex_app_server_worker_poll_attempts_from_env)
        .unwrap_or(3)
        .clamp(1, 20);
    let retry_interval_ms = requested_interval_ms
        .or_else(codex_app_server_worker_poll_interval_from_env)
        .unwrap_or(0)
        .min(30_000);
    let mut attempts = 0;
    let mut terminal = false;
    while attempts < max_attempts && !terminal {
        attempts += 1;
        match state
            .codex_app_server_client
            .get_turn_status(config, &turn.turn_id)
            .await
        {
            Ok(response) => {
                status = response
                    .status
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                terminal = codex_app_server_turn_status_is_terminal(&status);
                turn = response;
                state
                    .update_codex_app_server_run_status(
                        run_id,
                        status.clone(),
                        serde_json::to_value(&turn)?,
                        None,
                    )
                    .await?;
                state
                    .append_event(
                        "worker",
                        Some(run_id),
                        session_id,
                        "codex.task.event",
                        json!({
                            "runner": "app-server",
                            "run_id": run_id,
                            "turn_id": turn.turn_id,
                            "attempt": attempts,
                            "status": status,
                            "terminal": terminal,
                        }),
                    )
                    .await?;
            }
            Err(error) => {
                status = "poll_failed".to_string();
                state
                    .update_codex_app_server_run_status(
                        run_id,
                        status.clone(),
                        serde_json::to_value(&turn)?,
                        Some(json!({"message": error.message, "attempt": attempts})),
                    )
                    .await?;
                state
                    .append_event(
                        "worker",
                        Some(run_id),
                        session_id,
                        "codex.task.event",
                        json!({
                            "runner": "app-server",
                            "run_id": run_id,
                            "turn_id": turn.turn_id,
                            "attempt": attempts,
                            "status": status,
                            "terminal": false,
                            "error": error.message,
                        }),
                    )
                    .await?;
            }
        }
        if attempts < max_attempts && !terminal && retry_interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
        }
    }
    Ok(CodexAppServerWorkerPollResult {
        turn,
        attempts,
        terminal,
    })
}

fn codex_app_server_worker_poll_attempts_from_env() -> Option<u32> {
    std::env::var("MANDOFORGE_CODEX_APP_SERVER_WORKER_POLL_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
}

fn codex_app_server_worker_poll_interval_from_env() -> Option<u64> {
    std::env::var("MANDOFORGE_CODEX_APP_SERVER_WORKER_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn codex_app_server_turn_status_is_terminal(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "completed" | "failed" | "cancelled" | "canceled" | "interrupted"
    )
}

fn codex_app_server_turn_status_succeeded(status: &str) -> bool {
    status.trim().eq_ignore_ascii_case("completed")
}

#[allow(dead_code)]
async fn run_agent_cli(
    state: &AppState,
    session_id: Uuid,
    request: AgentCliRequest,
) -> Result<Value, AppError> {
    let profile = normalize_agent_cli_profile(&request.profile)?;
    let config = agent_cli_profile_config(state, &profile).await?;
    enforce_bound_agent_cli_profile(state, session_id, &profile).await?;
    if config.remote_computer_required {
        return Err(AppError::bad_request(format!(
            "agent runtime profile requires Remote Computer execution: {profile}"
        )));
    }
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;

    state
        .append_event(
            "tool",
            None,
            session_id,
            "agent_cli.task.started",
            json!({
                "profile": profile,
                "task": &request.task,
                "workspace": workspace,
                "runner": "agent-cli"
            }),
        )
        .await?;

    let mut command = Command::new(&config.command);
    command.current_dir(&workspace);
    for arg in config.args {
        command.arg(arg);
    }
    for arg in request.args {
        command.arg(arg);
    }
    command.arg(&request.task);
    command.env("MANDOFORGE_AGENT_CLI_PROFILE", &profile);
    command.env("MANDOFORGE_AGENT_TASK", &request.task);
    for (key, value) in config.env {
        command.env(key, value);
    }

    let timeout_seconds = request
        .timeout_seconds
        .or(config.timeout_seconds)
        .unwrap_or(180)
        .clamp(1, 900);
    let output = tokio::time::timeout(Duration::from_secs(timeout_seconds), command.output())
        .await
        .map_err(|_| AppError::bad_request("agent CLI execution timed out"))??;

    let stdout_full = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_full = String::from_utf8_lossy(&output.stderr).to_string();
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&stdout_full, limit);
    let stderr = truncate_output(&stderr_full, limit);
    let profile_source = match config.source {
        AgentCliProfileConfigSource::Managed => "managed",
        AgentCliProfileConfigSource::Environment => "environment",
    };
    let event_type = if output.status.success() {
        "agent_cli.task.completed"
    } else {
        "agent_cli.task.failed"
    };
    state
        .append_event(
            "tool",
            None,
            session_id,
            event_type,
            json!({
                "profile": profile,
                "profile_source": profile_source,
                "exit_code": output.status.code(),
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout.truncated,
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr.truncated,
                "runner": "agent-cli"
            }),
        )
        .await?;
    if !output.status.success() {
        return Err(AppError::bad_request(format!(
            "agent CLI execution failed with exit code {:?}",
            output.status.code()
        )));
    }

    Ok(json!({
        "runner": "agent-cli",
        "profile": profile,
        "profile_source": profile_source,
        "status": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentCliProfileConfigSource {
    Managed,
    Environment,
}

struct AgentCliProfileConfig {
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    timeout_seconds: Option<u64>,
    remote_computer_required: bool,
    source: AgentCliProfileConfigSource,
}

fn normalize_agent_cli_profile(profile: &str) -> Result<String, AppError> {
    let normalized = profile.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(AppError::bad_request(
            "agent CLI profile must be an allowlist-safe name",
        ));
    }
    Ok(normalized)
}

async fn enforce_bound_agent_cli_profile(
    state: &AppState,
    session_id: Uuid,
    requested_profile: &str,
) -> Result<(), AppError> {
    if env_flag("MANDOFORGE_ALLOW_REQUESTED_AGENT_CLI_PROFILE") {
        return Ok(());
    }
    let Some(bound_profile_name) = bound_agent_cli_profile_name(state, session_id).await? else {
        return Err(AppError::bad_request(
            "agent_cli.exec requires a session-bound runtime profile",
        ));
    };
    let bound_profile = normalize_agent_cli_profile(&bound_profile_name)?;
    if bound_profile != requested_profile {
        return Err(AppError::bad_request(format!(
            "agent_cli.exec profile must match session runtime profile: requested {requested_profile}, bound {bound_profile}"
        )));
    }
    Ok(())
}

async fn bound_agent_cli_profile_name(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<String>, AppError> {
    if let Some(assignment) = state
        .list_agent_handoff_assignments(Some(session_id))
        .await?
        .into_iter()
        .filter(|assignment| assignment.specialist_session_id == session_id)
        .max_by_key(|assignment| assignment.created_at)
    {
        if let Some(profile_id) = assignment.runtime_profile_id {
            let profile = state.get_agent_runtime_profile(profile_id).await?;
            return Ok(Some(profile.name));
        }
    }

    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    match agent.runtime_profile_id {
        Some(profile_id) => {
            let profile = state.get_agent_runtime_profile(profile_id).await?;
            Ok(Some(profile.name))
        }
        None => Ok(None),
    }
}

async fn agent_cli_profile_config(
    state: &AppState,
    profile: &str,
) -> Result<AgentCliProfileConfig, AppError> {
    if let Some(managed_profile) = state.get_agent_runtime_profile_by_name(profile).await? {
        if managed_profile.status != "enabled" {
            return Err(AppError::bad_request(format!(
                "agent runtime profile is not enabled: {profile}"
            )));
        }
        if managed_profile.runtime_type != "agent_cli" {
            return Err(AppError::bad_request(format!(
                "agent runtime profile {profile} is not an agent_cli runtime"
            )));
        }
        let env = managed_profile
            .env
            .as_object()
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return Ok(AgentCliProfileConfig {
            command: managed_profile.command,
            args: managed_profile.default_args,
            env,
            timeout_seconds: managed_profile
                .timeout_seconds
                .and_then(|value| u64::try_from(value).ok()),
            remote_computer_required: managed_profile.remote_computer_required,
            source: AgentCliProfileConfigSource::Managed,
        });
    }

    let allowed = std::env::var("MANDOFORGE_AGENT_CLI_ALLOWED_PROFILES").unwrap_or_default();
    let allowed_profiles: HashSet<String> = allowed
        .split(',')
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect();
    if !allowed_profiles.contains(profile) {
        return Err(AppError::bad_request(format!(
            "agent CLI profile is not allowlisted: {profile}"
        )));
    }

    let env_prefix = format!(
        "MANDOFORGE_AGENT_CLI_{}",
        profile
            .chars()
            .map(|ch| if ch == '-' { '_' } else { ch })
            .collect::<String>()
            .to_ascii_uppercase()
    );
    let command = std::env::var(format!("{env_prefix}_COMMAND"))
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(format!(
                "agent CLI profile {profile} is missing {env_prefix}_COMMAND"
            ))
        })?;
    let args = std::env::var(format!("{env_prefix}_ARGS"))
        .ok()
        .map(|value| {
            value
                .split_whitespace()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let timeout_seconds = std::env::var(format!("{env_prefix}_TIMEOUT_SECONDS"))
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok());

    Ok(AgentCliProfileConfig {
        command,
        args,
        env: Vec::new(),
        timeout_seconds,
        remote_computer_required: false,
        source: AgentCliProfileConfigSource::Environment,
    })
}

#[allow(dead_code)]
async fn run_codex_cli(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
) -> Result<Value, AppError> {
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let last_message = workspace.join("last_message.md");

    state
        .append_event(
            "tool",
            None,
            session_id,
            "codex.task.started",
            json!({"task": &request.task, "sandbox_mode": &request.sandbox_mode, "workspace": workspace, "runner": "cli"}),
        )
        .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(180),
        Command::new("codex")
            .arg("exec")
            .arg("--sandbox")
            .arg(&request.sandbox_mode)
            .arg("--json")
            .arg("--output-last-message")
            .arg(&last_message)
            .arg("--cd")
            .arg(&workspace)
            .arg(&request.task)
            .output(),
    )
    .await
    .map_err(|_| AppError::bad_request("codex exec timed out"))??;

    let stdout_full = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_full = String::from_utf8_lossy(&output.stderr).to_string();
    let final_message = tokio::fs::read_to_string(&last_message)
        .await
        .unwrap_or_default();
    for event in parse_codex_jsonl(&stdout_full) {
        state
            .append_event(
                "tool",
                None,
                session_id,
                "codex.event",
                json!({"codex_event_type": codex_jsonl_event_type(&event), "event": event}),
            )
            .await?;
    }
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&stdout_full, limit);
    let stderr = truncate_output(&stderr_full, limit);
    let final_output = truncate_output(&final_message, limit);
    if !final_message.trim().is_empty() {
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id,
            artifact_type: "markdown".to_string(),
            name: "codex-final-message.md".to_string(),
            path: Some("codex-final-message.md".to_string()),
            content: json!({
                "markdown": final_output.text.clone(),
                "markdown_bytes": final_output.original_bytes,
                "markdown_truncated": final_output.truncated
            }),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
            )
            .await?;
    }
    let event_type = if output.status.success() {
        "codex.task.completed"
    } else {
        "codex.task.failed"
    };
    state
        .append_event(
            "tool",
            None,
            session_id,
            event_type,
            json!({
                "exit_code": output.status.code(),
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout.truncated,
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr.truncated,
                "final_message": final_output.text,
                "final_message_bytes": final_output.original_bytes,
                "final_message_truncated": final_output.truncated,
                "runner": "cli"
            }),
        )
        .await?;
    if !output.status.success() {
        return Err(AppError::bad_request(format!(
            "codex exec failed with exit code {:?}",
            output.status.code()
        )));
    }
    Ok(json!({
        "runner": "cli",
        "status": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated,
        "final_message": final_output.text,
        "final_message_bytes": final_output.original_bytes,
        "final_message_truncated": final_output.truncated
    }))
}

pub(crate) fn parse_codex_jsonl(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(line).ok()
        })
        .collect()
}

pub(crate) fn codex_jsonl_event_type(event: &Value) -> String {
    event
        .get("type")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn execution_output_limit_bytes() -> usize {
    std::env::var("MANDOFORGE_EXECUTION_OUTPUT_LIMIT_BYTES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_OUTPUT_LIMIT_BYTES))
        .unwrap_or(DEFAULT_OUTPUT_LIMIT_BYTES)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TruncatedOutput {
    pub(crate) text: String,
    pub(crate) original_bytes: usize,
    pub(crate) truncated: bool,
}

pub(crate) fn truncate_output(value: &str, max_bytes: usize) -> TruncatedOutput {
    let original_bytes = value.len();
    if original_bytes <= max_bytes {
        return TruncatedOutput {
            text: value.to_string(),
            original_bytes,
            truncated: false,
        };
    }

    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    TruncatedOutput {
        text: value[..boundary].to_string(),
        original_bytes,
        truncated: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_codex_command_quotes_task_and_emits_final_markers() {
        let request = CodexRequest {
            task: "inspect README && echo 'done'".to_string(),
            sandbox_mode: "workspace-write".to_string(),
            execution_strategy: None,
            poll_attempts: None,
            poll_interval_ms: None,
        };
        let command = remote_codex_exec_command(&request);

        assert!(command.contains("cd /workspace"));
        assert!(command.contains("codex exec --sandbox 'workspace-write'"));
        assert!(command.contains("'inspect README && echo '\"'\"'done'\"'\"''"));
        assert!(command.contains(REMOTE_CODEX_FINAL_BEGIN));
        assert!(command.contains(REMOTE_CODEX_FINAL_END));
        assert!(command.contains("exit $code"));
    }

    #[test]
    fn remote_codex_output_splits_jsonl_from_final_message() {
        let output = split_remote_codex_output(&format!(
            "{{\"type\":\"session.started\"}}\n{}\n# Report\n\nDone\n{}\nignored",
            REMOTE_CODEX_FINAL_BEGIN, REMOTE_CODEX_FINAL_END
        ));

        assert_eq!(output.jsonl_stdout, "{\"type\":\"session.started\"}");
        assert_eq!(output.final_message, "# Report\n\nDone");
        assert_eq!(parse_codex_jsonl(&output.jsonl_stdout).len(), 1);
    }

    #[test]
    fn remote_file_write_command_uses_safe_heredoc_delimiter() {
        let command = remote_file_write_command(
            "reports/diagnostics.md",
            "line one\nMANDOFORGE_FILE_WRITE_EOF_0\nline two",
        );

        assert!(command.contains("cd /workspace"));
        assert!(command.contains("mkdir -p -- \"$(dirname -- 'reports/diagnostics.md')\""));
        assert!(command.contains("cat > 'reports/diagnostics.md'"));
        assert!(command.contains("MANDOFORGE_FILE_WRITE_EOF_1"));
        assert!(!command.contains("<<'MANDOFORGE_FILE_WRITE_EOF_0'"));
    }

    #[test]
    fn kubernetes_exec_status_defaults_and_failure_are_explicit() {
        assert!(kubernetes_exec_status_succeeded(&Value::Null));
        assert!(kubernetes_exec_status_succeeded(
            &json!({"status": "Success", "exitCode": 0})
        ));
        assert!(!kubernetes_exec_status_succeeded(
            &json!({"status": "Failure", "exitCode": 2})
        ));
    }
}
