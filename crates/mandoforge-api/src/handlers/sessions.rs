use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use uuid::Uuid;

use crate::{
    AppError, AppState, AuthorizationRequest, CreateSession, Permission, Session,
    enqueue_session_loop, ensure_primary_session_thread, authorize_request, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session))
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
