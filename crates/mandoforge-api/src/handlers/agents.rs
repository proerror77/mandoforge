use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::get,
};

use crate::{
    Agent, AppError, AppState, AuthorizationRequest, CreateAgent, Permission, authorize_request,
    principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/agents", get(list_agents).post(create_agent))
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
