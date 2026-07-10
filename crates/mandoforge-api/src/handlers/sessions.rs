use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use futures_util::StreamExt as _;
use uuid::Uuid;

use crate::{
    AddMessage, AppError, AppState, Artifact, AuditLog, AuthorizationRequest, ContextPacket,
    CreateSession, Permission, RenderContextPacketRequest, RenderedExecutionContext,
    SendSessionEvents, Session, SessionEvent, SessionThread, StreamEventsQuery, ToolCall,
    append_incoming_session_event, append_user_message_event, authorize_collection_request,
    authorize_request, authorize_session_run, enqueue_session_loop, ensure_primary_session_thread,
    generate_and_persist_context_packet, principal_from_request, project_session_event_to_loop,
    render_execution_context_for_packet, store_events, visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/run", post(run_session))
        .route("/api/sessions/{id}/messages", post(add_message))
        .route(
            "/api/sessions/{id}/events",
            get(list_events).post(send_session_events),
        )
        .route("/api/sessions/{id}/stream", get(stream_events))
        .route("/api/sessions/{id}/artifacts", get(list_artifacts))
        .route(
            "/api/sessions/{id}/tool-calls",
            get(list_session_tool_calls),
        )
        .route(
            "/api/sessions/{id}/audit-logs",
            get(list_session_audit_logs),
        )
        .route("/api/sessions/{id}/threads", get(list_session_threads))
        .route(
            "/api/sessions/{id}/context-packet",
            get(get_session_context_packet).post(create_session_context_packet),
        )
        .route(
            "/api/sessions/{id}/context-packets",
            get(list_session_context_packets),
        )
        .route("/api/session-threads", get(list_session_threads_collection))
        .route("/api/session-threads/{id}", get(get_session_thread))
        .route("/api/context-packets/{id}", get(get_context_packet))
        .route(
            "/api/context-packets/{id}/render",
            post(render_context_packet),
        )
}

async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<StreamEventsQuery>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>>, AppError>
{
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    let after_seq = stream_after_seq(&headers, query.after_seq)?;
    let live_events = store_events::subscribe_session_events();
    let events = state
        .list_events(id)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|event| after_seq.is_none_or(|after_seq| event.seq > after_seq))
        .collect::<Vec<_>>();
    let replay_high_water = events
        .iter()
        .map(|event| event.seq)
        .max()
        .or(after_seq)
        .unwrap_or_default();
    let replay_stream = futures_util::stream::iter(events.into_iter().map(sse_session_event));
    let live_stream =
        futures_util::stream::unfold((live_events, replay_high_water), move |state| async move {
            let (mut live_events, mut high_water) = state;
            loop {
                match live_events.recv().await {
                    Ok(event)
                        if event.session_id == id
                            && event.seq > high_water
                            && after_seq.is_none_or(|after_seq| event.seq > after_seq) =>
                    {
                        high_water = event.seq;
                        return Some((sse_session_event(event), (live_events, high_water)));
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        });
    let stream = replay_stream.chain(live_stream);
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

fn sse_session_event(event: SessionEvent) -> Result<Event, std::convert::Infallible> {
    Ok(Event::default()
        .event(event.event_type.clone())
        .id(event.seq.to_string())
        .json_data(event)
        .unwrap_or_else(|_| Event::default().event("error").data("serialization failed")))
}

fn stream_after_seq(
    headers: &HeaderMap,
    query_after_seq: Option<i64>,
) -> Result<Option<i64>, AppError> {
    let after_seq = match query_after_seq {
        Some(after_seq) => Some(after_seq),
        None => {
            let Some(raw) = headers
                .get("last-event-id")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return Ok(None);
            };
            Some(raw.parse::<i64>().map_err(|_| {
                AppError::bad_request("Last-Event-ID must be a session event sequence")
            })?)
        }
    };
    if after_seq.is_some_and(|seq| seq < 0) {
        return Err(AppError::bad_request(
            "stream cursor sequence must be non-negative",
        ));
    }
    Ok(after_seq)
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Artifact>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_artifacts(id).await?))
}

async fn list_session_tool_calls(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolCall>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_tool_calls(Some(id)).await?))
}

async fn list_session_audit_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    authorize_request(&state, &headers, Permission::AuditRead, "session", Some(id)).await?;
    Ok(Json(state.list_audit_logs(Some(id)).await?))
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
    if crate::store_entities::agent_release_enforcement_required() {
        return Err(AppError::forbidden(
            "production sessions require a WorkflowRun-issued TaskGrant",
        ));
    }
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

async fn run_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Session>, AppError> {
    authorize_session_run(&state, &headers, id).await?;
    let event = append_user_message_event(
        &state,
        id,
        "Compatibility run request from POST /api/sessions/:id/run".to_string(),
    )
    .await?;
    enqueue_session_loop(&state, id, Some(event.id), "compat.run").await?;
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

async fn list_session_threads(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionThread>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    state.get_session(id).await?;
    Ok(Json(state.list_session_threads(Some(id)).await?))
}

async fn list_session_threads_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionThread>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session_threads",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_session_threads(None)
            .await?
            .into_iter()
            .filter(|thread| visible_session_ids.contains(&thread.session_id))
            .collect(),
    ))
}

async fn get_session_thread(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SessionThread>, AppError> {
    let thread = state.get_session_thread(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(thread.session_id),
    )
    .await?;
    Ok(Json(thread))
}

async fn get_session_context_packet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ContextPacket>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    state.get_session(id).await?;
    let packet = state
        .list_context_packets(id)
        .await?
        .into_iter()
        .max_by_key(|packet| packet.version)
        .ok_or_else(|| AppError::not_found("context packet not found"))?;
    Ok(Json(packet))
}

async fn create_session_context_packet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ContextPacket>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    let packet = generate_and_persist_context_packet(&state, id).await?;
    Ok(Json(packet))
}

async fn list_session_context_packets(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ContextPacket>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    state.get_session(id).await?;
    Ok(Json(state.list_context_packets(id).await?))
}

async fn get_context_packet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ContextPacket>, AppError> {
    let packet = state.get_context_packet(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "context_packet",
        Some(packet.id),
    )
    .await?;
    Ok(Json(packet))
}

async fn render_context_packet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RenderContextPacketRequest>,
) -> Result<Json<RenderedExecutionContext>, AppError> {
    let packet = state.get_context_packet(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "context_packet",
        Some(packet.id),
    )
    .await?;
    Ok(Json(
        render_execution_context_for_packet(&state, &packet, input).await?,
    ))
}
