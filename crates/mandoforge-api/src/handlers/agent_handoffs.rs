use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AgentHandoffAssignment, AgentHandoffEvent, AppError, AppState,
    AttachAgentHandoffRemoteComputerAssignment, CreateAgentHandoffAssignment,
    CreateAgentHandoffEvent, EscalateAgentHandoffEvent, Permission, TransitionAgentHandoffEvent,
    assign_agent_handoff_event_for_runtime, authorize_collection_request, authorize_request,
    create_agent_handoff_event_for_session, normalize_handoff_human_escalation_status,
    record_agent_handoff_assignment_remote_computer_event, record_agent_handoff_audit_and_event,
    session_thread_event_payload, transition_agent_handoff_event,
    visible_session_ids_for_principal,
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
            "/api/agent-handoffs/{id}/accept",
            post(accept_agent_handoff_event),
        )
        .route(
            "/api/agent-handoffs/{id}/reject",
            post(reject_agent_handoff_event),
        )
        .route(
            "/api/agent-handoffs/{id}/fail",
            post(fail_agent_handoff_event),
        )
        .route(
            "/api/agent-handoffs/{id}/complete",
            post(complete_agent_handoff_event),
        )
        .route(
            "/api/agent-handoffs/{id}/assignment",
            get(get_agent_handoff_assignment_for_handoff).post(assign_agent_handoff_event),
        )
        .route(
            "/api/agent-handoff-assignments/{id}/remote-computer-assignment",
            post(attach_agent_handoff_remote_computer_assignment),
        )
        .route(
            "/api/agent-handoffs/{id}/escalate",
            post(escalate_agent_handoff_event),
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

async fn accept_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransitionAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    transition_agent_handoff_event(state, id, headers, input, "accepted").await
}

async fn reject_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransitionAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    transition_agent_handoff_event(state, id, headers, input, "rejected").await
}

async fn fail_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransitionAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    transition_agent_handoff_event(state, id, headers, input, "failed").await
}

async fn complete_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransitionAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    transition_agent_handoff_event(state, id, headers, input, "completed").await
}

async fn get_agent_handoff_assignment_for_handoff(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentHandoffAssignment>, AppError> {
    let handoff = state.get_agent_handoff_event(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(handoff.source_session_id),
    )
    .await?;
    let assignment = state
        .get_agent_handoff_assignment_for_handoff(id)
        .await?
        .ok_or_else(|| AppError::not_found("agent handoff assignment not found"))?;
    Ok(Json(assignment))
}

async fn assign_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentHandoffAssignment>,
) -> Result<Json<AgentHandoffAssignment>, AppError> {
    let handoff = state.get_agent_handoff_event(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(handoff.source_session_id),
    )
    .await?;
    Ok(Json(
        assign_agent_handoff_event_for_runtime(&state, &handoff, input).await?,
    ))
}

async fn attach_agent_handoff_remote_computer_assignment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<AttachAgentHandoffRemoteComputerAssignment>,
) -> Result<Json<AgentHandoffAssignment>, AppError> {
    let assignment = state.get_agent_handoff_assignment(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(assignment.specialist_session_id),
    )
    .await?;
    if !assignment.remote_computer_required {
        return Err(AppError::bad_request(
            "agent handoff assignment does not require Remote Computer",
        ));
    }
    let remote_assignment = state
        .list_remote_computer_job_assignments()
        .await?
        .into_iter()
        .find(|candidate| candidate.id == input.remote_computer_job_assignment_id)
        .ok_or_else(|| AppError::not_found("remote computer job assignment not found"))?;
    if remote_assignment.session_id != assignment.specialist_session_id {
        return Err(AppError::bad_request(
            "remote computer job assignment must belong to the specialist session",
        ));
    }
    if remote_assignment.status != "assigned" {
        return Err(AppError::bad_request(
            "remote computer job assignment must be assigned",
        ));
    }
    let updated = state
        .attach_agent_handoff_assignment_remote_computer_job(
            assignment.id,
            remote_assignment.id,
            input.metadata,
        )
        .await?;
    record_agent_handoff_assignment_remote_computer_event(&state, &updated, &remote_assignment)
        .await?;
    if let Some(thread) = state
        .session_thread_for_handoff(updated.agent_handoff_event_id)
        .await?
    {
        let thread = state
            .update_session_thread_status(thread.id, "running")
            .await?;
        state
            .append_event(
                "system",
                Some(thread.id),
                updated.source_session_id,
                "thread.status_changed",
                session_thread_event_payload(&thread),
            )
            .await?;
        state
            .append_event(
                "system",
                Some(thread.id),
                updated.specialist_session_id,
                "thread.status_changed",
                session_thread_event_payload(&thread),
            )
            .await?;
    }
    Ok(Json(updated))
}

async fn escalate_agent_handoff_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EscalateAgentHandoffEvent>,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    let current = state.get_agent_handoff_event(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::ApprovalsDecide,
        "session",
        Some(current.source_session_id),
    )
    .await?;
    if matches!(current.status.as_str(), "completed" | "rejected") {
        return Err(AppError::bad_request(
            "completed or rejected handoffs cannot be escalated",
        ));
    }
    let next_status =
        normalize_handoff_human_escalation_status(input.status.as_deref().unwrap_or("requested"))?;
    if next_status == "none" {
        return Err(AppError::bad_request(
            "handoff escalation status must be required, requested, or resolved",
        ));
    }
    let audit = record_agent_handoff_audit_and_event(
        &state,
        &current,
        "agent_handoff.escalated",
        input.reason,
    )
    .await?;
    let updated = state
        .update_agent_handoff_event_escalation(current.id, &next_status, Some(audit.id))
        .await?;
    Ok(Json(updated))
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
