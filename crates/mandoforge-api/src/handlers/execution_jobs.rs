use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateRemoteComputerJobAssignment, RemoteComputerJobAssignment,
    SessionLoopJob, WorkerLoadValidationRun, WorkerReadinessReport, Permission,
    assign_execution_job_remote_computer_lease as assign_execution_job_remote_computer_lease_impl,
    authorize_request, build_worker_readiness,
    cancel_execution_job_route as cancel_execution_job_route_impl,
    list_execution_jobs as list_execution_jobs_impl,
    list_session_loop_jobs as list_session_loop_jobs_impl,
    queue_notify_wait as queue_notify_wait_impl,
    run_execution_job_route as run_execution_job_route_impl,
    run_session_loop_job_route as run_session_loop_job_route_impl,
    run_worker_load_validation as run_worker_load_validation_impl,
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
    list_execution_jobs_impl(state, headers).await
}

async fn queue_notify_wait(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    queue_notify_wait_impl(state, headers, params).await
}

async fn list_session_loop_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionLoopJob>>, AppError> {
    list_session_loop_jobs_impl(state, headers).await
}

async fn run_session_loop_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SessionLoopJob>, AppError> {
    run_session_loop_job_route_impl(state, id, headers).await
}

async fn cancel_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::execution_queue::ExecutionJob>, AppError> {
    cancel_execution_job_route_impl(state, id, headers).await
}

async fn assign_execution_job_remote_computer_lease(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateRemoteComputerJobAssignment>,
) -> Result<Json<RemoteComputerJobAssignment>, AppError> {
    assign_execution_job_remote_computer_lease_impl(state, id, headers, input).await
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
    run_worker_load_validation_impl(state, headers).await
}

async fn run_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::execution_queue::ExecutionJob>, AppError> {
    run_execution_job_route_impl(state, id, headers).await
}
