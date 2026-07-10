use axum::http::HeaderMap;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn append_incoming_session_event(
    state: &AppState,
    session_id: Uuid,
    event: IncomingSessionEvent,
) -> Result<SessionEvent, AppError> {
    match event.event_type.as_str() {
        "user.message" => {
            let message = event
                .payload
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AppError::bad_request("user.message event requires payload.message")
                })?
                .to_string();
            append_user_message_event(state, session_id, message).await
        }
        "user.custom_tool_result" => {
            let stored = state
                .append_event("user", None, session_id, &event.event_type, event.payload)
                .await?;
            state
                .append_event(
                    "agent",
                    Some(stored.id),
                    session_id,
                    "agent.custom_tool_result",
                    json!({
                        "source_event_id": stored.id,
                        "content": stored.payload
                    }),
                )
                .await?;
            Ok(stored)
        }
        "session.goal.created"
        | "session.goal.updated"
        | "session.goal.completed"
        | "session.goal.blocked" => {
            validate_session_goal_event_payload(&event.event_type, &event.payload)?;
            state
                .append_event("user", None, session_id, &event.event_type, event.payload)
                .await
        }
        "user.interrupt" => {
            let stored = state
                .append_event("user", None, session_id, &event.event_type, event.payload)
                .await?;
            set_managed_session_status(
                state,
                session_id,
                SessionStatus::Terminated,
                "user interrupt",
            )
            .await?;
            state
                .append_event(
                    "system",
                    Some(stored.id),
                    session_id,
                    "session.interrupted",
                    json!({
                        "source_event_id": stored.id,
                        "reason": "user interrupt"
                    }),
                )
                .await?;
            Ok(stored)
        }
        other => Err(AppError::bad_request(format!(
            "unsupported session event type {other}"
        ))),
    }
}

pub(crate) fn session_loop_reason_for_event(event_type: &str) -> Option<&str> {
    if is_session_goal_event(event_type) {
        return Some(event_type);
    }
    match event_type {
        "user.message" | "user.custom_tool_result" | "tool.result" => Some(event_type),
        "approval.approved" => Some("approval approved"),
        "approval.rejected" => Some("approval rejected"),
        "execution.completed" => Some("approved execution completed"),
        _ => None,
    }
}

pub(crate) fn is_session_goal_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "session.goal.created"
            | "session.goal.updated"
            | "session.goal.completed"
            | "session.goal.blocked"
    )
}

pub(crate) fn validate_session_goal_event_payload(
    event_type: &str,
    payload: &Value,
) -> Result<(), AppError> {
    let objective = payload
        .get("objective")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let goal_id = payload
        .get("goal_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();

    if event_type == "session.goal.created" && objective.is_empty() {
        return Err(AppError::bad_request(
            "session.goal.created event requires payload.objective",
        ));
    }
    if event_type != "session.goal.created" && objective.is_empty() && goal_id.is_empty() {
        return Err(AppError::bad_request(format!(
            "{event_type} event requires payload.goal_id or payload.objective"
        )));
    }
    if event_type == "session.goal.blocked" {
        let reason = payload
            .get("reason")
            .or_else(|| payload.get("blocking_reason"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if reason.is_empty() {
            return Err(AppError::bad_request(
                "session.goal.blocked event requires payload.reason or payload.blocking_reason",
            ));
        }
    }
    Ok(())
}

pub(crate) async fn append_user_message_event(
    state: &AppState,
    session_id: Uuid,
    message: String,
) -> Result<SessionEvent, AppError> {
    state
        .append_event(
            "user",
            None,
            session_id,
            "user.message",
            json!({ "message": message }),
        )
        .await
}

pub(crate) async fn ensure_primary_session_thread(
    state: &AppState,
    session_id: Uuid,
) -> Result<SessionThread, AppError> {
    if let Some(thread) = state.primary_session_thread(session_id).await? {
        return Ok(thread);
    }
    let session = state.get_session(session_id).await?;
    let now = Utc::now();
    let thread = state
        .create_session_thread(SessionThread {
            id: Uuid::new_v4(),
            session_id,
            parent_thread_id: None,
            thread_kind: "primary".to_string(),
            agent_id: session.agent_id,
            agent_version_id: session.agent_version_id,
            environment_id: session.environment_id,
            source_handoff_id: None,
            specialist_session_id: None,
            status: managed_thread_status_for_session(&session.status).to_string(),
            title: session.title.clone(),
            context: json!({
                "origin": "session",
                "session_id": session.id,
                "agent_id": session.agent_id,
                "environment_id": session.environment_id
            }),
            created_at: now,
            updated_at: now,
        })
        .await?;
    state
        .append_event(
            "system",
            Some(thread.id),
            session_id,
            "thread.created",
            session_thread_event_payload(&thread),
        )
        .await?;
    Ok(thread)
}

pub(crate) async fn set_primary_session_thread_status(
    state: &AppState,
    session_id: Uuid,
    status: &str,
) -> Result<(), AppError> {
    let thread = ensure_primary_session_thread(state, session_id).await?;
    if thread.status != status {
        let updated = state
            .update_session_thread_status(thread.id, status)
            .await?;
        state
            .append_event(
                "system",
                Some(updated.id),
                session_id,
                "thread.status_changed",
                session_thread_event_payload(&updated),
            )
            .await?;
    }
    Ok(())
}

pub(crate) fn managed_thread_status_for_session(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Idle => "idle",
        SessionStatus::Running => "running",
        SessionStatus::RequiresAction => "requires_action",
        SessionStatus::Rescheduling => "rescheduling",
        SessionStatus::Terminated => "terminated",
        SessionStatus::Failed => "failed",
    }
}

pub(crate) fn managed_session_status_event(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Idle => "session.status_idle",
        SessionStatus::Running => "session.status_running",
        SessionStatus::RequiresAction => "session.status_requires_action",
        SessionStatus::Rescheduling => "session.status_rescheduling",
        SessionStatus::Terminated => "session.status_terminated",
        SessionStatus::Failed => "session.status_failed",
    }
}

pub(crate) async fn session_accepts_worker_execution(
    state: &AppState,
    session_id: Uuid,
) -> Result<bool, AppError> {
    let session = state.get_session(session_id).await?;
    Ok(matches!(
        session.status,
        SessionStatus::Idle
            | SessionStatus::Running
            | SessionStatus::RequiresAction
            | SessionStatus::Rescheduling
    ))
}

pub(crate) async fn set_managed_session_status(
    state: &AppState,
    session_id: Uuid,
    status: SessionStatus,
    reason: &str,
) -> Result<Session, AppError> {
    let terminal = matches!(status, SessionStatus::Terminated | SessionStatus::Failed);
    let session = state.set_session_status(session_id, status).await?;
    set_primary_session_thread_status(
        state,
        session_id,
        managed_thread_status_for_session(&session.status),
    )
    .await?;
    state
        .append_event(
            "system",
            None,
            session_id,
            managed_session_status_event(&session.status),
            json!({
                "status": session.status,
                "reason": reason,
                "environment_id": session.environment_id
            }),
        )
        .await?;
    if terminal {
        cleanup_remote_computer_session_runtimes(state, session_id, reason).await?;
    }
    Ok(session)
}

pub(crate) fn session_thread_event_payload(thread: &SessionThread) -> Value {
    json!({
        "thread_id": thread.id,
        "session_id": thread.session_id,
        "parent_thread_id": thread.parent_thread_id,
        "thread_kind": thread.thread_kind,
        "agent_id": thread.agent_id,
        "agent_version_id": thread.agent_version_id,
        "environment_id": thread.environment_id,
        "source_handoff_id": thread.source_handoff_id,
        "specialist_session_id": thread.specialist_session_id,
        "status": thread.status,
        "title": thread.title,
        "context": thread.context
    })
}

pub(crate) fn new_audit_log(
    session_id: Option<Uuid>,
    actor_type: &str,
    actor_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    details: Value,
) -> AuditLog {
    AuditLog {
        id: Uuid::new_v4(),
        session_id,
        actor_type: actor_type.to_string(),
        actor_id,
        action: action.to_string(),
        resource_type: resource_type.to_string(),
        resource_id,
        details,
        created_at: Utc::now(),
    }
}

pub(crate) fn capability_primary_action(agent: &Agent) -> &'static str {
    match agent.agent_role.as_str() {
        "manager" => "start_pack_manager_workflow",
        "specialist" => "claim_specialist_work",
        _ => "start_session",
    }
}

pub(crate) fn capability_failure_modes(agent: &Agent) -> Vec<&'static str> {
    let mut modes = vec!["missing_context", "approval_required"];
    if agent.tools.is_empty() {
        modes.push("no_tools_configured");
    }
    if agent.workflow_pack_ids.is_empty() {
        modes.push("no_workflow_pack_bound");
    }
    modes
}

pub(crate) fn capability_sample_tasks(agent: &Agent) -> Vec<String> {
    match agent.agent_role.as_str() {
        "manager" => vec![
            "拆解新的 WorkItem 并选择 specialist agent".to_string(),
            "巡检 blocked / overdue 队列并建立复审".to_string(),
        ],
        "specialist" => vec![
            "领取一个已派发的 WorkItem 并产出 evidence".to_string(),
            "根据 context packet 执行技能包内的步骤".to_string(),
        ],
        _ => vec!["启动一个受治理的 managed session".to_string()],
    }
}

pub(crate) async fn create_agent_handoff_event_for_session(
    state: &AppState,
    session_id: Uuid,
    input: CreateAgentHandoffEvent,
) -> Result<AgentHandoffEvent, AppError> {
    let session = state.get_session(session_id).await?;
    let source_version = state.agent_version_for_session(session_id).await?;
    let target_agent = state.get_agent(input.target_agent_id).await?;
    if target_agent.agent_role != "specialist" {
        return Err(AppError::bad_request(
            "agent handoff target must be a specialist agent",
        ));
    }
    let manager_plan = match input.manager_plan_id {
        Some(plan_id) => {
            let plan = state.get_manager_agent_plan(plan_id).await?;
            if plan.session_id != session_id {
                return Err(AppError::bad_request(
                    "manager_plan_id must belong to the source session",
                ));
            }
            if plan.manager_agent_id != session.agent_id {
                return Err(AppError::bad_request(
                    "manager_plan_id must belong to the source manager agent",
                ));
            }
            if let Some(planned_specialist_id) = plan.specialist_agent_id
                && planned_specialist_id != input.target_agent_id
            {
                return Err(AppError::bad_request(
                    "manager_plan_id specialist does not match target_agent_id",
                ));
            }
            Some(plan)
        }
        None => None,
    };
    let intent = validate_handoff_token("intent", &input.intent)?;
    let schema_version = validate_handoff_schema_version(&input.schema_version)?;
    let risk_level = normalize_handoff_risk_level(&input.risk_level)?;
    if risk_level == "high" && !input.approval_required {
        return Err(AppError::bad_request(
            "high-risk handoffs must require approval",
        ));
    }
    if !input.payload.is_object() {
        return Err(AppError::bad_request(
            "handoff payload must be a JSON object",
        ));
    }
    let rule = matching_handoff_rule(
        &source_version.runtime_config,
        input.target_agent_id,
        &intent,
        &schema_version,
        &risk_level,
        input.approval_required,
    )?;
    validate_handoff_payload_schema(&input.payload, rule.get("payload_schema"))?;
    let semantic_scopes = normalize_handoff_semantic_scopes(
        input
            .semantic_scopes
            .unwrap_or_else(|| target_agent.semantic_scopes.clone()),
    )?;
    let runtime_profile_id = input.runtime_profile_id.or(target_agent.runtime_profile_id);
    let runtime_profile = match runtime_profile_id {
        Some(profile_id) => Some(state.get_agent_runtime_profile(profile_id).await?),
        None => None,
    };
    let remote_computer_required = input.remote_computer_required.unwrap_or_else(|| {
        handoff_remote_computer_required(&target_agent, runtime_profile.as_ref())
    });
    let review_status = normalize_handoff_review_status(
        input
            .review_status
            .as_deref()
            .unwrap_or_else(|| default_handoff_review_status(manager_plan.as_ref())),
    )?;
    let human_escalation_status = normalize_handoff_human_escalation_status(
        input.human_escalation_status.as_deref().unwrap_or("none"),
    )?;

    let now = Utc::now();
    let event = state
        .create_agent_handoff_event(AgentHandoffEvent {
            id: Uuid::new_v4(),
            source_session_id: session_id,
            source_agent_id: session.agent_id,
            target_agent_id: input.target_agent_id,
            manager_plan_id: input.manager_plan_id,
            intent,
            payload: input.payload,
            schema_version,
            risk_level,
            approval_required: input.approval_required,
            semantic_scopes,
            runtime_profile_id,
            remote_computer_required,
            review_status,
            human_escalation_status,
            status: "requested".to_string(),
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let audit =
        record_agent_handoff_audit_and_event(state, &event, "agent_handoff.requested", None)
            .await?;
    let event = state
        .update_agent_handoff_event_status(event.id, "requested", Some(audit.id))
        .await?;
    Ok(event)
}

pub(crate) async fn materialize_workflow_handoff_assignment(
    state: &AppState,
    handoff: &AgentHandoffEvent,
    assignment: &AgentHandoffAssignment,
    manager_plan: &ManagerAgentPlan,
    target_agent: &Agent,
    specialist_session: &Session,
    child_thread: &SessionThread,
) -> Result<Option<(WorkflowStepRun, TaskGrant)>, AppError> {
    let Some((run, parent_grant)) =
        active_task_grant_for_session(state, handoff.source_session_id).await?
    else {
        return Ok(None);
    };
    let step_id = Uuid::new_v4();
    let now = Utc::now();
    let objective = manager_plan
        .task_intake
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(handoff.intent.as_str())
        .to_string();
    let child_grant = TaskGrant {
        id: Uuid::new_v4(),
        workflow_run_id: run.id,
        workflow_step_run_id: Some(step_id),
        session_id: Some(specialist_session.id),
        parent_grant_id: Some(parent_grant.id),
        source_event_id: None,
        source_handoff_id: Some(handoff.id),
        issuer_subject: assignment
            .assigned_by
            .clone()
            .unwrap_or_else(|| "system".to_string()),
        grantee_agent_id: Some(target_agent.id),
        grantee_session_id: Some(specialist_session.id),
        agent_class: Some("specialist".to_string()),
        objective,
        risk_level: handoff.risk_level.clone(),
        status: "active".to_string(),
        expires_at: parent_grant.expires_at,
        max_turns: parent_grant.max_turns,
        max_tool_calls: parent_grant.max_tool_calls,
        max_runtime_seconds: parent_grant.max_runtime_seconds,
        max_cost_usd_micros: parent_grant.max_cost_usd_micros,
        semantic_scopes: handoff.semantic_scopes.clone(),
        memory_scope: child_handoff_memory_scope(&parent_grant.memory_scope),
        tool_scope: child_tool_scope_for_agent(&parent_grant.tool_scope, target_agent),
        connector_scope: parent_grant.connector_scope.clone(),
        approval_policy: parent_grant.approval_policy.clone(),
        external_effects: parent_grant.external_effects.clone(),
        context_packet_id: None,
        policy_revision_id: parent_grant.policy_revision_id,
        immutable_args_hash: None,
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    };
    validate_task_grant_scope_objects(&child_grant)?;
    ensure_child_task_grant_within_parent(&parent_grant, &child_grant)?;

    let agent_version_id = Some(state.current_agent_version(target_agent.id).await?.id);
    let step = state
        .create_workflow_step_run(WorkflowStepRun {
            id: step_id,
            workflow_run_id: run.id,
            step_key: handoff.intent.clone(),
            step_type: "handoff".to_string(),
            agent_id: Some(target_agent.id),
            agent_version_id,
            session_id: Some(specialist_session.id),
            thread_id: Some(child_thread.id),
            handoff_id: Some(handoff.id),
            task_grant_id: None,
            environment_id: specialist_session.environment_id,
            status: if assignment.status == "waiting_remote_computer" {
                "requires_action".to_string()
            } else {
                "queued".to_string()
            },
            input_payload: json!({
                "agent_handoff_assignment_id": assignment.id,
                "agent_handoff_event_id": handoff.id,
                "manager_plan_id": manager_plan.id,
                "payload": handoff.payload.clone(),
                "semantic_scopes": handoff.semantic_scopes.clone(),
                "remote_computer_required": assignment.remote_computer_required
            }),
            output_payload: empty_json_object(),
            artifact_ids: Vec::new(),
            approval_ids: Vec::new(),
            tool_call_ids: Vec::new(),
            claimed_by_worker: None,
            lease_expires_at: None,
            context_packet_id: None,
            started_at: None,
            completed_at: None,
            scheduled_at: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let child_grant = state.create_task_grant(child_grant).await?;
    record_task_grant_issued(state, &child_grant, run.primary_session_id).await?;
    let step = state
        .update_workflow_step_run_task_grant(step.id, child_grant.id)
        .await?;
    record_workflow_step_run_created(state, &run, &step).await?;
    Ok(Some((step, child_grant)))
}

pub(crate) fn child_handoff_memory_scope(parent: &Value) -> Value {
    let mut memory_scope = parent.clone();
    if let Some(object) = memory_scope.as_object_mut()
        && object.contains_key("writeback_allowed")
    {
        object.insert("writeback_allowed".to_string(), json!(false));
    }
    memory_scope
}

pub(crate) fn child_tool_scope_for_agent(parent: &Value, agent: &Agent) -> Value {
    let agent_tools = agent
        .tools
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut scope = serde_json::Map::new();
    for key in ["read", "write", "external_write"] {
        let values = parent
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .flat_map(|tool| {
                        if tool == "*" {
                            agent_tools.iter().copied().collect::<Vec<_>>()
                        } else if agent_tools.contains(tool) {
                            vec![tool]
                        } else {
                            Vec::new()
                        }
                    })
                    .map(|tool| Value::String(tool.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        scope.insert(key.to_string(), Value::Array(values));
    }
    Value::Object(scope)
}

pub(crate) async fn transition_agent_handoff_event(
    state: AppState,
    id: Uuid,
    headers: HeaderMap,
    input: TransitionAgentHandoffEvent,
    next_status: &str,
) -> Result<Json<AgentHandoffEvent>, AppError> {
    let current = state.get_agent_handoff_event(id).await?;
    let permission = if next_status == "accepted"
        && (current.approval_required || current.risk_level == "high")
    {
        Permission::ApprovalsDecide
    } else {
        Permission::SessionsRun
    };
    authorize_request(
        &state,
        &headers,
        permission,
        "session",
        Some(current.source_session_id),
    )
    .await?;
    ensure_agent_handoff_transition(&current.status, next_status)?;
    let event_type = format!("agent_handoff.{next_status}");
    let audit =
        record_agent_handoff_audit_and_event(&state, &current, &event_type, input.reason).await?;
    let updated = state
        .update_agent_handoff_event_status(current.id, next_status, Some(audit.id))
        .await?;
    if matches!(next_status, "completed" | "failed")
        && let Some(thread) = state.session_thread_for_handoff(current.id).await?
    {
        let status = if next_status == "completed" {
            "terminated"
        } else {
            "failed"
        };
        let thread = state
            .update_session_thread_status(thread.id, status)
            .await?;
        state
            .append_event(
                "system",
                Some(thread.id),
                current.source_session_id,
                "thread.status_changed",
                session_thread_event_payload(&thread),
            )
            .await?;
        if let Some(specialist_session_id) = thread.specialist_session_id {
            state
                .append_event(
                    "system",
                    Some(thread.id),
                    specialist_session_id,
                    "thread.status_changed",
                    session_thread_event_payload(&thread),
                )
                .await?;
        }
    }
    Ok(Json(updated))
}

pub(crate) async fn record_agent_handoff_audit_and_event(
    state: &AppState,
    handoff: &AgentHandoffEvent,
    action: &str,
    reason: Option<String>,
) -> Result<AuditLog, AppError> {
    let details = json!({
        "agent_handoff_event_id": handoff.id,
        "source_session_id": handoff.source_session_id,
        "source_agent_id": handoff.source_agent_id,
        "target_agent_id": handoff.target_agent_id,
        "manager_plan_id": handoff.manager_plan_id,
        "intent": handoff.intent,
        "schema_version": handoff.schema_version,
        "risk_level": handoff.risk_level,
        "approval_required": handoff.approval_required,
        "semantic_scopes": handoff.semantic_scopes,
        "runtime_profile_id": handoff.runtime_profile_id,
        "remote_computer_required": handoff.remote_computer_required,
        "review_status": handoff.review_status,
        "human_escalation_status": handoff.human_escalation_status,
        "status": handoff.status,
        "reason": reason,
    });
    state
        .append_event(
            "agent",
            Some(handoff.source_agent_id),
            handoff.source_session_id,
            action,
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(handoff.source_session_id),
            "agent",
            Some(handoff.source_agent_id),
            action,
            "agent_handoff_event",
            Some(handoff.id),
            details,
        ))
        .await
}

pub(crate) async fn record_agent_handoff_assignment_audit_and_events(
    state: &AppState,
    assignment: &AgentHandoffAssignment,
    handoff: &AgentHandoffEvent,
) -> Result<AuditLog, AppError> {
    let details = json!({
        "agent_handoff_assignment_id": assignment.id,
        "agent_handoff_event_id": assignment.agent_handoff_event_id,
        "manager_plan_id": assignment.manager_plan_id,
        "source_session_id": assignment.source_session_id,
        "specialist_session_id": assignment.specialist_session_id,
        "source_agent_id": assignment.source_agent_id,
        "target_agent_id": assignment.target_agent_id,
        "intent": handoff.intent,
        "risk_level": handoff.risk_level,
        "approval_required": handoff.approval_required,
        "semantic_scopes": assignment.semantic_scopes,
        "runtime_profile_id": assignment.runtime_profile_id,
        "remote_computer_required": assignment.remote_computer_required,
        "remote_computer_job_assignment_id": assignment.remote_computer_job_assignment_id,
        "status": assignment.status,
        "assigned_by": assignment.assigned_by,
        "metadata": assignment.metadata,
    });
    state
        .append_event(
            "agent",
            Some(assignment.source_agent_id),
            assignment.source_session_id,
            "agent_handoff.assigned",
            details.clone(),
        )
        .await?;
    state
        .append_event(
            "agent",
            Some(assignment.source_agent_id),
            assignment.specialist_session_id,
            "agent_handoff.assignment_received",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(assignment.source_session_id),
            "agent",
            Some(assignment.source_agent_id),
            "agent_handoff.assigned",
            "agent_handoff_assignment",
            Some(assignment.id),
            details,
        ))
        .await
}

pub(crate) async fn record_agent_handoff_assignment_remote_computer_event(
    state: &AppState,
    assignment: &AgentHandoffAssignment,
    remote_assignment: &RemoteComputerJobAssignment,
) -> Result<(), AppError> {
    let details = json!({
        "agent_handoff_assignment_id": assignment.id,
        "agent_handoff_event_id": assignment.agent_handoff_event_id,
        "manager_plan_id": assignment.manager_plan_id,
        "source_session_id": assignment.source_session_id,
        "specialist_session_id": assignment.specialist_session_id,
        "remote_computer_job_assignment_id": remote_assignment.id,
        "execution_job_id": remote_assignment.execution_job_id,
        "remote_computer_id": remote_assignment.remote_computer_id,
        "lease_id": remote_assignment.lease_id,
        "status": assignment.status,
        "metadata": assignment.metadata,
    });
    state
        .append_event(
            "system",
            Some(remote_assignment.id),
            assignment.source_session_id,
            "agent_handoff.remote_computer_assignment_attached",
            details.clone(),
        )
        .await?;
    state
        .append_event(
            "system",
            Some(remote_assignment.id),
            assignment.specialist_session_id,
            "agent_handoff.remote_computer_assignment_attached",
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(assignment.specialist_session_id),
            "system",
            Some(remote_assignment.id),
            "agent_handoff.remote_computer_assignment_attached",
            "agent_handoff_assignment",
            Some(assignment.id),
            details,
        ))
        .await?;
    Ok(())
}

pub(crate) async fn create_handoff_session_thread(
    state: &AppState,
    assignment: &AgentHandoffAssignment,
    handoff: &AgentHandoffEvent,
    specialist_session: &Session,
    parent_thread_id: Option<Uuid>,
) -> Result<SessionThread, AppError> {
    if let Some(existing) = state.session_thread_for_handoff(handoff.id).await? {
        return Ok(existing);
    }
    let now = Utc::now();
    state
        .create_session_thread(SessionThread {
            id: Uuid::new_v4(),
            session_id: handoff.source_session_id,
            parent_thread_id,
            thread_kind: "specialist".to_string(),
            agent_id: assignment.target_agent_id,
            agent_version_id: specialist_session.agent_version_id,
            environment_id: specialist_session.environment_id,
            source_handoff_id: Some(handoff.id),
            specialist_session_id: Some(specialist_session.id),
            status: if assignment.status == "waiting_remote_computer" {
                "waiting_environment".to_string()
            } else {
                "running".to_string()
            },
            title: specialist_session.title.clone(),
            context: json!({
                "origin": "agent_handoff",
                "agent_handoff_assignment_id": assignment.id,
                "agent_handoff_event_id": handoff.id,
                "manager_plan_id": assignment.manager_plan_id,
                "intent": handoff.intent,
                "risk_level": handoff.risk_level,
                "approval_required": handoff.approval_required,
                "semantic_scopes": assignment.semantic_scopes,
                "remote_computer_required": assignment.remote_computer_required,
                "remote_computer_job_assignment_id": assignment.remote_computer_job_assignment_id
            }),
            created_at: now,
            updated_at: now,
        })
        .await
}

pub(crate) async fn record_manager_agent_plan_audit_and_event(
    state: &AppState,
    plan: &ManagerAgentPlan,
    action: &str,
) -> Result<AuditLog, AppError> {
    let details = json!({
        "manager_agent_plan_id": plan.id,
        "session_id": plan.session_id,
        "manager_agent_id": plan.manager_agent_id,
        "work_item_id": plan.work_item_id,
        "specialist_agent_id": plan.specialist_agent_id,
        "risk_classification": plan.risk_classification,
        "status": plan.status,
        "task_intake": plan.task_intake,
        "decomposition": plan.decomposition,
        "specialist_selection": plan.specialist_selection,
        "review": plan.review,
    });
    state
        .append_event(
            "agent",
            Some(plan.manager_agent_id),
            plan.session_id,
            action,
            details.clone(),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(plan.session_id),
            "agent",
            Some(plan.manager_agent_id),
            action,
            "manager_agent_plan",
            Some(plan.id),
            details,
        ))
        .await
}

pub(crate) async fn record_manager_agent_plan_work_item_activity(
    state: &AppState,
    plan: &ManagerAgentPlan,
    action: &str,
) -> Result<(), AppError> {
    let Some(work_item_id) = plan.work_item_id else {
        return Ok(());
    };
    let summary = match action {
        "manager_plan.created" => format!("Created manager plan: {}", plan.id),
        "manager_plan.reviewed" => format!("Reviewed manager plan with status: {}", plan.status),
        _ => format!("Updated manager plan: {}", plan.id),
    };
    state
        .append_work_item_activity_entry(
            work_item_id,
            action,
            Some(plan.manager_agent_id.to_string()),
            Some("manager_agent_plan"),
            Some(plan.id),
            summary,
            json!({
                "manager_agent_plan_id": plan.id,
                "session_id": plan.session_id,
                "manager_agent_id": plan.manager_agent_id,
                "specialist_agent_id": plan.specialist_agent_id,
                "risk_classification": plan.risk_classification,
                "status": plan.status,
                "review": plan.review,
            }),
        )
        .await?;
    Ok(())
}
