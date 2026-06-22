use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AgentRelease, AgentReleaseAutomationRunSummary, AgentReleaseRolloutSummary, AppError,
    AppState, AuthorizationRequest, CreateAgentRelease, Permission, authorize_request,
    build_agent_release_automation_run_summary, build_agent_release_rollout_summary,
    enforce_resource_scope, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/agents/releases/summary",
            get(get_agent_release_rollout_summary),
        )
        .route(
            "/api/agents/releases/automation-runs",
            get(get_agent_release_automation_runs),
        )
        .route(
            "/api/agents/{id}/releases",
            get(list_agent_releases).post(create_agent_release),
        )
}

async fn list_agent_releases(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRelease>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.list_agent_releases(id).await?))
}

async fn get_agent_release_rollout_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseRolloutSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "agent_release", None).await?;
    Ok(Json(build_agent_release_rollout_summary(
        state.list_all_agent_releases().await?,
        Utc::now(),
    )))
}

async fn get_agent_release_automation_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseAutomationRunSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "agent_release", None).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let rollout_summary =
        build_agent_release_rollout_summary(state.list_all_agent_releases().await?, Utc::now());
    Ok(Json(build_agent_release_automation_run_summary(
        &audit_logs,
        &rollout_summary,
        Utc::now(),
    )))
}

async fn create_agent_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentRelease>,
) -> Result<Json<AgentRelease>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        state
            .create_agent_release(id, input, principal.subject_id)
            .await?,
    ))
}
