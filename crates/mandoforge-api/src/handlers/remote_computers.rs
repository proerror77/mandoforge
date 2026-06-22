use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AppError, AppState, RemoteComputerArtifactDiscoverRequest, RemoteComputerArtifactSyncRequest,
    RemoteComputerArtifactSyncResponse, RemoteComputerReadinessReport,
    RemoteComputerRunnerDryRunRequest, RemoteComputerRunnerDryRunResponse,
    RemoteComputerRunnerReadiness, RemoteComputerStateSyncValidationRun,
    discover_remote_computer_artifacts as discover_remote_computer_artifacts_impl,
    dry_run_remote_computer_runner as dry_run_remote_computer_runner_impl,
    get_remote_computer_production_path as get_remote_computer_production_path_impl,
    get_remote_computer_readiness as get_remote_computer_readiness_impl,
    get_remote_computer_runner_readiness as get_remote_computer_runner_readiness_impl,
    mutate_remote_computer_runner as mutate_remote_computer_runner_impl,
    sync_remote_computer_artifacts as sync_remote_computer_artifacts_impl,
    validate_remote_computer_state_sync as validate_remote_computer_state_sync_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/remote-computers/readiness",
            get(get_remote_computer_readiness),
        )
        .route(
            "/api/remote-computers/production-path",
            get(get_remote_computer_production_path),
        )
        .route(
            "/api/remote-computers/state-sync/validate",
            post(validate_remote_computer_state_sync),
        )
        .route(
            "/api/remote-computers/artifacts/sync",
            post(sync_remote_computer_artifacts),
        )
        .route(
            "/api/remote-computers/artifacts/discover",
            post(discover_remote_computer_artifacts),
        )
        .route(
            "/api/remote-computers/runner/readiness",
            get(get_remote_computer_runner_readiness),
        )
        .route(
            "/api/remote-computers/runner/dry-run",
            post(dry_run_remote_computer_runner),
        )
        .route(
            "/api/remote-computers/runner/mutate",
            post(mutate_remote_computer_runner),
        )
}

async fn get_remote_computer_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerReadinessReport>, AppError> {
    get_remote_computer_readiness_impl(state, headers).await
}

async fn get_remote_computer_production_path(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    get_remote_computer_production_path_impl(state, headers).await
}

async fn validate_remote_computer_state_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerStateSyncValidationRun>, AppError> {
    validate_remote_computer_state_sync_impl(state, headers).await
}

async fn sync_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactSyncRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    sync_remote_computer_artifacts_impl(state, headers, input).await
}

async fn discover_remote_computer_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerArtifactDiscoverRequest>,
) -> Result<Json<RemoteComputerArtifactSyncResponse>, AppError> {
    discover_remote_computer_artifacts_impl(state, headers, input).await
}

async fn get_remote_computer_runner_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RemoteComputerRunnerReadiness>, AppError> {
    get_remote_computer_runner_readiness_impl(state, headers).await
}

async fn dry_run_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    dry_run_remote_computer_runner_impl(state, headers, input).await
}

async fn mutate_remote_computer_runner(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RemoteComputerRunnerDryRunRequest>,
) -> Result<Json<RemoteComputerRunnerDryRunResponse>, AppError> {
    mutate_remote_computer_runner_impl(state, headers, input).await
}
