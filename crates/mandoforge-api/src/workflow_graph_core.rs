use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) fn workflow_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|item| {
            if item.is_ascii_alphanumeric() || item == '-' || item == '_' {
                item.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "workflow".to_string()
    } else {
        slug
    }
}

pub(crate) fn require_non_empty(value: String, field: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_workflow_trigger_type(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "manual" | "schedule" | "webhook" | "work_item" | "connector_event" | "api" => {
            Ok(normalized)
        }
        _ => Err(AppError::bad_request(
            "workflow trigger_type must be manual, schedule, webhook, work_item, connector_event, or api",
        )),
    }
}

pub(crate) fn normalize_workflow_release_state(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "draft" | "staged" | "released" | "rolled_back" | "archived" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "workflow release_state must be draft, staged, released, rolled_back, or archived",
        )),
    }
}

pub(crate) fn workflow_handoff_rules_is_dynamic_materialization(handoff_rules: &Value) -> bool {
    handoff_rules
        .get("source")
        .and_then(Value::as_str)
        .is_some_and(|source| source == "dynamic_workflow_plan")
}

pub(crate) fn validate_dynamic_materialization_provenance_update(
    current: &Value,
    proposed: &Value,
) -> Result<(), AppError> {
    let current_is_dynamic = workflow_handoff_rules_is_dynamic_materialization(current);
    let proposed_is_dynamic = workflow_handoff_rules_is_dynamic_materialization(proposed);
    if current_is_dynamic != proposed_is_dynamic {
        return Err(AppError::bad_request(
            "dynamic workflow materialization provenance is immutable",
        ));
    }
    if current_is_dynamic {
        for field in [
            "source",
            "dynamic_workflow_plan_id",
            "materialization_approval",
        ] {
            if current.get(field) != proposed.get(field) {
                return Err(AppError::bad_request(format!(
                    "dynamic workflow materialization provenance field {field} is immutable"
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn normalize_workflow_execution_strategy(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "native_steps" | "delegated_runtime" | "native_dynamic" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "workflow execution_strategy must be native_steps, delegated_runtime, or native_dynamic",
        )),
    }
}

pub(crate) fn normalize_runtime_adapter(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "codex_app_server" | "codex_cli" | "claude_code" | "gemini" | "opencode" | "aider"
        | "hosted" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "runtime_adapter must be codex_app_server, codex_cli, claude_code, gemini, opencode, aider, or hosted",
        )),
    }
}

pub(crate) fn normalize_optional_runtime_adapter(
    value: Option<String>,
) -> Result<Option<String>, AppError> {
    value
        .and_then(normalize_optional_text)
        .map(|value| normalize_runtime_adapter(&value))
        .transpose()
}

pub(crate) fn normalize_runtime_mode(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "normal" | "ultracode" | "dynamic_workflow" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "runtime_mode must be normal, ultracode, or dynamic_workflow",
        )),
    }
}

pub(crate) fn normalize_optional_runtime_mode(
    value: Option<String>,
) -> Result<Option<String>, AppError> {
    value
        .and_then(normalize_optional_text)
        .map(|value| normalize_runtime_mode(&value))
        .transpose()
}

pub(crate) fn normalize_event_ingestion_policy(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "normalized" | "raw_only" | "disabled" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "event_ingestion_policy must be normalized, raw_only, or disabled",
        )),
    }
}

pub(crate) fn validate_workflow_execution_binding(
    execution_strategy: &str,
    runtime_adapter: Option<&str>,
    runtime_capability_contract: &Value,
) -> Result<(), AppError> {
    if !runtime_capability_contract.is_object() {
        return Err(AppError::bad_request(
            "runtime_capability_contract must be a JSON object",
        ));
    }
    if execution_strategy == "delegated_runtime" && runtime_adapter.is_none() {
        return Err(AppError::bad_request(
            "delegated_runtime workflow requires runtime_adapter",
        ));
    }
    Ok(())
}

pub(crate) fn workflow_definition_step_graph_for_execution(
    execution_strategy: &str,
    step_graph: &Value,
) -> Value {
    if execution_strategy != "delegated_runtime"
        || step_graph
            .get("steps")
            .and_then(Value::as_array)
            .is_some_and(|steps| !steps.is_empty())
    {
        return step_graph.clone();
    }
    json!({
        "source": "delegated_runtime_envelope",
        "steps": [
            {
                "key": "delegated-runtime",
                "type": "delegated_runtime",
                "start": true,
                "input": {
                    "objective": "Delegate this workflow run to the configured external agent runtime."
                }
            }
        ]
    })
}

pub(crate) fn normalize_workflow_run_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "queued" | "scheduled" | "running" | "requires_action" | "completed" | "failed"
        | "canceled" | "skipped" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "workflow status must be queued, scheduled, running, requires_action, completed, failed, canceled, or skipped",
        )),
    }
}

pub(crate) fn workflow_run_status_allows_execution(status: &str) -> bool {
    matches!(status, "queued" | "running" | "requires_action")
}

pub(crate) fn workflow_run_status_allows_step_creation(status: &str) -> bool {
    status == "initializing" || workflow_run_status_allows_execution(status)
}

pub(crate) fn workflow_run_execution_denial(status: &str) -> Option<&'static str> {
    if workflow_run_status_allows_execution(status) {
        None
    } else if matches!(status, "completed" | "failed" | "canceled" | "skipped") {
        Some("workflow run is not active")
    } else {
        Some("workflow run is not executable")
    }
}

pub(crate) async fn ensure_session_event_exists(
    state: &AppState,
    event_id: Uuid,
) -> Result<(), AppError> {
    for session in state.list_sessions().await? {
        if state
            .list_events(session.id)
            .await?
            .iter()
            .any(|event| event.id == event_id)
        {
            return Ok(());
        }
    }
    Err(AppError::not_found("session event not found"))
}

pub(crate) fn normalize_task_grant_risk_level(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "task grant risk_level must be low, medium, high, or critical",
        )),
    }
}

pub(crate) fn validate_task_grant_scope_objects(grant: &TaskGrant) -> Result<(), AppError> {
    for (field, value) in [
        ("semantic_scopes", &grant.semantic_scopes),
        ("memory_scope", &grant.memory_scope),
        ("tool_scope", &grant.tool_scope),
        ("connector_scope", &grant.connector_scope),
        ("approval_policy", &grant.approval_policy),
        ("external_effects", &grant.external_effects),
    ] {
        if !value.is_object() {
            return Err(AppError::bad_request(format!(
                "task grant {field} must be a JSON object"
            )));
        }
    }
    Ok(())
}

pub(crate) fn workflow_definition_root_task_grant_scope(
    definition: &WorkflowDefinition,
    key: &str,
    default_value: impl FnOnce() -> Value,
) -> Value {
    definition
        .handoff_rules
        .get("root_task_grant")
        .and_then(|value| value.get(key))
        .or_else(|| {
            definition
                .handoff_rules
                .get("task_grant_template")
                .and_then(|value| value.get(key))
        })
        .cloned()
        .unwrap_or_else(default_value)
}

fn workflow_definition_root_task_grant_value<'a>(
    definition: &'a WorkflowDefinition,
    key: &str,
) -> Option<&'a Value> {
    definition
        .handoff_rules
        .get("root_task_grant")
        .and_then(|value| value.get(key))
        .or_else(|| {
            definition
                .handoff_rules
                .get("task_grant_template")
                .and_then(|value| value.get(key))
        })
}

pub(crate) fn workflow_definition_root_task_grant_budget_i32(
    definition: &WorkflowDefinition,
    key: &str,
) -> Result<Option<i32>, AppError> {
    let Some(value) = workflow_definition_root_task_grant_value(definition, key) else {
        return Ok(None);
    };
    let value = value.as_i64().ok_or_else(|| {
        AppError::bad_request(format!("root task grant {key} must be a positive integer"))
    })?;
    let value = i32::try_from(value).map_err(|_| {
        AppError::bad_request(format!("root task grant {key} exceeds the supported range"))
    })?;
    if value <= 0 {
        return Err(AppError::bad_request(format!(
            "root task grant {key} must be a positive integer"
        )));
    }
    Ok(Some(value))
}

pub(crate) fn workflow_definition_root_task_grant_budget_i64(
    definition: &WorkflowDefinition,
    key: &str,
) -> Result<Option<i64>, AppError> {
    let Some(value) = workflow_definition_root_task_grant_value(definition, key) else {
        return Ok(None);
    };
    let value = value.as_i64().ok_or_else(|| {
        AppError::bad_request(format!("root task grant {key} must be a positive integer"))
    })?;
    if value <= 0 {
        return Err(AppError::bad_request(format!(
            "root task grant {key} must be a positive integer"
        )));
    }
    Ok(Some(value))
}

pub(crate) fn validate_task_grant_budgets(grant: &TaskGrant) -> Result<(), AppError> {
    for (field, value) in [
        ("max_turns", grant.max_turns),
        ("max_tool_calls", grant.max_tool_calls),
        ("max_runtime_seconds", grant.max_runtime_seconds),
    ] {
        if value.is_some_and(|value| value <= 0) {
            return Err(AppError::bad_request(format!(
                "task grant {field} must be positive"
            )));
        }
    }
    if grant.max_cost_usd_micros.is_some_and(|value| value <= 0) {
        return Err(AppError::bad_request(
            "task grant max_cost_usd_micros must be positive",
        ));
    }
    if grant.turns_used < 0 || grant.tool_calls_used < 0 || grant.cost_usd_micros_used < 0 {
        return Err(AppError::bad_request(
            "task grant usage counters cannot be negative",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskGrantRemainingBudgets {
    pub(crate) max_turns: Option<i32>,
    pub(crate) max_tool_calls: Option<i32>,
    pub(crate) max_runtime_seconds: Option<i32>,
    pub(crate) max_cost_usd_micros: Option<i64>,
}

pub(crate) fn task_grant_remaining_budgets(
    grant: &TaskGrant,
    now: DateTime<Utc>,
) -> Result<TaskGrantRemainingBudgets, AppError> {
    let remaining_i32 = |name: &str, limit: Option<i32>, used: i32| {
        let Some(limit) = limit else {
            return Ok(None);
        };
        let remaining = limit
            .checked_sub(used)
            .ok_or_else(|| AppError::forbidden(format!("task grant {name} budget is exhausted")))?;
        if remaining <= 0 {
            return Err(AppError::forbidden(format!(
                "task grant {name} budget is exhausted"
            )));
        }
        Ok(Some(remaining))
    };
    let max_runtime_seconds = match grant.max_runtime_seconds {
        Some(limit) => {
            let elapsed = now
                .signed_duration_since(grant.created_at)
                .num_seconds()
                .max(0);
            let remaining = i64::from(limit)
                .checked_sub(elapsed)
                .ok_or_else(|| AppError::forbidden("task grant runtime budget is exhausted"))?;
            if remaining <= 0 {
                return Err(AppError::forbidden(
                    "task grant runtime budget is exhausted",
                ));
            }
            Some(i32::try_from(remaining).map_err(|_| {
                AppError::bad_request("task grant runtime budget exceeds supported range")
            })?)
        }
        None => None,
    };
    let max_cost_usd_micros = match grant.max_cost_usd_micros {
        Some(limit) => {
            let remaining = limit
                .checked_sub(grant.cost_usd_micros_used)
                .ok_or_else(|| AppError::forbidden("task grant cost budget is exhausted"))?;
            if remaining <= 0 {
                return Err(AppError::forbidden("task grant cost budget is exhausted"));
            }
            Some(remaining)
        }
        None => None,
    };
    Ok(TaskGrantRemainingBudgets {
        max_turns: remaining_i32("turn", grant.max_turns, grant.turns_used)?,
        max_tool_calls: remaining_i32("tool call", grant.max_tool_calls, grant.tool_calls_used)?,
        max_runtime_seconds,
        max_cost_usd_micros,
    })
}

pub(crate) fn ensure_child_task_grant_within_parent(
    parent: &TaskGrant,
    child: &TaskGrant,
) -> Result<(), AppError> {
    let remaining = task_grant_remaining_budgets(parent, child.created_at)?;
    let within_parent = child.workflow_run_id == parent.workflow_run_id
        && limit_within_parent(remaining.max_turns, child.max_turns)
        && limit_within_parent(remaining.max_tool_calls, child.max_tool_calls)
        && limit_within_parent(remaining.max_runtime_seconds, child.max_runtime_seconds)
        && limit_within_parent(remaining.max_cost_usd_micros, child.max_cost_usd_micros)
        && json_scope_contains(&parent.semantic_scopes, &child.semantic_scopes)
        && json_scope_contains(&parent.memory_scope, &child.memory_scope)
        && json_scope_contains(&parent.tool_scope, &child.tool_scope)
        && json_scope_contains(&parent.connector_scope, &child.connector_scope)
        && json_scope_contains(&parent.approval_policy, &child.approval_policy)
        && json_scope_contains(&parent.external_effects, &child.external_effects);
    if within_parent {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "child task grant cannot expand parent grant",
        ))
    }
}

pub(crate) fn limit_within_parent<T>(parent: Option<T>, child: Option<T>) -> bool
where
    T: PartialOrd,
{
    match (parent, child) {
        (Some(parent), Some(child)) => child <= parent,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

pub(crate) fn json_scope_contains(parent: &Value, child: &Value) -> bool {
    match (parent, child) {
        (_, Value::Null) => true,
        (Value::Object(parent), Value::Object(child)) => child.iter().all(|(key, child_value)| {
            parent
                .get(key)
                .is_some_and(|parent_value| json_scope_contains(parent_value, child_value))
        }),
        (Value::Array(parent), Value::Array(child)) => child.iter().all(|child_value| {
            parent.iter().any(|parent_value| {
                parent_value == child_value
                    || (parent_value.as_str() == Some("*") && child_value.is_string())
            })
        }),
        (Value::Bool(true), Value::Bool(_)) => true,
        _ => parent == child,
    }
}

pub(crate) fn ensure_agent_handoff_transition(current: &str, next: &str) -> Result<(), AppError> {
    let allowed = matches!(
        (current, next),
        ("requested", "accepted")
            | ("requested", "rejected")
            | ("requested", "failed")
            | ("accepted", "completed")
            | ("accepted", "failed")
    );
    if allowed {
        Ok(())
    } else {
        Err(AppError::bad_request(format!(
            "cannot transition agent handoff from {current} to {next}"
        )))
    }
}

pub(crate) fn normalize_handoff_semantic_scopes(value: Value) -> Result<Value, AppError> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::bad_request(
            "handoff semantic_scopes must be a JSON object",
        ))
    }
}

pub(crate) fn default_handoff_review_status(plan: Option<&ManagerAgentPlan>) -> &'static str {
    if plan.is_some_and(|plan| plan.status == "approved" || plan.status == "reviewed") {
        "manager_reviewed"
    } else {
        "pending_review"
    }
}

pub(crate) fn normalize_handoff_review_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "pending_review"
        | "manager_reviewed"
        | "human_review_required"
        | "approved"
        | "rejected" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "review_status must be pending_review, manager_reviewed, human_review_required, approved, or rejected",
        )),
    }
}

pub(crate) fn normalize_handoff_human_escalation_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "none" | "required" | "requested" | "resolved" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "human_escalation_status must be none, required, requested, or resolved",
        )),
    }
}

pub(crate) fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn default_handoff_assignment_message(
    handoff: &AgentHandoffEvent,
    plan: &ManagerAgentPlan,
    target_agent: &Agent,
) -> String {
    let goal = plan
        .task_intake
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or("Execute the assigned manager-agent handoff.");
    format!(
        "Manager Agent assigned this task to specialist agent {}.\n\nIntent: {}\nRisk: {}\nRemote Computer required: {}\n\nGoal: {}\n\nPayload:\n{}",
        target_agent.name,
        handoff.intent,
        handoff.risk_level,
        handoff.remote_computer_required,
        goal,
        handoff.payload
    )
}

pub(crate) fn validate_handoff_token(field: &str, value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::bad_request(format!(
            "{field} must be a lowercase slug"
        )));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_handoff_schema_version(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '/'))
    {
        return Err(AppError::bad_request(
            "schema_version must be a non-empty schema identifier",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_handoff_risk_level(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "low" | "medium" | "high") {
        Ok(normalized)
    } else {
        Err(AppError::bad_request(
            "risk_level must be one of low, medium, or high",
        ))
    }
}

pub(crate) fn matching_handoff_rule<'a>(
    runtime_config: &'a Value,
    target_agent_id: Uuid,
    intent: &str,
    schema_version: &str,
    risk_level: &str,
    approval_required: bool,
) -> Result<&'a Value, AppError> {
    let rules = runtime_config
        .get("handoffs")
        .and_then(|handoffs| handoffs.get("allowed_targets"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::forbidden("source agent version has no handoff allowed_targets")
        })?;
    rules
        .iter()
        .find(|rule| {
            let rule_requires_approval =
                handoff_rule_bool(rule, "approval_required").unwrap_or(false);
            handoff_rule_target_id(rule) == Some(target_agent_id)
                && json_string_array_contains(rule.get("intents"), intent)
                && json_optional_string_array_contains(rule.get("schema_versions"), schema_version)
                && json_optional_string_array_contains(rule.get("risk_levels"), risk_level)
                && (!rule_requires_approval || approval_required)
        })
        .ok_or_else(|| {
            AppError::forbidden(
                "agent handoff target, intent, schema_version, risk_level, or approval requirement is not allowlisted",
            )
        })
}

pub(crate) fn handoff_rule_target_id(rule: &Value) -> Option<Uuid> {
    rule.get("target_agent_id")
        .or_else(|| rule.get("target_agent"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

pub(crate) fn handoff_rule_bool(rule: &Value, key: &str) -> Option<bool> {
    rule.get(key).and_then(Value::as_bool)
}

pub(crate) fn json_string_array_contains(value: Option<&Value>, expected: &str) -> bool {
    value.and_then(Value::as_array).is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .is_some_and(|value| value == "*" || value == expected)
        })
    })
}

pub(crate) fn json_optional_string_array_contains(value: Option<&Value>, expected: &str) -> bool {
    match value.and_then(Value::as_array) {
        Some(items) => items.iter().any(|item| item.as_str() == Some(expected)),
        None => true,
    }
}

pub(crate) fn validate_handoff_payload_schema(
    payload: &Value,
    schema: Option<&Value>,
) -> Result<(), AppError> {
    let Some(schema) = schema else {
        return Ok(());
    };
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(AppError::bad_request(
            "handoff payload_schema must declare type object",
        ));
    }
    let Some(payload_object) = payload.as_object() else {
        return Err(AppError::bad_request(
            "handoff payload must be a JSON object",
        ));
    };
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !payload_object.contains_key(key) {
                return Err(AppError::bad_request(format!(
                    "handoff payload missing required field {key}"
                )));
            }
        }
    }
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };
    for (key, property_schema) in properties {
        let Some(value) = payload_object.get(key) else {
            continue;
        };
        let Some(expected_type) = property_schema.get("type").and_then(Value::as_str) else {
            continue;
        };
        let matches_type = match expected_type {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            _ => {
                return Err(AppError::bad_request(format!(
                    "unsupported handoff payload_schema type {expected_type}"
                )));
            }
        };
        if !matches_type {
            return Err(AppError::bad_request(format!(
                "handoff payload field {key} must be {expected_type}"
            )));
        }
    }
    Ok(())
}

pub(crate) async fn materialize_workflow_graph_start_steps(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
    session: &Session,
    root_grant: &TaskGrant,
) -> Result<WorkflowRun, AppError> {
    let start_steps = workflow_graph_start_steps(&definition.step_graph)?;
    let mut materialized = Vec::new();
    for graph_step in start_steps {
        let step = materialize_workflow_graph_step(
            state, definition, run, session, root_grant, graph_step,
        )
        .await?;
        record_workflow_transition(
            state,
            run,
            None,
            Some(&step),
            "start",
            "materialized",
            json!({
                "source": "step_graph_start",
                "graph_step": graph_step
            }),
            empty_json_object(),
        )
        .await?;
        materialized.push(step);
    }
    let status = if materialized
        .iter()
        .any(|step| step.status == "requires_action")
    {
        "requires_action"
    } else {
        "queued"
    };
    state
        .update_workflow_run_status(run.id, status.to_string(), run.started_at, run.completed_at)
        .await
}

pub(crate) fn workflow_run_runtime_envelope(
    definition: &WorkflowDefinition,
    execution_strategy: &str,
    runtime_adapter: Option<&str>,
    runtime_mode: Option<&str>,
    external_run_ref: Option<&str>,
    request_envelope: &Value,
) -> Value {
    let mut envelope = serde_json::Map::new();
    envelope.insert("source".to_string(), json!("workflow_run"));
    envelope.insert("workflow_definition_id".to_string(), json!(definition.id));
    envelope.insert("execution_strategy".to_string(), json!(execution_strategy));
    envelope.insert("runtime_adapter".to_string(), json!(runtime_adapter));
    envelope.insert(
        "runtime_mode".to_string(),
        json!(runtime_mode.unwrap_or("normal")),
    );
    envelope.insert("external_run_ref".to_string(), json!(external_run_ref));
    envelope.insert(
        "event_ingestion_policy".to_string(),
        json!(definition.event_ingestion_policy),
    );
    envelope.insert(
        "runtime_capability_contract".to_string(),
        definition.runtime_capability_contract.clone(),
    );
    envelope.insert("request_envelope".to_string(), request_envelope.clone());
    Value::Object(envelope)
}

pub(crate) async fn workflow_run_runtime_envelope_with_pinned_ontology_release(
    state: &AppState,
    definition: &WorkflowDefinition,
    execution_strategy: &str,
    runtime_adapter: Option<&str>,
    runtime_mode: Option<&str>,
    external_run_ref: Option<&str>,
    request_envelope: &Value,
) -> Result<Value, AppError> {
    let mut runtime_envelope = workflow_run_runtime_envelope(
        definition,
        execution_strategy,
        runtime_adapter,
        runtime_mode,
        external_run_ref,
        request_envelope,
    );
    let ontology_scopes =
        workflow_definition_root_task_grant_scope(definition, "semantic_scopes", empty_json_object);
    if let Some(mut ontology_release) =
        active_ontology_release_metadata_for_scopes(state, &ontology_scopes).await?
    {
        if let Some(metadata) = ontology_release.as_object_mut() {
            metadata.insert("pinned_by".to_string(), json!("workflow_run_start"));
        }
        if let Some(envelope) = runtime_envelope.as_object_mut() {
            envelope.insert("ontology_release".to_string(), ontology_release);
        }
    }
    Ok(runtime_envelope)
}

pub(crate) async fn create_workflow_run_from_definition(
    state: &AppState,
    definition: &WorkflowDefinition,
    title: String,
    input_payload: Value,
    runtime_envelope_request: Value,
) -> Result<WorkflowRun, AppError> {
    if definition.release_state != "released" {
        return Err(AppError::bad_request(
            "workflow run requires a released workflow definition",
        ));
    }
    if let Some(environment_id) = definition.default_environment_id {
        state.get_environment(environment_id).await?;
    }
    let execution_strategy = definition.execution_strategy.clone();
    let runtime_adapter = definition.runtime_adapter.clone();
    let runtime_mode = definition.runtime_mode.clone();
    validate_workflow_execution_binding(
        &execution_strategy,
        runtime_adapter.as_deref(),
        &definition.runtime_capability_contract,
    )?;
    let session_input = CreateSession {
        agent_id: definition.default_agent_id,
        environment_id: definition.default_environment_id,
        title,
        message: None,
    };
    let session =
        match workflow_definition_agent_version_id(definition, definition.default_agent_id)? {
            Some(agent_version_id) => {
                state
                    .create_session_for_agent_version(session_input, agent_version_id)
                    .await?
            }
            None => state.create_session(session_input).await?,
        };
    ensure_primary_session_thread(state, session.id).await?;
    let now = Utc::now();
    let input_digest = workflow_input_digest(&input_payload);
    let delegation_status =
        (execution_strategy == "delegated_runtime").then_some("submitted".to_string());
    let runtime_envelope = workflow_run_runtime_envelope_with_pinned_ontology_release(
        state,
        definition,
        &execution_strategy,
        runtime_adapter.as_deref(),
        runtime_mode.as_deref(),
        None,
        &runtime_envelope_request,
    )
    .await?;
    let run = state
        .create_workflow_run(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_definition_id: definition.id,
            pack_installation_id: definition.pack_installation_id,
            source_event_id: None,
            source_work_item_id: None,
            source_schedule_id: None,
            status: "queued".to_string(),
            primary_session_id: session.id,
            root_task_grant_id: None,
            input_payload,
            input_digest,
            execution_strategy,
            runtime_adapter,
            runtime_mode,
            delegation_status,
            external_run_ref: None,
            runtime_event_cursor: None,
            runtime_envelope,
            started_at: None,
            completed_at: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let root_grant =
        issue_root_task_grant_for_workflow_run(state, &run, definition, &session).await?;
    let run = state
        .update_workflow_run_root_task_grant(run.id, root_grant.id)
        .await?;
    materialize_workflow_graph_start_steps(state, definition, &run, &session, &root_grant).await
}

pub(crate) async fn trigger_workflow_run_from_webhook(
    state: &AppState,
    workflow_definition_id: Uuid,
    input_payload: Value,
) -> Result<WorkflowRun, AppError> {
    let definition = state
        .get_workflow_definition(workflow_definition_id)
        .await?;
    create_workflow_run_from_definition(
        state,
        &definition,
        format!("Webhook: {}", definition.name),
        input_payload,
        serde_json::json!({}),
    )
    .await
}

pub(crate) async fn materialize_workflow_graph_step(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
    session: &Session,
    root_grant: &TaskGrant,
    graph_step: &Value,
) -> Result<WorkflowStepRun, AppError> {
    materialize_workflow_graph_step_with_policy_context(
        state,
        definition,
        run,
        session,
        root_grant,
        graph_step,
        "queued",
        None,
        empty_json_object(),
        empty_json_object(),
    )
    .await
}

pub(crate) async fn materialize_workflow_graph_step_with_policy_context(
    state: &AppState,
    definition: &WorkflowDefinition,
    run: &WorkflowRun,
    session: &Session,
    root_grant: &TaskGrant,
    graph_step: &Value,
    status: &str,
    scheduled_at: Option<DateTime<Utc>>,
    input_context: Value,
    output_payload: Value,
) -> Result<WorkflowStepRun, AppError> {
    let step_key = workflow_graph_step_key(graph_step)?;
    let step_type = workflow_graph_step_type(graph_step);
    let agent_id = workflow_graph_step_agent_id(definition, graph_step)?;
    let graph_agent_version_id = workflow_graph_step_agent_version_id(graph_step)?;
    let environment_id =
        workflow_graph_step_uuid(graph_step, "environment_id")?.or(session.environment_id);
    let now = Utc::now();
    let terminal = workflow_step_status_terminal(status);
    let risk_level = normalize_task_grant_risk_level(
        graph_step
            .get("risk_level")
            .and_then(Value::as_str)
            .unwrap_or("low"),
    )?;
    let approval_required = match graph_step.get("approval_required") {
        Some(value) => value.as_bool().ok_or_else(|| {
            AppError::bad_request("workflow graph step approval_required must be boolean")
        })?,
        None => false,
    };
    let approval_blocked = approval_required && !terminal;
    let isolated_handoff_context = workflow_graph_step_requires_isolated_handoff_context(
        graph_step,
        agent_id,
        session.agent_id,
    )?;
    if isolated_handoff_context && risk_level == "high" && !approval_required {
        return Err(AppError::bad_request(
            "high-risk workflow handoffs must require approval",
        ));
    }
    if (terminal || approval_blocked)
        && isolated_handoff_context
        && let Some(agent_id) = agent_id
    {
        let agent_version_id = graph_agent_version_id.ok_or_else(|| {
            AppError::bad_request(
                "workflow graph steps assigned to another agent must pin agent_version_id",
            )
        })?;
        state
            .list_agent_versions(agent_id)
            .await?
            .into_iter()
            .find(|version| version.id == agent_version_id)
            .ok_or_else(|| AppError::not_found("workflow step agent version not found"))?;
        let mut input_payload = workflow_graph_step_input_payload(run, graph_step, input_context);
        if approval_blocked && let Some(input) = input_payload.as_object_mut() {
            input.insert(
                "handoff_governance".to_string(),
                json!({
                    "source_agent_ref": graph_step.get("handoff_source_agent_ref"),
                    "intent": graph_step.get("handoff_intent"),
                    "risk_level": risk_level,
                    "approval_required": true,
                    "schema_ref": graph_step.get("handoff_schema_ref"),
                }),
            );
        }
        let step = state
            .create_workflow_step_run(WorkflowStepRun {
                id: Uuid::new_v4(),
                workflow_run_id: run.id,
                step_key,
                step_type,
                agent_id: Some(agent_id),
                agent_version_id: Some(agent_version_id),
                session_id: None,
                thread_id: None,
                handoff_id: None,
                task_grant_id: None,
                environment_id,
                status: if approval_blocked {
                    "requires_action".to_string()
                } else {
                    status.to_string()
                },
                input_payload,
                output_payload: if approval_blocked {
                    json!({"block_reason": "handoff_approval_required"})
                } else {
                    output_payload
                },
                artifact_ids: Vec::new(),
                approval_ids: Vec::new(),
                tool_call_ids: Vec::new(),
                claimed_by_worker: None,
                lease_expires_at: None,
                context_packet_id: None,
                started_at: Some(now),
                completed_at: terminal.then_some(now),
                scheduled_at,
                created_at: now,
                updated_at: now,
            })
            .await?;
        record_workflow_step_run_created(state, run, &step).await?;
        return Ok(step);
    }
    if isolated_handoff_context && let Some(agent_id) = agent_id {
        let agent_version_id = graph_agent_version_id.ok_or_else(|| {
            AppError::bad_request(
                "workflow graph steps assigned to another agent must pin agent_version_id",
            )
        })?;
        let target_agent = state.get_agent(agent_id).await?;
        let target_version = state
            .list_agent_versions(agent_id)
            .await?
            .into_iter()
            .find(|version| version.id == agent_version_id)
            .ok_or_else(|| AppError::not_found("workflow step agent version not found"))?;
        let remaining_budgets = task_grant_remaining_budgets(root_grant, now)?;
        let (connector_scope, external_effects) = child_connector_scopes_for_agent_version(
            &root_grant.connector_scope,
            &root_grant.external_effects,
            &target_version,
        )?;
        let step_id = Uuid::new_v4();
        let mut child_grant = TaskGrant {
            id: Uuid::new_v4(),
            workflow_run_id: run.id,
            workflow_step_run_id: Some(step_id),
            session_id: None,
            parent_grant_id: Some(root_grant.id),
            source_event_id: run.source_event_id,
            source_handoff_id: None,
            issuer_subject: "system".to_string(),
            grantee_agent_id: Some(agent_id),
            grantee_session_id: None,
            agent_class: Some(target_agent.agent_role.clone()),
            objective: graph_step
                .get("task")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&step_key)
                .to_string(),
            risk_level,
            status: "active".to_string(),
            expires_at: root_grant.expires_at,
            max_turns: remaining_budgets.max_turns,
            max_tool_calls: remaining_budgets.max_tool_calls,
            max_runtime_seconds: remaining_budgets.max_runtime_seconds,
            max_cost_usd_micros: remaining_budgets.max_cost_usd_micros,
            turns_used: 0,
            tool_calls_used: 0,
            cost_usd_micros_used: 0,
            semantic_scopes: root_grant.semantic_scopes.clone(),
            memory_scope: child_handoff_memory_scope(&root_grant.memory_scope),
            tool_scope: child_tool_scope_for_tools(&root_grant.tool_scope, &target_version.tools),
            connector_scope,
            approval_policy: root_grant.approval_policy.clone(),
            external_effects,
            context_packet_id: None,
            policy_revision_id: root_grant.policy_revision_id,
            immutable_args_hash: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        };
        validate_task_grant_scope_objects(&child_grant)?;
        validate_task_grant_budgets(&child_grant)?;
        ensure_child_task_grant_within_parent(root_grant, &child_grant)?;
        let child_session = state
            .create_session_for_agent_version(
                CreateSession {
                    agent_id,
                    environment_id,
                    title: format!("{} / {}", run.id, step_key),
                    message: None,
                },
                agent_version_id,
            )
            .await?;
        let child_thread = match ensure_primary_session_thread(state, child_session.id).await {
            Ok(thread) => thread,
            Err(error) => {
                let _ = set_managed_session_status(
                    state,
                    child_session.id,
                    SessionStatus::Failed,
                    "workflow child session initialization failed before thread creation",
                )
                .await;
                return Err(error);
            }
        };
        child_grant.session_id = Some(child_session.id);
        child_grant.grantee_session_id = Some(child_session.id);
        let step = WorkflowStepRun {
            id: step_id,
            workflow_run_id: run.id,
            step_key,
            step_type,
            agent_id: Some(agent_id),
            agent_version_id: Some(agent_version_id),
            session_id: Some(child_session.id),
            thread_id: Some(child_thread.id),
            handoff_id: None,
            task_grant_id: Some(child_grant.id),
            environment_id,
            status: status.to_string(),
            input_payload: workflow_graph_step_input_payload(run, graph_step, input_context),
            output_payload,
            artifact_ids: Vec::new(),
            approval_ids: Vec::new(),
            tool_call_ids: Vec::new(),
            claimed_by_worker: None,
            lease_expires_at: None,
            context_packet_id: None,
            started_at: terminal.then_some(now),
            completed_at: terminal.then_some(now),
            scheduled_at,
            created_at: now,
            updated_at: now,
        };
        let (step, child_grant) = match state
            .create_workflow_step_run_with_task_grant(step, child_grant)
            .await
        {
            Ok(created) => created,
            Err(error) => {
                let _ = set_managed_session_status(
                    state,
                    child_session.id,
                    SessionStatus::Failed,
                    "workflow child session initialization failed before step grant commit",
                )
                .await;
                return Err(error);
            }
        };
        record_task_grant_issued(state, &child_grant, run.primary_session_id).await?;
        record_workflow_step_run_created(state, run, &step).await?;
        return Ok(step);
    }
    if graph_agent_version_id.is_some_and(|version_id| session.agent_version_id != Some(version_id))
    {
        return Err(AppError::bad_request(
            "workflow graph step agent version must match its bound session version",
        ));
    }
    let agent_version_id = agent_id.and(session.agent_version_id);
    let step = state
        .create_workflow_step_run(WorkflowStepRun {
            id: Uuid::new_v4(),
            workflow_run_id: run.id,
            step_key,
            step_type,
            agent_id,
            agent_version_id,
            session_id: Some(session.id),
            thread_id: None,
            handoff_id: None,
            task_grant_id: Some(root_grant.id),
            environment_id,
            status: status.to_string(),
            input_payload: workflow_graph_step_input_payload(run, graph_step, input_context),
            output_payload,
            artifact_ids: Vec::new(),
            approval_ids: Vec::new(),
            tool_call_ids: Vec::new(),
            claimed_by_worker: None,
            lease_expires_at: None,
            context_packet_id: None,
            started_at: terminal.then_some(now),
            completed_at: terminal.then_some(now),
            scheduled_at,
            created_at: now,
            updated_at: now,
        })
        .await?;
    record_workflow_step_run_created(state, run, &step).await?;
    Ok(step)
}

pub(crate) fn workflow_graph_step_input_payload(
    run: &WorkflowRun,
    graph_step: &Value,
    input_context: Value,
) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("source".to_string(), json!("step_graph"));
    payload.insert("graph_step".to_string(), graph_step.clone());
    payload.insert("workflow_input".to_string(), run.input_payload.clone());
    payload.insert(
        "runtime_delegation".to_string(),
        json!({
            "execution_strategy": run.execution_strategy,
            "runtime_adapter": run.runtime_adapter,
            "runtime_mode": run.runtime_mode,
            "delegation_status": run.delegation_status,
            "external_run_ref": run.external_run_ref,
            "runtime_event_cursor": run.runtime_event_cursor,
            "runtime_envelope": run.runtime_envelope
        }),
    );
    if let Value::Object(context) = input_context {
        for (key, value) in context {
            payload.insert(key, value);
        }
    }
    Value::Object(payload)
}

pub(crate) fn workflow_graph_start_steps(step_graph: &Value) -> Result<Vec<&Value>, AppError> {
    let Some(steps) = step_graph.get("steps").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    if steps.is_empty() {
        return Ok(Vec::new());
    }
    let explicit_starts = steps
        .iter()
        .filter(|step| {
            step.get("start")
                .or_else(|| step.get("entrypoint"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if !explicit_starts.is_empty() {
        return Ok(explicit_starts);
    }
    Ok(vec![steps.first().ok_or_else(|| {
        AppError::bad_request("workflow graph steps cannot be empty")
    })?])
}

pub(crate) fn validate_workflow_graph_definition(step_graph: &Value) -> Result<(), AppError> {
    let Some(steps_value) = step_graph.get("steps") else {
        workflow_graph_fan_out_max_parallel(step_graph)?;
        return Ok(());
    };
    let Some(steps) = steps_value.as_array() else {
        return Err(AppError::bad_request(
            "workflow graph steps must be an array",
        ));
    };
    if steps.is_empty() {
        return Err(AppError::bad_request(
            "workflow graph steps must not be empty when provided",
        ));
    }
    let mut keys = BTreeSet::new();
    for step in steps {
        if !step.is_object() {
            return Err(AppError::bad_request(
                "workflow graph steps must be JSON objects",
            ));
        }
        let key = workflow_graph_step_key(step)?;
        if !keys.insert(key.clone()) {
            return Err(AppError::bad_request(format!(
                "workflow graph step key {key} is duplicated"
            )));
        }
    }
    for step in steps {
        let key = workflow_graph_step_key(step)?;
        let dependencies = workflow_graph_step_dependencies(step)?;
        if dependencies.iter().any(|dependency| dependency == &key) {
            return Err(AppError::bad_request(format!(
                "workflow graph step {key} cannot depend on itself"
            )));
        }
        for dependency in &dependencies {
            if !keys.contains(dependency) {
                return Err(AppError::bad_request(format!(
                    "workflow graph step {key} depends on unknown step {dependency}"
                )));
            }
        }
        for source in workflow_graph_step_failure_sources(step)? {
            if !keys.contains(&source) {
                return Err(AppError::bad_request(format!(
                    "workflow graph step {key} references unknown failure source {source}"
                )));
            }
        }
        workflow_graph_step_retry_policy(step)?;
        workflow_graph_fan_in_readiness(step, &dependencies, &HashMap::new())?;
        if let Some(condition) = step.get("condition").or_else(|| step.get("when")) {
            validate_workflow_graph_condition(condition, &keys)?;
        }
    }
    for start in workflow_graph_start_steps(step_graph)? {
        workflow_graph_step_key(start)?;
    }
    workflow_graph_fan_out_max_parallel(step_graph)?;
    Ok(())
}

pub(crate) fn validate_workflow_graph_condition(
    condition: &Value,
    step_keys: &BTreeSet<String>,
) -> Result<(), AppError> {
    if let Some(items) = condition.get("all") {
        let children = workflow_graph_condition_array(items, &[], "all")?;
        for child in children {
            validate_workflow_graph_condition(&child.condition, step_keys)?;
        }
        return Ok(());
    }
    if let Some(items) = condition.get("any") {
        let children = workflow_graph_condition_array(items, &[], "any")?;
        for child in children {
            validate_workflow_graph_condition(&child.condition, step_keys)?;
        }
        return Ok(());
    }
    if let Some(child) = condition.get("not") {
        return validate_workflow_graph_condition(child, step_keys);
    }
    let source_step = condition
        .get("source_step")
        .or_else(|| condition.get("step"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("workflow graph step condition requires source_step")
        })?;
    if !step_keys.contains(source_step) {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition references unknown source_step {source_step}"
        )));
    }
    condition
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("workflow graph step condition requires path"))?;
    workflow_graph_leaf_condition_result(condition, &Value::Null)?;
    Ok(())
}

pub(crate) fn workflow_graph_step_key(step: &Value) -> Result<String, AppError> {
    step.get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::bad_request("workflow graph step requires key"))
}

pub(crate) fn workflow_graph_step_type(graph_step: &Value) -> String {
    graph_step
        .get("type")
        .or_else(|| graph_step.get("step_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("agent")
        .to_string()
}

pub(crate) fn workflow_graph_step_is_adapter_owned_compensation(graph_step: &Value) -> bool {
    let step_type = workflow_graph_step_type(graph_step).to_ascii_lowercase();
    let type_is_adapter = matches!(
        step_type.as_str(),
        "rollback_adapter" | "compensation_adapter"
    );
    let adapter_kind = graph_step
        .get("adapter")
        .and_then(|adapter| {
            adapter
                .get("kind")
                .or_else(|| adapter.get("type"))
                .and_then(Value::as_str)
                .or_else(|| adapter.as_str())
        })
        .map(str::trim)
        .map(str::to_ascii_lowercase);
    let adapter_is_compensation = adapter_kind.as_deref().is_some_and(|kind| {
        matches!(
            kind,
            "internal_compensation" | "rollback" | "rollback_adapter" | "compensation"
        )
    });
    let explicit_flag = graph_step
        .get("rollback_adapter")
        .or_else(|| graph_step.get("adapter_owned"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let has_failure_source = graph_step
        .get("on_failure_of")
        .or_else(|| graph_step.get("compensates"))
        .or_else(|| graph_step.get("compensation_for"))
        .is_some();
    has_failure_source && (type_is_adapter || adapter_is_compensation || explicit_flag)
}

pub(crate) fn workflow_graph_step_requires_isolated_handoff_context(
    graph_step: &Value,
    target_agent_id: Option<Uuid>,
    primary_agent_id: Uuid,
) -> Result<bool, AppError> {
    let governed_handoff = match graph_step.get("handoff_source_agent_ref") {
        Some(value) => {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::bad_request(
                        "workflow graph step handoff_source_agent_ref must be a non-empty string",
                    )
                })?;
            true
        }
        None => false,
    };
    Ok(target_agent_id
        .is_some_and(|target_agent_id| target_agent_id != primary_agent_id || governed_handoff))
}

pub(crate) fn workflow_step_run_is_handoff_approval_blocked(step: &WorkflowStepRun) -> bool {
    step.status == "requires_action"
        && step
            .output_payload
            .get("block_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason == "handoff_approval_required")
}

pub(crate) fn workflow_graph_step_agent_id(
    definition: &WorkflowDefinition,
    graph_step: &Value,
) -> Result<Option<Uuid>, AppError> {
    if workflow_graph_step_is_adapter_owned_compensation(graph_step) {
        return Ok(None);
    }
    workflow_graph_step_uuid(graph_step, "agent_id")
        .map(|agent_id| agent_id.or(Some(definition.default_agent_id)))
}

pub(crate) fn workflow_graph_step_agent_version_id(
    graph_step: &Value,
) -> Result<Option<Uuid>, AppError> {
    if workflow_graph_step_is_adapter_owned_compensation(graph_step) {
        return Ok(None);
    }
    workflow_graph_step_uuid(graph_step, "agent_version_id")
}

pub(crate) fn workflow_definition_agent_version_id(
    definition: &WorkflowDefinition,
    agent_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let mut version_ids = BTreeSet::new();
    for step in definition
        .step_graph
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if workflow_graph_step_agent_id(definition, step)? == Some(agent_id)
            && let Some(version_id) = workflow_graph_step_agent_version_id(step)?
        {
            version_ids.insert(version_id);
        }
    }
    if version_ids.len() > 1 {
        return Err(AppError::bad_request(
            "workflow definition binds one agent to multiple agent versions",
        ));
    }
    Ok(version_ids.into_iter().next())
}

pub(crate) fn workflow_graph_step_dependencies(step: &Value) -> Result<Vec<String>, AppError> {
    let Some(value) = step
        .get("depends_on")
        .or_else(|| step.get("after"))
        .or_else(|| step.get("needs"))
    else {
        return Ok(Vec::new());
    };
    match value {
        Value::String(item) => Ok(normalize_optional_text(item.clone()).into_iter().collect()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .and_then(|value| normalize_optional_text(value.to_string()))
                    .ok_or_else(|| {
                        AppError::bad_request(
                            "workflow graph step dependencies must be non-empty strings",
                        )
                    })
            })
            .collect(),
        _ => Err(AppError::bad_request(
            "workflow graph step dependencies must be a string or array",
        )),
    }
}

pub(crate) fn workflow_graph_step_failure_sources(step: &Value) -> Result<Vec<String>, AppError> {
    let Some(value) = step
        .get("on_failure_of")
        .or_else(|| step.get("compensates"))
        .or_else(|| step.get("compensation_for"))
    else {
        return Ok(Vec::new());
    };
    workflow_graph_string_or_string_array(value, "workflow graph failure source")
}

pub(crate) fn workflow_graph_string_or_string_array(
    value: &Value,
    label: &str,
) -> Result<Vec<String>, AppError> {
    match value {
        Value::String(item) => Ok(normalize_optional_text(item.clone()).into_iter().collect()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .and_then(|value| normalize_optional_text(value.to_string()))
                    .ok_or_else(|| {
                        AppError::bad_request(format!("{label} must be non-empty strings"))
                    })
            })
            .collect(),
        _ => Err(AppError::bad_request(format!(
            "{label} must be a string or array"
        ))),
    }
}

pub(crate) fn workflow_graph_step_retry_policy(
    step: &Value,
) -> Result<WorkflowGraphRetryPolicy, AppError> {
    let Some(policy) = step.get("retry").or_else(|| step.get("retry_policy")) else {
        return Ok(WorkflowGraphRetryPolicy {
            max_attempts: 1,
            delay_seconds: 0,
        });
    };
    let Some(max_attempts) = policy.get("max_attempts").and_then(Value::as_u64) else {
        return Err(AppError::bad_request(
            "workflow graph step retry.max_attempts must be a positive integer",
        ));
    };
    if max_attempts == 0 {
        return Err(AppError::bad_request(
            "workflow graph step retry.max_attempts must be at least 1",
        ));
    }
    let max_attempts = usize::try_from(max_attempts).map_err(|_| {
        AppError::bad_request("workflow graph step retry.max_attempts is too large")
    })?;
    let delay_seconds = workflow_graph_retry_delay_seconds(policy)?;
    Ok(WorkflowGraphRetryPolicy {
        max_attempts,
        delay_seconds,
    })
}

pub(crate) fn workflow_graph_retry_delay_seconds(policy: &Value) -> Result<i64, AppError> {
    let direct_delay = policy
        .get("delay_seconds")
        .or_else(|| policy.get("backoff_seconds"))
        .and_then(Value::as_i64);
    let nested_delay = policy
        .get("backoff")
        .and_then(|backoff| {
            backoff
                .get("initial_seconds")
                .or_else(|| backoff.get("delay_seconds"))
                .or_else(|| backoff.get("backoff_seconds"))
        })
        .and_then(Value::as_i64);
    let delay_seconds = direct_delay.or(nested_delay).unwrap_or(0);
    if !(0..=86_400).contains(&delay_seconds) {
        return Err(AppError::bad_request(
            "workflow graph step retry delay_seconds must be between 0 and 86400",
        ));
    }
    Ok(delay_seconds)
}

pub(crate) fn workflow_graph_step_condition_evaluation(
    step: &Value,
    existing_steps: &[WorkflowStepRun],
) -> Result<Option<WorkflowGraphConditionEvaluation>, AppError> {
    let Some(condition) = step.get("condition").or_else(|| step.get("when")) else {
        return Ok(None);
    };
    workflow_graph_condition_evaluation(condition, existing_steps).map(Some)
}

pub(crate) fn workflow_graph_condition_evaluation(
    condition: &Value,
    existing_steps: &[WorkflowStepRun],
) -> Result<WorkflowGraphConditionEvaluation, AppError> {
    if let Some(items) = condition.get("all") {
        let children = workflow_graph_condition_array(items, existing_steps, "all")?;
        let matched = children.iter().all(|child| child.matched);
        return Ok(WorkflowGraphConditionEvaluation {
            condition: condition.clone(),
            source_step: children.iter().find_map(|child| child.source_step.clone()),
            path: None,
            actual: json!({
                "operator": "all",
                "matched": matched,
                "all": children
                    .iter()
                    .map(workflow_graph_condition_evaluation_payload)
                    .collect::<Vec<_>>()
            }),
            expected: json!(true),
            matched,
        });
    }
    if let Some(items) = condition.get("any") {
        let children = workflow_graph_condition_array(items, existing_steps, "any")?;
        let matched = children.iter().any(|child| child.matched);
        return Ok(WorkflowGraphConditionEvaluation {
            condition: condition.clone(),
            source_step: children.iter().find_map(|child| child.source_step.clone()),
            path: None,
            actual: json!({
                "operator": "any",
                "matched": matched,
                "any": children
                    .iter()
                    .map(workflow_graph_condition_evaluation_payload)
                    .collect::<Vec<_>>()
            }),
            expected: json!(true),
            matched,
        });
    }
    if let Some(child) = condition.get("not") {
        let child = workflow_graph_condition_evaluation(child, existing_steps)?;
        let matched = !child.matched;
        return Ok(WorkflowGraphConditionEvaluation {
            condition: condition.clone(),
            source_step: child.source_step.clone(),
            path: None,
            actual: json!({
                "operator": "not",
                "matched": matched,
                "not": workflow_graph_condition_evaluation_payload(&child)
            }),
            expected: json!(true),
            matched,
        });
    }

    let source_step_key = condition
        .get("source_step")
        .or_else(|| condition.get("step"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("workflow graph step condition requires source_step")
        })?;
    let path = condition
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("workflow graph step condition requires path"))?
        .to_string();
    let source_step = workflow_graph_latest_step(existing_steps, source_step_key).cloned();
    let actual = source_step
        .as_ref()
        .and_then(|step| workflow_graph_json_path(&step.output_payload, &path).cloned())
        .unwrap_or(Value::Null);
    let (expected, matched) = workflow_graph_leaf_condition_result(condition, &actual)?;
    Ok(WorkflowGraphConditionEvaluation {
        condition: condition.clone(),
        source_step,
        path: Some(path),
        actual,
        expected,
        matched,
    })
}

pub(crate) fn workflow_graph_condition_array(
    value: &Value,
    existing_steps: &[WorkflowStepRun],
    operator: &str,
) -> Result<Vec<WorkflowGraphConditionEvaluation>, AppError> {
    let Value::Array(items) = value else {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition {operator} must be an array"
        )));
    };
    if items.is_empty() {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition {operator} must not be empty"
        )));
    }
    items
        .iter()
        .map(|item| workflow_graph_condition_evaluation(item, existing_steps))
        .collect()
}

pub(crate) fn workflow_graph_condition_evaluation_payload(
    evaluation: &WorkflowGraphConditionEvaluation,
) -> Value {
    json!({
        "condition": evaluation.condition,
        "path": evaluation.path,
        "actual": evaluation.actual,
        "expected": evaluation.expected,
        "matched": evaluation.matched
    })
}

pub(crate) fn workflow_graph_leaf_condition_result(
    condition: &Value,
    actual: &Value,
) -> Result<(Value, bool), AppError> {
    if let Some(expected) = condition.get("equals").cloned() {
        return Ok((expected.clone(), actual == &expected));
    }
    if let Some(expected) = condition.get("not_equals").cloned() {
        return Ok((expected.clone(), actual != &expected));
    }
    if let Some(expected) = condition.get("in") {
        let Value::Array(items) = expected else {
            return Err(AppError::bad_request(
                "workflow graph step condition in must be an array",
            ));
        };
        return Ok((expected.clone(), items.iter().any(|item| item == actual)));
    }
    if let Some(expected) = condition.get("not_in") {
        let Value::Array(items) = expected else {
            return Err(AppError::bad_request(
                "workflow graph step condition not_in must be an array",
            ));
        };
        return Ok((expected.clone(), !items.iter().any(|item| item == actual)));
    }
    if let Some(expected) = condition.get("exists") {
        let Some(expected) = expected.as_bool() else {
            return Err(AppError::bad_request(
                "workflow graph step condition exists must be a boolean",
            ));
        };
        return Ok((json!(expected), (actual != &Value::Null) == expected));
    }
    for (key, comparator) in [
        ("greater_than", WorkflowGraphNumericComparator::GreaterThan),
        ("gt", WorkflowGraphNumericComparator::GreaterThan),
        (
            "greater_than_or_equals",
            WorkflowGraphNumericComparator::GreaterThanOrEquals,
        ),
        ("gte", WorkflowGraphNumericComparator::GreaterThanOrEquals),
        ("less_than", WorkflowGraphNumericComparator::LessThan),
        ("lt", WorkflowGraphNumericComparator::LessThan),
        (
            "less_than_or_equals",
            WorkflowGraphNumericComparator::LessThanOrEquals,
        ),
        ("lte", WorkflowGraphNumericComparator::LessThanOrEquals),
    ] {
        if let Some(expected) = condition.get(key).cloned() {
            let expected_number = workflow_graph_condition_number(&expected, key)?;
            let matched = workflow_graph_condition_number(actual, key)
                .map(|actual_number| comparator.matches(actual_number, expected_number))
                .unwrap_or(false);
            return Ok((expected, matched));
        }
    }
    for (key, comparator) in [
        ("after", WorkflowGraphTimeComparator::After),
        ("on_or_after", WorkflowGraphTimeComparator::OnOrAfter),
        ("before", WorkflowGraphTimeComparator::Before),
        ("on_or_before", WorkflowGraphTimeComparator::OnOrBefore),
    ] {
        if let Some(expected) = condition.get(key).cloned() {
            let expected_time = workflow_graph_condition_datetime(&expected, key)?;
            let matched = workflow_graph_condition_datetime(actual, key)
                .map(|actual_time| comparator.matches(actual_time, expected_time))
                .unwrap_or(false);
            return Ok((expected, matched));
        }
    }
    let expected = json!(true);
    Ok((expected.clone(), actual == &expected))
}

pub(crate) fn workflow_graph_condition_number(value: &Value, key: &str) -> Result<f64, AppError> {
    let Some(number) = value.as_f64() else {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition {key} must be a number"
        )));
    };
    if !number.is_finite() {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition {key} must be finite"
        )));
    }
    Ok(number)
}

pub(crate) fn workflow_graph_condition_datetime(
    value: &Value,
    key: &str,
) -> Result<DateTime<Utc>, AppError> {
    let Some(value) = value
        .as_str()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    else {
        return Err(AppError::bad_request(format!(
            "workflow graph step condition {key} must be an RFC3339 timestamp string"
        )));
    };
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| {
            AppError::bad_request(format!(
                "workflow graph step condition {key} must be an RFC3339 timestamp string"
            ))
        })
}

pub(crate) fn workflow_graph_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path
        .split('.')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        current = current.get(segment)?;
    }
    Some(current)
}

pub(crate) fn workflow_graph_step_keys(step_graph: &Value) -> Result<BTreeSet<String>, AppError> {
    let mut keys = BTreeSet::new();
    let Some(steps) = step_graph.get("steps").and_then(Value::as_array) else {
        return Ok(keys);
    };
    for step in steps {
        keys.insert(workflow_graph_step_key(step)?);
    }
    Ok(keys)
}

pub(crate) fn workflow_graph_ready_steps<'a>(
    step_graph: &'a Value,
    existing_steps: &[WorkflowStepRun],
) -> Result<Vec<WorkflowGraphReadyStep<'a>>, AppError> {
    let Some(steps) = step_graph.get("steps").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let existing_by_key = existing_steps
        .iter()
        .map(|step| (step.step_key.as_str(), step.status.as_str()))
        .collect::<HashMap<_, _>>();
    let mut ready = Vec::new();
    for graph_step in steps {
        let key = workflow_graph_step_key(graph_step)?;
        if existing_by_key.contains_key(key.as_str()) {
            continue;
        }
        let dependencies = workflow_graph_step_dependencies(graph_step)?;
        if dependencies.is_empty() {
            continue;
        }
        let fan_in = workflow_graph_fan_in_readiness(graph_step, &dependencies, &existing_by_key)?;
        if workflow_graph_fan_in_ready(&fan_in) {
            ready.push(WorkflowGraphReadyStep { graph_step, fan_in });
        }
    }
    Ok(ready)
}

pub(crate) fn workflow_graph_fan_in_readiness(
    graph_step: &Value,
    dependencies: &[String],
    existing_by_key: &HashMap<&str, &str>,
) -> Result<WorkflowGraphFanInReadiness, AppError> {
    let mode = workflow_graph_fan_in_mode(graph_step)?;
    let min_success = workflow_graph_fan_in_min_success(graph_step, &mode, dependencies.len())?;
    let mut successful_dependencies = Vec::new();
    let mut failed_dependencies = Vec::new();
    let mut pending_dependencies = Vec::new();
    for dependency in dependencies {
        match existing_by_key.get(dependency.as_str()).copied() {
            Some(status) if workflow_step_status_successful(status) => {
                successful_dependencies.push(dependency.clone());
            }
            Some("failed" | "canceled") => failed_dependencies.push(dependency.clone()),
            Some(_) | None => pending_dependencies.push(dependency.clone()),
        }
    }
    Ok(WorkflowGraphFanInReadiness {
        mode,
        min_success,
        dependencies: dependencies.to_vec(),
        successful_dependencies,
        failed_dependencies,
        pending_dependencies,
    })
}

pub(crate) fn workflow_graph_fan_in_mode(graph_step: &Value) -> Result<String, AppError> {
    let Some(policy) = graph_step.get("fan_in").or_else(|| graph_step.get("join")) else {
        return Ok("all".to_string());
    };
    let mode = match policy {
        Value::String(mode) => mode.as_str(),
        Value::Object(object) => object
            .get("mode")
            .or_else(|| object.get("strategy"))
            .and_then(Value::as_str)
            .unwrap_or("all"),
        _ => {
            return Err(AppError::bad_request(
                "workflow graph step fan_in must be a string or object",
            ));
        }
    }
    .trim()
    .to_ascii_lowercase();
    match mode.as_str() {
        "all" | "any" | "quorum" => Ok(mode),
        _ => Err(AppError::bad_request(
            "workflow graph step fan_in mode must be all, any, or quorum",
        )),
    }
}

pub(crate) fn workflow_graph_fan_in_min_success(
    graph_step: &Value,
    mode: &str,
    dependency_count: usize,
) -> Result<usize, AppError> {
    if mode == "all" {
        return Ok(dependency_count);
    }
    if mode == "any" {
        return Ok(1);
    }
    let min_success = graph_step
        .get("fan_in")
        .and_then(|policy| policy.get("min_success").or_else(|| policy.get("quorum")))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            AppError::bad_request("workflow graph step fan_in quorum requires min_success")
        })?;
    let min_success = usize::try_from(min_success).map_err(|_| {
        AppError::bad_request("workflow graph step fan_in min_success is too large")
    })?;
    if min_success == 0 || min_success > dependency_count {
        return Err(AppError::bad_request(
            "workflow graph step fan_in min_success must be between 1 and dependency count",
        ));
    }
    Ok(min_success)
}

pub(crate) fn workflow_graph_fan_in_ready(fan_in: &WorkflowGraphFanInReadiness) -> bool {
    fan_in.successful_dependencies.len() >= fan_in.min_success
}

pub(crate) fn workflow_graph_fan_in_payload(fan_in: &WorkflowGraphFanInReadiness) -> Value {
    json!({
        "mode": fan_in.mode,
        "min_success": fan_in.min_success,
        "dependencies": fan_in.dependencies,
        "successful_dependencies": fan_in.successful_dependencies,
        "failed_dependencies": fan_in.failed_dependencies,
        "pending_dependencies": fan_in.pending_dependencies
    })
}

pub(crate) fn workflow_graph_fan_out_max_parallel(
    step_graph: &Value,
) -> Result<Option<usize>, AppError> {
    let Some(policy) = step_graph
        .get("fan_out")
        .or_else(|| step_graph.get("fanout"))
    else {
        return Ok(None);
    };
    let max_parallel = match policy {
        Value::Object(object) => object
            .get("max_parallel")
            .or_else(|| object.get("parallelism"))
            .and_then(Value::as_u64),
        Value::Number(number) => number.as_u64(),
        _ => {
            return Err(AppError::bad_request(
                "workflow graph fan_out must be an object or positive integer",
            ));
        }
    }
    .ok_or_else(|| AppError::bad_request("workflow graph fan_out.max_parallel is required"))?;
    if max_parallel == 0 {
        return Err(AppError::bad_request(
            "workflow graph fan_out.max_parallel must be at least 1",
        ));
    }
    usize::try_from(max_parallel)
        .map(Some)
        .map_err(|_| AppError::bad_request("workflow graph fan_out.max_parallel is too large"))
}

pub(crate) fn workflow_graph_active_parallel_count(existing_steps: &[WorkflowStepRun]) -> usize {
    existing_steps
        .iter()
        .filter(|step| {
            !workflow_step_status_terminal(&step.status)
                && matches!(
                    step.status.as_str(),
                    "queued" | "scheduled" | "running" | "requires_action"
                )
        })
        .count()
}

pub(crate) fn workflow_graph_fan_out_payload(
    max_parallel: Option<usize>,
    active_parallel_count: usize,
) -> Value {
    json!({
        "max_parallel": max_parallel,
        "active_parallel_count": active_parallel_count
    })
}

pub(crate) fn workflow_graph_step_by_key<'a>(
    step_graph: &'a Value,
    step_key: &str,
) -> Result<Option<&'a Value>, AppError> {
    let Some(steps) = step_graph.get("steps").and_then(Value::as_array) else {
        return Ok(None);
    };
    for step in steps {
        if workflow_graph_step_key(step)? == step_key {
            return Ok(Some(step));
        }
    }
    Ok(None)
}

pub(crate) fn workflow_graph_step_uuid(step: &Value, key: &str) -> Result<Option<Uuid>, AppError> {
    step.get(key)
        .and_then(Value::as_str)
        .map(|value| {
            Uuid::parse_str(value.trim()).map_err(|_| {
                AppError::bad_request(format!("workflow graph step {key} must be a UUID"))
            })
        })
        .transpose()
}
