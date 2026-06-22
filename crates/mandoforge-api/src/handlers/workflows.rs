use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use uuid::Uuid;

use crate::{
    AgentInboxSnapshot, AppError, AppState, ClaimWorkflowStepRun, ClaimWorkflowStepRunResponse,
    CreateTaskGrant, CreateWorkflowDefinition, CreateWorkflowRun, CreateWorkflowStepRun,
    RunDueWorkflowSteps, RunWorkflowStepRun, RunWorkflowStepRunResponse, TaskBoardSnapshot,
    TaskGrant,
    UpdateWorkflowDefinition,
    UpdateWorkflowStepRun, WorkflowDefinition, WorkflowRun, WorkflowRunGraphConsole,
    WorkflowScheduledStepActivationRun, WorkflowStepRun, WorkflowTransition,
    WorkflowTransitionQuery,
    claim_workflow_step_run_route as claim_workflow_step_run_impl,
    create_workflow_task_grant_route as create_workflow_task_grant_impl,
    create_workflow_definition_route as create_workflow_definition_impl,
    create_workflow_run_route as create_workflow_run_impl,
    create_workflow_step_run_route as create_workflow_step_run_impl,
    get_workflow_run_graph_console_route as get_workflow_run_graph_console_impl,
    get_agent_inbox_route as get_agent_inbox_impl,
    get_task_board_route as get_task_board_impl,
    get_workflow_definition_route as get_workflow_definition_impl,
    get_workflow_run_route as get_workflow_run_impl,
    get_task_grant_route as get_task_grant_impl,
    list_workflow_definitions_route as list_workflow_definitions_impl,
    list_workflow_runs_route as list_workflow_runs_impl,
    list_workflow_step_runs_route as list_workflow_step_runs_impl,
    list_workflow_task_grants_route as list_workflow_task_grants_impl,
    list_workflow_transitions_route as list_workflow_transitions_impl,
    run_due_workflow_steps_route as run_due_workflow_steps_impl,
    run_workflow_step_run_route as run_workflow_step_run_impl,
    update_workflow_definition_route as update_workflow_definition_impl,
    update_workflow_step_run_route as update_workflow_step_run_impl,
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
        .route(
            "/api/workflow-runs/{id}/transitions",
            get(list_workflow_transitions),
        )
        .route(
            "/api/workflow-runs/{id}/graph",
            get(get_workflow_run_graph_console),
        )
        .route(
            "/api/workflow-runs/{id}/scheduled-steps/run-due",
            post(run_due_workflow_steps),
        )
        .route(
            "/api/workflow-runs/{id}/steps",
            get(list_workflow_step_runs).post(create_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}",
            patch(update_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}/claim",
            post(claim_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}/run",
            post(run_workflow_step_run),
        )
        .route(
            "/api/workflow-runs/{id}/task-grants",
            get(list_workflow_task_grants).post(create_workflow_task_grant),
        )
        .route("/api/task-grants/{id}", get(get_task_grant))
        .route("/api/task-board", get(get_task_board))
        .route("/api/agents/{id}/inbox", get(get_agent_inbox))
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

async fn list_workflow_transitions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkflowTransitionQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowTransition>>, AppError> {
    list_workflow_transitions_impl(state, id, query, headers).await
}

async fn get_workflow_run_graph_console(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRunGraphConsole>, AppError> {
    get_workflow_run_graph_console_impl(state, id, headers).await
}

async fn run_due_workflow_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunDueWorkflowSteps>,
) -> Result<Json<WorkflowScheduledStepActivationRun>, AppError> {
    run_due_workflow_steps_impl(state, id, headers, input).await
}

async fn list_workflow_step_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowStepRun>>, AppError> {
    list_workflow_step_runs_impl(state, id, headers).await
}

async fn create_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowStepRun>,
) -> Result<Json<WorkflowStepRun>, AppError> {
    create_workflow_step_run_impl(state, id, headers, input).await
}

async fn update_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateWorkflowStepRun>,
) -> Result<Json<WorkflowStepRun>, AppError> {
    update_workflow_step_run_impl(state, id, headers, input).await
}

async fn claim_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ClaimWorkflowStepRun>,
) -> Result<Json<ClaimWorkflowStepRunResponse>, AppError> {
    claim_workflow_step_run_impl(state, id, headers, input).await
}

async fn run_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunWorkflowStepRun>,
) -> Result<Json<RunWorkflowStepRunResponse>, AppError> {
    run_workflow_step_run_impl(state, id, headers, input).await
}

async fn list_workflow_task_grants(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<TaskGrant>>, AppError> {
    list_workflow_task_grants_impl(state, id, headers).await
}

async fn get_task_grant(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TaskGrant>, AppError> {
    get_task_grant_impl(state, id, headers).await
}

async fn create_workflow_task_grant(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTaskGrant>,
) -> Result<Json<TaskGrant>, AppError> {
    create_workflow_task_grant_impl(state, id, headers, input).await
}

async fn get_task_board(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TaskBoardSnapshot>, AppError> {
    get_task_board_impl(state, headers).await
}

async fn get_agent_inbox(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentInboxSnapshot>, AppError> {
    get_agent_inbox_impl(state, id, headers).await
}
