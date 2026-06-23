use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

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
    let Some(worker_environment_id) = worker_environment_id_from_headers(headers)? else {
        return Ok(());
    };
    let actual_environment_id = match job_environment_id {
        Some(environment_id) => Some(environment_id),
        None => state.get_session(session_id).await?.environment_id,
    };
    if actual_environment_id == Some(worker_environment_id) {
        return Ok(());
    }
    Err(AppError::not_found(
        "job not claimable for worker environment",
    ))
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
