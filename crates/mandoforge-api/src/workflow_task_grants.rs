use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn issue_root_task_grant_for_workflow_run(
    state: &AppState,
    run: &WorkflowRun,
    definition: &WorkflowDefinition,
    session: &Session,
) -> Result<TaskGrant, AppError> {
    let now = Utc::now();
    let grantee_agent = state.get_agent(definition.default_agent_id).await?;
    let agent_class = if grantee_agent.agent_role.trim().is_empty() {
        grantee_agent.kind.clone()
    } else {
        grantee_agent.agent_role.clone()
    };
    let grant = TaskGrant {
        id: Uuid::new_v4(),
        workflow_run_id: run.id,
        workflow_step_run_id: None,
        session_id: Some(session.id),
        parent_grant_id: None,
        source_event_id: run.source_event_id,
        source_handoff_id: None,
        issuer_subject: "system".to_string(),
        grantee_agent_id: Some(definition.default_agent_id),
        grantee_session_id: Some(session.id),
        agent_class: Some(agent_class),
        objective: session.title.clone(),
        risk_level: "low".to_string(),
        status: "active".to_string(),
        expires_at: None,
        max_turns: None,
        max_tool_calls: None,
        max_runtime_seconds: None,
        max_cost_usd_micros: None,
        semantic_scopes: workflow_definition_root_task_grant_scope(
            definition,
            "semantic_scopes",
            empty_json_object,
        ),
        memory_scope: workflow_definition_root_task_grant_scope(
            definition,
            "memory_scope",
            default_task_grant_memory_scope,
        ),
        tool_scope: workflow_definition_root_task_grant_scope(
            definition,
            "tool_scope",
            default_task_grant_tool_scope,
        ),
        connector_scope: workflow_definition_root_task_grant_scope(
            definition,
            "connector_scope",
            default_task_grant_connector_scope,
        ),
        approval_policy: workflow_definition_root_task_grant_scope(
            definition,
            "approval_policy",
            default_task_grant_approval_policy,
        ),
        external_effects: workflow_definition_root_task_grant_scope(
            definition,
            "external_effects",
            default_task_grant_external_effects,
        ),
        context_packet_id: None,
        policy_revision_id: None,
        immutable_args_hash: None,
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    };
    validate_task_grant_scope_objects(&grant)?;
    let grant = state.create_task_grant(grant).await?;
    record_task_grant_issued(state, &grant, run.primary_session_id).await?;
    Ok(grant)
}

pub(crate) async fn record_task_grant_issued(
    state: &AppState,
    grant: &TaskGrant,
    primary_session_id: Uuid,
) -> Result<(), AppError> {
    state
        .append_event(
            "system",
            Some(grant.id),
            primary_session_id,
            "task_grant.issued",
            json!({
                "task_grant_id": grant.id,
                "workflow_run_id": grant.workflow_run_id,
                "parent_grant_id": grant.parent_grant_id,
                "workflow_step_run_id": grant.workflow_step_run_id,
                "grantee_agent_id": grant.grantee_agent_id,
                "grantee_session_id": grant.grantee_session_id,
                "agent_class": grant.agent_class,
                "risk_level": grant.risk_level,
                "status": grant.status
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(primary_session_id),
            "system",
            Some(grant.id),
            "task_grant.issued",
            "task_grant",
            Some(grant.id),
            json!({
                "workflow_run_id": grant.workflow_run_id,
                "parent_grant_id": grant.parent_grant_id,
                "grantee_agent_id": grant.grantee_agent_id,
                "grantee_session_id": grant.grantee_session_id,
                "risk_level": grant.risk_level,
                "status": grant.status
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_workflow_step_run_created(
    state: &AppState,
    run: &WorkflowRun,
    step: &WorkflowStepRun,
) -> Result<(), AppError> {
    state
        .append_event(
            "system",
            Some(step.id),
            run.primary_session_id,
            "workflow.step.created",
            json!({
                "workflow_run_id": run.id,
                "workflow_step_run_id": step.id,
                "step_key": step.step_key,
                "step_type": step.step_type,
                "status": step.status,
                "scheduled_at": step.scheduled_at,
                "agent_id": step.agent_id,
                "session_id": step.session_id,
                "handoff_id": step.handoff_id,
                "task_grant_id": step.task_grant_id
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(step.id),
            "workflow_step_run.created",
            "workflow_step_run",
            Some(step.id),
            json!({
                "workflow_run_id": run.id,
                "step_key": step.step_key,
                "step_type": step.step_type,
                "status": step.status,
                "scheduled_at": step.scheduled_at,
                "handoff_id": step.handoff_id,
                "task_grant_id": step.task_grant_id
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_workflow_transition(
    state: &AppState,
    run: &WorkflowRun,
    from_step: Option<&WorkflowStepRun>,
    to_step: Option<&WorkflowStepRun>,
    transition_type: &str,
    status: &str,
    condition_payload: Value,
    result_payload: Value,
) -> Result<WorkflowTransition, AppError> {
    let transition_type =
        require_non_empty(transition_type.to_string(), "workflow transition type")?;
    let status = require_non_empty(status.to_string(), "workflow transition status")?;
    let now = Utc::now();
    let transition = state
        .create_workflow_transition(WorkflowTransition {
            id: Uuid::new_v4(),
            workflow_run_id: run.id,
            from_step_run_id: from_step.map(|step| step.id),
            from_step_key: from_step.map(|step| step.step_key.clone()),
            to_step_run_id: to_step.map(|step| step.id),
            to_step_key: to_step.map(|step| step.step_key.clone()),
            transition_type,
            status,
            condition_payload,
            result_payload,
            created_at: now,
        })
        .await?;
    state
        .append_event(
            "system",
            Some(transition.id),
            run.primary_session_id,
            "workflow.transition.created",
            json!({
                "workflow_transition_id": transition.id,
                "workflow_run_id": transition.workflow_run_id,
                "from_step_run_id": transition.from_step_run_id,
                "from_step_key": transition.from_step_key,
                "to_step_run_id": transition.to_step_run_id,
                "to_step_key": transition.to_step_key,
                "transition_type": transition.transition_type,
                "status": transition.status,
                "condition_payload": transition.condition_payload,
                "result_payload": transition.result_payload
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(transition.id),
            "workflow_transition.created",
            "workflow_transition",
            Some(transition.id),
            json!({
                "workflow_run_id": transition.workflow_run_id,
                "from_step_run_id": transition.from_step_run_id,
                "from_step_key": transition.from_step_key,
                "to_step_run_id": transition.to_step_run_id,
                "to_step_key": transition.to_step_key,
                "transition_type": transition.transition_type,
                "status": transition.status
            }),
        ))
        .await?;
    Ok(transition)
}

pub(crate) async fn record_task_grant_checked(
    state: &AppState,
    grant: &TaskGrant,
    session_id: Uuid,
    tool_name: &str,
) -> Result<(), AppError> {
    state
        .append_event(
            "system",
            Some(grant.id),
            session_id,
            "task_grant.checked",
            json!({
                "task_grant_id": grant.id,
                "workflow_run_id": grant.workflow_run_id,
                "tool": tool_name,
                "status": "allowed"
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "system",
            Some(grant.id),
            "task_grant.checked",
            "task_grant",
            Some(grant.id),
            json!({
                "workflow_run_id": grant.workflow_run_id,
                "tool": tool_name,
                "status": "allowed"
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn record_task_grant_denied(
    state: &AppState,
    session_id: Uuid,
    grant: Option<&TaskGrant>,
    workflow_run_id: Option<Uuid>,
    tool_name: &str,
    reason: &str,
) -> Result<(), AppError> {
    state
        .append_event(
            "system",
            grant.map(|grant| grant.id),
            session_id,
            "task_grant.denied",
            json!({
                "task_grant_id": grant.map(|grant| grant.id),
                "workflow_run_id": grant.map(|grant| grant.workflow_run_id).or(workflow_run_id),
                "tool": tool_name,
                "status": "denied",
                "reason": reason
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "system",
            grant.map(|grant| grant.id),
            "task_grant.denied",
            "task_grant",
            grant.map(|grant| grant.id),
            json!({
                "workflow_run_id": grant.map(|grant| grant.workflow_run_id).or(workflow_run_id),
                "tool": tool_name,
                "status": "denied",
                "reason": reason
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn enforce_task_grant_for_tool_invocation(
    state: &AppState,
    tool_name: &str,
    input: &ExecuteTool,
) -> Result<Option<TaskGrant>, AppError> {
    let workflow_run = workflow_run_for_session(state, input.session_id).await?;

    let Some(task_grant_id) = input.task_grant_id else {
        if let Some(run) = workflow_run {
            let reason = "task grant is required for workflow tool execution";
            record_task_grant_denied(
                state,
                input.session_id,
                None,
                Some(run.id),
                tool_name,
                reason,
            )
            .await?;
            return Err(AppError::forbidden(reason));
        }
        return Ok(None);
    };

    let grant = state.get_task_grant(task_grant_id).await?;
    let run = state.get_workflow_run(grant.workflow_run_id).await?;
    if workflow_step_status_terminal(&run.status) {
        let reason = "workflow run is not active";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    if workflow_run
        .as_ref()
        .is_some_and(|workflow_run| workflow_run.id != run.id)
        || !task_grant_session_matches(&grant, &run, input.session_id)
    {
        let reason = "task grant is not valid for this session";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    let session = state.get_session(input.session_id).await?;
    if grant
        .grantee_agent_id
        .is_some_and(|agent_id| agent_id != session.agent_id)
    {
        let reason = "task grant grantee agent does not match this session";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    if let Some(agent_class) = grant.agent_class.as_deref() {
        let agent = state.get_agent(session.agent_id).await?;
        if !task_grant_agent_class_matches(&agent, agent_class) {
            let reason = "task grant agent class does not match this session";
            record_task_grant_denied(
                state,
                input.session_id,
                Some(&grant),
                Some(run.id),
                tool_name,
                reason,
            )
            .await?;
            return Err(AppError::forbidden(reason));
        }
    }
    if grant.status != "active" {
        let reason = "task grant is not active";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    if grant
        .expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        let reason = "task grant is expired";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    if !task_grant_allows_tool(&grant, tool_name) {
        let reason = "task grant tool scope does not allow tool";
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }
    if let Some(reason) = task_grant_connector_invocation_denial(&grant, tool_name, &input.args)? {
        record_task_grant_denied(
            state,
            input.session_id,
            Some(&grant),
            Some(run.id),
            tool_name,
            &reason,
        )
        .await?;
        return Err(AppError::forbidden(reason));
    }

    record_task_grant_checked(state, &grant, input.session_id, tool_name).await?;
    Ok(Some(grant))
}

pub(crate) fn task_grant_session_matches(
    grant: &TaskGrant,
    run: &WorkflowRun,
    session_id: Uuid,
) -> bool {
    run.primary_session_id == session_id
        || grant.session_id == Some(session_id)
        || grant.grantee_session_id == Some(session_id)
}

pub(crate) fn task_grant_agent_class_matches(agent: &Agent, agent_class: &str) -> bool {
    let expected = agent_class.trim();
    expected.is_empty() || agent.kind == expected || agent.agent_role == expected
}

pub(crate) async fn workflow_run_owns_session(
    state: &AppState,
    run: &WorkflowRun,
    session_id: Uuid,
) -> Result<bool, AppError> {
    if run.primary_session_id == session_id {
        return Ok(true);
    }
    Ok(state
        .list_workflow_step_runs(run.id)
        .await?
        .into_iter()
        .any(|step| step.session_id == Some(session_id)))
}

pub(crate) async fn workflow_run_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<WorkflowRun>, AppError> {
    let runs = state.list_workflow_runs().await?;
    if let Some(run) = runs
        .iter()
        .find(|run| run.primary_session_id == session_id)
        .cloned()
    {
        return Ok(Some(run));
    }
    for run in runs {
        if workflow_run_owns_session(state, &run, session_id).await? {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

pub(crate) async fn active_task_grant_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<Option<(WorkflowRun, TaskGrant)>, AppError> {
    let Some(run) = workflow_run_for_session(state, session_id).await? else {
        return Ok(None);
    };
    if run.primary_session_id == session_id {
        let root_task_grant_id = run
            .root_task_grant_id
            .ok_or_else(|| AppError::forbidden("task grant is required for workflow session"))?;
        let grant = state.get_task_grant(root_task_grant_id).await?;
        if grant.status != "active" {
            return Err(AppError::forbidden("task grant is not active"));
        }
        if grant
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
        {
            return Err(AppError::forbidden("task grant is expired"));
        }
        return Ok(Some((run, grant)));
    }
    let mut grants = state
        .list_task_grants_for_workflow_run(run.id)
        .await?
        .into_iter()
        .filter(|grant| grant.status == "active")
        .filter(|grant| {
            grant
                .expires_at
                .is_none_or(|expires_at| expires_at > Utc::now())
        })
        .filter(|grant| task_grant_session_matches(grant, &run, session_id))
        .collect::<Vec<_>>();
    grants.sort_by_key(|grant| grant.created_at);
    let Some(grant) = grants.pop() else {
        return Err(AppError::forbidden(
            "task grant is required for workflow session",
        ));
    };
    Ok(Some((run, grant)))
}

pub(crate) async fn require_active_task_grant_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<(WorkflowRun, TaskGrant), AppError> {
    let (run, grant) = active_task_grant_for_session(state, session_id)
        .await?
        .ok_or_else(|| AppError::forbidden("workflow session requires an active TaskGrant"))?;
    if workflow_step_status_terminal(&run.status) {
        return Err(AppError::forbidden("workflow run is not active"));
    }
    Ok((run, grant))
}

pub(crate) fn task_grant_allows_tool(grant: &TaskGrant, tool_name: &str) -> bool {
    ["read", "write", "external_write"]
        .iter()
        .any(|key| json_string_array_contains(grant.tool_scope.get(*key), tool_name))
}

pub(crate) fn task_grant_connector_invocation_denial(
    grant: &TaskGrant,
    tool_name: &str,
    args: &Value,
) -> Result<Option<String>, AppError> {
    if tool_name == "mcp.call" {
        return task_grant_mcp_call_denial(grant, args);
    }
    if !native_connector_invocation_requested(tool_name, args) {
        return Ok(None);
    }
    let target = native_connector_call_target(args)?;
    let mode = grant
        .connector_scope
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if mode != "commit_write" {
        return Ok(Some(
            "native connector side effects require commit_write connector scope".to_string(),
        ));
    }
    if !json_string_array_contains(
        grant.connector_scope.get("allowed_connector_ids"),
        &target.connector_id,
    ) {
        return Ok(Some(
            "task grant connector scope does not allow native connector id".to_string(),
        ));
    }
    if !json_string_array_contains(
        grant.connector_scope.get("allowed_tool_names"),
        &target.operation,
    ) && !json_string_array_contains(grant.connector_scope.get("allowed_tool_names"), tool_name)
    {
        return Ok(Some(
            "task grant connector scope does not allow native connector operation".to_string(),
        ));
    }
    if !task_grant_allows_side_effect_class(grant, &target.side_effect_class) {
        return Ok(Some(format!(
            "task grant side effect class {} is not allowed",
            target.side_effect_class
        )));
    }
    Ok(None)
}

pub(crate) fn task_grant_mcp_call_denial(
    grant: &TaskGrant,
    args: &Value,
) -> Result<Option<String>, AppError> {
    let request: McpCallRequest = serde_json::from_value(args.clone())?;
    let server = request.server.trim();
    let tool = request.tool.trim();
    if server.is_empty() || tool.is_empty() {
        return Ok(Some(
            "task grant connector scope requires MCP server and tool".to_string(),
        ));
    }
    let mode = grant
        .connector_scope
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(mode, "read_only" | "draft_write" | "commit_write") {
        return Ok(Some(
            "task grant connector scope does not allow MCP call mode".to_string(),
        ));
    }
    if !json_string_array_contains(grant.connector_scope.get("allowed_connector_ids"), server) {
        return Ok(Some(
            "task grant connector scope does not allow MCP server".to_string(),
        ));
    }
    if !json_string_array_contains(grant.connector_scope.get("allowed_tool_names"), tool) {
        return Ok(Some(
            "task grant connector scope does not allow MCP tool".to_string(),
        ));
    }
    if let Some(side_effect_class) = request
        .args
        .get("side_effect_class")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && !task_grant_allows_side_effect_class(grant, side_effect_class)
    {
        return Ok(Some(format!(
            "task grant side effect class {} is not allowed",
            side_effect_class
        )));
    }
    Ok(None)
}

pub(crate) fn task_grant_allows_side_effect_class(
    grant: &TaskGrant,
    side_effect_class: &str,
) -> bool {
    json_string_array_contains(
        grant.connector_scope.get("side_effect_classes"),
        side_effect_class,
    ) && grant
        .external_effects
        .get(side_effect_class)
        .and_then(Value::as_bool)
        == Some(true)
}

pub(crate) fn native_connector_invocation_requested(tool_name: &str, args: &Value) -> bool {
    tool_name == "native.connector.call"
        || args.get("connector_id").is_some()
        || args.get("connector").is_some()
}

#[derive(Debug, Clone)]
pub(crate) struct NativeConnectorCallTarget {
    pub(crate) connector_id: String,
    pub(crate) operation: String,
    pub(crate) side_effect_class: String,
}

pub(crate) fn native_connector_call_target(
    args: &Value,
) -> Result<NativeConnectorCallTarget, AppError> {
    let object = args
        .as_object()
        .ok_or_else(|| AppError::bad_request("native connector args must be a JSON object"))?;
    let connector_id = object
        .get("connector_id")
        .or_else(|| object.get("connector"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("native connector call requires connector_id"))?
        .to_string();
    let operation = object
        .get("operation")
        .or_else(|| object.get("tool"))
        .or_else(|| object.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("native connector call requires operation"))?
        .to_string();
    let side_effect_class = object
        .get("side_effect_class")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("native connector call requires side_effect_class"))?
        .to_string();
    Ok(NativeConnectorCallTarget {
        connector_id,
        operation,
        side_effect_class,
    })
}

pub(crate) fn workflow_transition_filter_from_query(
    query: WorkflowTransitionQuery,
) -> Result<WorkflowTransitionFilter, AppError> {
    let limit = match query.limit {
        Some(limit) if limit == 0 || limit > 500 => {
            return Err(AppError::bad_request(
                "workflow transition filter limit must be between 1 and 500",
            ));
        }
        other => other,
    };
    Ok(WorkflowTransitionFilter {
        transition_type: normalize_optional_filter(query.transition_type),
        status: normalize_optional_filter(query.status),
        from_step_key: normalize_optional_filter(query.from_step_key),
        to_step_key: normalize_optional_filter(query.to_step_key),
        limit,
    })
}

pub(crate) fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) async fn build_task_board_snapshot(
    state: &AppState,
) -> Result<TaskBoardSnapshot, AppError> {
    let generated_at = Utc::now();
    let work_items = state
        .list_work_items()
        .await?
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let runs = state.list_workflow_runs().await?;
    let mut items = Vec::new();
    let mut status_counts = BTreeMap::new();
    let mut workflow_step_count = 0usize;
    let mut claimable_count = 0usize;
    for run in &runs {
        let work_item = run
            .source_work_item_id
            .and_then(|work_item_id| work_items.get(&work_item_id));
        for step in state.list_workflow_step_runs(run.id).await? {
            workflow_step_count += 1;
            *status_counts.entry(step.status.clone()).or_insert(0) += 1;
            let blockers = workflow_step_claim_blockers(&step, step.agent_id, generated_at);
            let claimable = blockers.is_empty();
            if claimable {
                claimable_count += 1;
            }
            items.push(TaskBoardItem {
                work_item_id: run.source_work_item_id,
                work_item_title: work_item.map(|item| item.title.clone()),
                work_item_priority: work_item.map(|item| item.priority.clone()),
                workflow_run_id: run.id,
                workflow_definition_id: run.workflow_definition_id,
                workflow_step_run_id: step.id,
                step_key: step.step_key,
                step_type: step.step_type,
                agent_id: step.agent_id,
                task_grant_id: step.task_grant_id,
                context_packet_id: step.context_packet_id,
                status: step.status,
                claimable,
                blockers,
                claimed_by_worker: step.claimed_by_worker,
                lease_expires_at: step.lease_expires_at,
                updated_at: step.updated_at,
            });
        }
    }
    items.sort_by_key(|item| item.updated_at);
    items.reverse();
    Ok(TaskBoardSnapshot {
        generated_at,
        work_item_count: work_items.len(),
        workflow_run_count: runs.len(),
        workflow_step_count,
        claimable_count,
        status_counts,
        items,
    })
}

pub(crate) async fn build_agent_inbox_snapshot(
    state: &AppState,
    agent_id: Uuid,
) -> Result<AgentInboxSnapshot, AppError> {
    let generated_at = Utc::now();
    let work_items = state
        .list_work_items()
        .await?
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let mut entries = Vec::new();
    for run in state.list_workflow_runs().await? {
        let work_item = run
            .source_work_item_id
            .and_then(|work_item_id| work_items.get(&work_item_id))
            .cloned();
        for step in state.list_workflow_step_runs(run.id).await? {
            if step.agent_id != Some(agent_id) || workflow_step_status_terminal(&step.status) {
                continue;
            }
            let blockers = workflow_step_claim_blockers(&step, Some(agent_id), generated_at);
            entries.push(AgentInboxEntry {
                workflow_run_id: run.id,
                workflow_definition_id: run.workflow_definition_id,
                workflow_step_run_id: step.id,
                step_key: step.step_key.clone(),
                step_type: step.step_type.clone(),
                status: step.status.clone(),
                task_grant_id: step.task_grant_id,
                context_packet_id: step.context_packet_id,
                work_item: work_item.clone(),
                claimable: blockers.is_empty(),
                blockers,
                claimed_by_worker: step.claimed_by_worker.clone(),
                lease_expires_at: step.lease_expires_at,
                input_summary: workflow_graph_console_summary(&step.input_payload),
                updated_at: step.updated_at,
            });
        }
    }
    entries.sort_by_key(|entry| entry.updated_at);
    entries.reverse();
    let claimable_count = entries.iter().filter(|entry| entry.claimable).count();
    Ok(AgentInboxSnapshot {
        agent_id,
        generated_at,
        entry_count: entries.len(),
        claimable_count,
        entries,
    })
}

pub(crate) fn workflow_step_claim_blockers(
    step: &WorkflowStepRun,
    expected_agent_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if workflow_step_status_terminal(&step.status) {
        blockers.push("terminal_status".to_string());
    }
    if step.status == "scheduled" {
        match step.scheduled_at {
            Some(scheduled_at) if scheduled_at > now => {
                blockers.push("scheduled_for_future".to_string())
            }
            Some(_) => blockers.push("scheduled_until_scheduler_activation".to_string()),
            None => blockers.push("scheduled_without_due_time".to_string()),
        }
    } else if step.status != "queued" {
        if step.status == "running" {
            if step
                .lease_expires_at
                .is_none_or(|lease_expires_at| lease_expires_at > now)
            {
                blockers.push("already_claimed".to_string());
            }
        } else {
            blockers.push(format!("status_{}", step.status));
        }
    }
    match (step.agent_id, expected_agent_id) {
        (None, _) => blockers.push("missing_agent_binding".to_string()),
        (Some(actual), Some(expected)) if actual != expected => {
            blockers.push("agent_mismatch".to_string())
        }
        _ => {}
    }
    if step.task_grant_id.is_none() {
        blockers.push("missing_task_grant".to_string());
    }
    blockers
}

pub(crate) async fn build_workflow_run_graph_console(
    state: &AppState,
    run: &WorkflowRun,
) -> Result<WorkflowRunGraphConsole, AppError> {
    let generated_at = Utc::now();
    let definition = state
        .get_workflow_definition(run.workflow_definition_id)
        .await?;
    let steps = state.list_workflow_step_runs(run.id).await?;
    let transitions = state.list_workflow_transitions(run.id).await?;
    let mut status_counts = BTreeMap::new();
    let due_scheduled_count = steps
        .iter()
        .filter(|step| {
            step.status == "scheduled"
                && step
                    .scheduled_at
                    .is_some_and(|scheduled_at| scheduled_at <= generated_at)
        })
        .count();
    let materialized_step_keys = steps
        .iter()
        .map(|step| step.step_key.clone())
        .collect::<BTreeSet<_>>();
    let mut nodes = steps
        .iter()
        .map(|step| -> Result<WorkflowGraphConsoleNode, AppError> {
            *status_counts.entry(step.status.clone()).or_insert(0) += 1;
            let graph_step = workflow_graph_step_by_key(&definition.step_graph, &step.step_key)?;
            let dependencies = graph_step
                .map(workflow_graph_step_dependencies)
                .transpose()?
                .unwrap_or_default();
            let definition_summary = graph_step
                .map(workflow_graph_console_summary)
                .unwrap_or_else(empty_json_object);
            Ok(WorkflowGraphConsoleNode {
                id: step.id,
                step_run_id: Some(step.id),
                step_key: step.step_key.clone(),
                step_type: step.step_type.clone(),
                status: step.status.clone(),
                declared: false,
                dependencies,
                agent_id: step.agent_id,
                task_grant_id: step.task_grant_id,
                context_packet_id: step.context_packet_id,
                claimed_by_worker: step.claimed_by_worker.clone(),
                lease_expires_at: step.lease_expires_at,
                scheduled_at: step.scheduled_at,
                due: step.status == "scheduled"
                    && step
                        .scheduled_at
                        .is_some_and(|scheduled_at| scheduled_at <= generated_at),
                started_at: step.started_at,
                completed_at: step.completed_at,
                definition_summary,
                input_summary: workflow_graph_console_summary(&step.input_payload),
                output_summary: workflow_graph_console_summary(&step.output_payload),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    if let Some(graph_steps) = definition.step_graph.get("steps").and_then(Value::as_array) {
        for graph_step in graph_steps {
            let step_key = workflow_graph_step_key(graph_step)?;
            if materialized_step_keys.contains(&step_key) {
                continue;
            }
            let status = "declared".to_string();
            *status_counts.entry(status.clone()).or_insert(0) += 1;
            nodes.push(WorkflowGraphConsoleNode {
                id: workflow_graph_declared_node_id(run.id, &step_key),
                step_run_id: None,
                step_key: step_key.clone(),
                step_type: workflow_graph_step_type(graph_step),
                status,
                declared: true,
                dependencies: workflow_graph_step_dependencies(graph_step)?,
                agent_id: workflow_graph_step_agent_id(&definition, graph_step)?,
                task_grant_id: None,
                context_packet_id: None,
                claimed_by_worker: None,
                lease_expires_at: None,
                scheduled_at: None,
                due: false,
                started_at: None,
                completed_at: None,
                definition_summary: workflow_graph_console_summary(graph_step),
                input_summary: json!({
                    "source": "workflow_definition",
                    "graph_step": workflow_graph_console_summary(graph_step)
                }),
                output_summary: empty_json_object(),
            });
        }
    }
    let mut edges = transitions
        .iter()
        .map(|transition| WorkflowGraphConsoleEdge {
            id: transition.id,
            from_step_key: transition.from_step_key.clone(),
            to_step_key: transition.to_step_key.clone(),
            transition_type: transition.transition_type.clone(),
            status: transition.status.clone(),
            declared: false,
            condition_summary: workflow_graph_console_summary(&transition.condition_payload),
            result_summary: workflow_graph_console_summary(&transition.result_payload),
            created_at: transition.created_at,
        })
        .collect::<Vec<_>>();
    if let Some(graph_steps) = definition.step_graph.get("steps").and_then(Value::as_array) {
        for graph_step in graph_steps {
            let to_step_key = workflow_graph_step_key(graph_step)?;
            if materialized_step_keys.contains(&to_step_key) {
                continue;
            }
            for from_step_key in workflow_graph_step_dependencies(graph_step)? {
                edges.push(WorkflowGraphConsoleEdge {
                    id: workflow_graph_declared_edge_id(run.id, &from_step_key, &to_step_key),
                    from_step_key: Some(from_step_key),
                    to_step_key: Some(to_step_key.clone()),
                    transition_type: "declared_dependency".to_string(),
                    status: "declared".to_string(),
                    declared: true,
                    condition_summary: workflow_graph_console_summary(graph_step),
                    result_summary: empty_json_object(),
                    created_at: generated_at,
                });
            }
        }
    }
    Ok(WorkflowRunGraphConsole {
        workflow_run_id: run.id,
        workflow_definition_id: run.workflow_definition_id,
        pack_installation_id: run.pack_installation_id,
        generated_at,
        status: run.status.clone(),
        node_count: nodes.len(),
        edge_count: edges.len(),
        due_scheduled_count,
        status_counts,
        nodes,
        edges,
    })
}

pub(crate) fn workflow_graph_declared_node_id(run_id: Uuid, step_key: &str) -> Uuid {
    workflow_graph_deterministic_uuid(run_id, "declared-node", &[step_key])
}

pub(crate) fn workflow_graph_declared_edge_id(
    run_id: Uuid,
    from_step_key: &str,
    to_step_key: &str,
) -> Uuid {
    workflow_graph_deterministic_uuid(run_id, "declared-edge", &[from_step_key, to_step_key])
}

pub(crate) fn workflow_graph_deterministic_uuid(run_id: Uuid, kind: &str, parts: &[&str]) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"mandoforge-workflow-graph-console");
    hasher.update(run_id.as_bytes());
    hasher.update(kind.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub(crate) fn workflow_graph_console_summary(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut summary = serde_json::Map::new();
            summary.insert("field_count".to_string(), json!(object.len()));
            let keys = object.keys().take(8).cloned().collect::<Vec<_>>();
            summary.insert("keys".to_string(), json!(keys));
            for key in [
                "source",
                "retry",
                "fan_in",
                "fan_out",
                "branch",
                "skip_reason",
                "error",
                "result",
            ] {
                if let Some(item) = object.get(key) {
                    summary.insert(key.to_string(), item.clone());
                }
            }
            Value::Object(summary)
        }
        Value::Array(items) => json!({"array_length": items.len()}),
        other => other.clone(),
    }
}
