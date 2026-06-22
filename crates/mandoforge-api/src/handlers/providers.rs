use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateProviderAccess, CreateProviderRecord, DecideProviderStatusApproval,
    ProviderAccess, ProviderDeploymentValidationRun, ProviderGovernanceSummary, ProviderHealth,
    ProviderPolicyGateReport, ProviderPolicyGateRunResponse, ProviderPolicyGateRunSummary,
    ProviderProductionRollbackRun, ProviderProductionRolloutRun, ProviderRecord,
    ProviderStatusApprovalResponse, RequestProviderStatusApproval, RotateProviderApiKeyRef,
    RunProviderProductionRollback, RunProviderProductionRollout, UpdateProviderAccess,
    UpdateProviderStatus,
    approve_provider_status_approval as approve_provider_status_approval_impl,
    archive_provider_access as archive_provider_access_impl,
    create_provider as create_provider_impl,
    create_provider_access as create_provider_access_impl,
    get_provider_policy_gate as get_provider_policy_gate_impl,
    get_provider_policy_gate_runs as get_provider_policy_gate_runs_impl,
    get_provider_health as get_provider_health_impl,
    get_provider_summary as get_provider_summary_impl,
    list_providers as list_providers_impl,
    list_provider_access as list_provider_access_impl,
    reject_provider_status_approval as reject_provider_status_approval_impl,
    request_provider_status_approval as request_provider_status_approval_impl,
    rotate_provider_api_key_ref as rotate_provider_api_key_ref_impl,
    run_provider_policy_gate as run_provider_policy_gate_impl,
    run_provider_production_rollback as run_provider_production_rollback_impl,
    run_provider_production_rollout as run_provider_production_rollout_impl,
    update_provider as update_provider_impl,
    update_provider_access as update_provider_access_impl,
    update_provider_status as update_provider_status_impl,
    validate_provider_deployment as validate_provider_deployment_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list_providers).post(create_provider))
        .route("/api/providers/{id}", patch(update_provider))
        .route("/api/providers/summary", get(get_provider_summary))
        .route(
            "/api/providers/deployment/validate",
            post(validate_provider_deployment),
        )
        .route("/api/providers/policy-gate", get(get_provider_policy_gate))
        .route(
            "/api/providers/policy-gate/run",
            post(run_provider_policy_gate),
        )
        .route(
            "/api/providers/policy-gate/runs",
            get(get_provider_policy_gate_runs),
        )
        .route(
            "/api/providers/production-rollout/run",
            post(run_provider_production_rollout),
        )
        .route(
            "/api/providers/production-rollout/rollback",
            post(run_provider_production_rollback),
        )
        .route("/api/providers/{id}/status", patch(update_provider_status))
        .route(
            "/api/providers/{id}/status-approval",
            post(request_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/status-approval/approve",
            post(approve_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/status-approval/reject",
            post(reject_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/api-key-ref/rotate",
            post(rotate_provider_api_key_ref),
        )
        .route("/api/providers/{id}/health", get(get_provider_health))
        .route(
            "/api/teams/{id}/provider-access",
            get(list_provider_access).post(create_provider_access),
        )
        .route("/api/provider-access/{id}", patch(update_provider_access))
        .route(
            "/api/provider-access/{id}/archive",
            post(archive_provider_access),
        )
}

async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderRecord>>, AppError> {
    list_providers_impl(state, headers).await
}

async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    create_provider_impl(state, headers, input).await
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    update_provider_impl(state, id, headers, input).await
}

async fn get_provider_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderGovernanceSummary>, AppError> {
    get_provider_summary_impl(state, headers).await
}

async fn validate_provider_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderDeploymentValidationRun>, AppError> {
    validate_provider_deployment_impl(state, headers).await
}

async fn get_provider_policy_gate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateReport>, AppError> {
    get_provider_policy_gate_impl(state, headers).await
}

async fn run_provider_policy_gate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateRunResponse>, AppError> {
    run_provider_policy_gate_impl(state, headers).await
}

async fn get_provider_policy_gate_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateRunSummary>, AppError> {
    get_provider_policy_gate_runs_impl(state, headers).await
}

async fn run_provider_production_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunProviderProductionRollout>,
) -> Result<Json<ProviderProductionRolloutRun>, AppError> {
    run_provider_production_rollout_impl(state, headers, input).await
}

async fn run_provider_production_rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunProviderProductionRollback>,
) -> Result<Json<ProviderProductionRollbackRun>, AppError> {
    run_provider_production_rollback_impl(state, headers, input).await
}

async fn update_provider_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderStatus>,
) -> Result<Json<ProviderRecord>, AppError> {
    update_provider_status_impl(state, id, headers, input).await
}

async fn request_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RequestProviderStatusApproval>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    request_provider_status_approval_impl(state, id, headers, input).await
}

async fn approve_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DecideProviderStatusApproval>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    approve_provider_status_approval_impl(state, id, headers, input).await
}

async fn reject_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DecideProviderStatusApproval>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    reject_provider_status_approval_impl(state, id, headers, input).await
}

async fn rotate_provider_api_key_ref(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RotateProviderApiKeyRef>,
) -> Result<Json<ProviderRecord>, AppError> {
    rotate_provider_api_key_ref_impl(state, id, headers, input).await
}

async fn get_provider_health(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderHealth>, AppError> {
    get_provider_health_impl(state, id, headers).await
}

async fn list_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderAccess>>, AppError> {
    list_provider_access_impl(state, id, headers).await
}

async fn create_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    create_provider_access_impl(state, id, headers, input).await
}

async fn update_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    update_provider_access_impl(state, id, headers, input).await
}

async fn archive_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderAccess>, AppError> {
    archive_provider_access_impl(state, id, headers).await
}
