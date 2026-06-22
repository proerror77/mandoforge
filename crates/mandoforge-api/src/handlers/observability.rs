use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AppError, AppState, ObservabilityCollectorClusterRolloutValidationRun,
    ObservabilityCollectorReadiness, ObservabilityRemediationPlan, ObservabilityRemediationRun,
    ObservabilitySummary, Permission, authorize_request,
    build_observability_collector_readiness, build_observability_remediation_plan,
    build_observability_summary,
    run_observability_remediation as run_observability_remediation_impl,
    validate_observability_collector_cluster_rollout as validate_observability_collector_cluster_rollout_impl,
    validate_observability_collector_deployment as validate_observability_collector_deployment_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/observability", get(get_observability_summary))
        .route(
            "/api/observability/collector-readiness",
            get(get_observability_collector_readiness),
        )
        .route(
            "/api/observability/collector/deployment/validate",
            post(validate_observability_collector_deployment),
        )
        .route(
            "/api/observability/collector/cluster/validate",
            post(validate_observability_collector_cluster_rollout),
        )
        .route(
            "/api/observability/remediation/plan",
            get(get_observability_remediation_plan),
        )
        .route(
            "/api/observability/remediation/run",
            post(run_observability_remediation),
        )
}

async fn get_observability_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilitySummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    Ok(Json(build_observability_summary(&state).await?))
}

async fn get_observability_collector_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityCollectorReadiness>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    Ok(Json(build_observability_collector_readiness(&state).await))
}

async fn validate_observability_collector_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    validate_observability_collector_deployment_impl(state, headers).await
}

async fn validate_observability_collector_cluster_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityCollectorClusterRolloutValidationRun>, AppError> {
    validate_observability_collector_cluster_rollout_impl(state, headers).await
}

async fn get_observability_remediation_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityRemediationPlan>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    let summary = build_observability_summary(&state).await?;
    Ok(Json(build_observability_remediation_plan(summary)))
}

async fn run_observability_remediation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityRemediationRun>, AppError> {
    run_observability_remediation_impl(state, headers).await
}
