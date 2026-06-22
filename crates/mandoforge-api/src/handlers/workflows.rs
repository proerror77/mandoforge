use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AgentInboxSnapshot, AppError, AppState, ClaimWorkflowStepRun, ClaimWorkflowStepRunResponse,
    CreateTaskGrant, CreateWorkflowDefinition, CreateWorkflowRun, CreateWorkflowStepRun,
    RunDueWorkflowSteps, RunWorkflowStepRun, RunWorkflowStepRunResponse, TaskBoardSnapshot,
    TaskGrant, UpdateWorkflowDefinition, UpdateWorkflowStepRun, WorkflowDefinition, WorkflowRun,
    WorkflowRunGraphConsole, WorkflowScheduledStepActivationRun, WorkflowStepRun,
    WorkflowTransition, WorkflowTransitionQuery, Permission, activate_due_workflow_steps_for_run,
    authorize_collection_request, authorize_request, build_agent_inbox_snapshot,
    build_task_board_snapshot, build_workflow_run_graph_console,
    claim_workflow_step_run_route as claim_workflow_step_run_impl,
    create_workflow_task_grant_route as create_workflow_task_grant_impl,
    create_workflow_definition_route as create_workflow_definition_impl,
    create_workflow_run_route as create_workflow_run_impl,
    create_workflow_step_run_route as create_workflow_step_run_impl,
    run_workflow_step_run_route as run_workflow_step_run_impl,
    update_workflow_definition_route as update_workflow_definition_impl,
    update_workflow_step_run_route as update_workflow_step_run_impl,
    visible_session_ids_for_principal, workflow_transition_filter_from_query,
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definitions",
        None,
    )
    .await?;
    Ok(Json(state.list_workflow_definitions().await?))
}

async fn get_workflow_definition(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowDefinition>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definition",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_workflow_definition(id).await?))
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
    let principal =
        authorize_collection_request(&state, &headers, Permission::SessionsRead, "workflow_runs")
            .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_workflow_runs()
            .await?
            .into_iter()
            .filter(|run| visible_session_ids.contains(&run.primary_session_id))
            .collect(),
    ))
}

async fn get_workflow_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRun>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(run))
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
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let filter = workflow_transition_filter_from_query(query)?;
    Ok(Json(
        state
            .list_workflow_transitions_with_filter(id, &filter)
            .await?,
    ))
}

async fn get_workflow_run_graph_console(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRunGraphConsole>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(build_workflow_run_graph_console(&state, &run).await?))
}

async fn run_due_workflow_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunDueWorkflowSteps>,
) -> Result<Json<WorkflowScheduledStepActivationRun>, AppError> {
    let _ = input;
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let checked_at = Utc::now();
    Ok(Json(
        activate_due_workflow_steps_for_run(&state, &run, checked_at).await?,
    ))
}

async fn list_workflow_step_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowStepRun>>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(state.list_workflow_step_runs(id).await?))
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
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(state.list_task_grants_for_workflow_run(id).await?))
}

async fn get_task_grant(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TaskGrant>, AppError> {
    let grant = state.get_task_grant(id).await?;
    let run = state.get_workflow_run(grant.workflow_run_id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(grant))
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
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "task_board",
        None,
    )
    .await?;
    Ok(Json(build_task_board_snapshot(&state).await?))
}

async fn get_agent_inbox(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentInboxSnapshot>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_inbox",
        Some(id),
    )
    .await?;
    state.get_agent(id).await?;
    Ok(Json(build_agent_inbox_snapshot(&state, id).await?))
}
