use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};

use crate::{
    AppError, AppState, SchedulerDeploymentValidationRun, SchedulerDuePlan, SchedulerDueRun,
    SchedulerOrchestrationSummary, SchedulerRunDueRequest,
    get_scheduler_due_plan as get_scheduler_due_plan_impl,
    get_scheduler_summary as get_scheduler_summary_impl,
    run_scheduler_due_tasks as run_scheduler_due_tasks_impl,
    validate_scheduler_deployment as validate_scheduler_deployment_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/scheduler/summary", get(get_scheduler_summary))
        .route("/api/scheduler/due-plan", get(get_scheduler_due_plan))
        .route(
            "/api/scheduler/deployment/validate",
            post(validate_scheduler_deployment),
        )
        .route("/api/scheduler/run-due", post(run_scheduler_due_tasks))
}

async fn get_scheduler_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerOrchestrationSummary>, AppError> {
    get_scheduler_summary_impl(state, headers).await
}

async fn get_scheduler_due_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerDuePlan>, AppError> {
    get_scheduler_due_plan_impl(state, headers).await
}

async fn validate_scheduler_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerDeploymentValidationRun>, AppError> {
    validate_scheduler_deployment_impl(state, headers).await
}

async fn run_scheduler_due_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    input: Option<Json<SchedulerRunDueRequest>>,
) -> Result<Json<SchedulerDueRun>, AppError> {
    run_scheduler_due_tasks_impl(state, headers, input.map(|Json(input)| input)).await
}
