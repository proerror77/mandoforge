use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    Agent, AgentRuntimeProfile, AgentRuntimeProfileReleaseGate, AppError, AppState,
    AuthorizationRequest, CreateAgent, CreateAgentRuntimeProfile, Permission,
    UpdateAgentRuntimeProfile, authorize_request, evaluate_agent_runtime_profile_release_gate,
    new_audit_log, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agents", get(list_agents).post(create_agent))
        .route(
            "/api/agent-runtime-profiles",
            get(list_agent_runtime_profiles).post(create_agent_runtime_profile),
        )
        .route(
            "/api/agent-runtime-profile-release-gates",
            get(list_agent_runtime_profile_release_gates),
        )
        .route(
            "/api/agent-runtime-profiles/{id}",
            get(get_agent_runtime_profile)
                .patch(update_agent_runtime_profile)
                .delete(archive_agent_runtime_profile),
        )
        .route(
            "/api/agent-runtime-profiles/{id}/release-gate",
            get(get_agent_runtime_profile_release_gate),
        )
}

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Agent>>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::AgentsRead,
        resource_type: "agents".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(state.list_agents_visible_to(&principal).await?))
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsWrite, "agents", None).await?;
    Ok(Json(state.create_agent(input).await?))
}

async fn list_agent_runtime_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRuntimeProfile>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    Ok(Json(state.list_agent_runtime_profiles().await?))
}

async fn list_agent_runtime_profile_release_gates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRuntimeProfileReleaseGate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    let profiles = state.list_agent_runtime_profiles().await?;
    Ok(Json(
        profiles
            .iter()
            .map(evaluate_agent_runtime_profile_release_gate)
            .collect(),
    ))
}

async fn create_agent_runtime_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentRuntimeProfile>,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profiles",
        None,
    )
    .await?;
    let profile = state.create_agent_runtime_profile(input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.created",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "remote_computer_required": profile.remote_computer_required,
                "status": profile.status
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn get_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_agent_runtime_profile(id).await?))
}

async fn get_agent_runtime_profile_release_gate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfileReleaseGate>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let profile = state.get_agent_runtime_profile(id).await?;
    Ok(Json(evaluate_agent_runtime_profile_release_gate(&profile)))
}

async fn update_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateAgentRuntimeProfile>,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let before = state.get_agent_runtime_profile(id).await?;
    let profile = state.update_agent_runtime_profile(id, input).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.updated",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "before": {
                    "status": before.status,
                    "command": before.command,
                    "default_args": before.default_args,
                    "env": before.env,
                    "timeout_seconds": before.timeout_seconds,
                    "remote_computer_required": before.remote_computer_required
                },
                "after": {
                    "status": profile.status,
                    "command": profile.command,
                    "default_args": profile.default_args,
                    "env": profile.env,
                    "timeout_seconds": profile.timeout_seconds,
                    "remote_computer_required": profile.remote_computer_required
                }
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn archive_agent_runtime_profile(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentRuntimeProfile>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsWrite,
        "agent_runtime_profile",
        Some(id),
    )
    .await?;
    let profile = state.archive_agent_runtime_profile(id).await?;
    let principal = principal_from_request(&state, &headers).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_runtime_profile.archived",
            "agent_runtime_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "runtime_type": profile.runtime_type,
                "status": profile.status,
                "archived_at": profile.archived_at
            }),
        ))
        .await?;
    Ok(Json(profile))
}
