use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::*;

pub(crate) const WORKER_JOB_LEASE_SECONDS: i64 = 300;
const WORKER_CANCELLATION_CHECK_INTERVAL: Duration = Duration::from_millis(500);

pub(crate) fn lease_renewal_interval(lease_seconds: i64) -> Duration {
    Duration::from_millis(((lease_seconds.max(1) as u64 * 1_000) / 3).clamp(100, 60_000))
}

pub(crate) fn handle_lease_renewal<T>(
    deadline: &mut Instant,
    lease_seconds: i64,
    result: Result<T, AppError>,
    lease_name: &str,
) -> Result<(), AppError> {
    let now = Instant::now();
    match result {
        Ok(_) => {
            *deadline = now + Duration::from_secs(lease_seconds.max(1) as u64);
            Ok(())
        }
        Err(error)
            if error.status.is_server_error()
                && now + lease_renewal_interval(lease_seconds) < *deadline =>
        {
            tracing::warn!(
                lease = lease_name,
                error = %error.message,
                "lease renewal failed; continuing within the current lease"
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
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

pub(crate) fn worker_environment_scope_required() -> bool {
    std::env::var("MANDOFORGE_REQUIRE_WORKER_ENVIRONMENT_SCOPE")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(!cfg!(test))
}

pub(crate) fn ensure_worker_process_role(state: &AppState) -> Result<(), AppError> {
    match state.process_role {
        crate::worker_daemon::ProcessRole::Worker => Ok(()),
        crate::worker_daemon::ProcessRole::Api => Err(AppError::forbidden(
            "job execution is disabled in the API process",
        )),
    }
}

pub(crate) fn ensure_http_execution_process_role(state: &AppState) -> Result<(), AppError> {
    if state.process_role == crate::worker_daemon::ProcessRole::Worker
        || insecure_dev_auth_enabled()
    {
        return Ok(());
    }
    Err(AppError::forbidden(
        "HTTP job execution is disabled in the API process; run the worker daemon or enable MANDOFORGE_INSECURE_DEV_AUTH only for local development",
    ))
}

pub(crate) async fn run_session_loop_with_lease_renewal(
    state: &AppState,
    job: &SessionLoopJob,
    worker_id: &str,
    workflow_step: Option<(Uuid, i64)>,
) -> Result<Session, AppError> {
    let work = run_session_loop(state, job);
    tokio::pin!(work);
    let renew_interval = workflow_step
        .map(|(_, lease_seconds)| lease_renewal_interval(lease_seconds))
        .unwrap_or_else(|| lease_renewal_interval(WORKER_JOB_LEASE_SECONDS))
        .min(lease_renewal_interval(WORKER_JOB_LEASE_SECONDS));
    let mut renewals = tokio::time::interval_at(Instant::now() + renew_interval, renew_interval);
    renewals.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut job_lease_deadline =
        Instant::now() + Duration::from_secs(WORKER_JOB_LEASE_SECONDS as u64);
    let mut workflow_step_lease_deadline = workflow_step.map(|(_, lease_seconds)| {
        Instant::now() + Duration::from_secs(lease_seconds.max(1) as u64)
    });
    loop {
        tokio::select! {
            result = &mut work => return result,
            _ = renewals.tick() => {
                handle_lease_renewal(
                    &mut job_lease_deadline,
                    WORKER_JOB_LEASE_SECONDS,
                    state
                        .renew_session_loop_job_lease(job.id, worker_id, WORKER_JOB_LEASE_SECONDS)
                        .await,
                    "session-loop job",
                )?;
                if let (Some((step_id, lease_seconds)), Some(deadline)) =
                    (workflow_step, workflow_step_lease_deadline.as_mut())
                {
                    handle_lease_renewal(
                        deadline,
                        lease_seconds,
                        state
                            .renew_workflow_step_run_lease(step_id, worker_id, lease_seconds)
                            .await,
                        "workflow-step run",
                    )?;
                }
            }
        }
    }
}

pub(crate) async fn run_execution_job_with_lease_renewal(
    state: &AppState,
    job_id: Uuid,
    worker_id: &str,
) -> Result<crate::execution_queue::ExecutionJob, AppError> {
    let claimed = claim_execution_job(state, job_id, worker_id).await?;
    let claim_generation = claimed.claim_generation;
    let work = run_claimed_execution_job(state, &claimed, worker_id);
    tokio::pin!(work);
    let renew_interval = lease_renewal_interval(WORKER_JOB_LEASE_SECONDS);
    let mut renewals = tokio::time::interval_at(Instant::now() + renew_interval, renew_interval);
    renewals.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut lease_deadline = Instant::now() + Duration::from_secs(WORKER_JOB_LEASE_SECONDS as u64);
    let mut cancellation_checks = tokio::time::interval_at(
        Instant::now() + WORKER_CANCELLATION_CHECK_INTERVAL,
        WORKER_CANCELLATION_CHECK_INTERVAL,
    );
    cancellation_checks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            result = &mut work => {
                return match result {
                    Ok(job) => Ok(job),
                    Err(error) => {
                        let current = state.execution_queue.get(job_id).await?;
                        match current.status {
                            ExecutionJobStatus::CancelRequested
                                if current.worker_id.as_deref() == Some(worker_id)
                                    && current.claim_generation == claim_generation =>
                            {
                                Ok(current)
                            }
                            ExecutionJobStatus::Canceled => Ok(current),
                            _ => Err(error),
                        }
                    }
                };
            }
            _ = renewals.tick() => {
                let renewal = handle_lease_renewal(
                    &mut lease_deadline,
                    WORKER_JOB_LEASE_SECONDS,
                    state
                        .execution_queue
                        .renew_started(
                            job_id,
                            worker_id,
                            claim_generation,
                            WORKER_JOB_LEASE_SECONDS,
                        )
                        .await,
                    "execution job",
                );
                if let Err(error) = renewal {
                    let current = state.execution_queue.get(job_id).await?;
                    if execution_job_cancel_requested_by_owner(
                        &current,
                        worker_id,
                        claim_generation,
                    )? {
                        return Ok(current);
                    }
                    if current.claim_generation == claim_generation
                        && matches!(
                            current.status,
                            ExecutionJobStatus::Queued
                                | ExecutionJobStatus::Completed
                                | ExecutionJobStatus::Failed
                                | ExecutionJobStatus::OutcomeUnknown
                                | ExecutionJobStatus::Canceled
                        )
                    {
                        continue;
                    }
                    if matches!(
                        current.status,
                        ExecutionJobStatus::Running
                            | ExecutionJobStatus::Executing
                            | ExecutionJobStatus::Finalizing
                    ) {
                        return Err(error);
                    }
                    return Ok(current);
                }
            }
            _ = cancellation_checks.tick() => {
                if let Some(job) = execution_job_interrupt_state(
                    state,
                    job_id,
                    worker_id,
                    claim_generation,
                ).await? {
                    return Ok(job);
                }
            }
        }
    }
}

async fn execution_job_interrupt_state(
    state: &AppState,
    job_id: Uuid,
    worker_id: &str,
    claim_generation: i64,
) -> Result<Option<crate::execution_queue::ExecutionJob>, AppError> {
    let job = state.execution_queue.get(job_id).await?;
    if execution_job_cancel_requested_by_owner(&job, worker_id, claim_generation)? {
        Ok(Some(job))
    } else if job.claim_generation != claim_generation {
        Err(AppError::not_found(
            "execution job lease is no longer owned by this worker claim",
        ))
    } else {
        Ok(None)
    }
}

fn execution_job_cancel_requested_by_owner(
    job: &crate::execution_queue::ExecutionJob,
    worker_id: &str,
    claim_generation: i64,
) -> Result<bool, AppError> {
    match job.status {
        ExecutionJobStatus::Running
        | ExecutionJobStatus::Executing
        | ExecutionJobStatus::Finalizing
            if job.worker_id.as_deref() == Some(worker_id) =>
        {
            if job.claim_generation == claim_generation {
                Ok(false)
            } else {
                Err(AppError::not_found(
                    "execution job lease is no longer owned by this worker claim",
                ))
            }
        }
        ExecutionJobStatus::CancelRequested
            if job.worker_id.as_deref() == Some(worker_id)
                && job.claim_generation == claim_generation =>
        {
            Ok(true)
        }
        ExecutionJobStatus::Running
        | ExecutionJobStatus::Executing
        | ExecutionJobStatus::Finalizing
        | ExecutionJobStatus::CancelRequested => Err(AppError::not_found(
            "execution job lease is no longer owned by this worker",
        )),
        ExecutionJobStatus::Queued
        | ExecutionJobStatus::Completed
        | ExecutionJobStatus::Failed
        | ExecutionJobStatus::OutcomeUnknown
        | ExecutionJobStatus::Canceled => Ok(false),
    }
}

pub(crate) fn worker_scope_headers_present(headers: &HeaderMap) -> Result<bool, AppError> {
    Ok(worker_environment_id_from_headers(headers)?.is_some()
        || worker_pool_from_headers(headers).is_some())
}

pub(crate) fn environment_worker_pool(worker_queue_binding: &Value) -> Option<String> {
    for key in ["queue", "worker_pool", "pool"] {
        if let Some(value) = worker_queue_binding
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

pub(crate) async fn enforce_worker_environment_binding(
    state: &AppState,
    headers: &HeaderMap,
    session_id: Uuid,
    job_environment_id: Option<Uuid>,
) -> Result<(), AppError> {
    let worker_environment_id = worker_environment_id_from_headers(headers)?;
    let worker_pool = worker_pool_from_headers(headers);
    let actual_environment_id = match job_environment_id {
        Some(environment_id) => Some(environment_id),
        None => state.get_session(session_id).await?.environment_id,
    };
    if let Some(worker_environment_id) = worker_environment_id {
        if actual_environment_id == Some(worker_environment_id) {
            return Ok(());
        }
        return Err(AppError::not_found(
            "job not claimable for worker environment",
        ));
    }
    if worker_environment_scope_required()
        && actual_environment_id.is_some()
        && worker_pool.is_none()
    {
        return Err(AppError::bad_request(
            "worker must provide x-mandoforge-environment-id or x-mandoforge-worker-pool for environment-bound jobs",
        ));
    }
    Ok(())
}

pub(crate) async fn enforce_worker_pool_binding(
    state: &AppState,
    headers: &HeaderMap,
    session_id: Uuid,
    job_environment_id: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(worker_pool) = worker_pool_from_headers(headers) else {
        return Ok(());
    };
    let actual_environment_id = match job_environment_id {
        Some(environment_id) => Some(environment_id),
        None => state.get_session(session_id).await?.environment_id,
    };
    let Some(environment_id) = actual_environment_id else {
        return Err(AppError::not_found("job not claimable for worker pool"));
    };
    let environment = state.get_environment(environment_id).await?;
    if environment_worker_pool(&environment.worker_queue_binding).as_deref()
        == Some(worker_pool.as_str())
    {
        return Ok(());
    }
    Err(AppError::not_found("job not claimable for worker pool"))
}

pub(crate) async fn authorize_execution_job_run(
    state: &AppState,
    headers: &HeaderMap,
    job_id: Uuid,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let insecure_dev_override = ensure_worker_execution_principal(&principal, headers)?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: if insecure_dev_override {
            Permission::SessionsRun
        } else {
            Permission::ExecutionJobsRun
        },
        resource_type: "execution_job".to_string(),
        resource_id: Some(job_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let job = state.execution_queue.get(job_id).await?;
    let session_request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsRead,
        resource_type: "session".to_string(),
        resource_id: Some(job.session_id),
    };
    enforce_resource_scope(state, &principal, &session_request).await
}

pub(crate) async fn authorize_session_loop_job_run(
    state: &AppState,
    headers: &HeaderMap,
    job_id: Uuid,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    ensure_worker_execution_principal(&principal, headers)?;
    let job = state.get_session_loop_job(job_id).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsRun,
        resource_type: "session".to_string(),
        resource_id: Some(job.session_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(state, &principal, &request).await
}

fn ensure_worker_execution_principal(
    principal: &Principal,
    headers: &HeaderMap,
) -> Result<bool, AppError> {
    let insecure_dev_override = insecure_dev_auth_enabled()
        && (principal.roles.contains(&Role::Admin) || principal.subject_id == "demo-operator");
    if !principal.roles.contains(&Role::Worker) && !insecure_dev_override {
        return Err(AppError::forbidden(
            "job execution endpoints are not allowed without a worker principal",
        ));
    }
    let Some(worker_id) = header_value(headers, "x-mandoforge-worker-id")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        if insecure_dev_override {
            return Ok(true);
        }
        return Err(AppError::bad_request(
            "x-mandoforge-worker-id header is required for job execution",
        ));
    };
    if worker_id == "api" || worker_id == "session-loop-worker" {
        return Err(AppError::bad_request(
            "x-mandoforge-worker-id must identify a concrete worker",
        ));
    }
    Ok(insecure_dev_override)
}

pub(crate) async fn execute_postgres_sql_query(
    pool: &PgPool,
    sql: &str,
    max_rows: i64,
) -> Result<Value, AppError> {
    let query = wrap_read_only_sql_for_json(sql, max_rows);
    let rows: Value = sqlx::query_scalar(&query).fetch_one(pool).await?;
    let row_count = rows.as_array().map_or(0, Vec::len);
    Ok(json!({"rows": rows, "row_count": row_count}))
}

pub(crate) fn wrap_read_only_sql_for_json(sql: &str, max_rows: i64) -> String {
    let bounded_max_rows = max_rows.clamp(1, 5_000);
    let inner = sql.trim().trim_end_matches(';').trim();
    format!(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb) \
         FROM (SELECT * FROM ({inner}) AS query_result LIMIT {bounded_max_rows}) AS t"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution_job(status: ExecutionJobStatus) -> crate::execution_queue::ExecutionJob {
        crate::execution_queue::ExecutionJob {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            environment_id: None,
            approval_id: Uuid::new_v4(),
            tool_call_id: Uuid::new_v4(),
            tool_name: "shell.exec".to_string(),
            status,
            enqueued_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: Some("worker-a".to_string()),
            lease_expires_at: None,
            claim_generation: 1,
            finalization_details: json!({}),
            attempt_count: 1,
            max_attempts: 3,
            last_error: None,
        }
    }

    #[test]
    fn transient_renewal_failure_is_tolerated_only_before_safety_window() {
        let mut deadline = Instant::now() + Duration::from_secs(300);
        assert!(
            handle_lease_renewal::<()>(
                &mut deadline,
                300,
                Err(AppError {
                    status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    message: "temporary store failure".to_string(),
                    execution_outcome_known: false,
                }),
                "test lease",
            )
            .is_ok()
        );

        deadline = Instant::now() + Duration::from_secs(30);
        assert!(
            handle_lease_renewal::<()>(
                &mut deadline,
                300,
                Err(AppError {
                    status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    message: "persistent store failure".to_string(),
                    execution_outcome_known: false,
                }),
                "test lease",
            )
            .is_err()
        );

        deadline = Instant::now() + Duration::from_secs(300);
        assert!(
            handle_lease_renewal::<()>(
                &mut deadline,
                300,
                Err(AppError::not_found("lease ownership lost")),
                "test lease",
            )
            .is_err()
        );
    }

    #[test]
    fn only_owned_cancel_requests_interrupt_execution_work() {
        for status in [
            ExecutionJobStatus::Queued,
            ExecutionJobStatus::Running,
            ExecutionJobStatus::Executing,
            ExecutionJobStatus::Finalizing,
            ExecutionJobStatus::Completed,
            ExecutionJobStatus::Failed,
            ExecutionJobStatus::OutcomeUnknown,
            ExecutionJobStatus::Canceled,
        ] {
            assert!(
                !execution_job_cancel_requested_by_owner(&execution_job(status), "worker-a", 1)
                    .expect("natural status should not interrupt work")
            );
        }
        assert!(
            execution_job_cancel_requested_by_owner(
                &execution_job(ExecutionJobStatus::CancelRequested),
                "worker-a",
                1,
            )
            .expect("owned cancellation should interrupt work")
        );
        assert!(
            execution_job_cancel_requested_by_owner(
                &execution_job(ExecutionJobStatus::Running),
                "worker-b",
                1,
            )
            .is_err()
        );
    }
}
