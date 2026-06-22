use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;
use uuid::Uuid;

use crate::codex_app_server::{
    CodexCommandRequest, CodexCommandResponse, CodexInterruptResponse, CodexThreadRequest,
    CodexThreadResponse, CodexTurnRequest, CodexTurnResponse,
};
use crate::{
    AppError, AppState, CodexAppServerControlPlaneSummary, CodexAppServerOpsValidationRun,
    CodexAppServerPollRequest, CodexAppServerPollResponse, CodexAppServerRun,
    CodexAppServerStalePollRequest, CodexAppServerStalePollRun, CodexAppServerTraceDetail,
    CodexAppServerTraceSummary, CodexArtifactSyncRequest, CodexArtifactSyncResponse,
    create_codex_thread as create_codex_thread_impl,
    create_codex_turn as create_codex_turn_impl,
    execute_codex_command as execute_codex_command_impl,
    get_codex_app_server_control_plane_summary as get_codex_app_server_control_plane_summary_impl,
    get_codex_app_server_health as get_codex_app_server_health_impl,
    get_codex_app_server_trace_detail as get_codex_app_server_trace_detail_impl,
    get_codex_app_server_traces as get_codex_app_server_traces_impl,
    interrupt_codex_turn as interrupt_codex_turn_impl,
    list_codex_app_server_runs as list_codex_app_server_runs_impl,
    poll_codex_app_server_run as poll_codex_app_server_run_impl,
    poll_stale_codex_app_server_runs as poll_stale_codex_app_server_runs_impl,
    sync_codex_artifacts as sync_codex_artifacts_impl,
    validate_codex_app_server_deployment as validate_codex_app_server_deployment_impl,
    validate_codex_app_server_ops as validate_codex_app_server_ops_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/codex-app-server/health",
            get(get_codex_app_server_health),
        )
        .route(
            "/api/codex-app-server/deployment/validate",
            post(validate_codex_app_server_deployment),
        )
        .route(
            "/api/codex-app-server/ops/validate",
            post(validate_codex_app_server_ops),
        )
        .route(
            "/api/codex-app-server/runs",
            get(list_codex_app_server_runs),
        )
        .route(
            "/api/codex-app-server/traces",
            get(get_codex_app_server_traces),
        )
        .route(
            "/api/codex-app-server/control-plane/summary",
            get(get_codex_app_server_control_plane_summary),
        )
        .route(
            "/api/codex-app-server/traces/{trace_key}",
            get(get_codex_app_server_trace_detail),
        )
        .route(
            "/api/codex-app-server/runs/{run_id}/poll",
            post(poll_codex_app_server_run),
        )
        .route(
            "/api/codex-app-server/runs/poll-stale",
            post(poll_stale_codex_app_server_runs),
        )
        .route("/api/codex-app-server/threads", post(create_codex_thread))
        .route(
            "/api/codex-app-server/threads/{thread_id}/turns",
            post(create_codex_turn),
        )
        .route(
            "/api/codex-app-server/turns/{turn_id}/interrupt",
            post(interrupt_codex_turn),
        )
        .route(
            "/api/codex-app-server/turns/{turn_id}/commands",
            post(execute_codex_command),
        )
        .route(
            "/api/codex-app-server/artifacts/sync",
            post(sync_codex_artifacts),
        )
}

async fn get_codex_app_server_health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    get_codex_app_server_health_impl(state, headers).await
}

async fn validate_codex_app_server_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    validate_codex_app_server_deployment_impl(state, headers).await
}

async fn validate_codex_app_server_ops(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerOpsValidationRun>, AppError> {
    validate_codex_app_server_ops_impl(state, headers).await
}

async fn list_codex_app_server_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CodexAppServerRun>>, AppError> {
    list_codex_app_server_runs_impl(state, headers).await
}

async fn get_codex_app_server_traces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerTraceSummary>, AppError> {
    get_codex_app_server_traces_impl(state, headers).await
}

async fn get_codex_app_server_control_plane_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerControlPlaneSummary>, AppError> {
    get_codex_app_server_control_plane_summary_impl(state, headers).await
}

async fn get_codex_app_server_trace_detail(
    State(state): State<AppState>,
    Path(trace_key): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CodexAppServerTraceDetail>, AppError> {
    get_codex_app_server_trace_detail_impl(state, trace_key, headers).await
}

async fn poll_codex_app_server_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CodexAppServerPollRequest>,
) -> Result<Json<CodexAppServerPollResponse>, AppError> {
    poll_codex_app_server_run_impl(state, run_id, headers, input).await
}

async fn poll_stale_codex_app_server_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexAppServerStalePollRequest>,
) -> Result<Json<CodexAppServerStalePollRun>, AppError> {
    poll_stale_codex_app_server_runs_impl(state, headers, input).await
}

async fn create_codex_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexThreadRequest>,
) -> Result<Json<CodexThreadResponse>, AppError> {
    create_codex_thread_impl(state, headers, input).await
}

async fn create_codex_turn(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<CodexTurnRequest>,
) -> Result<Json<CodexTurnResponse>, AppError> {
    create_codex_turn_impl(state, thread_id, headers, input).await
}

async fn interrupt_codex_turn(
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CodexInterruptResponse>, AppError> {
    interrupt_codex_turn_impl(state, turn_id, headers).await
}

async fn execute_codex_command(
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<CodexCommandRequest>,
) -> Result<Json<CodexCommandResponse>, AppError> {
    execute_codex_command_impl(state, turn_id, headers, input).await
}

async fn sync_codex_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CodexArtifactSyncRequest>,
) -> Result<Json<CodexArtifactSyncResponse>, AppError> {
    sync_codex_artifacts_impl(state, headers, input).await
}
