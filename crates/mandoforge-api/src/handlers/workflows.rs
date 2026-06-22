use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateWorkflowDefinition, CreateWorkflowRun, UpdateWorkflowDefinition,
    WorkflowDefinition, WorkflowRun,
    create_workflow_definition_route as create_workflow_definition_impl,
    create_workflow_run_route as create_workflow_run_impl,
    get_workflow_definition_route as get_workflow_definition_impl,
    get_workflow_run_route as get_workflow_run_impl,
    list_workflow_definitions_route as list_workflow_definitions_impl,
    list_workflow_runs_route as list_workflow_runs_impl,
    update_workflow_definition_route as update_workflow_definition_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workflow-definitions",
            get(list_workflow_definitions).post(create_workflow_definition),
        )
        .route(
            "/api/workflow-definitions/{id}",
            get(get_workflow_definition).patch(update_workflow_definition),
        )
        .route(
            "/api/workflow-runs",
            get(list_workflow_runs).post(create_workflow_run),
        )
        .route("/api/workflow-runs/{id}", get(get_workflow_run))
}

async fn list_workflow_definitions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowDefinition>>, AppError> {
    list_workflow_definitions_impl(state, headers).await
}

async fn get_workflow_definition(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowDefinition>, AppError> {
    get_workflow_definition_impl(state, id, headers).await
}

async fn create_workflow_definition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowDefinition>,
) -> Result<Json<WorkflowDefinition>, AppError> {
    create_workflow_definition_impl(state, headers, input).await
}

async fn update_workflow_definition(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateWorkflowDefinition>,
) -> Result<Json<WorkflowDefinition>, AppError> {
    update_workflow_definition_impl(state, id, headers, input).await
}

async fn list_workflow_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowRun>>, AppError> {
    list_workflow_runs_impl(state, headers).await
}

async fn get_workflow_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRun>, AppError> {
    get_workflow_run_impl(state, id, headers).await
}

async fn create_workflow_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowRun>,
) -> Result<Json<WorkflowRun>, AppError> {
    create_workflow_run_impl(state, headers, input).await
}
