use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateMcpServerRecord, McpServerDeploymentValidationRun, McpServerHealth,
    McpServerHealthRun, McpServerRecord, McpServerRolloutDueRun, McpServerRolloutResponse,
    McpServerRolloutRunSummary, McpServerRolloutSummary, McpServerScheduledHealthRun,
    RequestMcpServerRollout, UpdateMcpServerRecord, UpdateMcpServerStatus,
    apply_mcp_server_rollout as apply_mcp_server_rollout_impl,
    create_mcp_server as create_mcp_server_impl,
    discover_mcp_server_tools as discover_mcp_server_tools_impl,
    get_mcp_server_health as get_mcp_server_health_impl,
    get_mcp_server_rollout_runs as get_mcp_server_rollout_runs_impl,
    get_mcp_server_rollout_summary as get_mcp_server_rollout_summary_impl,
    list_mcp_servers as list_mcp_servers_impl,
    run_due_mcp_server_health_checks as run_due_mcp_server_health_checks_impl,
    run_due_mcp_server_rollouts as run_due_mcp_server_rollouts_impl,
    run_mcp_server_health_checks as run_mcp_server_health_checks_impl,
    request_mcp_server_rollout as request_mcp_server_rollout_impl,
    rollback_mcp_server_rollout as rollback_mcp_server_rollout_impl,
    update_mcp_server as update_mcp_server_impl,
    update_mcp_server_status as update_mcp_server_status_impl,
    validate_mcp_server_deployment as validate_mcp_server_deployment_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/teams/{id}/mcp-servers",
            get(list_mcp_servers).post(create_mcp_server),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}",
            patch(update_mcp_server),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/status",
            patch(update_mcp_server_status),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/health",
            get(get_mcp_server_health),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/health/run",
            post(run_mcp_server_health_checks),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/health/run-due",
            post(run_due_mcp_server_health_checks),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/deployment/validate",
            post(validate_mcp_server_deployment),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/run-due",
            post(run_due_mcp_server_rollouts),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/summary",
            get(get_mcp_server_rollout_summary),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/rollouts/runs",
            get(get_mcp_server_rollout_runs),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts",
            post(request_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts/{rollout_id}/apply",
            post(apply_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/rollouts/{rollout_id}/rollback",
            post(rollback_mcp_server_rollout),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/discover",
            post(discover_mcp_server_tools),
        )
}

async fn list_mcp_servers(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<McpServerRecord>>, AppError> {
    list_mcp_servers_impl(state, id, headers).await
}

async fn create_mcp_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateMcpServerRecord>,
) -> Result<Json<McpServerRecord>, AppError> {
    create_mcp_server_impl(state, id, headers, input).await
}

async fn update_mcp_server(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<UpdateMcpServerRecord>,
) -> Result<Json<McpServerRecord>, AppError> {
    update_mcp_server_impl(state, team_id, server_id, headers, input).await
}

async fn update_mcp_server_status(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<UpdateMcpServerStatus>,
) -> Result<Json<McpServerRecord>, AppError> {
    update_mcp_server_status_impl(state, team_id, server_id, headers, input).await
}

async fn get_mcp_server_health(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerHealth>, AppError> {
    get_mcp_server_health_impl(state, team_id, server_id, headers).await
}

async fn run_mcp_server_health_checks(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerHealthRun>, AppError> {
    run_mcp_server_health_checks_impl(state, team_id, headers).await
}

async fn run_due_mcp_server_health_checks(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerScheduledHealthRun>, AppError> {
    run_due_mcp_server_health_checks_impl(state, team_id, headers).await
}

async fn validate_mcp_server_deployment(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerDeploymentValidationRun>, AppError> {
    validate_mcp_server_deployment_impl(state, team_id, headers).await
}

async fn run_due_mcp_server_rollouts(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutDueRun>, AppError> {
    run_due_mcp_server_rollouts_impl(state, team_id, headers).await
}

async fn get_mcp_server_rollout_summary(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutSummary>, AppError> {
    get_mcp_server_rollout_summary_impl(state, team_id, headers).await
}

async fn get_mcp_server_rollout_runs(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutRunSummary>, AppError> {
    get_mcp_server_rollout_runs_impl(state, team_id, headers).await
}

async fn request_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RequestMcpServerRollout>,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    request_mcp_server_rollout_impl(state, team_id, server_id, headers, input).await
}

async fn apply_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id, rollout_id)): Path<(Uuid, Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    apply_mcp_server_rollout_impl(state, team_id, server_id, rollout_id, headers).await
}

async fn rollback_mcp_server_rollout(
    State(state): State<AppState>,
    Path((team_id, server_id, rollout_id)): Path<(Uuid, Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRolloutResponse>, AppError> {
    rollback_mcp_server_rollout_impl(state, team_id, server_id, rollout_id, headers).await
}

async fn discover_mcp_server_tools(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRecord>, AppError> {
    discover_mcp_server_tools_impl(state, team_id, server_id, headers).await
}
