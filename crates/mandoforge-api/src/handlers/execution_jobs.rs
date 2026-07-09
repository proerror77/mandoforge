use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateRemoteComputerJobAssignment, ExecutionJobStatus, Permission,
    RemoteComputerJobAssignment, RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    Role, SessionLoopJob, SessionStatus, StoreBackend, WorkerLoadValidationRun,
    WorkerReadinessReport, authorize_collection_request, authorize_execution_job_run,
    authorize_request, authorize_session_loop_job_run, build_worker_readiness,
    enforce_worker_environment_binding, enforce_worker_pool_binding, environment_worker_pool,
    execute_worker_load_validation, header_value, new_audit_log,
    reconcile_workflow_steps_after_session_loop_job, record_remote_computer_job_assignment_event,
    remote_computer_pod_execution_requested_from_env, remote_computer_runner_for_config,
    run_execution_job, run_session_loop, session_accepts_worker_execution,
    set_managed_session_status, visible_session_ids_for_principal,
    worker_environment_scope_required, worker_scope_headers_present,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/execution-jobs", get(list_execution_jobs))
        .route("/api/queue/notify-wait", get(queue_notify_wait))
        .route("/api/session-loop-jobs", get(list_session_loop_jobs))
        .route(
            "/api/session-loop-jobs/{id}/run",
            post(run_session_loop_job_route),
        )
        .route(
            "/api/execution-jobs/{id}/cancel",
            post(cancel_execution_job_route),
        )
        .route(
            "/api/execution-jobs/{id}/remote-computer-lease",
            post(assign_execution_job_remote_computer_lease),
        )
        .route(
            "/api/execution-jobs/worker-readiness",
            get(get_worker_readiness),
        )
        .route(
            "/api/execution-jobs/worker-load-validation/run",
            post(run_worker_load_validation),
        )
        .route(
            "/api/execution-jobs/{id}/run",
            post(run_execution_job_route),
        )
}

async fn list_execution_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::execution_queue::ExecutionJob>>, AppError> {
    let principal =
        authorize_collection_request(&state, &headers, Permission::SessionsRead, "execution_jobs")
            .await?;
    if principal.roles.contains(&Role::Worker)
        && worker_environment_scope_required()
        && !worker_scope_headers_present(&headers)?
    {
        return Err(AppError::bad_request(
            "worker polling requires x-mandoforge-environment-id or x-mandoforge-worker-pool",
        ));
    }
    let worker_environment_id = worker_environment_id_from_headers(&headers)?;
    let worker_pool = worker_pool_from_headers(&headers);
    let worker_pool_environment_ids =
        worker_pool_environment_ids(&state, worker_pool.as_deref()).await?;
    let visible_sessions = state.list_sessions_visible_to(&principal).await?;
    let visible_session_ids: HashSet<_> =
        visible_sessions.iter().map(|session| session.id).collect();
    Ok(Json(
        state
            .execution_queue
            .list()
            .await?
            .into_iter()
            .filter(|job| visible_session_ids.contains(&job.session_id))
            .filter(|job| {
                worker_environment_id
                    .is_none_or(|environment_id| job.environment_id == Some(environment_id))
            })
            .filter(|job| {
                worker_pool.as_ref().is_none_or(|_| {
                    job.environment_id.is_some_and(|environment_id| {
                        worker_pool_environment_ids.contains(&environment_id)
                    })
                })
            })
            .collect(),
    ))
}

async fn queue_notify_wait(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err(e) = authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "queue_notify_wait",
        None,
    )
    .await
    {
        return e.into_response();
    }

    let timeout_ms = params
        .get("timeout_ms")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5_000)
        .min(29_000);

    let Some(channel) = state.execution_queue.notify_channel() else {
        tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
        return axum::http::StatusCode::NO_CONTENT.into_response();
    };

    let StoreBackend::Postgres(pool) = &state.store else {
        tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
        return axum::http::StatusCode::NO_CONTENT.into_response();
    };

    let mut listener = match sqlx::postgres::PgListener::connect_with(pool).await {
        Ok(l) => l,
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
            return axum::http::StatusCode::NO_CONTENT.into_response();
        }
    };

    if listener.listen(&channel).await.is_err() {
        return axum::http::StatusCode::NO_CONTENT.into_response();
    }

    let deadline = tokio::time::sleep(Duration::from_millis(timeout_ms));
    tokio::select! {
        _ = listener.recv() => axum::http::StatusCode::OK.into_response(),
        _ = deadline => axum::http::StatusCode::NO_CONTENT.into_response(),
    }
}

async fn list_session_loop_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionLoopJob>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session_loop_jobs",
    )
    .await?;
    if principal.roles.contains(&Role::Worker)
        && worker_environment_scope_required()
        && !worker_scope_headers_present(&headers)?
    {
        return Err(AppError::bad_request(
            "worker polling requires x-mandoforge-environment-id or x-mandoforge-worker-pool",
        ));
    }
    let worker_environment_id = worker_environment_id_from_headers(&headers)?;
    let worker_pool = worker_pool_from_headers(&headers);
    let worker_pool_environment_ids =
        worker_pool_environment_ids(&state, worker_pool.as_deref()).await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_session_loop_jobs()
            .await?
            .into_iter()
            .filter(|job| visible_session_ids.contains(&job.session_id))
            .filter(|job| {
                worker_environment_id
                    .is_none_or(|environment_id| job.environment_id == Some(environment_id))
            })
            .filter(|job| {
                worker_pool.as_ref().is_none_or(|_| {
                    job.environment_id.is_some_and(|environment_id| {
                        worker_pool_environment_ids.contains(&environment_id)
                    })
                })
            })
            .collect(),
    ))
}

async fn run_session_loop_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SessionLoopJob>, AppError> {
    authorize_session_loop_job_run(&state, &headers, id).await?;
    let job = state.get_session_loop_job(id).await?;
    enforce_worker_environment_binding(&state, &headers, job.session_id, job.environment_id)
        .await?;
    enforce_worker_pool_binding(&state, &headers, job.session_id, job.environment_id).await?;
    if !session_accepts_worker_execution(&state, job.session_id).await? {
        let skipped = state
            .discard_session_loop_job(
                job.id,
                "session is terminal and cannot run session loop work",
            )
            .await?;
        state
            .append_event(
                "worker",
                Some(skipped.id),
                skipped.session_id,
                "session.loop.skipped",
                json!({
                    "session_loop_job_id": skipped.id,
                    "status": skipped.status,
                    "reason": "session is terminal",
                }),
            )
            .await?;
        return Ok(Json(skipped));
    }
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("session-loop-worker");
    let running = state.start_session_loop_job(id, worker_id).await?;
    state
        .append_event(
            "worker",
            Some(running.id),
            running.session_id,
            "session.loop.started",
            json!({
                "session_loop_job_id": running.id,
                "environment_id": running.environment_id,
                "worker_id": worker_id,
                "attempt_count": running.attempt_count
            }),
        )
        .await?;
    match run_session_loop(&state, &running).await {
        Ok(session) => {
            let completed = state
                .complete_session_loop_job(running.id, worker_id)
                .await?;
            state
                .append_event(
                    "worker",
                    Some(completed.id),
                    completed.session_id,
                    "session.loop.completed",
                    json!({
                        "session_loop_job_id": completed.id,
                        "status": completed.status,
                        "session_status": session.status,
                        "worker_id": worker_id
                    }),
                )
                .await?;
            reconcile_workflow_steps_after_session_loop_job(
                &state, &session, &completed, worker_id,
            )
            .await?;
            Ok(Json(completed))
        }
        Err(error) => {
            let failed = state
                .fail_session_loop_job(running.id, worker_id, &error.message)
                .await?;
            let session_is_terminal =
                error.message == "session is terminal and cannot run session loop work";
            if !session_is_terminal {
                set_managed_session_status(
                    &state,
                    failed.session_id,
                    SessionStatus::Failed,
                    "session loop failed",
                )
                .await?;
                state
                    .append_event(
                        "system",
                        Some(failed.id),
                        failed.session_id,
                        "session.failed",
                        json!({
                            "session_loop_job_id": failed.id,
                            "reason": "session loop failed",
                            "error": error.message
                        }),
                    )
                    .await?;
            }
            state
                .append_event(
                    "worker",
                    Some(failed.id),
                    failed.session_id,
                    "session.loop.failed",
                    json!({
                        "session_loop_job_id": failed.id,
                        "status": failed.status,
                        "error": error.message,
                        "worker_id": worker_id
                    }),
                )
                .await?;
            if session_is_terminal {
                return Err(error);
            }
            Err(error)
        }
    }
}

async fn cancel_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::execution_queue::ExecutionJob>, AppError> {
    let job = state.execution_queue.get(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(job.session_id),
    )
    .await?;
    if matches!(
        job.status,
        ExecutionJobStatus::Completed | ExecutionJobStatus::Failed | ExecutionJobStatus::Canceled
    ) {
        return Ok(Json(job));
    }
    let propagation = propagate_remote_computer_execution_cancel(&state, &job).await?;
    let canceled = state.execution_queue.cancel(id).await?;
    state
        .append_event(
            "worker",
            Some(canceled.id),
            canceled.session_id,
            "execution.canceled",
            json!({
                "execution_job_id": canceled.id,
                "approval_id": canceled.approval_id,
                "tool_call_id": canceled.tool_call_id,
                "tool": canceled.tool_name,
                "previous_status": job.status,
                "remote_computer": propagation,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(canceled.session_id),
            "worker",
            Some(canceled.id),
            "execution.canceled",
            "execution_job",
            Some(canceled.id),
            json!({
                "tool": canceled.tool_name,
                "previous_status": job.status,
                "remote_computer": propagation,
            }),
        ))
        .await?;
    Ok(Json(canceled))
}

async fn assign_execution_job_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerJobAssignment>,
) -> Result<Json<RemoteComputerJobAssignment>, AppError> {
    let job = state.execution_queue.get(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(job.session_id),
    )
    .await?;
    if job.status != ExecutionJobStatus::Queued && job.status != ExecutionJobStatus::Running {
        return Err(AppError::bad_request(
            "only queued or running execution jobs can be assigned to remote computer leases",
        ));
    }
    let assignment = state
        .create_remote_computer_job_assignment(id, job.session_id, input)
        .await?;
    record_remote_computer_job_assignment_event(
        &state,
        &assignment,
        &job,
        "remote_computer.execution_handoff_planned",
    )
    .await?;
    Ok(Json(assignment))
}

async fn get_worker_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WorkerReadinessReport>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "execution_jobs", None).await?;
    Ok(Json(build_worker_readiness(&state).await?))
}

async fn run_worker_load_validation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WorkerLoadValidationRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "worker_load_validation",
        None,
    )
    .await?;
    Ok(Json(execute_worker_load_validation(&state).await?))
}

async fn run_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::execution_queue::ExecutionJob>, AppError> {
    authorize_execution_job_run(&state, &headers, id).await?;
    let job = state.execution_queue.get(id).await?;
    enforce_worker_environment_binding(&state, &headers, job.session_id, job.environment_id)
        .await?;
    enforce_worker_pool_binding(&state, &headers, job.session_id, job.environment_id).await?;
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("api");
    let completed = run_execution_job(&state, id, worker_id).await?;
    Ok(Json(completed))
}

fn worker_environment_id_from_headers(headers: &HeaderMap) -> Result<Option<Uuid>, AppError> {
    let Some(value) = header_value(headers, "x-mandoforge-environment-id")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    Uuid::parse_str(value)
        .map(Some)
        .map_err(|_| AppError::bad_request("x-mandoforge-environment-id must be a UUID"))
}

fn worker_pool_from_headers(headers: &HeaderMap) -> Option<String> {
    header_value(headers, "x-mandoforge-worker-pool")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

async fn worker_pool_environment_ids(
    state: &AppState,
    worker_pool: Option<&str>,
) -> Result<HashSet<Uuid>, AppError> {
    let Some(worker_pool) = worker_pool.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(HashSet::new());
    };
    Ok(state
        .list_environments()
        .await?
        .into_iter()
        .filter(|environment| environment.archived_at.is_none())
        .filter(|environment| {
            environment_worker_pool(&environment.worker_queue_binding).as_deref()
                == Some(worker_pool)
        })
        .map(|environment| environment.id)
        .collect())
}

pub(crate) async fn propagate_remote_computer_execution_cancel(
    state: &AppState,
    job: &crate::execution_queue::ExecutionJob,
) -> Result<Value, AppError> {
    let Some(assignment) = state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .find(|assignment| {
            assignment.execution_job_id == job.id && assignment.status == "assigned"
        })
    else {
        return Ok(json!({"assigned": false, "pod_delete_attempted": false}));
    };
    let mut metadata = json!({
        "execution_job_status": "canceled",
        "canceled_at": Utc::now(),
    });
    let mut pod_delete_attempted = false;
    let mut pod_delete_status = None;
    let mut pod_delete_message = None;
    if remote_computer_pod_execution_requested_from_env() {
        let remote_computer = state
            .list_remote_computers()
            .await?
            .into_iter()
            .find(|computer| computer.id == assignment.remote_computer_id)
            .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
        if let Some(pod_name) = remote_computer_delete_name(&remote_computer) {
            let config = RemoteComputerRunnerConfig::from_env();
            let response = remote_computer_runner_for_config(&config)
                .mutate(
                    &config,
                    RemoteComputerRunnerDryRunRequest {
                        operation: Some("live_delete".to_string()),
                        remote_computer_id: Some(remote_computer.id),
                        session_id: Some(job.session_id),
                        pod_name: Some(pod_name.clone()),
                        metadata: Some(json!({
                            "execution_job_id": job.id,
                            "tool_call_id": job.tool_call_id,
                            "cancel_reason": "execution_job_cancel",
                        })),
                    },
                )
                .await;
            pod_delete_attempted = response.live_mutation_attempted;
            pod_delete_status = Some(response.status.clone());
            pod_delete_message = Some(response.message.clone());
            metadata["pod_delete"] = json!({
                "attempted": response.live_mutation_attempted,
                "status": response.status,
                "message": response.message,
                "pod_name": pod_name,
            });
            if response.status != "mutation_ok" {
                return Err(AppError::bad_request(format!(
                    "Remote Computer Pod cancellation failed: {}",
                    response.message
                )));
            }
        }
    }
    let updated = state
        .update_remote_computer_job_assignment_status(assignment.id, "canceled", metadata)
        .await?;
    record_remote_computer_job_assignment_event(
        state,
        &updated,
        job,
        "remote_computer.execution_handoff_canceled",
    )
    .await?;
    Ok(json!({
        "assigned": true,
        "assignment_id": updated.id,
        "remote_computer_id": updated.remote_computer_id,
        "lease_id": updated.lease_id,
        "pod_delete_attempted": pod_delete_attempted,
        "pod_delete_status": pod_delete_status,
        "pod_delete_message": pod_delete_message,
    }))
}

fn remote_computer_delete_name(remote_computer: &crate::RemoteComputer) -> Option<String> {
    remote_computer
        .metadata
        .get("sandbox_claim_name")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| remote_computer.pod_name.clone())
}
