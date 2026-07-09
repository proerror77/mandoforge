use std::{
    collections::HashSet,
    path::{Component, Path as FsPath, PathBuf},
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use axum::http::StatusCode;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::process::Command;
use uuid::Uuid;

use crate::codex_app_server::{CodexThreadRequest, CodexTurnRequest, CodexTurnResponse};
use crate::execution_queue::{ExecutionJob, ExecutionJobRequest, ExecutionJobStatus};
use crate::mcp_gateway::McpCallRequest;
use crate::remote_computer_runner::{
    RemoteComputerRunner, RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    poll_kubernetes_pod_running, remote_computer_runner_for_config,
};
use crate::shell_runner::{shell_command, shell_runner};
use crate::{
    AgentRuntimeProfile, AppError, AppState, Approval, Artifact, CreateRemoteComputer,
    CreateRemoteComputerJobAssignment, CreateRemoteComputerLease, Environment, RemoteComputer,
    RemoteComputerJobAssignment, RemoteComputerLease, ToolCall, new_audit_log,
    record_remote_computer_job_assignment_event, resolve_mcp_runtime_secret_refs,
};

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const DEFAULT_RUNTIME_ADAPTER_EVENT_LIMIT: usize = 200;
const MAX_RUNTIME_ADAPTER_EVENT_LIMIT: usize = 2_000;
const REMOTE_CODEX_FINAL_BEGIN: &str = "__MANDOFORGE_CODEX_FINAL_BEGIN__";
const REMOTE_CODEX_FINAL_END: &str = "__MANDOFORGE_CODEX_FINAL_END__";
const REMOTE_COMPUTER_DEFAULT_LEASE_SECONDS: i64 = 900;

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
    pub(crate) profile: String,
    pub(crate) task: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(default)]
    pub(crate) timeout_seconds: Option<u64>,
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

#[derive(Debug, Clone)]
pub(crate) enum ExecutionWorkerOutcome {
    Completed { job_id: Option<Uuid> },
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
            return Ok(ExecutionWorkerOutcome::Completed { job_id: None });
        };
        let completed = run_execution_job(state, job.id, "inline").await?;
        Ok(ExecutionWorkerOutcome::Completed {
            job_id: Some(completed.id),
        })
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
            return Ok(ExecutionWorkerOutcome::Completed { job_id: None });
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
    let environment_id = state.get_session(approval.session_id).await?.environment_id;
    Ok(Some(
        state
            .execution_queue
            .enqueue(ExecutionJobRequest {
                session_id: approval.session_id,
                environment_id,
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
    run_started_execution_job(state, job, worker_id).await
}

pub(crate) async fn run_started_execution_job(
    state: &AppState,
    job: ExecutionJob,
    worker_id: &str,
) -> Result<ExecutionJob, AppError> {
    if !crate::session_accepts_worker_execution(state, job.session_id).await? {
        return retry_or_fail_started_execution_job(
            state,
            &job,
            None,
            AppError::bad_request("execution job session is no longer active"),
            json!({"stage": "session_status"}),
        )
        .await;
    }
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
    let remote_pod_execution_enabled = remote_computer_pod_execution_requested();
    if remote_environment_contract.is_some() && !remote_pod_execution_enabled {
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
    let result = match (
        tool_call.tool_name.as_str(),
        remote_computer_assignment.as_ref(),
        remote_pod_execution_enabled,
    ) {
        ("file.write", Some(assignment), true) => {
            execute_approved_remote_computer_file_write(state, &approval, &tool_call, assignment)
                .await
        }
        ("file.write", _, _) => execute_approved_file_write(state, &approval, &tool_call).await,
        ("shell.exec", Some(assignment), true) => {
            execute_approved_remote_computer_shell(state, &approval, &tool_call, assignment).await
        }
        ("shell.exec", _, _) => execute_approved_shell(state, &approval, &tool_call).await,
        ("codex.exec", Some(assignment), true) => {
            execute_approved_remote_computer_codex(state, &approval, &tool_call, assignment).await
        }
        ("codex.exec", _, _) => execute_approved_codex(state, &approval, &tool_call).await,
        ("agent_cli.exec", Some(assignment), true) => {
            execute_approved_remote_computer_agent_cli(state, &approval, &tool_call, assignment)
                .await
        }
        ("agent_cli.exec", _, _) => execute_approved_agent_cli(state, &approval, &tool_call).await,
        ("mcp.call", _, _) => execute_approved_mcp_call(state, &approval, &tool_call).await,
        _ => execute_approved_native_connector_or_generic_tool(state, &approval, &tool_call).await,
    };
    if result.is_ok() {
        let completed = state
            .execution_queue
            .complete_started(job.id, worker_id)
            .await?;
        finalize_remote_computer_assignment_for_job(
            state,
            &job,
            remote_computer_assignment.as_ref(),
            "completed",
            "remote_computer.execution_handoff_completed",
            json!({"execution_job_status": "completed"}),
        )
        .await?;
        record_execution_completed_event(state, &completed).await?;
        Ok(completed)
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

async fn execute_approved_native_connector_or_generic_tool(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    if tool_call.tool_name == "native.connector.call" {
        let connector_id = tool_call
            .args
            .get("connector_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if crate::native_connectors::is_supported_ecommerce_connector(connector_id) {
            return execute_approved_ecommerce_native_connector(state, approval, tool_call).await;
        }
        if crate::native_connectors::is_supported_github_connector(connector_id) {
            return execute_approved_github_native_connector(state, approval, tool_call).await;
        }
    }

    let result = if tool_call.normalized_args_hash.is_some() {
        let token =
            crate::consume_valid_approval_commit_token_for_tool_call(state, approval, tool_call)
                .await?;
        json!({
            "approval": "approved",
            "status": "native_connector_committed",
            "approval_commit_token_id": token.id,
            "normalized_args_hash": token.normalized_args_hash,
            "target_binding": token.target_binding,
        })
    } else {
        json!({"approval": "approved"})
    };
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result), None)
        .await?;
    Ok(())
}

async fn execute_approved_ecommerce_native_connector(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let token =
        crate::consume_valid_approval_commit_token_for_tool_call(state, approval, tool_call)
            .await?;
    let adapter_result =
        crate::native_connectors::execute_ecommerce_connector_call(&tool_call.args).await?;
    let result = json!({
        "approval": "approved",
        "status": "native_connector_committed",
        "approval_commit_token_id": token.id,
        "normalized_args_hash": token.normalized_args_hash,
        "target_binding": token.target_binding,
        "adapter_result": adapter_result,
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
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "worker",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "approval_id": approval.id,
                "approval_commit_token_id": token.id,
                "normalized_args_hash": token.normalized_args_hash,
                "connector_id": tool_call.args.get("connector_id").cloned().unwrap_or(Value::Null),
                "operation": tool_call.args.get("operation").cloned().unwrap_or(Value::Null),
                "adapter_status": result["adapter_result"]["status"],
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_github_native_connector(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let token =
        crate::consume_valid_approval_commit_token_for_tool_call(state, approval, tool_call)
            .await?;
    let adapter_result =
        crate::native_connectors::execute_github_connector_call(&tool_call.args).await?;
    let result = json!({
        "approval": "approved",
        "status": "native_connector_committed",
        "approval_commit_token_id": token.id,
        "normalized_args_hash": token.normalized_args_hash,
        "target_binding": token.target_binding,
        "adapter_result": adapter_result,
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
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "worker",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "approval_id": approval.id,
                "approval_commit_token_id": token.id,
                "normalized_args_hash": token.normalized_args_hash,
                "connector_id": tool_call.args.get("connector_id").cloned().unwrap_or(Value::Null),
                "operation": tool_call.args.get("operation").cloned().unwrap_or(Value::Null),
                "adapter_status": result["adapter_result"]["status"],
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_mcp_call(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let config = state
        .mcp_gateway_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("MCP gateway is not configured"))?;
    let request: McpCallRequest = serde_json::from_value(tool_call.args.clone())?;
    let scoped_server = state
        .mcp_server_for_session_tool(approval.session_id, &request.server, &request.tool)
        .await?;
    let secret_refs_resolved = if let Some(server) = scoped_server.as_ref() {
        resolve_mcp_runtime_secret_refs(server).await?
    } else {
        0
    };
    let token =
        crate::consume_valid_approval_commit_token_for_tool_call(state, approval, tool_call)
            .await?;
    let response = state.mcp_gateway_client.call(config, request).await?;
    let result = json!({
        "approval": "approved",
        "status": "called",
        "approval_commit_token_id": token.id,
        "normalized_args_hash": token.normalized_args_hash,
        "target_binding": token.target_binding,
        "secret_refs_resolved_count": secret_refs_resolved,
        "result": response.result,
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
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "worker",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "approval_id": approval.id,
                "approval_commit_token_id": token.id,
                "normalized_args_hash": token.normalized_args_hash,
                "resumed_after_approval": true
            }),
        ))
        .await?;
    Ok(())
}

async fn record_execution_completed_event(
    state: &AppState,
    job: &ExecutionJob,
) -> Result<(), AppError> {
    let event = state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            "execution.completed",
            json!({
                "execution_job_id": job.id,
                "approval_id": job.approval_id,
                "tool_call_id": job.tool_call_id,
                "tool": job.tool_name,
                "status": job.status,
                "worker_id": job.worker_id,
                "reason": "approved execution completed"
            }),
        )
        .await?;
    project_latest_tool_result_for_execution_job(state, job).await?;
    crate::project_session_event_to_loop(state, &event).await?;
    Ok(())
}

async fn project_latest_tool_result_for_execution_job(
    state: &AppState,
    job: &ExecutionJob,
) -> Result<(), AppError> {
    let events = state.list_events(job.session_id).await?;
    if let Some(tool_result_event) = events.iter().rev().find(|event| {
        event.event_type == "tool.result"
            && event
                .payload
                .get("tool_call_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                == Some(job.tool_call_id)
    }) {
        crate::project_session_event_to_loop(state, tool_result_event).await?;
    }
    Ok(())
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
        .retry_or_fail_started(
            job.id,
            job.worker_id.as_deref().unwrap_or(""),
            &error_message,
        )
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
    let Some(contract) = environment_contract else {
        return Ok(None);
    };
    let active_assignments: Vec<_> = state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .filter(|assignment| assignment.status == "assigned")
        .collect();
    if active_assignments
        .iter()
        .any(|assignment| assignment.session_id == job.session_id)
    {
        return Err(AppError::bad_request(
            "remote_computer session already has an active Remote Computer assignment; retry after the running job finishes",
        ));
    }
    let assigned_lease_ids: HashSet<_> = active_assignments
        .iter()
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
            None => match provision_remote_computer_pod_for_job(state, job, worker_id, contract)
                .await?
            {
                Some(lease) => lease,
                None => return Ok(None),
            },
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
                    "session_workspace_path": remote_session_workspace_path_for_computer_id(
                        state,
                        lease.remote_computer_id,
                        job.session_id
                    )
                    .await?,
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
    let mut candidates = state
        .list_remote_computers()
        .await?
        .into_iter()
        .filter(|computer| {
            computer.status == "available"
                && contract.matches_computer(computer)
                && computer
                    .metadata
                    .get("warm_pool")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|computer| std::cmp::Reverse(computer.updated_at));
    if candidates.is_empty() {
        return Ok(None);
    }
    let mut last_race_error = None;
    let (computer, lease) = loop {
        let Some(computer) = candidates.pop() else {
            return Ok(None);
        };
        match state
            .create_remote_computer_lease(
                computer.id,
                CreateRemoteComputerLease {
                    session_id: Some(job.session_id),
                    worker_id: Some(worker_id.to_string()),
                    lease_seconds: Some(REMOTE_COMPUTER_DEFAULT_LEASE_SECONDS),
                    metadata: Some(json!({
                        "handoff_mode": "environment-warm-pool-lease",
                        "source": "run_execution_job",
                        "execution_job_id": job.id,
                        "tool_call_id": job.tool_call_id,
                        "session_workspace_path": remote_session_workspace_path(&computer, job.session_id),
                        "environment_contract": contract.evidence()
                    })),
                },
            )
            .await
        {
            Ok(lease) => break (computer, lease),
            Err(error) if remote_computer_lease_race_error(&error) => {
                last_race_error = Some(error.message);
                continue;
            }
            Err(error) => return Err(error),
        }
    };
    if let Some(error) = last_race_error {
        state
            .append_event(
                "worker",
                Some(job.id),
                job.session_id,
                "remote_computer.warm_pool_claim_race_recovered",
                json!({
                    "execution_job_id": job.id,
                    "tool_call_id": job.tool_call_id,
                    "recovered_remote_computer_id": computer.id,
                    "last_race_error": error,
                }),
            )
            .await?;
    }
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

/// Creates a Pod on-demand for jobs that find no available warm-pool Remote Computer.
/// Requires the full triple-gate (`mutation_enabled`, `live_mutation_enabled`, `execution_enabled`)
/// plus `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes` to be active; otherwise returns
/// `Ok(None)`. Environment-bound Remote Computer jobs then fail closed at assignment validation
/// instead of falling back to host execution.
async fn provision_remote_computer_pod_for_job(
    state: &AppState,
    job: &ExecutionJob,
    worker_id: &str,
    contract: &RemoteComputerEnvironmentContract,
) -> Result<Option<RemoteComputerLease>, AppError> {
    if !remote_computer_pod_execution_requested() {
        return Ok(None);
    }
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    // Deterministic pod name: lowercase hex, max 63 chars, dashes allowed
    let pod_name = format!("agent-rc-{}", &job.session_id.simple().to_string()[..20]);
    let remote_computer_id = on_demand_remote_computer_id(job.session_id);
    let mut created_pod = false;
    // Check if a remote_computer record already exists for this pod (race guard)
    let existing = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|c| c.pod_name.as_deref() == Some(&pod_name));
    let computer = if let Some(existing_computer) = existing {
        existing_computer
    } else {
        // Attempt to create the Pod via the runner
        let create_response = runner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: Some(remote_computer_id),
                    session_id: Some(job.session_id),
                    pod_name: Some(pod_name.clone()),
                    metadata: Some(json!({
                        "tenant_id": state.current_tenant_id(),
                        "assignment_id": "",
                        "session_workspace_path": remote_session_workspace_path_from_base("/workspace", job.session_id),
                        "artifact_dir": remote_session_artifacts_path_from_base("/workspace", job.session_id),
                        "artifact_discovery_enabled": true
                    })),
                },
            )
            .await;
        // Gates not open or runner blocked; environment-bound callers fail closed at assignment validation.
        if create_response.status == "blocked" {
            return Ok(None);
        }
        if create_response.status != "mutation_ok"
            && !remote_computer_pod_create_already_exists(&create_response)
        {
            return Err(AppError::internal(format!(
                "Remote Computer Pod creation failed: {}",
                create_response.message
            )));
        }
        created_pod = create_response.status == "mutation_ok";
        // Wait for Pod to reach Running phase
        let ready_timeout = Duration::from_secs(
            std::env::var("MANDOFORGE_REMOTE_COMPUTER_POD_READY_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(60),
        );
        let ready_interval = Duration::from_millis(
            std::env::var("MANDOFORGE_REMOTE_COMPUTER_POD_READY_POLL_INTERVAL_MS")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(2000),
        );
        if let Err(error) =
            poll_kubernetes_pod_running(&config, &pod_name, ready_timeout, ready_interval).await
        {
            let cleanup_error = if created_pod {
                cleanup_on_demand_remote_computer_pod_after_failed_provision(
                    state,
                    runner.as_ref(),
                    &config,
                    &pod_name,
                    job,
                    worker_id,
                    "pod_not_ready",
                )
                .await
            } else {
                None
            };
            return Err(remote_computer_pod_provision_error(
                format!("Remote Computer Pod not ready: {error}"),
                cleanup_error,
            ));
        }
        // Persist the remote_computer record so the assignment chain can find pod_name.
        // On concurrent provisioning, a unique-constraint violation can occur — in that
        // case the winner's record is already in the DB; re-read it.
        match state
            .create_remote_computer(CreateRemoteComputer {
                id: Some(remote_computer_id),
                name: pod_name.clone(),
                profile: Some("workspace-write".to_string()),
                namespace: Some(config.namespace.clone()),
                pod_name: Some(pod_name.clone()),
                workspace_path: Some("/workspace".to_string()),
                state_mount_path: Some("/agent-state".to_string()),
                metadata: Some(json!({
                    "on_demand": true,
                    "session_id": job.session_id,
                    "session_workspace_path": remote_session_workspace_path_from_base("/workspace", job.session_id),
                    "provisioned_by_worker": worker_id,
                    "environment_contract": contract.evidence()
                })),
            })
            .await
        {
            Ok(computer) => computer,
            Err(error) => {
                // Duplicate insert — another worker won the race; use the existing record
                if let Some(existing) = state
                    .list_remote_computers()
                    .await?
                    .into_iter()
                    .find(|c| c.pod_name.as_deref() == Some(&pod_name))
                {
                    existing
                } else {
                    if created_pod {
                        let cleanup_error =
                            cleanup_on_demand_remote_computer_pod_after_failed_provision(
                                state,
                                runner.as_ref(),
                                &config,
                                &pod_name,
                                job,
                                worker_id,
                                "remote_computer_record_failed",
                            )
                            .await;
                        return Err(remote_computer_pod_provision_error(
                            error.message,
                            cleanup_error,
                        ));
                    }
                    return Err(error);
                }
            }
        }
    };
    // Create the lease tied to this session
    let lease = match state
        .create_remote_computer_lease(
            computer.id,
            CreateRemoteComputerLease {
                session_id: Some(job.session_id),
                worker_id: Some(worker_id.to_string()),
                lease_seconds: Some(REMOTE_COMPUTER_DEFAULT_LEASE_SECONDS),
                metadata: Some(json!({
                    "handoff_mode": "environment-on-demand-pod",
                    "source": "provision_remote_computer_pod_for_job",
                    "on_demand": true,
                    "execution_job_id": job.id,
                    "tool_call_id": job.tool_call_id,
                    "session_workspace_path": remote_session_workspace_path(&computer, job.session_id),
                    "environment_contract": contract.evidence()
                })),
            },
        )
        .await
    {
        Ok(lease) => lease,
        Err(error) => {
            if remote_computer_lease_race_error(&error) {
                return Err(error);
            }
            if created_pod {
                let cleanup_error = cleanup_on_demand_remote_computer_pod_after_failed_provision(
                    state,
                    runner.as_ref(),
                    &config,
                    &pod_name,
                    job,
                    worker_id,
                    "remote_computer_lease_failed",
                )
                .await;
                let _ = state.delete_remote_computer_if_unleased(computer.id).await;
                return Err(remote_computer_pod_provision_error(
                    error.message,
                    cleanup_error,
                ));
            }
            return Err(error);
        }
    };
    let details = json!({
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "pod_name": pod_name,
        "session_id": lease.session_id,
        "worker_id": lease.worker_id,
        "status": lease.status,
        "lease_expires_at": lease.lease_expires_at,
        "source": "auto-on-demand-pod",
        "environment_contract": contract.evidence(),
        "execution_job_id": job.id,
        "tool_call_id": job.tool_call_id,
    });
    state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            "remote_computer.on_demand_pod_provisioned",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(job.session_id),
            "worker",
            Some(job.id),
            "remote_computer.on_demand_pod_provisioned",
            "remote_computer_lease",
            Some(lease.id),
            details,
        ))
        .await?;
    Ok(Some(lease))
}

async fn cleanup_on_demand_remote_computer_pod_after_failed_provision(
    state: &AppState,
    runner: &dyn RemoteComputerRunner,
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    job: &ExecutionJob,
    worker_id: &str,
    reason: &str,
) -> Option<String> {
    let response = runner
        .mutate(
            config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_delete".to_string()),
                remote_computer_id: None,
                session_id: Some(job.session_id),
                pod_name: Some(pod_name.to_string()),
                metadata: Some(json!({
                    "source": "cleanup_on_demand_remote_computer_pod_after_failed_provision",
                    "reason": reason,
                    "execution_job_id": job.id,
                    "tool_call_id": job.tool_call_id,
                    "worker_id": worker_id,
                })),
            },
        )
        .await;
    let cleanup_failed = response.status != "mutation_ok";
    let event_type = if cleanup_failed {
        "remote_computer.on_demand_pod_cleanup_failed"
    } else {
        "remote_computer.on_demand_pod_cleaned_up"
    };
    let details = json!({
        "pod_name": pod_name,
        "reason": reason,
        "execution_job_id": job.id,
        "tool_call_id": job.tool_call_id,
        "worker_id": worker_id,
        "cleanup_status": response.status,
        "cleanup_message": response.message,
        "cleanup_status_code": response.live_mutation_status_code,
    });
    let _ = state
        .append_event(
            "worker",
            Some(job.id),
            job.session_id,
            event_type,
            details.clone(),
        )
        .await;
    let _ = state
        .append_audit_log(new_audit_log(
            Some(job.session_id),
            "worker",
            Some(job.id),
            event_type,
            "remote_computer_pod",
            None,
            details,
        ))
        .await;
    cleanup_failed.then_some(response.message)
}

fn remote_computer_pod_provision_error(
    message: impl Into<String>,
    cleanup_error: Option<String>,
) -> AppError {
    AppError::internal(remote_computer_pod_provision_error_message(
        message,
        cleanup_error,
    ))
}

fn remote_computer_pod_provision_error_message(
    message: impl Into<String>,
    cleanup_error: Option<String>,
) -> String {
    let message = message.into();
    match cleanup_error {
        Some(cleanup_error) => format!("{message}; cleanup failed: {cleanup_error}"),
        None => message,
    }
}

fn remote_computer_lease_race_error(error: &AppError) -> bool {
    error.status == StatusCode::BAD_REQUEST
        && matches!(
            error.message.as_str(),
            "Remote computer is not available for lease"
                | "Remote computer already has an active lease"
        )
}

fn remote_computer_pod_create_already_exists(
    response: &crate::RemoteComputerRunnerDryRunResponse,
) -> bool {
    response.would_create_pod
        && response.live_mutation_attempted
        && response.live_mutation_status_code == Some(StatusCode::CONFLICT.as_u16())
}

fn on_demand_remote_computer_id(session_id: Uuid) -> Uuid {
    Uuid::from_u128(session_id.as_u128() ^ 0x6d616e646f666f7267655f72635f7631)
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
    let Some(environment_id) = job.environment_id else {
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

struct RemoteComputerPodExecTarget {
    remote_computer: RemoteComputer,
    pod_name: String,
    workspace_path: String,
}

async fn remote_computer_pod_exec_target(
    state: &AppState,
    session_id: Uuid,
    assignment: &crate::RemoteComputerJobAssignment,
) -> Result<RemoteComputerPodExecTarget, AppError> {
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
    let workspace_path = remote_session_workspace_path(&remote_computer, session_id);
    Ok(RemoteComputerPodExecTarget {
        remote_computer,
        pod_name,
        workspace_path,
    })
}

async fn run_remote_computer_pod_exec(
    target: &RemoteComputerPodExecTarget,
    session_id: Uuid,
    command: String,
    metadata: Value,
    missing_output_message: &str,
) -> Result<Value, AppError> {
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let response = runner
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_exec".to_string()),
                remote_computer_id: Some(target.remote_computer.id),
                session_id: Some(session_id),
                pod_name: Some(target.pod_name.clone()),
                metadata: Some(merge_json_object(
                    json!({
                        "command": command,
                        "session_workspace_path": target.workspace_path,
                    }),
                    metadata,
                )),
            },
        )
        .await;
    let exec_result = response.exec_result.clone().ok_or_else(|| {
        AppError::bad_request(format!("{missing_output_message}: {}", response.message))
    })?;
    Ok(exec_result)
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
    let workspace_path = remote_session_workspace_path(&remote_computer, approval.session_id);
    let command = remote_file_write_command(&workspace_path, relative_path, content);
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
                metadata: Some(json!({"command": command, "tool_call_id": tool_call.id, "session_workspace_path": workspace_path})),
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
        "workspace": workspace_path,
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
                "workspace": workspace_path,
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
                "workspace": workspace_path,
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
    let target = remote_computer_pod_exec_target(state, approval.session_id, assignment).await?;
    let remote_command = remote_shell_exec_command(&target.workspace_path, command);
    let exec_result = run_remote_computer_pod_exec(
        &target,
        approval.session_id,
        remote_command,
        json!({"tool_call_id": tool_call.id, "requested_command": command}),
        "Remote Computer Pod exec did not return output",
    )
    .await?;
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
    if !kubernetes_exec_status_succeeded(&status) {
        return Err(AppError::bad_request(format!(
            "Remote Computer shell.exec failed with status {status}"
        )));
    }
    let result = json!({
        "approval": "approved",
        "command": command,
        "runner": "remote_computer_pod_exec",
        "remote_computer_id": target.remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": target.remote_computer.namespace,
        "pod_name": target.pod_name,
        "workspace": target.workspace_path,
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
                "remote_computer_id": target.remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": target.pod_name,
                "workspace": target.workspace_path,
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
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "workspace": target.workspace_path,
                "pod_name": target.pod_name,
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
    let runtime_selection = codex_runtime_selection(state, approval.session_id, &request).await?;
    if runtime_selection.strategy != CodexExecutionStrategy::Cli {
        return Err(AppError::bad_request(
            "remote computer codex.exec requires a codex_cli environment runtime profile",
        ));
    }
    let runtime_binding = codex_runtime_selection_metadata(&runtime_selection);
    let target = remote_computer_pod_exec_target(state, approval.session_id, assignment).await?;
    let command = remote_codex_exec_command(&request, &target.workspace_path);
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
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
                "namespace": target.remote_computer.namespace,
                "pod_name": target.pod_name,
                "workspace": target.workspace_path,
                "runtime_binding": runtime_binding,
            }),
        )
        .await?;
    let exec_result = run_remote_computer_pod_exec(
        &target,
        approval.session_id,
        command,
        json!({"tool_call_id": tool_call.id}),
        "Remote Computer Codex exec did not return output",
    )
    .await?;

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
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "runtime_binding": runtime_binding,
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

    let exec_succeeded = kubernetes_exec_status_succeeded(&status);
    let event_type = if exec_succeeded {
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
                "remote_computer_id": target.remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": target.pod_name,
                "tool": tool_call.tool_name,
                "workspace": target.workspace_path,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "status": status,
                "execution_enabled": true,
                "runtime_binding": runtime_binding,
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
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
                "runtime_binding": runtime_binding,
            }),
        )
        .await?;
    let result = json!({
        "runner": "remote_computer_pod_exec",
        "remote_computer_id": target.remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": target.remote_computer.namespace,
        "pod_name": target.pod_name,
        "workspace": target.workspace_path,
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
        "runtime_binding": runtime_binding,
    });
    if !exec_succeeded {
        let error_payload = json!({
            "error": "Remote Computer Codex exec failed",
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
                json!({
                    "tool": tool_call.tool_name,
                    "runner": "remote_computer_pod_exec",
                    "remote_computer_id": target.remote_computer.id,
                    "assignment_id": assignment.id,
                    "workspace": target.workspace_path,
                    "pod_name": target.pod_name,
                    "status": status,
                    "stdout_chars": stdout.text.chars().count(),
                    "stderr_chars": stderr.text.chars().count(),
                    "final_message_chars": final_output.text.chars().count(),
                    "runtime_binding": runtime_binding,
                    "resumed_after_approval": true
                }),
            ))
            .await?;
        return Err(AppError::bad_request("Remote Computer Codex exec failed"));
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
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "workspace": target.workspace_path,
                "pod_name": target.pod_name,
                "status": status,
                "stdout_chars": stdout.text.chars().count(),
                "stderr_chars": stderr.text.chars().count(),
                "final_message_chars": final_output.text.chars().count(),
                "runtime_binding": runtime_binding,
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
    let target = remote_computer_pod_exec_target(state, approval.session_id, assignment).await?;
    let profile_config = agent_cli_profile_config(state, &profile).await?;
    let bound_profile =
        enforce_bound_agent_cli_profile(state, approval.session_id, &profile).await?;
    let runtime_binding_source = bound_profile.source;
    let profile_source = match profile_config.source {
        AgentCliProfileConfigSource::Managed => "managed",
        AgentCliProfileConfigSource::Environment => "environment",
    };
    let runtime_type = profile_config.runtime_type.clone();
    let command = remote_agent_cli_exec_command(&request, &profile_config, &target.workspace_path)?;
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "agent_cli.task.started",
            json!({
                "profile": profile,
                "profile_source": profile_source,
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
                "task": &request.task,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
                "namespace": target.remote_computer.namespace,
                "pod_name": target.pod_name,
                "workspace": target.workspace_path,
            }),
        )
        .await?;
    let exec_result = run_remote_computer_pod_exec(
        &target,
        approval.session_id,
        command,
        json!({"tool_call_id": tool_call.id, "profile": profile, "profile_source": profile_source, "runtime_binding_source": runtime_binding_source, "runtime_type": runtime_type}),
        "Remote Computer agent CLI exec did not return output",
    )
    .await?;

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
    let stdout_full = exec_result
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let adapter_events = parse_runtime_adapter_events(&runtime_type, stdout_full);
    record_runtime_adapter_events(
        state,
        approval.session_id,
        &profile,
        &profile_config,
        &adapter_events,
        Some(tool_call.id),
    )
    .await?;
    let turn_recording = record_runtime_adapter_turn_metadata(
        state,
        approval.session_id,
        &profile,
        &profile_config,
        &adapter_events,
        &request.args,
        Some(tool_call.id),
    )
    .await?;
    let status = exec_result.get("status").cloned().unwrap_or(Value::Null);
    let exec_succeeded = kubernetes_exec_status_succeeded(&status);
    let event_type = if exec_succeeded {
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
                "remote_computer_id": target.remote_computer.id,
                "lease_id": assignment.lease_id,
                "pod_name": target.pod_name,
                "tool": tool_call.tool_name,
                "profile": profile,
                "workspace": target.workspace_path,
                "profile_source": profile_source,
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
                "stdout_bytes": stdout.original_bytes,
                "stderr_bytes": stderr.original_bytes,
                "runtime_adapter_event_count": adapter_events.len(),
                "runtime_turn_event_count": turn_recording.event_count,
                "runtime_final_artifact_count": turn_recording.final_artifact_count,
                "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
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
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
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
                "runtime_adapter_event_count": adapter_events.len(),
                "runtime_turn_event_count": turn_recording.event_count,
                "runtime_final_artifact_count": turn_recording.final_artifact_count,
                "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "lease_id": assignment.lease_id,
            }),
        )
        .await?;
    let result = json!({
        "runner": "remote_computer_pod_exec",
        "profile": profile,
        "profile_source": profile_source,
        "runtime_binding_source": runtime_binding_source,
        "runtime_type": runtime_type,
        "remote_computer_id": target.remote_computer.id,
        "assignment_id": assignment.id,
        "lease_id": assignment.lease_id,
        "namespace": target.remote_computer.namespace,
        "pod_name": target.pod_name,
        "workspace": target.workspace_path,
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
        "runtime_adapter_event_count": adapter_events.len(),
        "runtime_turn_event_count": turn_recording.event_count,
        "runtime_final_artifact_count": turn_recording.final_artifact_count,
        "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
    });
    if !exec_succeeded {
        let error_payload = json!({
            "error": "Remote Computer agent CLI exec failed",
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
                json!({
                    "tool": tool_call.tool_name,
                    "profile": profile,
                    "profile_source": profile_source,
                    "runtime_binding_source": runtime_binding_source,
                    "runtime_type": runtime_type,
                    "runner": "remote_computer_pod_exec",
                    "remote_computer_id": target.remote_computer.id,
                    "assignment_id": assignment.id,
                    "workspace": target.workspace_path,
                    "pod_name": target.pod_name,
                    "status": status,
                    "stdout_chars": stdout.text.chars().count(),
                    "stderr_chars": stderr.text.chars().count(),
                    "runtime_adapter_event_count": adapter_events.len(),
                    "runtime_turn_event_count": turn_recording.event_count,
                    "runtime_final_artifact_count": turn_recording.final_artifact_count,
                    "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
                    "resumed_after_approval": true
                }),
            ))
            .await?;
        return Err(AppError::bad_request(
            "Remote Computer agent CLI exec failed",
        ));
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
                "profile_source": profile_source,
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
                "runner": "remote_computer_pod_exec",
                "remote_computer_id": target.remote_computer.id,
                "assignment_id": assignment.id,
                "workspace": target.workspace_path,
                "pod_name": target.pod_name,
                "status": status,
                "stdout_chars": stdout.text.chars().count(),
                "stderr_chars": stderr.text.chars().count(),
                "runtime_adapter_event_count": adapter_events.len(),
                "runtime_turn_event_count": turn_recording.event_count,
                "runtime_final_artifact_count": turn_recording.final_artifact_count,
                "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
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
                    json!({
                        "tool": tool_call.tool_name,
                        "runtime_binding": result.get("runtime_binding").cloned().unwrap_or(Value::Null),
                        "resumed_after_approval": true
                    }),
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
    let runtime_binding_source = match normalize_agent_cli_profile(&request.profile) {
        Ok(profile) => enforce_bound_agent_cli_profile(state, approval.session_id, &profile)
            .await
            .ok()
            .map(|bound_profile| bound_profile.source),
        Err(_) => None,
    };
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
                    json!({
                        "tool": tool_call.tool_name,
                        "profile": result.get("profile"),
                        "profile_source": result.get("profile_source"),
                        "runtime_binding_source": result.get("runtime_binding_source"),
                        "runtime_type": result.get("runtime_type"),
                        "runner": result.get("runner"),
                        "runtime_adapter_event_count": result.get("runtime_adapter_event_count"),
                        "runtime_turn_event_count": result.get("runtime_turn_event_count"),
                        "runtime_final_artifact_count": result.get("runtime_final_artifact_count"),
                        "runtime_final_artifact_ids": result.get("runtime_final_artifact_ids"),
                        "resumed_after_approval": true
                    }),
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
                    json!({
                        "tool": tool_call.tool_name,
                        "runtime_binding_source": runtime_binding_source,
                        "error": error_payload,
                        "resumed_after_approval": true
                    }),
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

async fn remote_session_workspace_path_for_computer_id(
    state: &AppState,
    remote_computer_id: Uuid,
    session_id: Uuid,
) -> Result<String, AppError> {
    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    Ok(remote_session_workspace_path(&remote_computer, session_id))
}

fn remote_session_workspace_path(remote_computer: &RemoteComputer, session_id: Uuid) -> String {
    remote_session_workspace_path_from_base(&remote_computer.workspace_path, session_id)
}

fn remote_session_workspace_path_from_base(base_path: &str, session_id: Uuid) -> String {
    let base_path = normalize_remote_workspace_base_path(base_path);
    format!("{base_path}/sessions/{session_id}")
}

fn remote_session_artifacts_path_from_base(base_path: &str, session_id: Uuid) -> String {
    format!(
        "{}/artifacts",
        remote_session_workspace_path_from_base(base_path, session_id)
    )
}

fn normalize_remote_workspace_base_path(base_path: &str) -> String {
    let trimmed = base_path.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "/workspace".to_string()
    } else {
        trimmed.to_string()
    }
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
    let runtime_selection = codex_runtime_selection(state, session_id, &request).await?;
    match runtime_selection.strategy {
        CodexExecutionStrategy::Cli => {
            run_codex_cli(state, session_id, request, &runtime_selection).await
        }
        CodexExecutionStrategy::AppServer => {
            run_codex_app_server(state, session_id, request, &runtime_selection).await
        }
        CodexExecutionStrategy::Auto => {
            if state.codex_app_server_config.is_none() {
                return run_codex_cli(state, session_id, request, &runtime_selection).await;
            }
            let fallback_request = request.clone();
            match run_codex_app_server(state, session_id, request, &runtime_selection).await {
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
                    run_codex_cli(state, session_id, fallback_request, &runtime_selection).await
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

#[derive(Debug, Clone)]
struct CodexRuntimeSelection {
    strategy: CodexExecutionStrategy,
    binding_source: &'static str,
    runtime_profile_id: Option<Uuid>,
    runtime_profile_name: Option<String>,
    runtime_type: Option<String>,
}

async fn codex_runtime_selection(
    state: &AppState,
    session_id: Uuid,
    request: &CodexRequest,
) -> Result<CodexRuntimeSelection, AppError> {
    let requested_strategy = codex_execution_strategy(request)?;
    if let Some(profile) = bound_environment_runtime_profile(state, session_id).await? {
        let bound_strategy = codex_strategy_for_runtime_profile(&profile)?;
        if request.execution_strategy.is_some() && requested_strategy != bound_strategy {
            return Err(AppError::bad_request(format!(
                "codex.exec execution_strategy must match session environment runtime profile: requested {}, bound {}",
                requested_strategy.as_str(),
                profile.runtime_type
            )));
        }
        return Ok(CodexRuntimeSelection {
            strategy: bound_strategy,
            binding_source: "environment",
            runtime_profile_id: Some(profile.id),
            runtime_profile_name: Some(profile.name),
            runtime_type: Some(profile.runtime_type),
        });
    }
    Ok(CodexRuntimeSelection {
        strategy: requested_strategy,
        binding_source: "request_or_env",
        runtime_profile_id: None,
        runtime_profile_name: None,
        runtime_type: None,
    })
}

async fn bound_environment_runtime_profile(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<AgentRuntimeProfile>, AppError> {
    let session = state.get_session(session_id).await?;
    let Some(environment_id) = session.environment_id else {
        return Ok(None);
    };
    let environment = state.get_environment(environment_id).await?;
    let Some(profile_id) = environment.runtime_profile_id else {
        return Ok(None);
    };
    state.get_agent_runtime_profile(profile_id).await.map(Some)
}

fn codex_strategy_for_runtime_profile(
    profile: &AgentRuntimeProfile,
) -> Result<CodexExecutionStrategy, AppError> {
    match profile.runtime_type.as_str() {
        "codex_cli" => Ok(CodexExecutionStrategy::Cli),
        "codex_app_server" => Ok(CodexExecutionStrategy::AppServer),
        runtime_type => Err(AppError::bad_request(format!(
            "codex.exec requires a codex runtime profile, found session environment runtime type {runtime_type}"
        ))),
    }
}

impl CodexExecutionStrategy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cli => "cli",
            Self::AppServer => "app-server",
        }
    }
}

fn codex_runtime_selection_metadata(selection: &CodexRuntimeSelection) -> Value {
    json!({
        "runtime_binding_source": selection.binding_source,
        "runtime_profile_id": selection.runtime_profile_id,
        "runtime_profile_name": selection.runtime_profile_name,
        "runtime_type": selection.runtime_type,
    })
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

fn remote_codex_exec_command(request: &CodexRequest, workspace_path: &str) -> String {
    let final_path = format!("{workspace_path}/.mandoforge/codex-final-message.md");
    format!(
        "set -u\nmkdir -p {workspace}/.mandoforge\ncd {workspace}\ncodex exec --sandbox {} --json --output-last-message {} --cd {workspace} {}\ncode=$?\nprintf '\\n{}\\n'\ncat {} 2>/dev/null || true\nprintf '\\n{}\\n'\nexit $code",
        shell_single_quote(&request.sandbox_mode),
        shell_single_quote(&final_path),
        shell_single_quote(&request.task),
        REMOTE_CODEX_FINAL_BEGIN,
        shell_single_quote(&final_path),
        REMOTE_CODEX_FINAL_END,
        workspace = shell_single_quote(workspace_path),
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn remote_agent_cli_exec_command(
    request: &AgentCliRequest,
    config: &AgentCliProfileConfig,
    workspace_path: &str,
) -> Result<String, AppError> {
    let profile = normalize_agent_cli_profile(&request.profile)?;
    if config.source == AgentCliProfileConfigSource::Managed {
        let mut command = String::new();
        command.push_str(&format!(
            "set -eu\nmkdir -p {workspace}\ncd {workspace}\n",
            workspace = shell_single_quote(workspace_path)
        ));
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
    command.push_str(&format!(
        "set -eu\nmkdir -p {workspace}\ncd {workspace}\nagent_cli_profile=",
        workspace = shell_single_quote(workspace_path)
    ));
    command.push_str(&shell_single_quote(&profile));
    command.push('\n');
    command.push_str(
        "allowed=\",${MANDOFORGE_AGENT_CLI_ALLOWED_PROFILES:-},\"\n\
case \"$allowed\" in *\",$agent_cli_profile,\"*) ;; *) echo \"agent CLI profile is not allowlisted: $agent_cli_profile\" >&2; exit 64 ;; esac\n\
command_var=\"MANDOFORGE_AGENT_CLI_$(printf '%s' \"$agent_cli_profile\" | tr '[:lower:]-' '[:upper:]_')_COMMAND\"\n\
args_var=\"MANDOFORGE_AGENT_CLI_$(printf '%s' \"$agent_cli_profile\" | tr '[:lower:]-' '[:upper:]_')_ARGS\"\n\
agent_command=\"$(printenv \"$command_var\" 2>/dev/null || true)\"\n\
agent_args=\"$(printenv \"$args_var\" 2>/dev/null || true)\"\n\
if [ -z \"$agent_command\" ]; then echo \"agent CLI profile $agent_cli_profile is missing $command_var\" >&2; exit 64; fi\n",
    );
    command.push_str("set --\n");
    command.push_str("if [ -n \"$agent_args\" ]; then\n  # Profile args intentionally use simple whitespace splitting; wrap complex CLIs in a shim.\n  set -f\n  set -- $agent_args\n  set +f\nfi\n");
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

fn remote_shell_exec_command(workspace_path: &str, command: &str) -> String {
    format!(
        "set -e\nmkdir -p {workspace}\ncd {workspace}\n{command}",
        workspace = shell_single_quote(workspace_path),
        command = command
    )
}

fn remote_file_write_command(workspace_path: &str, relative_path: &str, content: &str) -> String {
    let delimiter = heredoc_delimiter(content);
    format!(
        "set -eu\nmkdir -p {workspace}\ncd {workspace}\nmkdir -p -- \"$(dirname -- {})\"\ncat > {} <<'{}'\n{}\n{}\nprintf 'wrote file %s\\n' {}",
        shell_single_quote(relative_path),
        shell_single_quote(relative_path),
        delimiter,
        content,
        delimiter,
        shell_single_quote(relative_path),
        workspace = shell_single_quote(workspace_path)
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
    runtime_selection: &CodexRuntimeSelection,
) -> Result<Value, AppError> {
    let config = state
        .codex_app_server_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("Codex App Server is not configured"))?;
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let runtime_binding = codex_runtime_selection_metadata(runtime_selection);
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
                "runtime_binding": runtime_binding,
            }),
        )
        .await?;

    let thread_request = CodexThreadRequest {
        metadata: json!({
            "session_id": session_id,
            "workspace": workspace,
            "sandbox_mode": &request.sandbox_mode,
            "source": "approved_codex_exec",
            "runtime_binding": runtime_binding,
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
            "runtime_binding": runtime_binding,
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
    record_codex_app_server_runtime_turn_started(
        state,
        session_id,
        turn_run.id,
        &thread.thread_id,
        &turn,
        &turn_request,
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
                "runtime_binding": runtime_binding,
            }),
        )
        .await?;

    if poll_result.terminal {
        record_codex_app_server_runtime_turn_completed(
            state,
            session_id,
            turn_run.id,
            &thread.thread_id,
            &final_turn,
            &final_status,
            poll_result.attempts,
        )
        .await?;
    }

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
        "runtime_binding": runtime_binding,
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
                record_codex_app_server_runtime_item(
                    state, session_id, run_id, &turn, attempts, &status, terminal,
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

async fn record_codex_app_server_runtime_turn_started(
    state: &AppState,
    session_id: Uuid,
    run_id: Uuid,
    thread_id: &str,
    turn: &CodexTurnResponse,
    turn_request: &CodexTurnRequest,
) -> Result<(), AppError> {
    state
        .append_event(
            "runtime_adapter",
            Some(run_id),
            session_id,
            "runtime.turn.started",
            json!({
                "profile": "codex-app-server",
                "runtime_type": "codex_app_server",
                "source": "codex_app_server",
                "source_operation": "turn.create",
                "run_id": run_id,
                "thread_id": thread_id,
                "turn_id": turn.turn_id,
                "status": turn.status,
                "resume_handle": {
                    "source": "codex_app_server",
                    "thread_id": thread_id,
                    "turn_id": turn.turn_id,
                },
                "request": {
                    "message": turn_request.message,
                    "metadata": turn_request.metadata,
                },
            }),
        )
        .await
        .map(|_| ())
}

async fn record_codex_app_server_runtime_item(
    state: &AppState,
    session_id: Uuid,
    run_id: Uuid,
    turn: &CodexTurnResponse,
    attempt: u32,
    status: &str,
    terminal: bool,
) -> Result<(), AppError> {
    state
        .append_event(
            "runtime_adapter",
            Some(run_id),
            session_id,
            "runtime.item",
            json!({
                "profile": "codex-app-server",
                "runtime_type": "codex_app_server",
                "source": "codex_app_server",
                "source_operation": "turn.poll",
                "run_id": run_id,
                "turn_id": turn.turn_id,
                "thread_id": turn.thread_id,
                "item": {
                    "attempt": attempt,
                    "status": status,
                    "terminal": terminal,
                    "result": turn.result,
                },
            }),
        )
        .await
        .map(|_| ())
}

async fn record_codex_app_server_runtime_turn_completed(
    state: &AppState,
    session_id: Uuid,
    run_id: Uuid,
    thread_id: &str,
    turn: &CodexTurnResponse,
    status: &str,
    poll_attempts: u32,
) -> Result<(), AppError> {
    let usage = runtime_adapter_usage(&turn.result);
    let tool_calls = codex_app_server_tool_calls(&turn.result);
    for (index, tool_call) in tool_calls.iter().enumerate() {
        state
            .append_event(
                "runtime_adapter",
                Some(run_id),
                session_id,
                "runtime.tool_call",
                json!({
                    "profile": "codex-app-server",
                    "runtime_type": "codex_app_server",
                    "source": "codex_app_server",
                    "source_operation": "turn.completed",
                    "run_id": run_id,
                    "thread_id": thread_id,
                    "turn_id": turn.turn_id,
                    "tool_call_index": index,
                    "tool_call": tool_call,
                }),
            )
            .await?;
    }
    if let Some(usage) = usage.as_ref() {
        state
            .append_event(
                "runtime_adapter",
                Some(run_id),
                session_id,
                "runtime.usage",
                json!({
                    "profile": "codex-app-server",
                    "runtime_type": "codex_app_server",
                    "source": "codex_app_server",
                    "source_operation": "turn.completed",
                    "run_id": run_id,
                    "thread_id": thread_id,
                    "turn_id": turn.turn_id,
                    "usage": usage,
                }),
            )
            .await?;
    }

    let final_message = codex_app_server_final_message(&turn.result)
        .unwrap_or_else(|| status.trim().to_string())
        .trim()
        .to_string();
    let final_output = truncate_output(&final_message, execution_output_limit_bytes());
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id,
        artifact_type: "markdown".to_string(),
        name: "codex-app-server-runtime-final-message.md".to_string(),
        path: Some("codex-app-server-runtime-final-message.md".to_string()),
        content: json!({
            "markdown": final_output.text,
            "markdown_bytes": final_output.original_bytes,
            "markdown_truncated": final_output.truncated,
            "profile": "codex-app-server",
            "runtime_type": "codex_app_server",
            "turn_id": turn.turn_id,
            "thread_id": thread_id,
            "source": "codex_app_server",
            "run_id": run_id,
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
            json!({
                "artifact_id": artifact.id,
                "name": artifact.name,
                "path": artifact.path,
                "artifact_type": artifact.artifact_type,
                "source": "runtime.final",
                "runtime_type": "codex_app_server",
                "thread_id": thread_id,
                "turn_id": turn.turn_id,
                "run_id": run_id,
            }),
        )
        .await?;
    state
        .append_event(
            "runtime_adapter",
            Some(run_id),
            session_id,
            "runtime.final",
            json!({
                "profile": "codex-app-server",
                "runtime_type": "codex_app_server",
                "source": "codex_app_server",
                "source_operation": "turn.completed",
                "run_id": run_id,
                "thread_id": thread_id,
                "turn_id": turn.turn_id,
                "status": status,
                "final_message": final_message,
                "artifact_id": artifact.id,
            }),
        )
        .await?;
    state
        .append_event(
            "runtime_adapter",
            Some(run_id),
            session_id,
            "runtime.turn.completed",
            json!({
                "profile": "codex-app-server",
                "runtime_type": "codex_app_server",
                "source": "codex_app_server",
                "source_operation": "turn.completed",
                "run_id": run_id,
                "thread_id": thread_id,
                "turn_id": turn.turn_id,
                "status": status,
                "poll_attempts": poll_attempts,
                "usage": usage,
                "tool_call_count": tool_calls.len(),
                "final_artifact_id": artifact.id,
            }),
        )
        .await
        .map(|_| ())
}

fn codex_app_server_final_message(result: &Value) -> Option<String> {
    if let Some(message) = result.as_str() {
        return non_empty_string(message);
    }
    for key in [
        "final_message",
        "message",
        "content",
        "output",
        "result",
        "text",
    ] {
        if let Some(value) = result.get(key) {
            if let Some(message) = value.as_str()
                && let Some(message) = non_empty_string(message)
            {
                return Some(message);
            }
            if let Some(message) = string_value_at(value, &["content"])
                && let Some(message) = non_empty_string(&message)
            {
                return Some(message);
            }
        }
    }
    if !result.is_null() {
        serde_json::to_string(result)
            .ok()
            .and_then(|message| non_empty_string(&message))
    } else {
        None
    }
}

fn codex_app_server_tool_calls(result: &Value) -> Vec<Value> {
    [
        result.get("tool_calls"),
        result.get("toolCalls"),
        result.pointer("/result/tool_calls"),
        result.pointer("/result/toolCalls"),
    ]
    .into_iter()
    .flatten()
    .find_map(Value::as_array)
    .map(|calls| {
        calls
            .iter()
            .filter(|call| !call.is_null())
            .cloned()
            .collect()
    })
    .unwrap_or_default()
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[allow(dead_code)]
pub(crate) async fn run_agent_cli(
    state: &AppState,
    session_id: Uuid,
    request: AgentCliRequest,
) -> Result<Value, AppError> {
    let profile = normalize_agent_cli_profile(&request.profile)?;
    let config = agent_cli_profile_config(state, &profile).await?;
    let bound_profile = enforce_bound_agent_cli_profile(state, session_id, &profile).await?;
    let runtime_binding_source = bound_profile.source;
    let runtime_type = config.runtime_type.clone();
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
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
                "task": &request.task,
                "workspace": workspace,
                "runner": "agent-cli"
            }),
        )
        .await?;

    let mut command = Command::new(&config.command);
    command.current_dir(&workspace);
    for arg in &config.args {
        command.arg(arg);
    }
    let request_args = request.args.clone();
    for arg in &request_args {
        command.arg(arg);
    }
    command.arg(&request.task);
    command.env("MANDOFORGE_AGENT_CLI_PROFILE", &profile);
    command.env("MANDOFORGE_AGENT_TASK", &request.task);
    for (key, value) in &config.env {
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
    let adapter_events = parse_runtime_adapter_events(&runtime_type, &stdout_full);
    record_runtime_adapter_events(state, session_id, &profile, &config, &adapter_events, None)
        .await?;
    let turn_recording = record_runtime_adapter_turn_metadata(
        state,
        session_id,
        &profile,
        &config,
        &adapter_events,
        &request_args,
        None,
    )
    .await?;
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
                "runtime_binding_source": runtime_binding_source,
                "runtime_type": runtime_type,
                "exit_code": output.status.code(),
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout.truncated,
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr.truncated,
                "runtime_adapter_event_count": adapter_events.len(),
                "runtime_turn_event_count": turn_recording.event_count,
                "runtime_final_artifact_count": turn_recording.final_artifact_count,
                "runtime_final_artifact_ids": turn_recording.final_artifact_ids,
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
        "runtime_binding_source": runtime_binding_source,
        "runtime_type": runtime_type,
        "status": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated,
        "runtime_adapter_event_count": adapter_events.len(),
        "runtime_turn_event_count": turn_recording.event_count,
        "runtime_final_artifact_count": turn_recording.final_artifact_count,
        "runtime_final_artifact_ids": turn_recording.final_artifact_ids
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
    runtime_type: String,
    source: AgentCliProfileConfigSource,
}

#[derive(Debug, Clone)]
struct BoundAgentCliProfile {
    name: String,
    source: &'static str,
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
) -> Result<BoundAgentCliProfile, AppError> {
    let Some(bound_profile) = bound_agent_cli_profile(state, session_id).await? else {
        if env_flag("MANDOFORGE_ALLOW_REQUESTED_AGENT_CLI_PROFILE") {
            return Ok(BoundAgentCliProfile {
                name: requested_profile.to_string(),
                source: "requested",
            });
        }
        return Err(AppError::bad_request(
            "agent_cli.exec requires a session-bound runtime profile",
        ));
    };
    let bound_profile_name = normalize_agent_cli_profile(&bound_profile.name)?;
    if bound_profile_name != requested_profile {
        return Err(AppError::bad_request(format!(
            "agent_cli.exec profile must match session {} runtime profile: requested {requested_profile}, bound {bound_profile_name}",
            bound_profile.source
        )));
    }
    Ok(bound_profile)
}

async fn bound_agent_cli_profile(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<BoundAgentCliProfile>, AppError> {
    let session = state.get_session(session_id).await?;
    if let Some(environment_id) = session.environment_id {
        let environment = state.get_environment(environment_id).await?;
        if let Some(profile_id) = environment.runtime_profile_id {
            let profile = state.get_agent_runtime_profile(profile_id).await?;
            return Ok(Some(BoundAgentCliProfile {
                name: profile.name,
                source: "environment",
            }));
        }
    }

    if let Some(assignment) = state
        .list_agent_handoff_assignments(Some(session_id))
        .await?
        .into_iter()
        .filter(|assignment| assignment.specialist_session_id == session_id)
        .max_by_key(|assignment| assignment.created_at)
        && let Some(profile_id) = assignment.runtime_profile_id
    {
        let profile = state.get_agent_runtime_profile(profile_id).await?;
        return Ok(Some(BoundAgentCliProfile {
            name: profile.name,
            source: "handoff",
        }));
    }

    let agent = state.get_agent(session.agent_id).await?;
    match agent.runtime_profile_id {
        Some(profile_id) => {
            let profile = state.get_agent_runtime_profile(profile_id).await?;
            Ok(Some(BoundAgentCliProfile {
                name: profile.name,
                source: "agent",
            }))
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
        if !agent_runtime_profile_is_cli_executable(&managed_profile.runtime_type) {
            return Err(AppError::bad_request(format!(
                "agent runtime profile {profile} is not executable through agent_cli.exec"
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
            runtime_type: managed_profile.runtime_type,
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
        runtime_type: "agent_cli".to_string(),
        source: AgentCliProfileConfigSource::Environment,
    })
}

fn agent_runtime_profile_is_cli_executable(runtime_type: &str) -> bool {
    matches!(
        runtime_type,
        "agent_cli" | "codex_cli" | "claude_code" | "gemini" | "opencode" | "aider"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeAdapterLogMode {
    CodexJsonl,
    ClaudeStreamJson,
    GenericJsonl,
    Stdout,
}

#[derive(Debug, Clone)]
struct RuntimeAdapterEvent {
    index: usize,
    adapter_event_type: String,
    event: Value,
}

fn runtime_adapter_log_mode(runtime_type: &str) -> RuntimeAdapterLogMode {
    match runtime_type {
        "codex_cli" => RuntimeAdapterLogMode::CodexJsonl,
        "claude_code" => RuntimeAdapterLogMode::ClaudeStreamJson,
        "gemini" | "opencode" | "aider" => RuntimeAdapterLogMode::GenericJsonl,
        _ => RuntimeAdapterLogMode::Stdout,
    }
}

fn runtime_adapter_log_mode_name(mode: RuntimeAdapterLogMode) -> &'static str {
    match mode {
        RuntimeAdapterLogMode::CodexJsonl => "codex_jsonl",
        RuntimeAdapterLogMode::ClaudeStreamJson => "claude_stream_json",
        RuntimeAdapterLogMode::GenericJsonl => "generic_jsonl",
        RuntimeAdapterLogMode::Stdout => "stdout",
    }
}

fn parse_runtime_adapter_events(runtime_type: &str, stdout: &str) -> Vec<RuntimeAdapterEvent> {
    let mode = runtime_adapter_log_mode(runtime_type);
    if mode == RuntimeAdapterLogMode::Stdout {
        return Vec::new();
    }
    let parse_limit = runtime_adapter_event_limit().saturating_add(1);
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(line).ok()
        })
        .take(parse_limit)
        .enumerate()
        .map(|(index, event)| {
            let event = redact_runtime_adapter_event(event);
            RuntimeAdapterEvent {
                index,
                adapter_event_type: runtime_adapter_event_type(&event),
                event,
            }
        })
        .collect()
}

fn runtime_adapter_event_type(event: &Value) -> String {
    event
        .get("type")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("event_type"))
        .or_else(|| event.get("subtype"))
        .or_else(|| event.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn redact_runtime_adapter_event(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if runtime_adapter_redacts_key(&lower) {
                        (key, Value::String("[REDACTED]".to_string()))
                    } else {
                        (key, redact_runtime_adapter_event(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(redact_runtime_adapter_event)
                .collect(),
        ),
        value => value,
    }
}

fn runtime_adapter_redacts_key(lower_key: &str) -> bool {
    if runtime_adapter_usage_counter_key(lower_key) {
        return false;
    }
    lower_key.contains("token")
        || lower_key.contains("secret")
        || lower_key.contains("password")
        || lower_key.contains("api_key")
        || lower_key.contains("apikey")
        || lower_key.contains("auth")
        || lower_key.contains("credential")
}

fn runtime_adapter_usage_counter_key(lower_key: &str) -> bool {
    matches!(
        lower_key,
        "input_tokens"
            | "output_tokens"
            | "prompt_tokens"
            | "completion_tokens"
            | "cached_input_tokens"
            | "reasoning_output_tokens"
            | "total_tokens"
    )
}

async fn record_runtime_adapter_events(
    state: &AppState,
    session_id: Uuid,
    profile: &str,
    config: &AgentCliProfileConfig,
    events: &[RuntimeAdapterEvent],
    actor_id: Option<Uuid>,
) -> Result<(), AppError> {
    if events.is_empty() {
        return Ok(());
    }
    let mode = runtime_adapter_log_mode(config.runtime_type.as_str());
    let limit = runtime_adapter_event_limit();
    for event in events.iter().take(limit) {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime_adapter.event",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "log_mode": runtime_adapter_log_mode_name(mode),
                    "adapter_event_type": &event.adapter_event_type,
                    "event_index": event.index,
                    "event": &event.event,
                }),
            )
            .await?;
    }
    if events.len() > limit {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime_adapter.events_truncated",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "log_mode": runtime_adapter_log_mode_name(mode),
                    "event_count": events.len(),
                    "recorded_event_count": limit,
                }),
            )
            .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct RuntimeAdapterTurnRecording {
    event_count: usize,
    final_artifact_count: usize,
    final_artifact_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
struct RuntimeAdapterTurnMetadataValue {
    event_index: usize,
    value: Value,
}

#[derive(Debug, Clone, Default)]
struct RuntimeAdapterTurnMetadata {
    turn_id: Option<String>,
    resume_handle: Option<Value>,
    output_schema: Option<Value>,
    output_schema_validation: Option<Value>,
    usage: Option<Value>,
    timing: Option<Value>,
    duration_ms: Option<i64>,
    status: Option<String>,
    started_event_index: Option<usize>,
    completed_event_index: Option<usize>,
    final_message: Option<String>,
    final_message_event_index: Option<usize>,
    items: Vec<RuntimeAdapterTurnMetadataValue>,
    tool_calls: Vec<RuntimeAdapterTurnMetadataValue>,
    usage_events: Vec<RuntimeAdapterTurnMetadataValue>,
}

impl RuntimeAdapterTurnMetadata {
    fn has_metadata(&self) -> bool {
        self.started_event_index.is_some()
            || self.completed_event_index.is_some()
            || self.resume_handle.is_some()
            || self.output_schema.is_some()
            || self.output_schema_validation.is_some()
            || self.usage.is_some()
            || self.timing.is_some()
            || self.final_message.is_some()
            || !self.items.is_empty()
            || !self.tool_calls.is_empty()
            || !self.usage_events.is_empty()
    }
}

async fn record_runtime_adapter_turn_metadata(
    state: &AppState,
    session_id: Uuid,
    profile: &str,
    config: &AgentCliProfileConfig,
    events: &[RuntimeAdapterEvent],
    request_args: &[String],
    actor_id: Option<Uuid>,
) -> Result<RuntimeAdapterTurnRecording, AppError> {
    if !runtime_adapter_turn_metadata_supported(&config.runtime_type) {
        return Ok(RuntimeAdapterTurnRecording::default());
    }
    let metadata = build_runtime_adapter_turn_metadata(events, request_args);
    if !metadata.has_metadata() {
        return Ok(RuntimeAdapterTurnRecording::default());
    }

    let mut recorded_event_count = 0;
    let mut final_artifact_count = 0;
    let mut final_artifact_id = None;
    let mut final_artifact_ids = Vec::new();

    if let Some(started_event_index) = metadata.started_event_index {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.turn.started",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "resume_handle": &metadata.resume_handle,
                    "output_schema": &metadata.output_schema,
                    "timing": &metadata.timing,
                    "source_event_index": started_event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    for item in &metadata.items {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.item",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "item": &item.value,
                    "source_event_index": item.event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    for tool_call in &metadata.tool_calls {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.tool_call",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "tool_call": &tool_call.value,
                    "source_event_index": tool_call.event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    let usage_events = if metadata.usage_events.is_empty() {
        metadata
            .usage
            .as_ref()
            .map(|usage| {
                vec![RuntimeAdapterTurnMetadataValue {
                    event_index: metadata
                        .completed_event_index
                        .or(metadata.started_event_index)
                        .unwrap_or_default(),
                    value: usage.clone(),
                }]
            })
            .unwrap_or_default()
    } else {
        metadata.usage_events.clone()
    };
    for usage in &usage_events {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.usage",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "usage": &usage.value,
                    "source_event_index": usage.event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    if let Some(final_message) = metadata.final_message.as_ref() {
        let final_output = truncate_output(final_message, execution_output_limit_bytes());
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id,
            artifact_type: "markdown".to_string(),
            name: "runtime-final-message.md".to_string(),
            path: Some("runtime-final-message.md".to_string()),
            content: json!({
                "markdown": final_output.text,
                "markdown_bytes": final_output.original_bytes,
                "markdown_truncated": final_output.truncated,
                "profile": profile,
                "runtime_type": &config.runtime_type,
                "turn_id": &metadata.turn_id,
                "source_event_index": metadata.final_message_event_index,
            }),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        final_artifact_id = Some(artifact.id);
        final_artifact_ids.push(artifact.id);
        final_artifact_count += 1;
        state
            .append_event(
                "system",
                Some(artifact.id),
                session_id,
                "artifact.created",
                json!({
                    "artifact_id": artifact.id,
                    "name": artifact.name,
                    "path": artifact.path,
                    "artifact_type": artifact.artifact_type,
                    "source": "runtime.final",
                    "turn_id": &metadata.turn_id,
                }),
            )
            .await?;
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.final",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "final_message": final_message,
                    "artifact_id": final_artifact_id,
                    "source_event_index": metadata.final_message_event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    if let Some(completed_event_index) = metadata.completed_event_index {
        state
            .append_event(
                "runtime_adapter",
                actor_id,
                session_id,
                "runtime.turn.completed",
                json!({
                    "profile": profile,
                    "runtime_type": &config.runtime_type,
                    "turn_id": &metadata.turn_id,
                    "status": &metadata.status,
                    "duration_ms": metadata.duration_ms,
                    "timing": &metadata.timing,
                    "usage": &metadata.usage,
                    "output_schema": &metadata.output_schema,
                    "output_schema_validation": &metadata.output_schema_validation,
                    "final_artifact_id": final_artifact_id,
                    "item_count": metadata.items.len(),
                    "tool_call_count": metadata.tool_calls.len(),
                    "source_event_index": completed_event_index,
                }),
            )
            .await?;
        recorded_event_count += 1;
    }

    Ok(RuntimeAdapterTurnRecording {
        event_count: recorded_event_count,
        final_artifact_count,
        final_artifact_ids,
    })
}

fn runtime_adapter_turn_metadata_supported(runtime_type: &str) -> bool {
    matches!(runtime_type, "codex_cli" | "claude_code")
}

fn build_runtime_adapter_turn_metadata(
    events: &[RuntimeAdapterEvent],
    request_args: &[String],
) -> RuntimeAdapterTurnMetadata {
    let mut metadata = RuntimeAdapterTurnMetadata {
        output_schema: runtime_adapter_output_schema_from_args(request_args),
        ..RuntimeAdapterTurnMetadata::default()
    };

    for event in events {
        let adapter_event_type = event.adapter_event_type.as_str();
        if metadata.turn_id.is_none() {
            metadata.turn_id = runtime_adapter_turn_id(adapter_event_type, &event.event);
        }
        if metadata.resume_handle.is_none() {
            metadata.resume_handle =
                runtime_adapter_resume_handle(adapter_event_type, &event.event);
        }
        if let Some(output_schema) = runtime_adapter_output_schema_from_event(&event.event) {
            metadata.output_schema = Some(output_schema);
        }
        if let Some(output_schema_validation) =
            runtime_adapter_output_schema_validation(&event.event)
        {
            metadata.output_schema_validation = Some(output_schema_validation);
        }
        if let Some(timing) = runtime_adapter_timing(&event.event) {
            metadata.timing = Some(timing);
        }
        if let Some(duration_ms) = runtime_adapter_duration_ms(&event.event) {
            metadata.duration_ms = Some(duration_ms);
        }
        if let Some(usage) = runtime_adapter_usage(&event.event) {
            metadata.usage = Some(usage.clone());
            if is_runtime_usage_event(adapter_event_type) {
                metadata.usage_events.push(RuntimeAdapterTurnMetadataValue {
                    event_index: event.index,
                    value: usage,
                });
            }
        }

        if is_runtime_turn_started_event_value(adapter_event_type, &event.event) {
            metadata.started_event_index.get_or_insert(event.index);
            metadata.status.get_or_insert_with(|| "running".to_string());
        }
        if is_runtime_item_event(adapter_event_type) {
            metadata.items.push(RuntimeAdapterTurnMetadataValue {
                event_index: event.index,
                value: runtime_adapter_item_value(&event.event),
            });
        }
        for tool_call in runtime_adapter_tool_call_values(adapter_event_type, &event.event) {
            metadata.tool_calls.push(RuntimeAdapterTurnMetadataValue {
                event_index: event.index,
                value: tool_call,
            });
        }
        if let Some(final_message) = runtime_adapter_final_message(adapter_event_type, &event.event)
        {
            metadata.final_message = Some(final_message);
            metadata.final_message_event_index = Some(event.index);
        }
        if is_runtime_turn_completed_event_value(adapter_event_type, &event.event) {
            metadata.completed_event_index = Some(event.index);
            metadata.status = runtime_adapter_status(adapter_event_type, &event.event);
        }
    }

    metadata
}

fn is_runtime_turn_started_event(adapter_event_type: &str) -> bool {
    matches!(
        adapter_event_type,
        "turn.started" | "response.started" | "session.started" | "task.started"
    )
}

fn is_runtime_turn_started_event_value(adapter_event_type: &str, event: &Value) -> bool {
    is_runtime_turn_started_event(adapter_event_type)
        || (adapter_event_type == "system"
            && (string_value_at(event, &["session_id"]).is_some()
                || string_value_at(event, &["conversation_id"]).is_some()
                || string_value_at(event, &["thread_id"]).is_some()))
}

fn is_runtime_turn_completed_event(adapter_event_type: &str) -> bool {
    matches!(
        adapter_event_type,
        "turn.completed"
            | "turn.failed"
            | "response.completed"
            | "response.failed"
            | "session.completed"
            | "session.failed"
            | "task.completed"
            | "task.failed"
    )
}

fn is_runtime_turn_completed_event_value(adapter_event_type: &str, event: &Value) -> bool {
    is_runtime_turn_completed_event(adapter_event_type)
        || (adapter_event_type == "result"
            && event
                .get("subtype")
                .and_then(Value::as_str)
                .is_some_and(|subtype| {
                    matches!(
                        subtype,
                        "success" | "completed" | "failed" | "failure" | "error"
                    )
                }))
}

fn is_runtime_item_event(adapter_event_type: &str) -> bool {
    adapter_event_type.starts_with("item.")
        || adapter_event_type.ends_with(".item")
        || adapter_event_type == "item"
        || adapter_event_type == "assistant"
}

fn is_runtime_tool_call_event(adapter_event_type: &str) -> bool {
    adapter_event_type.starts_with("tool_call.")
        || adapter_event_type.starts_with("tool.call.")
        || adapter_event_type == "tool_call"
        || adapter_event_type == "tool.called"
}

fn is_runtime_usage_event(adapter_event_type: &str) -> bool {
    adapter_event_type == "usage" || adapter_event_type.starts_with("usage.")
}

fn runtime_adapter_turn_id(adapter_event_type: &str, event: &Value) -> Option<String> {
    string_value_at(event, &["turn_id"])
        .or_else(|| string_value_at(event, &["turn", "id"]))
        .or_else(|| string_value_at(event, &["session_id"]))
        .or_else(|| string_value_at(event, &["conversation_id"]))
        .or_else(|| string_value_at(event, &["thread_id"]))
        .or_else(|| {
            if is_runtime_turn_started_event(adapter_event_type)
                || is_runtime_turn_completed_event(adapter_event_type)
            {
                string_value_at(event, &["id"])
            } else {
                None
            }
        })
}

fn runtime_adapter_resume_handle(adapter_event_type: &str, event: &Value) -> Option<Value> {
    event
        .get("resume_handle")
        .cloned()
        .or_else(|| event.get("resume").cloned())
        .or_else(|| {
            if !is_runtime_turn_started_event_value(adapter_event_type, event) {
                return None;
            }
            string_value_at(event, &["session_id"])
                .or_else(|| string_value_at(event, &["conversation_id"]))
                .or_else(|| string_value_at(event, &["thread_id"]))
                .or_else(|| string_value_at(event, &["id"]))
                .map(|session_id| {
                    json!({
                        "session_id": session_id,
                        "source": "codex_event"
                    })
                })
        })
        .map(|value| runtime_adapter_value_with_source(value, "codex_event"))
}

fn runtime_adapter_output_schema_from_event(event: &Value) -> Option<Value> {
    event
        .get("output_schema")
        .cloned()
        .or_else(|| event.get("outputSchema").cloned())
        .or_else(|| event.get("schema").cloned())
        .map(|value| runtime_adapter_value_with_source(value, "codex_event"))
}

fn runtime_adapter_output_schema_from_args(args: &[String]) -> Option<Value> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--output-schema" {
            return args.next().map(|path| {
                json!({
                    "path": path,
                    "source": "cli_args"
                })
            });
        }
        if let Some(path) = arg.strip_prefix("--output-schema=")
            && !path.trim().is_empty()
        {
            return Some(json!({
                "path": path,
                "source": "cli_args"
            }));
        }
    }
    None
}

fn runtime_adapter_output_schema_validation(event: &Value) -> Option<Value> {
    event
        .get("output_schema_validation")
        .cloned()
        .or_else(|| event.get("schema_validation").cloned())
        .or_else(|| event.get("structured_output_validation").cloned())
}

fn runtime_adapter_usage(event: &Value) -> Option<Value> {
    if let Some(usage) = event.get("usage") {
        return Some(usage.clone());
    }
    let mut usage = Map::new();
    for key in [
        "input_tokens",
        "output_tokens",
        "prompt_tokens",
        "completion_tokens",
        "cached_input_tokens",
        "reasoning_output_tokens",
        "total_tokens",
    ] {
        if let Some(value) = event.get(key) {
            usage.insert(key.to_string(), value.clone());
        }
    }
    if usage.is_empty() {
        None
    } else {
        Some(Value::Object(usage))
    }
}

fn runtime_adapter_timing(event: &Value) -> Option<Value> {
    if let Some(timing) = event.get("timing") {
        return Some(timing.clone());
    }
    let mut timing = Map::new();
    for key in [
        "started_at",
        "completed_at",
        "duration_ms",
        "duration_seconds",
        "elapsed_ms",
    ] {
        if let Some(value) = event.get(key) {
            timing.insert(key.to_string(), value.clone());
        }
    }
    if timing.is_empty() {
        None
    } else {
        Some(Value::Object(timing))
    }
}

fn runtime_adapter_duration_ms(event: &Value) -> Option<i64> {
    event
        .get("duration_ms")
        .and_then(json_i64)
        .or_else(|| {
            event
                .get("timing")
                .and_then(|timing| timing.get("duration_ms"))
                .and_then(json_i64)
        })
        .or_else(|| {
            event.get("elapsed_ms").and_then(json_i64).or_else(|| {
                event
                    .get("timing")
                    .and_then(|timing| timing.get("elapsed_ms"))
                    .and_then(json_i64)
            })
        })
}

fn runtime_adapter_status(adapter_event_type: &str, event: &Value) -> Option<String> {
    event
        .get("status")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            if adapter_event_type.ends_with(".completed") {
                Some("completed".to_string())
            } else if adapter_event_type.ends_with(".failed") {
                Some("failed".to_string())
            } else if adapter_event_type == "result" {
                event
                    .get("subtype")
                    .and_then(Value::as_str)
                    .and_then(|subtype| match subtype {
                        "success" | "completed" => Some("completed".to_string()),
                        "failed" | "failure" | "error" => Some("failed".to_string()),
                        _ => None,
                    })
            } else {
                None
            }
        })
}

fn runtime_adapter_item_value(event: &Value) -> Value {
    event.get("item").cloned().unwrap_or_else(|| event.clone())
}

fn runtime_adapter_tool_call_value(adapter_event_type: &str, event: &Value) -> Value {
    if let Some(tool_call) = event.get("tool_call") {
        return tool_call.clone();
    }
    let mut tool_call = Map::new();
    tool_call.insert(
        "adapter_event_type".to_string(),
        Value::String(adapter_event_type.to_string()),
    );
    for key in [
        "call_id",
        "id",
        "tool",
        "name",
        "args",
        "arguments",
        "status",
    ] {
        if let Some(value) = event.get(key) {
            tool_call.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(tool_call)
}

fn runtime_adapter_tool_call_values(adapter_event_type: &str, event: &Value) -> Vec<Value> {
    if is_runtime_tool_call_event(adapter_event_type) {
        return vec![runtime_adapter_tool_call_value(adapter_event_type, event)];
    }
    claude_stream_json_tool_use_values(adapter_event_type, event)
}

fn claude_stream_json_tool_use_values(adapter_event_type: &str, event: &Value) -> Vec<Value> {
    if !matches!(
        adapter_event_type,
        "assistant" | "message" | "assistant.message"
    ) {
        return Vec::new();
    }
    event
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| event.get("content"))
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                .map(|block| {
                    let mut tool_call = Map::new();
                    tool_call.insert(
                        "adapter_event_type".to_string(),
                        Value::String(adapter_event_type.to_string()),
                    );
                    if let Some(id) = block.get("id") {
                        tool_call.insert("call_id".to_string(), id.clone());
                    }
                    if let Some(name) = block.get("name") {
                        tool_call.insert("tool".to_string(), name.clone());
                    }
                    if let Some(input) = block.get("input") {
                        tool_call.insert("args".to_string(), input.clone());
                    }
                    Value::Object(tool_call)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn runtime_adapter_final_message(adapter_event_type: &str, event: &Value) -> Option<String> {
    if !(is_runtime_turn_completed_event_value(adapter_event_type, event)
        || matches!(
            adapter_event_type,
            "final" | "final.message" | "agent.message" | "message.output"
        ))
    {
        return None;
    }
    for key in [
        "final_message",
        "last_message",
        "message",
        "text",
        "content",
        "output",
        "result",
    ] {
        if let Some(value) = event.get(key) {
            if let Some(message) = value.as_str()
                && !message.trim().is_empty()
            {
                return Some(message.to_string());
            }
            if let Some(message) = string_value_at(value, &["content"])
                && !message.trim().is_empty()
            {
                return Some(message);
            }
        }
    }
    None
}

fn runtime_adapter_value_with_source(value: Value, source: &str) -> Value {
    match value {
        Value::Object(mut object) => {
            object
                .entry("source".to_string())
                .or_insert_with(|| Value::String(source.to_string()));
            Value::Object(object)
        }
        Value::String(path) => json!({
            "path": path,
            "source": source
        }),
        value => json!({
            "value": value,
            "source": source
        }),
    }
}

fn string_value_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToString::to_string)
}

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn runtime_adapter_event_limit() -> usize {
    std::env::var("MANDOFORGE_RUNTIME_ADAPTER_EVENT_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_RUNTIME_ADAPTER_EVENT_LIMIT))
        .unwrap_or(DEFAULT_RUNTIME_ADAPTER_EVENT_LIMIT)
}

#[allow(dead_code)]
async fn run_codex_cli(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
    runtime_selection: &CodexRuntimeSelection,
) -> Result<Value, AppError> {
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let last_message = workspace.join("last_message.md");
    let runtime_binding = codex_runtime_selection_metadata(runtime_selection);

    state
        .append_event(
            "tool",
            None,
            session_id,
            "codex.task.started",
            json!({"task": &request.task, "sandbox_mode": &request.sandbox_mode, "workspace": workspace, "runner": "cli", "runtime_binding": runtime_binding}),
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
                "runner": "cli",
                "runtime_binding": runtime_binding
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
        "final_message_truncated": final_output.truncated,
        "runtime_binding": runtime_binding
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

    static ENV_VAR_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn remote_codex_command_quotes_task_and_emits_final_markers() {
        let request = CodexRequest {
            task: "inspect README && echo 'done'".to_string(),
            sandbox_mode: "workspace-write".to_string(),
            execution_strategy: None,
            poll_attempts: None,
            poll_interval_ms: None,
        };
        let command = remote_codex_exec_command(&request, "/workspace/sessions/session-1");

        assert!(command.contains("cd '/workspace/sessions/session-1'"));
        assert!(command.contains("codex exec --sandbox 'workspace-write'"));
        assert!(command.contains("--cd '/workspace/sessions/session-1'"));
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
    fn runtime_adapter_parses_codex_jsonl_events() {
        let events = parse_runtime_adapter_events(
            "codex_cli",
            "{\"type\":\"turn.started\",\"token\":\"secret\"}\nnot-json\n{\"msg\":\"tool.completed\"}",
        );

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].adapter_event_type, "turn.started");
        assert_eq!(events[0].event["token"], "[REDACTED]");
        assert_eq!(events[1].adapter_event_type, "tool.completed");
    }

    #[test]
    fn runtime_adapter_parses_claude_stream_json_events() {
        let events = parse_runtime_adapter_events(
            "claude_code",
            "{\"type\":\"system\",\"credential\":\"secret\"}\n{\"type\":\"assistant\",\"message\":{\"content\":\"ok\"}}",
        );

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].adapter_event_type, "system");
        assert_eq!(events[0].event["credential"], "[REDACTED]");
        assert_eq!(events[1].adapter_event_type, "assistant");
    }

    #[test]
    fn runtime_adapter_maps_claude_stream_json_tool_use_to_runtime_tool_call() {
        let events = parse_runtime_adapter_events(
            "claude_code",
            r#"{"type":"system","session_id":"claude-session-1"}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01abc","name":"Bash","input":{"command":"pwd","description":"show cwd"}}]}}"#,
        );

        let metadata = build_runtime_adapter_turn_metadata(&events, &[]);

        assert_eq!(metadata.tool_calls.len(), 1);
        assert_eq!(metadata.tool_calls[0].event_index, 1);
        assert_eq!(metadata.tool_calls[0].value["call_id"], "toolu_01abc");
        assert_eq!(metadata.tool_calls[0].value["tool"], "Bash");
        assert_eq!(
            metadata.tool_calls[0].value["args"],
            json!({"command": "pwd", "description": "show cwd"})
        );
    }

    #[test]
    fn remote_file_write_command_uses_safe_heredoc_delimiter() {
        let command = remote_file_write_command(
            "/workspace/sessions/session-1",
            "reports/diagnostics.md",
            "line one\nMANDOFORGE_FILE_WRITE_EOF_0\nline two",
        );

        assert!(command.contains("cd '/workspace/sessions/session-1'"));
        assert!(command.contains("mkdir -p -- \"$(dirname -- 'reports/diagnostics.md')\""));
        assert!(command.contains("cat > 'reports/diagnostics.md'"));
        assert!(command.contains("MANDOFORGE_FILE_WRITE_EOF_1"));
        assert!(!command.contains("<<'MANDOFORGE_FILE_WRITE_EOF_0'"));
    }

    #[test]
    fn remote_shell_command_runs_inside_session_workspace() {
        let command =
            remote_shell_exec_command("/workspace/sessions/session-1", "pwd && touch marker.txt");

        assert!(command.contains("mkdir -p '/workspace/sessions/session-1'"));
        assert!(command.contains("cd '/workspace/sessions/session-1'"));
        assert!(command.ends_with("pwd && touch marker.txt"));
    }

    #[test]
    fn remote_agent_cli_environment_profile_avoids_eval_and_globbing() {
        let request = AgentCliRequest {
            profile: "legacy-coder".to_string(),
            task: "summarize repo".to_string(),
            args: vec!["--json".to_string()],
            timeout_seconds: None,
        };
        let config = AgentCliProfileConfig {
            command: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            timeout_seconds: None,
            remote_computer_required: true,
            runtime_type: "agent_cli".to_string(),
            source: AgentCliProfileConfigSource::Environment,
        };

        let command = remote_agent_cli_exec_command(&request, &config, "/workspace/sessions/s1")
            .expect("agent cli command");

        assert!(command.contains("printenv \"$command_var\""));
        assert!(command.contains("printenv \"$args_var\""));
        assert!(!command.contains("eval \"printf"));
        assert!(command.contains("set -f\n  set -- $agent_args\n  set +f"));
        assert!(command.contains("MANDOFORGE_AGENT_CLI_$(printf '%s' \"$agent_cli_profile\""));
        assert!(command.contains("'legacy-coder'"));
        assert!(command.contains("'--json'"));
        assert!(command.contains("'summarize repo'"));
    }

    #[test]
    fn kubernetes_exec_status_defaults_and_failure_are_explicit() {
        assert!(!kubernetes_exec_status_succeeded(&Value::Null));
        assert!(kubernetes_exec_status_succeeded(
            &json!({"status": "Success", "exitCode": 0})
        ));
        assert!(!kubernetes_exec_status_succeeded(
            &json!({"status": "Failure", "exitCode": 2})
        ));
    }

    #[test]
    fn remote_computer_pod_create_conflict_is_recoverable() {
        let response = crate::RemoteComputerRunnerDryRunResponse {
            status: "mutation_failed".to_string(),
            operation: "live_create".to_string(),
            configured: true,
            would_create_pod: true,
            would_delete_pod: false,
            live_probe_attempted: false,
            live_probe_status_code: None,
            live_mutation_attempted: true,
            live_mutation_status_code: Some(StatusCode::CONFLICT.as_u16()),
            kubernetes_api_path: Some("/api/v1/namespaces/agent-os/pods".to_string()),
            namespace: Some("agent-os".to_string()),
            pod_name: Some("agent-rc-session".to_string()),
            pod_template_path: Some("deploy/k8s/agent-remote-computer.yaml".to_string()),
            execution_enabled: false,
            message: "Kubernetes Pod API returned HTTP 409".to_string(),
            request: json!({"operation": "live_create"}),
            exec_result: None,
        };

        assert!(remote_computer_pod_create_already_exists(&response));

        let non_create_conflict = crate::RemoteComputerRunnerDryRunResponse {
            would_create_pod: false,
            operation: "live_delete".to_string(),
            ..response
        };
        assert!(!remote_computer_pod_create_already_exists(
            &non_create_conflict
        ));
    }

    #[test]
    fn remote_computer_pod_provision_error_reports_cleanup_failure() {
        let message = remote_computer_pod_provision_error_message(
            "Remote Computer Pod not ready: timed out",
            Some("Kubernetes Pod API returned HTTP 403".to_string()),
        );

        assert!(message.contains("Pod not ready"));
        assert!(message.contains("cleanup failed"));
        assert!(message.contains("HTTP 403"));
    }

    #[test]
    fn on_demand_remote_computer_id_is_stable_per_session() {
        let session_id = Uuid::new_v4();

        assert_eq!(
            on_demand_remote_computer_id(session_id),
            on_demand_remote_computer_id(session_id)
        );
        assert_ne!(on_demand_remote_computer_id(session_id), session_id);
    }

    #[test]
    fn provision_remote_computer_pod_for_job_returns_none_when_k8s_transport_disabled() {
        // remote_computer_pod_execution_requested() reads env vars.
        // When MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED is unset,
        // the gate is closed and provision_... must return Ok(None) immediately.
        // We can't call the async fn directly without an AppState, so we test
        // the gate function directly — same branch covered.
        let _guard = ENV_VAR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT");
        }
        assert!(
            !remote_computer_pod_execution_requested(),
            "gate should be closed when execution env var is absent"
        );
    }

    #[test]
    fn provision_remote_computer_pod_for_job_returns_none_when_transport_is_not_kubernetes() {
        let _guard = ENV_VAR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED", "true");
            std::env::set_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT", "reserved");
        }
        assert!(
            !remote_computer_pod_execution_requested(),
            "gate should be closed when transport is not kubernetes"
        );
        unsafe {
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT");
        }
    }

    #[test]
    fn provision_remote_computer_pod_for_job_gate_opens_for_kubernetes_transport() {
        let _guard = ENV_VAR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED", "true");
            std::env::set_var(
                "MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT",
                "kubernetes",
            );
        }
        assert!(
            remote_computer_pod_execution_requested(),
            "gate should open for kubernetes transport"
        );
        unsafe {
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT");
        }
    }

    #[test]
    fn provision_remote_computer_pod_for_job_gate_opens_for_k8s_alias() {
        let _guard = ENV_VAR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED", "true");
            std::env::set_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT", "k8s");
        }
        assert!(
            remote_computer_pod_execution_requested(),
            "gate should open for k8s alias"
        );
        unsafe {
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
            std::env::remove_var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT");
        }
    }
}
