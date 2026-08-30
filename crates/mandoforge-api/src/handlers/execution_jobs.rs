use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use uuid::Uuid;

use crate::execution::{
    publish_execution_completion_tail, recover_expired_execution_with_durable_outcome,
};
use crate::{
    AppError, AppState, CreateRemoteComputerJobAssignment, ExecutionJobStatus, Permission,
    RemoteComputerJobAssignment, Role, SessionLoopJob, SessionStatus, StoreBackend,
    WorkerLoadValidationRun, WorkerReadinessReport, authorize_collection_request,
    authorize_execution_job_run, authorize_request, authorize_session_loop_job_run,
    build_worker_readiness, cleanup_remote_computer_lease_runtime, deterministic_record_id,
    enforce_worker_environment_binding, enforce_worker_pool_binding,
    ensure_http_execution_process_role, ensure_worker_process_role, environment_worker_pool,
    execute_worker_load_validation, header_value, new_audit_log, principal_from_request,
    reconcile_workflow_steps_after_session_loop_job, record_remote_computer_job_assignment_event,
    record_remote_computer_job_assignment_event_for_execution_claim,
    replay_remote_computer_lease_runtime_cleanup_evidence, run_execution_job_with_lease_renewal,
    run_session_loop_with_lease_renewal, session_accepts_worker_execution,
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

fn trusted_worker_principal(
    state: &AppState,
    principal: &crate::Principal,
) -> Result<(), AppError> {
    ensure_worker_process_role(state)?;
    if principal.roles.contains(&Role::Worker) {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "worker daemon requires a worker principal",
        ))
    }
}

fn trusted_worker_id(headers: &HeaderMap) -> Result<String, AppError> {
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("x-mandoforge-worker-id header is required for job execution")
        })?;
    if worker_id == "api" || worker_id == "session-loop-worker" {
        return Err(AppError::bad_request(
            "x-mandoforge-worker-id must identify a concrete worker",
        ));
    }
    Ok(worker_id.to_string())
}

pub(crate) async fn worker_list_execution_jobs(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Vec<crate::execution_queue::ExecutionJob>, AppError> {
    let principal =
        authorize_collection_request(state, headers, Permission::SessionsRead, "execution_jobs")
            .await?;
    trusted_worker_principal(state, &principal)?;
    let has_worker_scope = worker_scope_headers_present(headers)?;
    let worker_environment_id = worker_environment_id_from_headers(headers)?;
    let worker_pool = worker_pool_from_headers(headers);
    let worker_pool_environment_ids =
        worker_pool_environment_ids(state, worker_pool.as_deref()).await?;
    let visible_session_ids = visible_session_ids_for_principal(state, &principal).await?;
    Ok(state
        .execution_queue
        .list()
        .await?
        .into_iter()
        .filter(|job| visible_session_ids.contains(&job.session_id))
        .filter(|job| {
            has_worker_scope || !worker_environment_scope_required() || job.environment_id.is_none()
        })
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
        .collect())
}

pub(crate) async fn worker_list_session_loop_jobs(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Vec<SessionLoopJob>, AppError> {
    let principal = authorize_collection_request(
        state,
        headers,
        Permission::SessionsRead,
        "session_loop_jobs",
    )
    .await?;
    trusted_worker_principal(state, &principal)?;
    let has_worker_scope = worker_scope_headers_present(headers)?;
    let worker_environment_id = worker_environment_id_from_headers(headers)?;
    let worker_pool = worker_pool_from_headers(headers);
    let worker_pool_environment_ids =
        worker_pool_environment_ids(state, worker_pool.as_deref()).await?;
    let visible_session_ids = visible_session_ids_for_principal(state, &principal).await?;
    Ok(state
        .list_session_loop_jobs()
        .await?
        .into_iter()
        .filter(|job| visible_session_ids.contains(&job.session_id))
        .filter(|job| {
            has_worker_scope || !worker_environment_scope_required() || job.environment_id.is_none()
        })
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
        .collect())
}

pub(crate) async fn worker_run_session_loop_job(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
) -> Result<SessionLoopJob, AppError> {
    let principal = principal_from_request(state, headers).await?;
    trusted_worker_principal(state, &principal)?;
    authorize_session_loop_job_run(state, headers, id).await?;
    let worker_id = trusted_worker_id(headers)?;
    run_session_loop_job_as_worker(state, id, headers, &worker_id).await
}

pub(crate) async fn worker_run_execution_job(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let principal = principal_from_request(state, headers).await?;
    trusted_worker_principal(state, &principal)?;
    authorize_execution_job_run(state, headers, id).await?;
    let worker_id = trusted_worker_id(headers)?;
    run_execution_job_as_worker(state, id, headers, &worker_id).await
}

async fn run_session_loop_job_as_worker(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
    worker_id: &str,
) -> Result<SessionLoopJob, AppError> {
    let job = state.get_session_loop_job(id).await?;
    enforce_worker_environment_binding(state, headers, job.session_id, job.environment_id).await?;
    enforce_worker_pool_binding(state, headers, job.session_id, job.environment_id).await?;
    if !session_accepts_worker_execution(state, job.session_id).await? {
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
        return Ok(skipped);
    }
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
    match run_session_loop_with_lease_renewal(state, &running, worker_id, None).await {
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
            reconcile_workflow_steps_after_session_loop_job(state, &session, &completed, worker_id)
                .await?;
            Ok(completed)
        }
        Err(error) => {
            let failed = state
                .fail_session_loop_job(running.id, worker_id, &error.message)
                .await?;
            let session_is_terminal =
                error.message == "session is terminal and cannot run session loop work";
            if !session_is_terminal {
                set_managed_session_status(
                    state,
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

async fn run_execution_job_as_worker(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
    worker_id: &str,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let job = state.execution_queue.get(id).await?;
    enforce_worker_environment_binding(state, headers, job.session_id, job.environment_id).await?;
    enforce_worker_pool_binding(state, headers, job.session_id, job.environment_id).await?;
    if job.status == ExecutionJobStatus::Completed
        && job.finalization_details["stage"] == "completion_pending"
    {
        publish_execution_completion_tail(state, &job).await?;
        return state.execution_queue.get(id).await;
    }
    if job.status == ExecutionJobStatus::CancelRequested {
        let owned = job.worker_id.as_deref() == Some(worker_id)
            && job
                .lease_expires_at
                .is_some_and(|lease_expires_at| lease_expires_at > chrono::Utc::now());
        let claimed = if owned {
            job
        } else {
            state
                .execution_queue
                .claim_cancellation(id, worker_id)
                .await?
        };
        return finalize_execution_cancellation(state, &claimed).await;
    }
    let finished = run_execution_job_with_lease_renewal(state, id, worker_id).await?;
    if finished.status == ExecutionJobStatus::CancelRequested {
        return finalize_execution_cancellation(state, &finished).await;
    }
    Ok(finished)
}

pub(crate) async fn worker_recover_execution_cancellation(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let principal = principal_from_request(state, headers).await?;
    trusted_worker_principal(state, &principal)?;
    authorize_execution_job_run(state, headers, id).await?;
    let worker_id = trusted_worker_id(headers)?;
    let job = state.execution_queue.get(id).await?;
    enforce_worker_environment_binding(state, headers, job.session_id, job.environment_id).await?;
    enforce_worker_pool_binding(state, headers, job.session_id, job.environment_id).await?;
    if job.status != ExecutionJobStatus::CancelRequested {
        return Err(AppError::not_found("execution job not found"));
    }
    let claimed = state
        .execution_queue
        .claim_cancellation(job.id, &worker_id)
        .await?;
    finalize_execution_cancellation(state, &claimed).await
}

pub(crate) async fn worker_record_execution_outcome_unknown(
    state: &AppState,
    id: Uuid,
    headers: &HeaderMap,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let principal = principal_from_request(state, headers).await?;
    trusted_worker_principal(state, &principal)?;
    authorize_execution_job_run(state, headers, id).await?;
    let worker_id = trusted_worker_id(headers)?;
    let job = state.execution_queue.get(id).await?;
    enforce_worker_environment_binding(state, headers, job.session_id, job.environment_id).await?;
    enforce_worker_pool_binding(state, headers, job.session_id, job.environment_id).await?;
    let now = chrono::Utc::now();
    if job.status != ExecutionJobStatus::Executing
        || job
            .lease_expires_at
            .is_some_and(|lease_expires_at| lease_expires_at > now)
    {
        return Err(AppError::not_found("execution job not found"));
    }
    recover_expired_execution_with_durable_outcome(state, &job, &worker_id).await
}

async fn finalize_execution_cancellation(
    state: &AppState,
    job: &crate::execution_queue::ExecutionJob,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let job = state
        .execution_queue
        .begin_cancellation_cleanup(
            job.id,
            job.worker_id.as_deref().unwrap_or(""),
            job.claim_generation,
        )
        .await?;
    record_execution_cancel_requested(state, &job).await?;
    propagate_remote_computer_execution_cancel(state, &job).await?;
    ensure_execution_cancellation_claim(state, &job).await?;
    let event_type = "execution.canceled";
    let event_id = deterministic_record_id(job.id, "execution-cancellation-event", &[event_type]);
    let details = json!({
        "execution_job_id": job.id,
        "approval_id": job.approval_id,
        "tool_call_id": job.tool_call_id,
        "tool": job.tool_name,
        "attempt_count": job.attempt_count,
        "cancellation_confirmed": true,
        "reason": "remote cleanup completed before cancellation terminal state",
    });
    state
        .append_event_once_for_execution_claim(
            &job,
            ExecutionJobStatus::CancelRequested,
            event_id,
            "worker",
            Some(job.id),
            job.session_id,
            event_type,
            details.clone(),
        )
        .await?;
    let mut audit = new_audit_log(
        Some(job.session_id),
        "worker",
        Some(job.id),
        event_type,
        "execution_job",
        Some(job.id),
        details,
    );
    audit.id = deterministic_record_id(event_id, "audit", &[event_type]);
    state
        .append_audit_log_for_execution_claim(&job, ExecutionJobStatus::CancelRequested, audit)
        .await?;
    ensure_execution_cancellation_claim(state, &job).await?;
    state
        .execution_queue
        .acknowledge_canceled(
            job.id,
            job.worker_id.as_deref().unwrap_or(""),
            job.claim_generation,
        )
        .await
}

async fn ensure_execution_cancellation_claim(
    state: &AppState,
    job: &crate::execution_queue::ExecutionJob,
) -> Result<(), AppError> {
    let current = state.execution_queue.get(job.id).await?;
    if current.status == ExecutionJobStatus::CancelRequested
        && current.worker_id == job.worker_id
        && current.claim_generation == job.claim_generation
        && current
            .lease_expires_at
            .is_some_and(|lease_expires_at| lease_expires_at > chrono::Utc::now())
    {
        Ok(())
    } else {
        Err(AppError::not_found(
            "execution cancellation claim is no longer owned",
        ))
    }
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
    ensure_http_execution_process_role(&state)?;
    authorize_session_loop_job_run(&state, &headers, id).await?;
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("session-loop-worker")
        .to_string();
    Ok(Json(
        run_session_loop_job_as_worker(&state, id, &headers, &worker_id).await?,
    ))
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
        ExecutionJobStatus::Executing
            | ExecutionJobStatus::Finalizing
            | ExecutionJobStatus::Completed
            | ExecutionJobStatus::Failed
            | ExecutionJobStatus::OutcomeUnknown
            | ExecutionJobStatus::Canceled
    ) {
        return Ok(Json(job));
    }
    let cancel_requested = state.execution_queue.cancel(id).await?;
    if cancel_requested.status != ExecutionJobStatus::CancelRequested {
        return Ok(Json(cancel_requested));
    }
    if job.status == ExecutionJobStatus::Running {
        record_execution_cancel_requested(&state, &cancel_requested).await?;
        return Ok(Json(cancel_requested));
    }
    let claimed = match state
        .execution_queue
        .claim_cancellation(cancel_requested.id, "api-cancel")
        .await
    {
        Ok(claimed) => claimed,
        Err(_) => return Ok(Json(state.execution_queue.get(cancel_requested.id).await?)),
    };
    Ok(Json(
        finalize_execution_cancellation(&state, &claimed).await?,
    ))
}

async fn record_execution_cancel_requested(
    state: &AppState,
    job: &crate::execution_queue::ExecutionJob,
) -> Result<(), AppError> {
    let event_type = "execution.cancel_requested";
    let event_id = deterministic_record_id(job.id, "execution-cancellation-event", &[event_type]);
    let details = json!({
        "execution_job_id": job.id,
        "approval_id": job.approval_id,
        "tool_call_id": job.tool_call_id,
        "tool": job.tool_name,
        "attempt_count": job.attempt_count,
        "reason": "authorized cancellation awaiting worker cleanup",
    });
    state
        .append_event_once_for_execution_claim(
            job,
            ExecutionJobStatus::CancelRequested,
            event_id,
            "worker",
            Some(job.id),
            job.session_id,
            event_type,
            details.clone(),
        )
        .await?;
    let mut audit = new_audit_log(
        Some(job.session_id),
        "worker",
        Some(job.id),
        event_type,
        "execution_job",
        Some(job.id),
        details,
    );
    audit.id = deterministic_record_id(event_id, "audit", &[event_type]);
    state
        .append_audit_log_for_execution_claim(job, ExecutionJobStatus::CancelRequested, audit)
        .await
        .map(|_| ())
}

async fn assign_execution_job_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(mut input): Json<CreateRemoteComputerJobAssignment>,
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
    let attempt_count = if job.status == ExecutionJobStatus::Queued {
        job.attempt_count + 1
    } else {
        job.attempt_count
    };
    let claim_generation = if job.status == ExecutionJobStatus::Queued {
        job.claim_generation + 1
    } else {
        job.claim_generation
    };
    let mut metadata = input.metadata.take().unwrap_or_else(|| json!({}));
    let Some(metadata) = metadata.as_object_mut() else {
        return Err(AppError::bad_request(
            "remote computer assignment metadata must be an object",
        ));
    };
    metadata.insert("execution_attempt_count".to_string(), json!(attempt_count));
    metadata.insert(
        "execution_claim_generation".to_string(),
        json!(claim_generation),
    );
    input.metadata = Some(Value::Object(metadata.clone()));
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
    ensure_http_execution_process_role(&state)?;
    authorize_execution_job_run(&state, &headers, id).await?;
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("api")
        .to_string();
    let completed = run_execution_job_as_worker(&state, id, &headers, &worker_id).await?;
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
    let attempt_count = job.attempt_count.max(1);
    let assignments = state.list_remote_computer_job_assignments().await?;
    let assignment = assignments.iter().find(|assignment| {
        assignment.execution_job_id == job.id
            && assignment.metadata["execution_attempt_count"].as_i64()
                == Some(i64::from(attempt_count))
            && matches!(assignment.status.as_str(), "assigned" | "canceled")
    });
    let Some(assignment) = assignment else {
        if assignments.iter().any(|assignment| {
            assignment.execution_job_id == job.id && assignment.status == "assigned"
        }) {
            return Err(AppError::internal(
                "active Remote Computer assignment does not match the canceled execution attempt",
            ));
        }
        return Ok(json!({"assigned": false, "pod_delete_attempted": false}));
    };
    let lease = state
        .list_remote_computer_leases()
        .await?
        .into_iter()
        .find(|lease| lease.id == assignment.lease_id)
        .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
    if assignment.status == "canceled" {
        replay_remote_computer_lease_runtime_cleanup_evidence(
            state,
            &lease,
            None,
            "execution_job_cancel",
        )
        .await?;
        record_remote_computer_job_assignment_event_for_execution_claim(
            state,
            assignment,
            job,
            "remote_computer.execution_handoff_canceled",
            ExecutionJobStatus::CancelRequested,
        )
        .await?;
        return Ok(json!({
            "assigned": false,
            "assignment_id": assignment.id,
            "remote_computer_id": assignment.remote_computer_id,
            "lease_id": assignment.lease_id,
            "cancellation_evidence_replayed": true,
        }));
    }
    let cleanup_operation_id = deterministic_record_id(
        job.id,
        "execution-cancellation-cleanup",
        &[attempt_count.to_string().as_str()],
    );
    state
        .finalize_remote_computer_job_assignment_for_attempt(
            assignment.id,
            job.id,
            attempt_count,
            job.claim_generation,
            job.worker_id.as_deref().unwrap_or(""),
            ExecutionJobStatus::CancelRequested,
            "assigned",
            json!({
                "execution_cancellation_cleanup": {
                    "operation_id": cleanup_operation_id,
                    "status": "started"
                }
            }),
        )
        .await?;
    let cleanup = cleanup_remote_computer_lease_runtime(
        state,
        &lease,
        None,
        "execution_job_cancel",
        "canceled",
    )
    .await?;
    ensure_execution_cancellation_claim(state, job).await?;
    let updated = state
        .finalize_remote_computer_job_assignment_for_attempt(
            assignment.id,
            job.id,
            attempt_count,
            job.claim_generation,
            job.worker_id.as_deref().unwrap_or(""),
            ExecutionJobStatus::CancelRequested,
            "canceled",
            json!({
                "execution_cancellation_confirmed": true,
                "execution_cancellation_cleanup": {
                    "operation_id": cleanup_operation_id,
                    "status": "completed",
                    "result": cleanup.clone()
                }
            }),
        )
        .await?;
    record_remote_computer_job_assignment_event_for_execution_claim(
        state,
        &updated,
        job,
        "remote_computer.execution_handoff_canceled",
        ExecutionJobStatus::CancelRequested,
    )
    .await?;
    Ok(json!({
        "assigned": true,
        "assignment_id": updated.id,
        "remote_computer_id": updated.remote_computer_id,
        "lease_id": updated.lease_id,
        "runtime_cleanup": cleanup,
    }))
}
