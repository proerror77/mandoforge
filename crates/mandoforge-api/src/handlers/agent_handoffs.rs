use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use uuid::Uuid;

use crate::{
    AgentHandoffAssignment, AgentHandoffEvent, AppError, AppState, CreateAgentHandoffEvent,
    Permission, authorize_collection_request, authorize_request,
    create_agent_handoff_event_for_session, visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agent-handoffs", get(list_agent_handoff_events))
        .route("/api/agent-handoffs/{id}", get(get_agent_handoff_event))
        .route(
            "/api/sessions/{id}/agent-handoffs",
            get(list_session_agent_handoff_events).post(create_agent_handoff_event),
        )
        .route(
            "/api/agent-handoff-assignments",
            get(list_agent_handoff_assignments),
        )
        .route(
            "/api/agent-handoff-assignments/{id}",
            get(get_agent_handoff_assignment),
        )
        .route(
            "/api/sessions/{id}/agent-handoff-assignments",
            get(list_session_agent_handoff_assignments),
        )
}

async fn list_agent_handoff_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentHandoffEvent>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_handoff_events",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_agent_handoff_events(None)
            .await?
            .into_iter()
            .filter(|event| visible_session_ids.contains(&event.source_session_id))
            .collect(),
    ))
}

async fn get_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    let event = state.get_agent_handoff_event(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(event.source_session_id),
    )
    .await?;
    Ok(Json(event))
}

async fn list_session_agent_handoff_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentHandoffEvent>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_agent_handoff_events(Some(id)).await?))
}

async fn create_agent_handoff_event(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    Ok(Json(
        create_agent_handoff_event_for_session(&state, session_id, input).await?,
    ))
}

async fn list_agent_handoff_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentHandoffAssignment>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_handoff_assignments",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_agent_handoff_assignments(None)
            .await?
            .into_iter()
            .filter(|assignment| visible_session_ids.contains(&assignment.source_session_id))
            .collect(),
    ))
}

async fn list_session_agent_handoff_assignments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentHandoffAssignment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_agent_handoff_assignments(Some(id)).await?))
}

async fn get_agent_handoff_assignment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentHandoffAssignment>, AppError> {
    let assignment = state.get_agent_handoff_assignment(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(assignment.source_session_id),
    )
    .await?;
    Ok(Json(assignment))
}
