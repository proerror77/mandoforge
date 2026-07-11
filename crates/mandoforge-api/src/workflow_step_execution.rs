use std::collections::HashSet;

use axum::{Json, http::HeaderMap};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn claim_workflow_step_run(
    state: &AppState,
    headers: &HeaderMap,
    current: WorkflowStepRun,
    run: WorkflowRun,
    input: ClaimWorkflowStepRun,
) -> Result<ClaimWorkflowStepRunResponse, AppError> {
    authorize_request(
        state,
        headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    if let Some(reason) = workflow_run_execution_denial(&run.status) {
        return Err(AppError::forbidden(reason));
    }
    let principal = principal_from_request(state, headers).await?;
    let agent = state.get_agent(input.agent_id).await?;
    if current.agent_id != Some(agent.id) {
        return Err(AppError::forbidden(
            "workflow step is not assigned to the claiming agent",
        ));
    }
    let now = Utc::now();
    let blockers = workflow_step_claim_blockers(&current, Some(agent.id), now);
    if !blockers.is_empty() {
        return Err(AppError::bad_request(format!(
            "workflow step is not claimable: {}",
            blockers.join(", ")
        )));
    }
    let task_grant_id = current
        .task_grant_id
        .ok_or_else(|| AppError::bad_request("workflow step claim requires task grant"))?;
    let grant = state.get_task_grant(task_grant_id).await?;
    if grant.workflow_run_id != run.id {
        return Err(AppError::bad_request(
            "workflow step task grant must belong to workflow run",
        ));
    }
    if grant.status != "active" {
        return Err(AppError::forbidden(
            "workflow step task grant is not active",
        ));
    }
    if grant
        .grantee_agent_id
        .is_some_and(|grantee_agent_id| grantee_agent_id != agent.id)
    {
        return Err(AppError::forbidden(
            "workflow step task grant is not issued to the claiming agent",
        ));
    }
    if grant.expires_at.is_some_and(|expires_at| expires_at <= now) {
        return Err(AppError::forbidden("workflow step task grant is expired"));
    }
    let session_id = current.session_id.unwrap_or(run.primary_session_id);
    if !task_grant_session_matches(&grant, &run, session_id) {
        return Err(AppError::forbidden(
            "workflow step task grant is not valid for the step session",
        ));
    }
    let session = state.get_session(session_id).await?;
    if session.agent_id != agent.id {
        return Err(AppError::forbidden(
            "workflow step session is not bound to the claiming agent",
        ));
    }
    if current.agent_version_id != session.agent_version_id {
        return Err(AppError::forbidden(
            "workflow step agent version does not match its session binding",
        ));
    }
    let lease_seconds = input.lease_seconds.unwrap_or(300);
    if !(1..=86_400).contains(&lease_seconds) {
        return Err(AppError::bad_request(
            "workflow step claim lease_seconds must be between 1 and 86400",
        ));
    }
    let worker_id = input
        .worker_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("agent:{}", agent.id));

    let context_packet = generate_and_persist_context_packet(state, session_id).await?;
    let task_grant = state
        .update_task_grant_context_packet(grant.id, context_packet.id)
        .await?;
    record_task_grant_checked(state, &task_grant, session_id, "agent_inbox.claim").await?;

    let mut next = current;
    next.status = "running".to_string();
    next.claimed_by_worker = Some(worker_id.clone());
    next.lease_expires_at = Some(now + ChronoDuration::seconds(lease_seconds));
    next.context_packet_id = Some(context_packet.id);
    next.started_at = next.started_at.or(Some(now));
    next.updated_at = now;
    let step = state.update_workflow_step_run(next).await?;
    record_agent_inbox_claimed(
        state,
        &run,
        &step,
        &task_grant,
        &context_packet,
        &principal.subject_id,
        &worker_id,
    )
    .await?;
    if let Some(work_item_id) = run.source_work_item_id {
        state
            .append_work_item_activity_entry(
                work_item_id,
                "agent_inbox.claimed",
                Some(principal.subject_id),
                Some("workflow_step_run"),
                Some(step.id),
                format!(
                    "Agent {} claimed workflow step {}",
                    agent.name, step.step_key
                ),
                json!({
                    "workflow_run_id": run.id,
                    "workflow_step_run_id": step.id,
                    "task_grant_id": task_grant.id,
                    "context_packet_id": context_packet.id,
                    "agent_id": agent.id,
                    "worker_id": worker_id,
                    "lease_expires_at": step.lease_expires_at
                }),
            )
            .await?;
    }
    Ok(ClaimWorkflowStepRunResponse {
        step,
        task_grant,
        context_packet,
    })
}

pub(crate) async fn run_workflow_delegated_runtime_step(
    state: &AppState,
    headers: &HeaderMap,
    current: WorkflowStepRun,
    run: WorkflowRun,
    input: RunWorkflowStepRun,
) -> Result<Json<RunWorkflowStepRunResponse>, AppError> {
    let agent_id = input
        .agent_id
        .or(current.agent_id)
        .ok_or_else(|| AppError::bad_request("delegated runtime step requires agent_id"))?;
    let worker_id = input
        .worker_id
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            headers
                .get("x-mandoforge-worker-id")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("agent:{agent_id}"))
        });
    let claim = claim_workflow_step_run(
        state,
        headers,
        current,
        run.clone(),
        ClaimWorkflowStepRun {
            agent_id,
            worker_id: Some(worker_id.clone()),
            lease_seconds: input.lease_seconds,
        },
    )
    .await?;
    let session_id = claim.step.session_id.unwrap_or(run.primary_session_id);
    record_workflow_step_worker_started(
        state,
        &run,
        &claim.step,
        &worker_id,
        claim.context_packet.id,
    )
    .await?;
    let dispatch_event = state
        .append_event(
            "worker",
            Some(claim.step.id),
            session_id,
            "workflow.delegated_runtime.dispatch_requested",
            json!({
                "workflow_run_id": run.id,
                "workflow_step_run_id": claim.step.id,
                "worker_id": worker_id,
                "runtime_adapter": run.runtime_adapter,
                "runtime_mode": run.runtime_mode,
                "delegation_status": run.delegation_status,
                "runtime_envelope": run.runtime_envelope
            }),
        )
        .await?;
    let queued = enqueue_session_loop(
        state,
        session_id,
        Some(dispatch_event.id),
        "workflow.delegated_runtime.run",
    )
    .await?;
    let running_job = state.start_session_loop_job(queued.id, &worker_id).await?;
    state
        .append_event(
            "worker",
            Some(running_job.id),
            running_job.session_id,
            "session.loop.started",
            json!({
                "session_loop_job_id": running_job.id,
                "environment_id": running_job.environment_id,
                "worker_id": worker_id,
                "attempt_count": running_job.attempt_count,
                "workflow_step_run_id": claim.step.id,
                "delegated_runtime": true
            }),
        )
        .await?;

    let adapter = run.runtime_adapter.as_deref().unwrap_or("unconfigured");
    let execution = match adapter {
        "codex_app_server" => {
            run_codex_app_server_delegated_runtime(state, &run, &claim.step, &worker_id).await
        }
        "codex_cli" | "claude_code" => {
            run_agent_cli_delegated_runtime(state, &run, &claim.step, session_id, adapter).await
        }
        _ => Err(AppError::bad_request(format!(
            "delegated runtime adapter {adapter} requires an external worker adapter"
        ))),
    };

    match execution {
        Ok(output_payload) => {
            let delegated_status = delegated_runtime_output_status(&output_payload);
            if delegated_runtime_status_is_nonterminal(&delegated_status) {
                let completed_job = state
                    .complete_session_loop_job(running_job.id, &worker_id)
                    .await?;
                let session = set_managed_session_status(
                    state,
                    session_id,
                    SessionStatus::Running,
                    "delegated runtime is still running",
                )
                .await?;
                let previous_status = claim.step.status.clone();
                let now = Utc::now();
                let mut pending_step = claim.step.clone();
                pending_step.status = "running".to_string();
                pending_step.output_payload = json!({
                    "delegated_runtime": output_payload,
                    "worker_id": worker_id,
                    "session_loop_job_id": completed_job.id,
                    "context_packet_id": pending_step.context_packet_id,
                    "poll_status": delegated_status.clone()
                });
                pending_step.lease_expires_at = Some(now);
                pending_step.updated_at = now;
                let pending_step = state.update_workflow_step_run(pending_step).await?;
                record_workflow_step_run_updated(state, &run, &pending_step, &previous_status)
                    .await?;
                record_workflow_step_worker_completed(
                    state,
                    &run,
                    &pending_step,
                    &worker_id,
                    &completed_job,
                )
                .await?;
                state
                    .append_event(
                        "worker",
                        Some(pending_step.id),
                        session_id,
                        "workflow.delegated_runtime.poll_pending",
                        json!({
                            "workflow_run_id": run.id,
                            "workflow_step_run_id": pending_step.id,
                            "worker_id": worker_id,
                            "runtime_adapter": adapter,
                            "status": delegated_status.clone(),
                            "output": pending_step.output_payload
                        }),
                    )
                    .await?;
                return Ok(Json(RunWorkflowStepRunResponse {
                    step: pending_step,
                    task_grant: claim.task_grant,
                    context_packet: claim.context_packet,
                    session,
                    session_loop_job: completed_job,
                }));
            }
            if !delegated_runtime_status_is_terminal_success(&delegated_status) {
                return complete_delegated_runtime_requires_action(
                    state,
                    &run,
                    &claim.step,
                    claim.task_grant,
                    claim.context_packet,
                    running_job,
                    &worker_id,
                    session_id,
                    adapter,
                    format!(
                        "delegated runtime returned non-success terminal status {delegated_status}"
                    ),
                    Some(output_payload),
                )
                .await;
            }
            let artifact = create_delegated_runtime_artifact(
                state,
                session_id,
                "delegated-runtime-result.json",
                output_payload.clone(),
            )
            .await?;
            let completed_job = state
                .complete_session_loop_job(running_job.id, &worker_id)
                .await?;
            let session = set_managed_session_status(
                state,
                session_id,
                SessionStatus::Terminated,
                "delegated runtime completed",
            )
            .await?;
            let previous_status = claim.step.status.clone();
            let now = Utc::now();
            let mut completed_step = claim.step.clone();
            completed_step.status = "completed".to_string();
            completed_step.artifact_ids = vec![artifact.id];
            completed_step.output_payload = json!({
                "delegated_runtime": output_payload,
                "artifact_ids": completed_step.artifact_ids,
                "worker_id": worker_id,
                "session_loop_job_id": completed_job.id,
                "context_packet_id": completed_step.context_packet_id
            });
            completed_step.completed_at = Some(now);
            completed_step.updated_at = now;
            let completed_step = state.update_workflow_step_run(completed_step).await?;
            record_workflow_step_run_updated(state, &run, &completed_step, &previous_status)
                .await?;
            record_workflow_step_worker_completed(
                state,
                &run,
                &completed_step,
                &worker_id,
                &completed_job,
            )
            .await?;
            state
                .append_event(
                    "worker",
                    Some(completed_step.id),
                    session_id,
                    "workflow.delegated_runtime.completed",
                    json!({
                        "workflow_run_id": run.id,
                        "workflow_step_run_id": completed_step.id,
                        "worker_id": worker_id,
                        "runtime_adapter": adapter,
                        "artifact_id": artifact.id,
                        "output": completed_step.output_payload
                    }),
                )
                .await?;
            advance_workflow_graph_after_step_update(state, &run, &completed_step).await?;
            Ok(Json(RunWorkflowStepRunResponse {
                step: completed_step,
                task_grant: claim.task_grant,
                context_packet: claim.context_packet,
                session,
                session_loop_job: completed_job,
            }))
        }
        Err(error) => {
            complete_delegated_runtime_requires_action(
                state,
                &run,
                &claim.step,
                claim.task_grant,
                claim.context_packet,
                running_job,
                &worker_id,
                session_id,
                adapter,
                error.message,
                None,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn complete_delegated_runtime_requires_action(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    task_grant: TaskGrant,
    context_packet: ContextPacket,
    running_job: SessionLoopJob,
    worker_id: &str,
    session_id: Uuid,
    adapter: &str,
    reason: String,
    output_payload: Option<Value>,
) -> Result<Json<RunWorkflowStepRunResponse>, AppError> {
    let completed_job = state
        .complete_session_loop_job(running_job.id, worker_id)
        .await?;
    let session = set_managed_session_status(
        state,
        session_id,
        SessionStatus::RequiresAction,
        "delegated runtime requires action",
    )
    .await?;
    let previous_status = step.status.clone();
    let mut blocked_step = step.clone();
    blocked_step.status = "requires_action".to_string();
    blocked_step.output_payload = json!({
        "delegated_runtime": {
            "status": "requires_action",
            "reason": reason,
            "runtime_adapter": adapter,
            "runtime_mode": run.runtime_mode,
            "runtime_envelope": run.runtime_envelope,
            "output": output_payload
        },
        "worker_id": worker_id,
        "session_loop_job_id": completed_job.id,
        "context_packet_id": blocked_step.context_packet_id
    });
    blocked_step.updated_at = Utc::now();
    let blocked_step = state.update_workflow_step_run(blocked_step).await?;
    record_workflow_step_run_updated(state, run, &blocked_step, &previous_status).await?;
    record_workflow_step_worker_completed(state, run, &blocked_step, worker_id, &completed_job)
        .await?;
    state
        .append_event(
            "worker",
            Some(blocked_step.id),
            session_id,
            "workflow.delegated_runtime.requires_action",
            json!({
                "workflow_run_id": run.id,
                "workflow_step_run_id": blocked_step.id,
                "worker_id": worker_id,
                "runtime_adapter": adapter,
                "reason": reason
            }),
        )
        .await?;
    Ok(Json(RunWorkflowStepRunResponse {
        step: blocked_step,
        task_grant,
        context_packet,
        session,
        session_loop_job: completed_job,
    }))
}

pub(crate) fn delegated_runtime_output_status(output_payload: &Value) -> String {
    output_payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn delegated_runtime_status_is_nonterminal(status: &str) -> bool {
    matches!(
        status,
        "created" | "queued" | "pending" | "submitted" | "running" | "in_progress"
    )
}

pub(crate) fn delegated_runtime_status_is_terminal_success(status: &str) -> bool {
    matches!(status, "completed" | "complete" | "succeeded" | "success")
}

pub(crate) async fn run_codex_app_server_delegated_runtime(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    worker_id: &str,
) -> Result<Value, AppError> {
    let config = codex_app_server_config(state)?;
    let metadata = json!({
        "source": "mandoforge_delegated_runtime",
        "workflow_run_id": run.id,
        "workflow_step_run_id": step.id,
        "runtime_mode": run.runtime_mode,
        "worker_id": worker_id,
        "runtime_envelope": run.runtime_envelope
    });
    let thread_request = CodexThreadRequest {
        metadata: metadata.clone(),
    };
    let thread = state
        .codex_app_server_client
        .create_thread(config, thread_request.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "delegated_runtime.thread.create",
            Some(thread.thread_id.clone()),
            None,
            None,
            serde_json::to_value(&thread_request)?,
            serde_json::to_value(&thread)?,
        )
        .await?;

    let turn_request = CodexTurnRequest {
        message: delegated_runtime_turn_message(run, step),
        metadata,
    };
    let turn = state
        .codex_app_server_client
        .create_turn(config, &thread.thread_id, turn_request.clone())
        .await?;
    state
        .record_codex_app_server_run(
            "delegated_runtime.turn.create",
            Some(thread.thread_id.clone()),
            Some(turn.turn_id.clone()),
            None,
            serde_json::to_value(&turn_request)?,
            serde_json::to_value(&turn)?,
        )
        .await?;
    let polled = state
        .codex_app_server_client
        .get_turn_status(config, &turn.turn_id)
        .await?;
    state
        .record_codex_app_server_run(
            "delegated_runtime.turn.poll",
            Some(thread.thread_id.clone()),
            Some(polled.turn_id.clone()),
            None,
            json!({"turn_id": turn.turn_id}),
            serde_json::to_value(&polled)?,
        )
        .await?;
    let status = polled.status.as_deref().unwrap_or("unknown");
    if matches!(status, "failed" | "canceled" | "error") {
        return Err(AppError::bad_request(format!(
            "Codex App Server delegated runtime returned {status}"
        )));
    }
    Ok(json!({
        "status": status,
        "runtime_adapter": "codex_app_server",
        "runtime_mode": run.runtime_mode,
        "thread": thread,
        "turn": turn,
        "poll": polled
    }))
}

pub(crate) async fn run_agent_cli_delegated_runtime(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    session_id: Uuid,
    adapter: &str,
) -> Result<Value, AppError> {
    let result = run_agent_cli(
        state,
        session_id,
        AgentCliRequest {
            profile: adapter.to_string(),
            task: delegated_runtime_turn_message(run, step),
            args: Vec::new(),
            timeout_seconds: run
                .runtime_envelope
                .get("runtime_capability_contract")
                .and_then(|contract| contract.get("timeout_seconds"))
                .and_then(Value::as_u64),
        },
    )
    .await?;
    Ok(json!({
        "status": "completed",
        "runtime_adapter": adapter,
        "runtime_mode": run.runtime_mode,
        "agent_cli": result
    }))
}

pub(crate) fn delegated_runtime_turn_message(run: &WorkflowRun, step: &WorkflowStepRun) -> String {
    let objective = step
        .input_payload
        .get("graph_step")
        .and_then(|graph_step| graph_step.get("input"))
        .and_then(|input| input.get("objective"))
        .and_then(Value::as_str)
        .or_else(|| run.input_payload.get("objective").and_then(Value::as_str))
        .unwrap_or("Execute the delegated runtime workflow.");
    format!(
        "Run delegated workflow for MandoForge workflow run {}.\nObjective: {}\nRuntime envelope: {}",
        run.id,
        objective,
        workflow_graph_console_summary(&run.runtime_envelope)
    )
}

pub(crate) async fn create_delegated_runtime_artifact(
    state: &AppState,
    session_id: Uuid,
    name: &str,
    content: Value,
) -> Result<Artifact, AppError> {
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id,
        artifact_type: "json".to_string(),
        name: name.to_string(),
        path: Some(format!("delegated-runtime/{session_id}/{name}")),
        content,
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
                "artifact_type": artifact.artifact_type,
                "name": artifact.name,
                "path": artifact.path,
                "source": "delegated_runtime"
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "system",
            None,
            "artifact.created",
            "artifact",
            Some(artifact.id),
            json!({
                "name": artifact.name,
                "artifact_type": artifact.artifact_type,
                "path": artifact.path,
                "source": "delegated_runtime"
            }),
        ))
        .await?;
    Ok(artifact)
}

pub(crate) fn workflow_step_is_adapter_owned_compensation(step: &WorkflowStepRun) -> bool {
    step.input_payload
        .get("graph_step")
        .is_some_and(workflow_graph_step_is_adapter_owned_compensation)
}

pub(crate) fn workflow_compensation_adapter_kind(step: &WorkflowStepRun) -> String {
    step.input_payload
        .get("graph_step")
        .and_then(|graph_step| graph_step.get("adapter"))
        .and_then(|adapter| {
            adapter
                .get("kind")
                .or_else(|| adapter.get("type"))
                .and_then(Value::as_str)
                .or_else(|| adapter.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(step.step_type.as_str())
        .to_string()
}

pub(crate) fn workflow_compensation_adapter_blockers(
    step: &WorkflowStepRun,
    now: DateTime<Utc>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if workflow_step_status_terminal(&step.status) {
        blockers.push("terminal_status".to_string());
    }
    if step.status == "scheduled" {
        match step.scheduled_at {
            Some(scheduled_at) if scheduled_at > now => {
                blockers.push("scheduled_for_future".to_string())
            }
            Some(_) => blockers.push("scheduled_until_scheduler_activation".to_string()),
            None => blockers.push("scheduled_without_due_time".to_string()),
        }
    } else if step.status != "queued" {
        if step.status == "running" {
            if step
                .lease_expires_at
                .is_none_or(|lease_expires_at| lease_expires_at > now)
            {
                blockers.push("already_claimed".to_string());
            }
        } else {
            blockers.push(format!("status_{}", step.status));
        }
    }
    if step.task_grant_id.is_none() {
        blockers.push("missing_task_grant".to_string());
    }
    blockers
}

pub(crate) async fn run_workflow_compensation_adapter_step(
    state: &AppState,
    headers: &HeaderMap,
    current: WorkflowStepRun,
    run: WorkflowRun,
    input: RunWorkflowStepRun,
) -> Result<Json<RunWorkflowStepRunResponse>, AppError> {
    authorize_request(
        state,
        headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let session_id = current.session_id.unwrap_or(run.primary_session_id);
    let session = state.get_session(session_id).await?;
    let now = Utc::now();
    let blockers = workflow_compensation_adapter_blockers(&current, now);
    if !blockers.is_empty() {
        return Err(AppError::bad_request(format!(
            "workflow compensation adapter step is not runnable: {}",
            blockers.join(", ")
        )));
    }
    let task_grant_id = current.task_grant_id.ok_or_else(|| {
        AppError::bad_request("workflow compensation adapter requires task grant")
    })?;
    let grant = state.get_task_grant(task_grant_id).await?;
    if grant.workflow_run_id != run.id {
        return Err(AppError::bad_request(
            "workflow compensation adapter task grant must belong to workflow run",
        ));
    }
    if grant.status != "active" {
        return Err(AppError::forbidden(
            "workflow compensation adapter task grant is not active",
        ));
    }
    if grant.expires_at.is_some_and(|expires_at| expires_at <= now) {
        return Err(AppError::forbidden(
            "workflow compensation adapter task grant is expired",
        ));
    }

    let worker_id = input
        .worker_id
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            headers
                .get("x-mandoforge-worker-id")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("rollback-adapter:{}", current.id))
        });
    let lease_seconds = input.lease_seconds.unwrap_or(300);
    if !(1..=86_400).contains(&lease_seconds) {
        return Err(AppError::bad_request(
            "workflow step claim lease_seconds must be between 1 and 86400",
        ));
    }
    let context_packet = generate_and_persist_context_packet(state, session_id).await?;
    let task_grant = state
        .update_task_grant_context_packet(grant.id, context_packet.id)
        .await?;
    record_task_grant_checked(
        state,
        &task_grant,
        session_id,
        "workflow.compensation_adapter",
    )
    .await?;

    let mut running_step = current.clone();
    let previous_status = running_step.status.clone();
    running_step.status = "running".to_string();
    running_step.claimed_by_worker = Some(worker_id.clone());
    running_step.lease_expires_at = Some(now + ChronoDuration::seconds(lease_seconds));
    running_step.context_packet_id = Some(context_packet.id);
    running_step.started_at = running_step.started_at.or(Some(now));
    running_step.updated_at = now;
    let running_step = state.update_workflow_step_run(running_step).await?;
    record_workflow_step_run_updated(state, &run, &running_step, &previous_status).await?;
    record_workflow_step_worker_started(state, &run, &running_step, &worker_id, context_packet.id)
        .await?;
    record_workflow_transition(
        state,
        &run,
        Some(&running_step),
        Some(&running_step),
        "compensation",
        "running",
        json!({
            "source": "workflow_compensation_adapter",
            "adapter_kind": workflow_compensation_adapter_kind(&running_step),
            "failure_trigger": running_step.input_payload.get("failure_trigger").cloned().unwrap_or(Value::Null),
            "graph_step": running_step.input_payload.get("graph_step").cloned().unwrap_or(Value::Null)
        }),
        json!({
            "workflow_step_run_id": running_step.id,
            "worker_id": worker_id
        }),
    )
    .await?;

    let queued =
        enqueue_session_loop(state, session_id, None, "workflow.compensation_adapter").await?;
    let loop_running = state.start_session_loop_job(queued.id, &worker_id).await?;
    state
        .append_event(
            "worker",
            Some(loop_running.id),
            loop_running.session_id,
            "session.loop.started",
            json!({
                "session_loop_job_id": loop_running.id,
                "environment_id": loop_running.environment_id,
                "worker_id": worker_id,
                "attempt_count": loop_running.attempt_count,
                "workflow_step_run_id": running_step.id,
                "adapter_owned": true
            }),
        )
        .await?;
    let completed_job = state
        .complete_session_loop_job(loop_running.id, &worker_id)
        .await?;
    state
        .append_event(
            "worker",
            Some(completed_job.id),
            completed_job.session_id,
            "session.loop.completed",
            json!({
                "session_loop_job_id": completed_job.id,
                "status": completed_job.status,
                "session_status": session.status,
                "worker_id": worker_id,
                "workflow_step_run_id": running_step.id,
                "adapter_owned": true
            }),
        )
        .await?;

    let mut completed_step = running_step.clone();
    let previous_status = completed_step.status.clone();
    let completed_at = Utc::now();
    completed_step.status = "completed".to_string();
    completed_step.output_payload = json!({
        "rollback_adapter": {
            "status": "completed",
            "adapter_kind": workflow_compensation_adapter_kind(&running_step),
            "worker_id": worker_id,
            "session_id": session_id,
            "context_packet_id": context_packet.id,
            "session_loop_job_id": completed_job.id,
            "failure_trigger": running_step.input_payload.get("failure_trigger").cloned().unwrap_or(Value::Null),
            "graph_step": workflow_graph_console_summary(
                running_step
                    .input_payload
                    .get("graph_step")
                    .unwrap_or(&Value::Null)
            )
        }
    });
    completed_step.completed_at = Some(completed_at);
    completed_step.lease_expires_at = None;
    completed_step.updated_at = completed_at;
    let completed_step = state.update_workflow_step_run(completed_step).await?;
    record_workflow_step_run_updated(state, &run, &completed_step, &previous_status).await?;
    record_workflow_step_worker_completed(state, &run, &completed_step, &worker_id, &completed_job)
        .await?;
    record_workflow_transition(
        state,
        &run,
        Some(&completed_step),
        Some(&completed_step),
        "compensation",
        "completed",
        json!({
            "source": "workflow_compensation_adapter",
            "adapter_kind": workflow_compensation_adapter_kind(&completed_step),
            "failure_trigger": completed_step.output_payload["rollback_adapter"]["failure_trigger"].clone()
        }),
        json!({
            "workflow_step_run_id": completed_step.id,
            "worker_id": worker_id,
            "session_loop_job_id": completed_job.id,
            "adapter_owned": true
        }),
    )
    .await?;
    advance_workflow_graph_after_step_update(state, &run, &completed_step).await?;

    Ok(Json(RunWorkflowStepRunResponse {
        step: completed_step,
        task_grant,
        context_packet,
        session,
        session_loop_job: completed_job,
    }))
}

pub(crate) async fn record_workflow_step_worker_started(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    worker_id: &str,
    context_packet_id: Uuid,
) -> Result<(), AppError> {
    let session_id = step.session_id.unwrap_or(run.primary_session_id);
    let payload = json!({
        "workflow_run_id": run.id,
        "workflow_step_run_id": step.id,
        "step_key": step.step_key,
        "worker_id": worker_id,
        "context_packet_id": context_packet_id
    });
    state
        .append_event(
            "worker",
            Some(step.id),
            session_id,
            "workflow.step.worker_started",
            payload.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "worker",
            Some(step.id),
            "workflow_step_run.worker_started",
            "workflow_step_run",
            Some(step.id),
            payload,
        ))
        .await?;
    Ok(())
}

pub(crate) fn workflow_step_worker_message(step: &WorkflowStepRun, grant: &TaskGrant) -> String {
    let objective = step
        .input_payload
        .get("graph_step")
        .and_then(|graph_step| graph_step.get("input"))
        .and_then(|input| input.get("objective"))
        .and_then(Value::as_str)
        .or_else(|| step.input_payload.get("objective").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(grant.objective.as_str());
    format!(
        "Execute workflow step `{}` ({}) for workflow run {}.\nObjective: {}\nInput payload: {}",
        step.step_key,
        step.step_type,
        step.workflow_run_id,
        objective,
        workflow_graph_console_summary(&step.input_payload)
    )
}

pub(crate) async fn collect_session_runtime_refs(
    state: &AppState,
    session_id: Uuid,
) -> Result<SessionRuntimeRefs, AppError> {
    Ok(SessionRuntimeRefs {
        artifact_ids: state
            .list_artifacts(session_id)
            .await?
            .into_iter()
            .map(|artifact| artifact.id)
            .collect(),
        approval_ids: state
            .list_approvals()
            .await?
            .into_iter()
            .filter(|approval| approval.session_id == session_id)
            .map(|approval| approval.id)
            .collect(),
        tool_call_ids: state
            .list_tool_calls(Some(session_id))
            .await?
            .into_iter()
            .map(|tool_call| tool_call.id)
            .collect(),
    })
}

pub(crate) fn diff_session_runtime_refs(
    before: &SessionRuntimeRefs,
    after: &SessionRuntimeRefs,
) -> SessionRuntimeRefs {
    let before_artifacts = before.artifact_ids.iter().copied().collect::<HashSet<_>>();
    let before_approvals = before.approval_ids.iter().copied().collect::<HashSet<_>>();
    let before_tool_calls = before.tool_call_ids.iter().copied().collect::<HashSet<_>>();
    SessionRuntimeRefs {
        artifact_ids: after
            .artifact_ids
            .iter()
            .copied()
            .filter(|id| !before_artifacts.contains(id))
            .collect(),
        approval_ids: after
            .approval_ids
            .iter()
            .copied()
            .filter(|id| !before_approvals.contains(id))
            .collect(),
        tool_call_ids: after
            .tool_call_ids
            .iter()
            .copied()
            .filter(|id| !before_tool_calls.contains(id))
            .collect(),
    }
}

pub(crate) async fn update_workflow_step_after_worker_session(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    session: &Session,
    session_loop_job: &SessionLoopJob,
    worker_id: &str,
    refs: SessionRuntimeRefs,
    error_message: Option<String>,
    session_loop_resume: bool,
) -> Result<WorkflowStepRun, AppError> {
    let previous_status = step.status.clone();
    let now = Utc::now();
    let next_status = if error_message.is_some()
        || session.status == SessionStatus::Failed
        || session_loop_job.status == SessionLoopJobStatus::Failed
    {
        "failed"
    } else if session.status == SessionStatus::RequiresAction {
        "requires_action"
    } else {
        "completed"
    };
    let mut next = step.clone();
    next.status = next_status.to_string();
    next.artifact_ids = refs.artifact_ids.clone();
    next.approval_ids = refs.approval_ids.clone();
    next.tool_call_ids = refs.tool_call_ids.clone();
    next.output_payload = json!({
        "worker_execution": {
            "status": next_status,
            "worker_id": worker_id,
            "session_id": session.id,
            "session_status": session.status,
            "session_loop_job_id": session_loop_job.id,
            "session_loop_job_status": session_loop_job.status,
            "context_packet_id": step.context_packet_id,
            "artifact_ids": refs.artifact_ids,
            "approval_ids": refs.approval_ids,
            "tool_call_ids": refs.tool_call_ids,
            "error": error_message,
            "session_loop_resume": session_loop_resume
        }
    });
    if workflow_step_status_terminal(next_status) {
        next.completed_at = next.completed_at.or(Some(now));
    }
    next.updated_at = now;
    let updated = state.update_workflow_step_run(next).await?;
    record_workflow_step_run_updated(state, run, &updated, &previous_status).await?;
    record_workflow_step_worker_completed(state, run, &updated, worker_id, session_loop_job)
        .await?;
    if previous_status != updated.status {
        advance_workflow_graph_after_step_update(state, run, &updated).await?;
    }
    Ok(updated)
}

pub(crate) async fn record_workflow_step_worker_completed(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    worker_id: &str,
    session_loop_job: &SessionLoopJob,
) -> Result<(), AppError> {
    let session_id = step.session_id.unwrap_or(run.primary_session_id);
    let payload = json!({
        "workflow_run_id": run.id,
        "workflow_step_run_id": step.id,
        "step_key": step.step_key,
        "status": step.status,
        "worker_id": worker_id,
        "session_loop_job_id": session_loop_job.id,
        "session_loop_job_status": session_loop_job.status,
        "artifact_ids": step.artifact_ids,
        "approval_ids": step.approval_ids,
        "tool_call_ids": step.tool_call_ids
    });
    state
        .append_event(
            "worker",
            Some(step.id),
            session_id,
            "workflow.step.worker_completed",
            payload.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "worker",
            Some(step.id),
            "workflow_step_run.worker_completed",
            "workflow_step_run",
            Some(step.id),
            payload,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn reconcile_workflow_steps_after_session_loop_job(
    state: &AppState,
    session: &Session,
    session_loop_job: &SessionLoopJob,
    worker_id: &str,
) -> Result<Vec<WorkflowStepRun>, AppError> {
    let runtime_refs = collect_session_runtime_refs(state, session.id).await?;
    let mut updated_steps = Vec::new();
    for run in state.list_workflow_runs().await? {
        let steps = state.list_workflow_step_runs(run.id).await?;
        for step in steps.into_iter().filter(|step| {
            matches!(step.status.as_str(), "running" | "requires_action")
                && (step.session_id == Some(session.id) || run.primary_session_id == session.id)
        }) {
            let updated = update_workflow_step_after_worker_session(
                state,
                &run,
                &step,
                session,
                session_loop_job,
                worker_id,
                runtime_refs.clone(),
                None,
                true,
            )
            .await?;
            updated_steps.push(updated);
        }
    }
    Ok(updated_steps)
}

pub(crate) async fn record_agent_inbox_claimed(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
    task_grant: &TaskGrant,
    context_packet: &ContextPacket,
    subject: &str,
    worker_id: &str,
) -> Result<(), AppError> {
    let session_id = step.session_id.unwrap_or(run.primary_session_id);
    let payload = json!({
        "workflow_run_id": run.id,
        "workflow_step_run_id": step.id,
        "step_key": step.step_key,
        "agent_id": step.agent_id,
        "task_grant_id": task_grant.id,
        "context_packet_id": context_packet.id,
        "claimed_by_worker": worker_id,
        "lease_expires_at": step.lease_expires_at,
        "subject": subject
    });
    state
        .append_event(
            "system",
            Some(step.id),
            session_id,
            "agent_inbox.claimed",
            payload.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "system",
            Some(step.id),
            "agent_inbox.claimed",
            "workflow_step_run",
            Some(step.id),
            payload,
        ))
        .await?;
    Ok(())
}

pub(crate) fn workflow_step_status_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "canceled" | "skipped")
}

pub(crate) fn workflow_step_status_successful(status: &str) -> bool {
    matches!(status, "completed" | "skipped")
}

pub(crate) async fn activate_due_workflow_steps_for_run(
    state: &AppState,
    run: &WorkflowRun,
    checked_at: DateTime<Utc>,
) -> Result<WorkflowScheduledStepActivationRun, AppError> {
    let mut scheduled_steps = state
        .list_workflow_step_runs(run.id)
        .await?
        .into_iter()
        .filter(|step| step.status == "scheduled")
        .collect::<Vec<_>>();
    scheduled_steps.sort_by_key(|step| step.scheduled_at);

    let mut activated_step_ids = Vec::new();
    for step in scheduled_steps {
        let Some(scheduled_at) = step.scheduled_at else {
            continue;
        };
        if scheduled_at > checked_at {
            continue;
        }
        let Some(updated) = state
            .activate_scheduled_workflow_step_run(step.id, checked_at)
            .await?
        else {
            continue;
        };
        let previous_status = "scheduled";
        record_workflow_step_run_updated(state, run, &updated, previous_status).await?;
        record_workflow_transition(
            state,
            run,
            Some(&updated),
            Some(&updated),
            "schedule",
            "queued",
            json!({
                "source": "scheduled_step_due",
                "scheduled_at": scheduled_at,
                "checked_at": checked_at
            }),
            json!({
                "workflow_step_run_id": updated.id,
                "workflow_status": "running"
            }),
        )
        .await?;
        activated_step_ids.push(updated.id);
    }
    if !activated_step_ids.is_empty() {
        update_workflow_run_status_and_record(state, run, "running").await?;
    }
    let remaining_scheduled_count = state
        .list_workflow_step_runs(run.id)
        .await?
        .into_iter()
        .filter(|step| step.status == "scheduled")
        .count();
    Ok(WorkflowScheduledStepActivationRun {
        workflow_run_id: run.id,
        checked_at,
        activated_count: activated_step_ids.len(),
        activated_step_ids,
        remaining_scheduled_count,
    })
}

pub(crate) async fn execute_due_workflow_scheduled_steps(
    state: &AppState,
    checked_at: DateTime<Utc>,
) -> Result<WorkflowScheduledStepActivationSweep, AppError> {
    let runs = state.list_workflow_runs().await?;
    let mut workflow_run_count = 0usize;
    let mut scheduled_step_count = 0usize;
    let mut due_step_count = 0usize;
    let mut activated_count = 0usize;
    let mut remaining_scheduled_count = 0usize;
    let mut activated_step_ids = Vec::new();
    for run in runs {
        if workflow_step_status_terminal(&run.status) {
            continue;
        }
        let scheduled_steps = state
            .list_workflow_step_runs(run.id)
            .await?
            .into_iter()
            .filter(|step| step.status == "scheduled")
            .collect::<Vec<_>>();
        if scheduled_steps.is_empty() {
            continue;
        }
        workflow_run_count += 1;
        scheduled_step_count += scheduled_steps.len();
        due_step_count += scheduled_steps
            .iter()
            .filter(|step| {
                step.scheduled_at
                    .is_some_and(|scheduled_at| scheduled_at <= checked_at)
            })
            .count();
        let activation = activate_due_workflow_steps_for_run(state, &run, checked_at).await?;
        activated_count += activation.activated_count;
        remaining_scheduled_count += activation.remaining_scheduled_count;
        activated_step_ids.extend(activation.activated_step_ids);
    }
    let mut actions = Vec::new();
    if activated_count > 0 {
        actions.push("activate_due_workflow_scheduled_steps".to_string());
    }
    let status = if activated_count > 0 {
        "completed"
    } else if scheduled_step_count > 0 {
        "waiting"
    } else {
        "noop"
    }
    .to_string();
    Ok(WorkflowScheduledStepActivationSweep {
        status,
        checked_at,
        workflow_run_count,
        scheduled_step_count,
        due_step_count,
        activated_count,
        activated_step_ids,
        remaining_scheduled_count,
        actions,
    })
}
