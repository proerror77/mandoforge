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
    state.ensure_session_runnable(session.id).await?;
    if crate::store_entities::agent_release_enforcement_required() {
        require_active_task_grant_for_session(state, session.id).await?;
    }
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
    let target_version = state
        .runnable_agent_version(input.target_agent_id, session.environment_id)
        .await?;
    let semantic_scopes = normalize_handoff_semantic_scopes(
        input
            .semantic_scopes
            .unwrap_or_else(|| target_version.semantic_scopes.clone()),
    )?;
    if crate::store_entities::agent_release_enforcement_required()
        && input.runtime_profile_id.is_some()
        && input.runtime_profile_id != target_version.runtime_profile_id
    {
        return Err(AppError::forbidden(
            "production handoff runtime profile must match the promoted target agent version",
        ));
    }
    let runtime_profile_id = input
        .runtime_profile_id
        .or(target_version.runtime_profile_id);
    let runtime_profile = match runtime_profile_id {
        Some(profile_id)
            if Some(profile_id) == target_version.runtime_profile_id
                && target_version
                    .runtime_profile_snapshot
                    .as_object()
                    .is_some_and(|snapshot| !snapshot.is_empty()) =>
        {
            Some(
                serde_json::from_value::<AgentRuntimeProfile>(
                    target_version.runtime_profile_snapshot.clone(),
                )
                .map_err(|error| {
                    AppError::forbidden(format!(
                        "target agent version runtime profile snapshot is invalid: {error}"
                    ))
                })?,
            )
        }
        Some(profile_id) => Some(state.get_agent_runtime_profile(profile_id).await?),
        None => None,
    };
    let mut governed_remote_computer_required = target_version
        .remote_computer_profile
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || runtime_profile
            .as_ref()
            .is_some_and(|profile| profile.remote_computer_required);
    if let Some(environment_id) = session.environment_id {
        let environment = state.get_environment(environment_id).await?;
        governed_remote_computer_required |= environment.environment_type == "remote_computer"
            || environment
                .remote_computer_profile
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if let Some(profile_id) = environment.runtime_profile_id {
            governed_remote_computer_required |= state
                .get_agent_runtime_profile(profile_id)
                .await?
                .remote_computer_required;
        }
    }
    if crate::store_entities::agent_release_enforcement_required()
        && governed_remote_computer_required
        && input.remote_computer_required == Some(false)
    {
        return Err(AppError::forbidden(
            "production handoff cannot disable the governed Remote Computer requirement",
        ));
    }
    let remote_computer_required = input
        .remote_computer_required
        .unwrap_or(governed_remote_computer_required);
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
    if let Some(reason) = workflow_run_execution_denial(&run.status) {
        return Err(AppError::forbidden(reason));
    }
    let specialist_version = state
        .agent_version_for_session(specialist_session.id)
        .await?;
    let step_id = Uuid::new_v4();
    let now = Utc::now();
    let remaining_budgets = task_grant_remaining_budgets(&parent_grant, now)?;
    let (connector_scope, external_effects) = child_connector_scopes_for_agent_version(
        &parent_grant.connector_scope,
        &parent_grant.external_effects,
        &specialist_version,
    )?;
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
        max_turns: remaining_budgets.max_turns,
        max_tool_calls: remaining_budgets.max_tool_calls,
        max_runtime_seconds: remaining_budgets.max_runtime_seconds,
        max_cost_usd_micros: remaining_budgets.max_cost_usd_micros,
        turns_used: 0,
        tool_calls_used: 0,
        cost_usd_micros_used: 0,
        semantic_scopes: handoff.semantic_scopes.clone(),
        memory_scope: child_handoff_memory_scope(&parent_grant.memory_scope),
        tool_scope: child_tool_scope_for_tools(&parent_grant.tool_scope, &specialist_version.tools),
        connector_scope,
        approval_policy: parent_grant.approval_policy.clone(),
        external_effects,
        context_packet_id: None,
        policy_revision_id: parent_grant.policy_revision_id,
        immutable_args_hash: None,
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    };
    validate_task_grant_scope_objects(&child_grant)?;
    ensure_child_task_grant_within_parent(&parent_grant, &child_grant)?;

    let agent_version_id = specialist_session.agent_version_id;
    let step = WorkflowStepRun {
        id: step_id,
        workflow_run_id: run.id,
        step_key: handoff.intent.clone(),
        step_type: "handoff".to_string(),
        agent_id: Some(target_agent.id),
        agent_version_id,
        session_id: Some(specialist_session.id),
        thread_id: Some(child_thread.id),
        handoff_id: Some(handoff.id),
        task_grant_id: Some(child_grant.id),
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
    };
    let (step, child_grant) = state
        .create_workflow_step_run_with_task_grant(step, child_grant)
        .await?;
    record_task_grant_issued(state, &child_grant, run.primary_session_id).await?;
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

pub(crate) fn child_tool_scope_for_tools(parent: &Value, tools: &[String]) -> Value {
    let agent_tools = tools.iter().map(String::as_str).collect::<HashSet<_>>();
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

pub(crate) fn child_connector_scopes_for_agent_version(
    parent_connector_scope: &Value,
    parent_external_effects: &Value,
    agent_version: &AgentVersion,
) -> Result<(Value, Value), AppError> {
    if !agent_version
        .tools
        .iter()
        .any(|tool| tool == "native.connector.call")
    {
        return Ok((
            parent_connector_scope.clone(),
            parent_external_effects.clone(),
        ));
    }

    let Some(agent_connector_scope) = agent_version.approval_policy.get("connector_scope") else {
        return Ok(deny_child_native_connector_scope(
            parent_connector_scope,
            parent_external_effects,
        ));
    };
    if !agent_connector_scope.is_object() {
        return Err(AppError::bad_request(
            "agent version connector_scope policy must be an object",
        ));
    }
    if agent_connector_scope.get("mode").and_then(Value::as_str) != Some("commit_write") {
        return Ok(deny_child_native_connector_scope(
            parent_connector_scope,
            parent_external_effects,
        ));
    }
    let agent_external_effects = agent_version
        .approval_policy
        .get("external_effects")
        .unwrap_or(&Value::Null);
    if !agent_external_effects.is_null() && !agent_external_effects.is_object() {
        return Err(AppError::bad_request(
            "agent version external_effects policy must be an object",
        ));
    }

    let mut child_connector_scope = parent_connector_scope.clone();
    let child_object = child_connector_scope.as_object_mut().ok_or_else(|| {
        AppError::bad_request("parent task grant connector_scope must be an object")
    })?;
    let parent_bindings = native_operation_bindings(parent_connector_scope)?;
    let agent_bindings = native_operation_bindings(agent_connector_scope)?;
    let retained_bindings = match (parent_bindings.as_ref(), agent_bindings.as_ref()) {
        (Some(parent), Some(agent)) => agent
            .iter()
            .filter(|binding| {
                parent.contains(binding)
                    && native_operation_binding_within_scope(binding, parent_connector_scope)
                    && native_operation_binding_within_scope(binding, agent_connector_scope)
            })
            .cloned()
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let connector_ids = retained_bindings
        .iter()
        .filter_map(|binding| binding.get("connector_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let operation_ids = retained_bindings
        .iter()
        .filter_map(|binding| binding.get("operation").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let side_effect_classes = retained_bindings
        .iter()
        .filter_map(|binding| binding.get("side_effect_class").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();

    child_object.insert("allowed_connector_ids".to_string(), json!(connector_ids));
    child_object.insert("allowed_tool_names".to_string(), json!(operation_ids));
    child_object.insert(
        "side_effect_classes".to_string(),
        json!(side_effect_classes),
    );
    if parent_bindings.is_some() {
        child_object.insert(
            "native_operation_bindings".to_string(),
            Value::Array(retained_bindings),
        );
    }
    if let (Some(parent_tenant_scope), Some(agent_tenant_scope)) = (
        parent_connector_scope.get("tenant_scope"),
        agent_connector_scope.get("tenant_scope"),
    ) && agent_tenant_scope
        .as_object()
        .is_some_and(|scope| !scope.is_empty())
        && json_scope_contains(parent_tenant_scope, agent_tenant_scope)
    {
        child_object.insert("tenant_scope".to_string(), agent_tenant_scope.clone());
    }

    let child_external_effects = intersect_external_effects(
        parent_external_effects,
        agent_external_effects,
        &side_effect_classes,
    )?;
    Ok((child_connector_scope, child_external_effects))
}

fn deny_child_native_connector_scope(
    parent_connector_scope: &Value,
    parent_external_effects: &Value,
) -> (Value, Value) {
    let mut connector_scope = parent_connector_scope.clone();
    if let Some(object) = connector_scope.as_object_mut() {
        for key in [
            "allowed_connector_ids",
            "allowed_tool_names",
            "side_effect_classes",
        ] {
            object.insert(key.to_string(), json!([]));
        }
        if object.contains_key("native_operation_bindings") {
            object.insert("native_operation_bindings".to_string(), json!([]));
        }
    }
    let external_effects = parent_external_effects
        .as_object()
        .map(|effects| {
            Value::Object(
                effects
                    .keys()
                    .map(|key| (key.clone(), json!(false)))
                    .collect(),
            )
        })
        .unwrap_or_else(empty_json_object);
    (connector_scope, external_effects)
}

fn native_operation_bindings(scope: &Value) -> Result<Option<Vec<Value>>, AppError> {
    let Some(bindings) = scope.get("native_operation_bindings") else {
        return Ok(None);
    };
    let bindings = bindings.as_array().ok_or_else(|| {
        AppError::bad_request("connector_scope native_operation_bindings must be an array")
    })?;
    Ok(Some(bindings.clone()))
}

fn native_operation_binding_within_scope(binding: &Value, scope: &Value) -> bool {
    let Some(connector_id) = binding.get("connector_id").and_then(Value::as_str) else {
        return false;
    };
    let Some(operation) = binding.get("operation").and_then(Value::as_str) else {
        return false;
    };
    let Some(side_effect_class) = binding.get("side_effect_class").and_then(Value::as_str) else {
        return false;
    };
    json_string_array_contains(scope.get("allowed_connector_ids"), connector_id)
        && json_string_array_contains(scope.get("allowed_tool_names"), operation)
        && json_string_array_contains(scope.get("side_effect_classes"), side_effect_class)
}

fn intersect_external_effects(
    parent: &Value,
    agent: &Value,
    allowed_side_effects: &BTreeSet<String>,
) -> Result<Value, AppError> {
    let parent = parent
        .as_object()
        .ok_or_else(|| AppError::bad_request("parent external_effects must be an object"))?;
    let agent = agent.as_object();
    Ok(Value::Object(
        parent
            .iter()
            .map(|(key, value)| {
                let allowed = value.as_bool() == Some(true)
                    && allowed_side_effects.contains(key.as_str())
                    && agent
                        .and_then(|effects| effects.get(key))
                        .and_then(Value::as_bool)
                        == Some(true);
                (key.clone(), json!(allowed))
            })
            .collect(),
    ))
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
    let terminal_transition = matches!(next_status, "completed" | "failed" | "rejected");
    if !terminal_transition {
        state
            .ensure_session_runnable(current.source_session_id)
            .await?;
    }
    if crate::store_entities::agent_release_enforcement_required() && !terminal_transition {
        require_active_task_grant_for_session(&state, current.source_session_id).await?;
    }
    ensure_agent_handoff_transition(&current.status, next_status)?;
    let event_type = format!("agent_handoff.{next_status}");
    let audit =
        record_agent_handoff_audit_and_event(&state, &current, &event_type, input.reason).await?;
    let updated = state
        .update_agent_handoff_event_status(current.id, next_status, Some(audit.id))
        .await?;
    if matches!(next_status, "completed" | "failed") {
        close_workflow_handoff_step(&state, &current, next_status).await?;
    }
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

async fn close_workflow_handoff_step(
    state: &AppState,
    handoff: &AgentHandoffEvent,
    handoff_status: &str,
) -> Result<(), AppError> {
    for run in state.list_workflow_runs().await? {
        let Some(step) = state
            .list_workflow_step_runs(run.id)
            .await?
            .into_iter()
            .find(|step| step.handoff_id == Some(handoff.id))
        else {
            continue;
        };
        if workflow_step_status_terminal(&step.status) {
            return Ok(());
        }
        let previous_status = step.status.clone();
        let now = Utc::now();
        let mut next = step;
        next.status = if handoff_status == "completed" {
            "completed".to_string()
        } else {
            "failed".to_string()
        };
        next.started_at = next.started_at.or(Some(now));
        next.completed_at = Some(now);
        next.updated_at = now;
        let updated = state.update_workflow_step_run(next).await?;
        record_workflow_step_run_updated(state, &run, &updated, &previous_status).await?;
        advance_workflow_graph_after_step_update(state, &run, &updated).await?;
        return Ok(());
    }
    Ok(())
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
