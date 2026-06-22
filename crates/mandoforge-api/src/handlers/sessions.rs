use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AddMessage, AppError, AppState, AuthorizationRequest, CreateSession, Permission,
    SendSessionEvents, Session, SessionEvent, append_incoming_session_event,
    append_user_message_event, authorize_request, enqueue_session_loop,
    ensure_primary_session_thread, principal_from_request, project_session_event_to_loop,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/messages", post(add_message))
        .route(
            "/api/sessions/{id}/events",
            get(list_events).post(send_session_events),
        )
}

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Session>>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsRead,
        resource_type: "sessions".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(state.list_sessions_visible_to(&principal).await?))
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSession>,
) -> Result<Json<Session>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "sessions",
        None,
    )
    .await?;
    let has_initial_message = input
        .message
        .as_ref()
        .is_some_and(|message| !message.trim().is_empty());
    let session = state.create_session(input).await?;
    ensure_primary_session_thread(&state, session.id).await?;
    if has_initial_message {
        let trigger_event_id = state
            .list_events(session.id)
            .await?
            .into_iter()
            .rev()
            .find(|event| event.event_type == "user.message")
            .map(|event| event.id);
        enqueue_session_loop(&state, session.id, trigger_event_id, "user.message").await?;
    }
    Ok(Json(session))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Session>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_session(id).await?))
}

async fn add_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<AddMessage>,
) -> Result<Json<SessionEvent>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "session",
        Some(id),
    )
    .await?;
    let event = append_user_message_event(&state, id, input.message).await?;
    project_session_event_to_loop(&state, &event).await?;
    Ok(Json(event))
}

async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionEvent>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_events(id).await?))
}

async fn send_session_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<SendSessionEvents>,
) -> Result<Json<Vec<SessionEvent>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "session",
        Some(id),
    )
    .await?;
    let mut stored_events = Vec::new();
    for event in input.events {
        let stored = append_incoming_session_event(&state, id, event).await?;
        project_session_event_to_loop(&state, &stored).await?;
        stored_events.push(stored);
    }
    Ok(Json(stored_events))
}
