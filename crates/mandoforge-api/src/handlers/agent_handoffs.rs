use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AgentHandoffAssignment, AgentHandoffEvent, AppError, AppState,
    AttachAgentHandoffRemoteComputerAssignment, CreateAgentHandoffAssignment,
    CreateAgentHandoffEvent, CreateSession, EscalateAgentHandoffEvent, Permission,
    TransitionAgentHandoffEvent, active_task_grant_for_session, authorize_collection_request,
    authorize_request, create_agent_handoff_event_for_session, create_handoff_session_thread,
    default_handoff_assignment_message, ensure_primary_session_thread,
    materialize_workflow_handoff_assignment, normalize_handoff_human_escalation_status,
    normalize_optional_text, record_agent_handoff_assignment_audit_and_events,
    record_agent_handoff_assignment_remote_computer_event, record_agent_handoff_audit_and_event,
    require_active_task_grant_for_session, session_thread_event_payload,
    task_grant_remaining_budgets, transition_agent_handoff_event,
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
    if handoff.status != "accepted" {
        return Err(AppError::bad_request(
            "agent handoff must be accepted before assignment",
        ));
    }
    let manager_plan_id = handoff.manager_plan_id.ok_or_else(|| {
        AppError::bad_request("agent handoff assignment requires manager_plan_id")
    })?;
    let manager_plan = state.get_manager_agent_plan(manager_plan_id).await?;
    if manager_plan.status != "approved" && manager_plan.status != "reviewed" {
        return Err(AppError::bad_request(
            "manager plan must be reviewed or approved before handoff assignment",
        ));
    }
    if manager_plan.session_id != handoff.source_session_id
        || manager_plan.manager_agent_id != handoff.source_agent_id
    {
        return Err(AppError::bad_request(
            "manager plan does not match handoff source session and agent",
        ));
    }
    if let Some(specialist_agent_id) = manager_plan.specialist_agent_id
        && specialist_agent_id != handoff.target_agent_id
    {
        return Err(AppError::bad_request(
            "manager plan specialist does not match handoff target agent",
        ));
    }
    let target_agent = state.get_agent(handoff.target_agent_id).await?;
    if target_agent.agent_role != "specialist" {
        return Err(AppError::bad_request(
            "agent handoff assignment target must be a specialist agent",
        ));
    }
    if state
        .get_agent_handoff_assignment_for_handoff(handoff.id)
        .await?
        .is_some()
    {
        return Err(AppError::bad_request(
            "agent handoff already has an assignment",
        ));
    }
    let release_enforced = crate::store_entities::agent_release_enforcement_required();
    if !release_enforced && let Some(profile_id) = handoff.runtime_profile_id {
        let profile = state.get_agent_runtime_profile(profile_id).await?;
        if profile.status != "enabled" {
            return Err(AppError::bad_request(
                "agent handoff runtime profile must be enabled",
            ));
        }
    }
    if let Some(remote_assignment_id) = input.remote_computer_job_assignment_id {
        let remote_assignment = state
            .list_remote_computer_job_assignments()
            .await?
            .into_iter()
            .find(|assignment| assignment.id == remote_assignment_id)
            .ok_or_else(|| AppError::not_found("remote computer job assignment not found"))?;
        if remote_assignment.session_id != handoff.source_session_id {
            return Err(AppError::bad_request(
                "remote computer job assignment must belong to the source session until the specialist execution job exists",
            ));
        }
    }
    let source_session = state.get_session(handoff.source_session_id).await?;
    state.ensure_session_runnable(source_session.id).await?;
    if release_enforced {
        let (_, parent_grant) =
            require_active_task_grant_for_session(&state, source_session.id).await?;
        task_grant_remaining_budgets(&parent_grant, Utc::now())?;
    } else if let Some((_, parent_grant)) =
        active_task_grant_for_session(&state, source_session.id).await?
    {
        task_grant_remaining_budgets(&parent_grant, Utc::now())?;
    }
    let expected_target_version = state
        .runnable_agent_version(handoff.target_agent_id, source_session.environment_id)
        .await?;
    if release_enforced && handoff.runtime_profile_id != expected_target_version.runtime_profile_id
    {
        return Err(AppError::forbidden(
            "production handoff runtime profile no longer matches the promoted target agent version",
        ));
    }
    let parent_thread = ensure_primary_session_thread(&state, source_session.id).await?;

    let specialist_session = match input.specialist_session_id {
        Some(session_id) => {
            if crate::store_entities::agent_release_enforcement_required() {
                return Err(AppError::forbidden(
                    "production handoff assignment cannot reuse an existing specialist session",
                ));
            }
            let session = state.get_session(session_id).await?;
            if session.agent_id != handoff.target_agent_id {
                return Err(AppError::bad_request(
                    "specialist_session_id must belong to the handoff target agent",
                ));
            }
            if session.environment_id != source_session.environment_id {
                return Err(AppError::bad_request(
                    "specialist_session_id must use the same environment as the source session",
                ));
            }
            state.ensure_session_runnable(session.id).await?;
            session
        }
        None => {
            let message = input.message.or_else(|| {
                Some(default_handoff_assignment_message(
                    &handoff,
                    &manager_plan,
                    &target_agent,
                ))
            });
            state
                .create_session(CreateSession {
                    agent_id: handoff.target_agent_id,
                    environment_id: source_session.environment_id,
                    title: input.title.unwrap_or_else(|| {
                        format!("Handoff {} for {}", handoff.intent, target_agent.name)
                    }),
                    message,
                })
                .await?
        }
    };
    let specialist_version = state
        .agent_version_for_session(specialist_session.id)
        .await?;
    if release_enforced {
        if specialist_session.agent_version_id != Some(specialist_version.id)
            || specialist_version.agent_id != handoff.target_agent_id
        {
            return Err(AppError::forbidden(
                "production handoff specialist session version binding is invalid",
            ));
        }
        if handoff.runtime_profile_id != specialist_version.runtime_profile_id {
            return Err(AppError::forbidden(
                "production handoff runtime profile no longer matches the promoted target agent version",
            ));
        }
    }
    ensure_primary_session_thread(&state, specialist_session.id).await?;
    let now = Utc::now();
    let assignment = state
        .create_agent_handoff_assignment(AgentHandoffAssignment {
            id: Uuid::new_v4(),
            agent_handoff_event_id: handoff.id,
            manager_plan_id,
            source_session_id: handoff.source_session_id,
            specialist_session_id: specialist_session.id,
            source_agent_id: handoff.source_agent_id,
            target_agent_id: handoff.target_agent_id,
            semantic_scopes: handoff.semantic_scopes.clone(),
            runtime_profile_id: if release_enforced {
                None
            } else {
                handoff.runtime_profile_id
            },
            remote_computer_required: handoff.remote_computer_required,
            remote_computer_job_assignment_id: input.remote_computer_job_assignment_id,
            status: if handoff.remote_computer_required
                && input.remote_computer_job_assignment_id.is_none()
            {
                "waiting_remote_computer".to_string()
            } else {
                "assigned".to_string()
            },
            assigned_by: input.assigned_by.and_then(normalize_optional_text),
            metadata: input.metadata,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let audit =
        record_agent_handoff_assignment_audit_and_events(&state, &assignment, &handoff).await?;
    let assignment = state
        .update_agent_handoff_assignment_audit_trace(assignment.id, audit.id)
        .await?;
    let child_thread = create_handoff_session_thread(
        &state,
        &assignment,
        &handoff,
        &specialist_session,
        Some(parent_thread.id),
    )
    .await?;
    materialize_workflow_handoff_assignment(
        &state,
        &handoff,
        &assignment,
        &manager_plan,
        &target_agent,
        &specialist_session,
        &child_thread,
    )
    .await?;
    state
        .append_event(
            "system",
            Some(child_thread.id),
            handoff.source_session_id,
            "thread.started",
            session_thread_event_payload(&child_thread),
        )
        .await?;
    state
        .append_event(
            "system",
            Some(child_thread.id),
            specialist_session.id,
            "thread.started",
            session_thread_event_payload(&child_thread),
        )
        .await?;
    Ok(Json(assignment))
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
