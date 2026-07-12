use std::collections::BTreeSet;

use axum::http::HeaderMap;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn authorize_dynamic_workflow_plan_read(
    state: &AppState,
    headers: &HeaderMap,
    plan: &DynamicWorkflowPlan,
) -> Result<(), AppError> {
    if let Some(session_id) = plan.source_session_id {
        authorize_request(
            state,
            headers,
            Permission::SessionsRead,
            "session",
            Some(session_id),
        )
        .await
    } else {
        authorize_request(
            state,
            headers,
            Permission::SessionsRead,
            "dynamic_workflow_plan",
            Some(plan.id),
        )
        .await
    }
}

pub(crate) async fn authorize_dynamic_workflow_plan_run(
    state: &AppState,
    headers: &HeaderMap,
    plan: &DynamicWorkflowPlan,
) -> Result<(), AppError> {
    if let Some(session_id) = plan.source_session_id {
        authorize_request(
            state,
            headers,
            Permission::SessionsRun,
            "session",
            Some(session_id),
        )
        .await
    } else {
        authorize_request(
            state,
            headers,
            Permission::SessionsRun,
            "dynamic_workflow_plan",
            Some(plan.id),
        )
        .await
    }
}

pub(crate) async fn record_dynamic_workflow_plan_audit(
    state: &AppState,
    plan: &DynamicWorkflowPlan,
    action: &str,
    details: Value,
) -> Result<AuditLog, AppError> {
    let audit = state
        .append_audit_log(new_audit_log(
            plan.source_session_id,
            "system",
            Some(plan.id),
            action,
            "dynamic_workflow_plan",
            Some(plan.id),
            json!({
                "dynamic_workflow_plan_id": plan.id,
                "source_work_item_id": plan.source_work_item_id,
                "source_session_id": plan.source_session_id,
                "objective": plan.objective,
                "status": plan.status,
                "details": details.clone()
            }),
        ))
        .await?;
    if let Some(session_id) = plan.source_session_id {
        state
            .append_event(
                "system",
                Some(plan.id),
                session_id,
                action,
                json!({
                    "dynamic_workflow_plan_id": plan.id,
                    "objective": plan.objective,
                    "status": plan.status,
                    "details": details.clone()
                }),
            )
            .await?;
    }
    if let Some(work_item_id) = plan.source_work_item_id {
        state
            .append_work_item_activity_entry(
                work_item_id,
                action,
                None,
                Some("dynamic_workflow_plan"),
                Some(plan.id),
                format!("Dynamic workflow plan {} {}", plan.objective, plan.status),
                json!({
                    "dynamic_workflow_plan_id": plan.id,
                    "status": plan.status,
                    "details": details.clone()
                }),
            )
            .await?;
    }
    Ok(audit)
}

pub(crate) fn normalize_manager_plan_risk(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "risk_classification must be one of low, medium, or high",
        )),
    }
}

pub(crate) fn normalize_manager_plan_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "planned" | "reviewed" | "approved" | "needs_changes" | "blocked" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "manager agent plan status must be planned, reviewed, approved, needs_changes, or blocked",
        )),
    }
}

pub(crate) fn normalize_dynamic_workflow_plan_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "proposed" | "reviewed" | "approved" | "rejected" | "materialized" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "dynamic workflow plan status must be proposed, reviewed, approved, rejected, or materialized",
        )),
    }
}

pub(crate) fn validate_dynamic_workflow_phases(phases: Value) -> Result<Value, AppError> {
    let items = phases.as_array().ok_or_else(|| {
        AppError::bad_request("dynamic workflow plan phases must be a JSON array")
    })?;
    if items.is_empty() {
        return Err(AppError::bad_request(
            "dynamic workflow plan phases must not be empty",
        ));
    }
    let mut keys = BTreeSet::new();
    for (index, phase) in items.iter().enumerate() {
        let object = phase.as_object().ok_or_else(|| {
            AppError::bad_request("dynamic workflow plan phases must be JSON objects")
        })?;
        let key = dynamic_phase_key(phase, index)?;
        if !keys.insert(key.clone()) {
            return Err(AppError::bad_request(format!(
                "dynamic workflow plan phase key {key} is duplicated"
            )));
        }
        if dynamic_phase_count(phase)? == 0 {
            return Err(AppError::bad_request(format!(
                "dynamic workflow plan phase {key} agent count must be at least 1"
            )));
        }
        if object
            .get("prompt")
            .or_else(|| object.get("objective"))
            .and_then(Value::as_str)
            .and_then(|value| normalize_optional_text(value.to_string()))
            .is_none()
        {
            return Err(AppError::bad_request(format!(
                "dynamic workflow plan phase {key} requires prompt or objective"
            )));
        }
    }
    Ok(phases)
}

pub(crate) fn validate_dynamic_workflow_agent_fleet_policy(
    policy: Value,
) -> Result<Value, AppError> {
    if !policy.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow agent_fleet_policy must be a JSON object",
        ));
    }
    let max_total = dynamic_policy_u64(&policy, "max_total_agents", 64)?;
    let max_parallel = dynamic_policy_u64(&policy, "max_parallel_agents", 16)?;
    if max_total == 0 || max_total > 1000 {
        return Err(AppError::bad_request(
            "dynamic workflow max_total_agents must be between 1 and 1000",
        ));
    }
    if max_parallel == 0 || max_parallel > 16 {
        return Err(AppError::bad_request(
            "dynamic workflow max_parallel_agents must be between 1 and 16",
        ));
    }
    if max_parallel > max_total {
        return Err(AppError::bad_request(
            "dynamic workflow max_parallel_agents cannot exceed max_total_agents",
        ));
    }
    Ok(policy)
}

pub(crate) fn validate_dynamic_workflow_governance(governance: Value) -> Result<Value, AppError> {
    if !governance.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow governance must be a JSON object",
        ));
    }
    for field in [
        "memory_scope",
        "tool_scope",
        "connector_scope",
        "approval_policy",
        "external_effects",
    ] {
        if let Some(value) = governance.get(field)
            && !value.is_object()
        {
            return Err(AppError::bad_request(format!(
                "dynamic workflow governance.{field} must be a JSON object"
            )));
        }
    }
    if let Some(risk) = governance.get("risk_level").and_then(Value::as_str) {
        normalize_task_grant_risk_level(risk)?;
    }
    Ok(governance)
}

pub(crate) fn validate_dynamic_workflow_validation(validation: Value) -> Result<Value, AppError> {
    if !validation.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow validation must be a JSON object",
        ));
    }
    if validation
        .get("cross_check_required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && validation
            .get("vote_threshold")
            .and_then(Value::as_f64)
            .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold))
    {
        return Err(AppError::bad_request(
            "dynamic workflow validation.vote_threshold must be between 0 and 1",
        ));
    }
    Ok(validation)
}

pub(crate) fn validate_dynamic_workflow_materialization(
    materialization: Value,
) -> Result<Value, AppError> {
    if !materialization.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow materialization must be a JSON object",
        ));
    }
    let strategy = materialization
        .get("execution_strategy")
        .and_then(Value::as_str)
        .unwrap_or("delegated_runtime");
    let strategy = normalize_workflow_execution_strategy(strategy)?;
    let adapter = materialization
        .get("runtime_adapter")
        .and_then(Value::as_str)
        .map(str::to_string);
    let adapter = normalize_optional_runtime_adapter(adapter)?;
    let contract = materialization
        .get("runtime_capability_contract")
        .cloned()
        .unwrap_or_else(empty_json_object);
    validate_workflow_execution_binding(&strategy, adapter.as_deref(), &contract)?;
    if let Some(mode) = materialization.get("runtime_mode").and_then(Value::as_str) {
        normalize_runtime_mode(mode)?;
    }
    Ok(materialization)
}

pub(crate) fn compile_dynamic_workflow_phases(
    objective: &str,
    max_total_agents: u64,
    max_parallel_agents: u64,
) -> Value {
    if max_total_agents == 1 {
        return json!([
            {
                "key": "synthesize",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": format!("Investigate and synthesize a concise final report for this objective: {objective}"),
                "validation_strategy": {
                    "artifact": "final_report",
                    "vote": true
                }
            }
        ]);
    }
    if max_total_agents == 2 {
        return json!([
            {
                "key": "survey",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": format!("Investigate this objective and produce cited findings: {objective}"),
                "validation_strategy": {
                    "artifact": "survey_findings",
                    "vote": true
                }
            },
            {
                "key": "synthesize",
                "after": "survey",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": "Synthesize findings into the final report and list uncertain claims.",
                "validation_strategy": {
                    "artifact": "final_report",
                    "vote": true
                }
            }
        ]);
    }
    let review_agents = if max_total_agents >= 4 { 2 } else { 1 };
    let research_agents = max_total_agents
        .saturating_sub(review_agents + 1)
        .clamp(1, 6);
    json!([
        {
            "key": "survey",
            "agent_count": research_agents,
            "max_parallel": max_parallel_agents.min(research_agents),
            "prompt": format!("Independently investigate this objective and produce cited findings: {objective}"),
            "validation_strategy": {
                "artifact": "survey_findings",
                "vote": true
            }
        },
        {
            "key": "cross_check",
            "after": "survey",
            "agent_count": review_agents,
            "max_parallel": max_parallel_agents.min(review_agents),
            "prompt": "Cross-check the survey findings, identify contradictions, and vote pass/fail with reasons.",
            "validation_strategy": {
                "artifact": "cross_check_report",
                "vote": true
            }
        },
        {
            "key": "synthesize",
            "after": "cross_check",
            "agent_count": 1,
            "max_parallel": 1,
            "prompt": "Synthesize accepted findings into the final report and list rejected or uncertain claims.",
            "validation_strategy": {
                "artifact": "final_report",
                "vote": true
            }
        }
    ])
}

pub(crate) fn compile_native_dynamic_workflow_phases(
    objective: &str,
    max_total_agents: u64,
    max_parallel_agents: u64,
) -> Value {
    if max_total_agents == 1 {
        return json!([
            {
                "key": "dynamic_feedback_loop",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": format!("Implement, evaluate, repair, test, and gate this objective as a single bounded dynamic workflow loop: {objective}"),
                "validation_strategy": {
                    "artifact": "integration_gate_decision",
                    "required_paths": ["/integration/no_gap"],
                    "vote": true
                }
            }
        ]);
    }
    if max_total_agents == 2 {
        return json!([
            {
                "key": "implement",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": format!("Implement the requested milestone and emit structured evidence: {objective}"),
                "validation_strategy": {
                    "artifact": "implementation_result",
                    "vote": true
                }
            },
            {
                "key": "evaluate_and_gate",
                "after": "implement",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": "Evaluate implementation evidence, report gaps/issues, and make the integration gate decision.",
                "validation_strategy": {
                    "artifact": "integration_gate_decision",
                    "required_paths": ["/evaluation/found_gap", "/evaluation/found_issues", "/integration/no_gap"],
                    "vote": true
                }
            }
        ]);
    }
    if max_total_agents == 3 {
        return json!([
            {
                "key": "implement",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": format!("Implement the requested milestone and emit structured evidence: {objective}"),
                "validation_strategy": {
                    "artifact": "implementation_result",
                    "vote": true
                }
            },
            {
                "key": "implementation_evaluator",
                "after": "implement",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": "Evaluate implementation evidence and route found gaps or issues.",
                "validation_strategy": {
                    "artifact": "implementation_evaluation",
                    "required_paths": ["/evaluation/found_gap", "/evaluation/found_issues"],
                    "vote": true
                }
            },
            {
                "key": "integration_gate",
                "after": "implementation_evaluator",
                "agent_count": 1,
                "max_parallel": 1,
                "prompt": "Run integration validation and make the final gate decision.",
                "validation_strategy": {
                    "artifact": "integration_gate_decision",
                    "required_paths": ["/integration/no_gap"],
                    "vote": true
                }
            }
        ]);
    }
    let repair_agents = max_total_agents.saturating_sub(3);
    json!([
        {
            "key": "implement",
            "agent_count": 1,
            "max_parallel": 1,
            "prompt": format!("Implement the requested milestone and emit structured evidence: {objective}"),
            "validation_strategy": {
                "artifact": "implementation_result",
                "vote": true
            }
        },
        {
            "key": "implementation_evaluator",
            "after": "implement",
            "agent_count": 1,
            "max_parallel": 1,
            "prompt": "Evaluate implementation evidence and route found gaps or issues.",
            "validation_strategy": {
                "artifact": "implementation_evaluation",
                "required_paths": ["/evaluation/found_gap", "/evaluation/found_issues"],
                "vote": true
            }
        },
        {
            "key": "repair_loop",
            "after": "implementation_evaluator",
            "agent_count": repair_agents,
            "max_parallel": max_parallel_agents.min(repair_agents),
            "prompt": "Close evaluator gaps, troubleshoot reported issues, and emit repair evidence.",
            "validation_strategy": {
                "artifact": "repair_or_troubleshooting_report",
                "vote": true
            }
        },
        {
            "key": "integration_gate",
            "after": "repair_loop",
            "agent_count": 1,
            "max_parallel": 1,
            "prompt": "Run integration validation and make the final gate decision.",
            "validation_strategy": {
                "artifact": "integration_gate_decision",
                "required_paths": ["/integration/no_gap"],
                "vote": true
            }
        }
    ])
}

pub(crate) fn dynamic_workflow_step_vote(output_payload: &Value) -> Option<bool> {
    output_payload
        .pointer("/validation/vote")
        .or_else(|| output_payload.pointer("/delegated_runtime/validation/vote"))
        .or_else(|| output_payload.pointer("/delegated_runtime/poll/vote"))
        .and_then(Value::as_bool)
}

pub(crate) fn analyze_dynamic_workflow_plan(
    phases: &Value,
    agent_fleet_policy: &Value,
    governance: &Value,
    validation: &Value,
    materialization: &Value,
) -> Result<Value, AppError> {
    let phase_items = phases.as_array().ok_or_else(|| {
        AppError::bad_request("dynamic workflow plan phases must be a JSON array")
    })?;
    let max_total = dynamic_policy_u64(agent_fleet_policy, "max_total_agents", 64)?;
    let max_parallel = dynamic_policy_u64(agent_fleet_policy, "max_parallel_agents", 16)?;
    let mut total_agents = 0u64;
    let mut max_phase_parallel = 0u64;
    let mut phase_summaries = Vec::new();
    for (index, phase) in phase_items.iter().enumerate() {
        let key = dynamic_phase_key(phase, index)?;
        let count = dynamic_phase_count(phase)?;
        let parallel = dynamic_phase_max_parallel(phase, max_parallel)?;
        total_agents += count;
        max_phase_parallel = max_phase_parallel.max(parallel);
        phase_summaries.push(json!({
            "key": key,
            "agent_count": count,
            "max_parallel": parallel,
            "depends_on": phase.get("depends_on").or_else(|| phase.get("after")).cloned().unwrap_or_else(|| json!([])),
            "validation_strategy": phase.get("validation_strategy").cloned().unwrap_or_else(|| validation.clone())
        }));
    }
    if total_agents > max_total {
        return Err(AppError::bad_request(format!(
            "dynamic workflow plan requests {total_agents} agents but max_total_agents is {max_total}"
        )));
    }
    if max_phase_parallel > max_parallel {
        return Err(AppError::bad_request(format!(
            "dynamic workflow plan max phase parallelism {max_phase_parallel} exceeds max_parallel_agents {max_parallel}"
        )));
    }
    Ok(json!({
        "status": "ready_for_review",
        "phase_count": phase_items.len(),
        "total_agent_count": total_agents,
        "max_parallel_agents": max_parallel,
        "max_phase_parallel": max_phase_parallel,
        "cross_check_required": validation.get("cross_check_required").and_then(Value::as_bool).unwrap_or(false),
        "risk_level": governance.get("risk_level").and_then(Value::as_str).unwrap_or("medium"),
        "execution_strategy": materialization.get("execution_strategy").and_then(Value::as_str).unwrap_or("delegated_runtime"),
        "runtime_adapter": materialization.get("runtime_adapter").cloned().unwrap_or(Value::Null),
        "runtime_mode": materialization.get("runtime_mode").cloned().unwrap_or(Value::Null),
        "phases": phase_summaries
    }))
}

pub(crate) fn dynamic_policy_u64(
    policy: &Value,
    key: &str,
    default_value: u64,
) -> Result<u64, AppError> {
    match policy.get(key) {
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
            AppError::bad_request(format!("dynamic workflow {key} must be a positive integer"))
        }),
        Some(_) => Err(AppError::bad_request(format!(
            "dynamic workflow {key} must be a positive integer"
        ))),
        None => Ok(default_value),
    }
}

pub(crate) fn dynamic_phase_key(phase: &Value, index: usize) -> Result<String, AppError> {
    phase
        .get("key")
        .or_else(|| phase.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_optional_text(value.to_string()))
        .or_else(|| Some(format!("phase-{}", index + 1)))
        .ok_or_else(|| AppError::bad_request("dynamic workflow phase requires key"))
}

pub(crate) fn dynamic_phase_count(phase: &Value) -> Result<u64, AppError> {
    match phase
        .get("agent_count")
        .or_else(|| phase.get("count"))
        .or_else(|| phase.get("agents"))
    {
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
            AppError::bad_request("dynamic workflow phase agent_count must be a positive integer")
        }),
        Some(Value::Array(agents)) => Ok(agents.len() as u64),
        Some(_) => Err(AppError::bad_request(
            "dynamic workflow phase agent_count must be a positive integer or agents array",
        )),
        None => Ok(1),
    }
}

pub(crate) fn dynamic_phase_max_parallel(
    phase: &Value,
    default_value: u64,
) -> Result<u64, AppError> {
    match phase
        .get("max_parallel")
        .or_else(|| phase.get("parallelism"))
    {
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
            AppError::bad_request("dynamic workflow phase max_parallel must be a positive integer")
        }),
        Some(_) => Err(AppError::bad_request(
            "dynamic workflow phase max_parallel must be a positive integer",
        )),
        None => Ok(default_value.min(dynamic_phase_count(phase)?)),
    }
}

pub(crate) fn dynamic_workflow_plan_execution_strategy(
    plan: &DynamicWorkflowPlan,
) -> Result<String, AppError> {
    normalize_workflow_execution_strategy(
        plan.materialization
            .get("execution_strategy")
            .and_then(Value::as_str)
            .unwrap_or("delegated_runtime"),
    )
}

pub(crate) fn dynamic_workflow_plan_runtime_adapter(
    plan: &DynamicWorkflowPlan,
) -> Result<Option<String>, AppError> {
    normalize_optional_runtime_adapter(
        plan.materialization
            .get("runtime_adapter")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    )
}

pub(crate) fn dynamic_workflow_plan_runtime_mode(
    plan: &DynamicWorkflowPlan,
) -> Result<Option<String>, AppError> {
    normalize_optional_runtime_mode(
        plan.materialization
            .get("runtime_mode")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    )
}

pub(crate) fn dynamic_workflow_plan_runtime_capability_contract(
    plan: &DynamicWorkflowPlan,
) -> Result<Value, AppError> {
    let contract = plan
        .materialization
        .get("runtime_capability_contract")
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "max_total_agents": plan.agent_fleet_policy.get("max_total_agents").cloned().unwrap_or(json!(64)),
                "max_parallel_agents": plan.agent_fleet_policy.get("max_parallel_agents").cloned().unwrap_or(json!(16)),
                "validation": plan.validation
            })
        });
    if !contract.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow runtime_capability_contract must be a JSON object",
        ));
    }
    Ok(contract)
}

pub(crate) fn dynamic_workflow_plan_event_ingestion_policy(
    plan: &DynamicWorkflowPlan,
) -> Result<String, AppError> {
    normalize_event_ingestion_policy(
        plan.materialization
            .get("event_ingestion_policy")
            .and_then(Value::as_str)
            .unwrap_or("normalized"),
    )
}

pub(crate) fn validate_dynamic_workflow_plan_approval_review(
    review: &Value,
) -> Result<String, AppError> {
    review
        .get("approved_by")
        .and_then(Value::as_str)
        .and_then(|value| normalize_optional_text(value.to_string()))
        .ok_or_else(|| {
            AppError::bad_request("approved dynamic workflow plan review requires approved_by")
        })
}

pub(crate) fn dynamic_workflow_plan_materialization_approval(
    plan: &DynamicWorkflowPlan,
    agent_id: Uuid,
    agent_version_id: Uuid,
    environment_id: Option<Uuid>,
) -> Result<Value, AppError> {
    if plan.status != "approved" {
        return Err(AppError::bad_request(
            "dynamic workflow plan must be approved before materialization",
        ));
    }
    let approved_by = validate_dynamic_workflow_plan_approval_review(&plan.review)?;
    let reviewed_at = plan.reviewed_at.ok_or_else(|| {
        AppError::bad_request("approved dynamic workflow plan requires reviewed_at evidence")
    })?;
    let review_audit_trace_id = plan.audit_trace_id.ok_or_else(|| {
        AppError::bad_request("approved dynamic workflow plan requires review audit evidence")
    })?;
    Ok(json!({
        "scope": "single_materialization",
        "dynamic_workflow_plan_id": plan.id,
        "status": plan.status,
        "approved_by": approved_by,
        "review": plan.review,
        "reviewed_at": reviewed_at,
        "review_audit_trace_id": review_audit_trace_id,
        "agent_id": agent_id,
        "agent_version_id": agent_version_id,
        "environment_id": environment_id,
        "reusable_workflow_release_state": "staged"
    }))
}

pub(crate) fn bind_dynamic_workflow_agent_version(
    step_graph: &mut Value,
    agent_id: Uuid,
    agent_version_id: Uuid,
) -> Result<(), AppError> {
    let steps = step_graph
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| AppError::bad_request("dynamic workflow step_graph requires steps"))?;
    for step in steps {
        if workflow_graph_step_is_adapter_owned_compensation(step) {
            continue;
        }
        let step_key = step
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let explicit_agent_id = match step.get("agent_id") {
            Some(Value::String(value)) => Some(Uuid::parse_str(value).map_err(|_| {
                AppError::bad_request(format!(
                    "dynamic workflow step {step_key} has invalid agent_id"
                ))
            })?),
            Some(Value::Null) | None => None,
            Some(_) => {
                return Err(AppError::bad_request(format!(
                    "dynamic workflow step {step_key} agent_id must be a UUID string"
                )));
            }
        };
        if explicit_agent_id.is_some_and(|value| value != agent_id) {
            return Err(AppError::bad_request(format!(
                "dynamic workflow step {step_key} cannot target an unversioned secondary agent"
            )));
        }
        let step = step.as_object_mut().ok_or_else(|| {
            AppError::bad_request("dynamic workflow step_graph steps must be JSON objects")
        })?;
        step.insert("agent_id".to_string(), json!(agent_id));
        step.insert("agent_version_id".to_string(), json!(agent_version_id));
    }
    Ok(())
}

pub(crate) fn dynamic_workflow_plan_handoff_rules(
    plan: &DynamicWorkflowPlan,
    materialization_approval: Value,
) -> Value {
    json!({
        "source": "dynamic_workflow_plan",
        "dynamic_workflow_plan_id": plan.id,
        "objective": plan.objective,
        "materialization_approval": materialization_approval,
        "root_task_grant": {
            "semantic_scopes": plan.governance.get("semantic_scopes").cloned().unwrap_or_else(empty_json_object),
            "memory_scope": plan.governance.get("memory_scope").cloned().unwrap_or_else(default_task_grant_memory_scope),
            "tool_scope": plan.governance.get("tool_scope").cloned().unwrap_or_else(default_task_grant_tool_scope),
            "connector_scope": plan.governance.get("connector_scope").cloned().unwrap_or_else(default_task_grant_connector_scope),
            "approval_policy": plan.governance.get("approval_policy").cloned().unwrap_or_else(default_task_grant_approval_policy),
            "external_effects": plan.governance.get("external_effects").cloned().unwrap_or_else(default_task_grant_external_effects),
            "risk_level": plan.governance.get("risk_level").cloned().unwrap_or(json!("medium")),
            "max_turns": plan.agent_fleet_policy.get("max_total_agents").cloned().unwrap_or(json!(64)),
            "max_runtime_seconds": plan.agent_fleet_policy.get("timeout_seconds").cloned().unwrap_or(json!(3600))
        },
        "validation": plan.validation,
        "agent_fleet_policy": plan.agent_fleet_policy
    })
}

pub(crate) fn dynamic_workflow_plan_step_graph(
    plan: &DynamicWorkflowPlan,
    execution_strategy: &str,
) -> Result<Value, AppError> {
    if execution_strategy == "delegated_runtime" {
        return Ok(workflow_definition_step_graph_for_execution(
            execution_strategy,
            &empty_json_object(),
        ));
    }
    if execution_strategy == "native_dynamic" {
        return dynamic_workflow_native_dynamic_step_graph(plan);
    }
    let phases = plan.phases.as_array().ok_or_else(|| {
        AppError::bad_request("dynamic workflow plan phases must be a JSON array")
    })?;
    let mut steps = Vec::new();
    let mut previous_phase_keys: Vec<String> = Vec::new();
    let mut phase_step_keys: HashMap<String, Vec<String>> = HashMap::new();
    for (index, phase) in phases.iter().enumerate() {
        let phase_key = workflow_slug(&dynamic_phase_key(phase, index)?);
        let count = dynamic_phase_count(phase)?;
        let explicit_dependencies = dynamic_phase_dependencies(phase)?;
        let dependencies = if explicit_dependencies.is_empty() {
            previous_phase_keys.clone()
        } else {
            explicit_dependencies
                .into_iter()
                .flat_map(|dependency| {
                    phase_step_keys
                        .get(&dependency)
                        .cloned()
                        .unwrap_or_else(|| vec![dependency])
                })
                .collect()
        };
        let mut current_phase_keys = Vec::new();
        for agent_index in 0..count {
            let step_key = if count == 1 {
                phase_key.clone()
            } else {
                format!("{}-{}", phase_key, agent_index + 1)
            };
            current_phase_keys.push(step_key.clone());
            let mut step = serde_json::Map::new();
            step.insert("key".to_string(), json!(step_key));
            step.insert("type".to_string(), json!("agent"));
            step.insert("dynamic_workflow_plan_id".to_string(), json!(plan.id));
            step.insert("phase_key".to_string(), json!(phase_key));
            step.insert("phase_index".to_string(), json!(index));
            step.insert("agent_index".to_string(), json!(agent_index));
            step.insert(
                "agent_role".to_string(),
                phase
                    .get("agent_role")
                    .cloned()
                    .unwrap_or_else(|| json!("specialist")),
            );
            step.insert(
                "input".to_string(),
                json!({
                    "objective": phase.get("objective").or_else(|| phase.get("prompt")).cloned().unwrap_or_else(|| json!(plan.objective)),
                    "prompt": phase.get("prompt").cloned().unwrap_or_else(|| json!(plan.objective)),
                    "validation_strategy": phase.get("validation_strategy").cloned().unwrap_or_else(|| plan.validation.clone()),
                    "output_schema": phase.get("output_schema").cloned().unwrap_or(Value::Null)
                }),
            );
            if index == 0 && dependencies.is_empty() {
                step.insert("start".to_string(), json!(true));
            } else if !dependencies.is_empty() {
                step.insert("depends_on".to_string(), json!(dependencies));
            }
            steps.push(Value::Object(step));
        }
        phase_step_keys.insert(phase_key, current_phase_keys.clone());
        previous_phase_keys = current_phase_keys;
    }
    Ok(json!({
        "source": "dynamic_workflow_plan",
        "dynamic_workflow_plan_id": plan.id,
        "fan_out": {
            "max_parallel": dynamic_policy_u64(&plan.agent_fleet_policy, "max_parallel_agents", 16)?
        },
        "steps": steps
    }))
}

pub(crate) fn dynamic_workflow_native_dynamic_step_graph(
    plan: &DynamicWorkflowPlan,
) -> Result<Value, AppError> {
    let max_parallel = dynamic_policy_u64(&plan.agent_fleet_policy, "max_parallel_agents", 16)?;
    let objective = plan.objective.clone();
    Ok(json!({
        "source": "dynamic_workflow_plan",
        "dynamic_workflow_plan_id": plan.id,
        "fan_out": {
            "max_parallel": max_parallel
        },
        "dynamic_loop": {
            "pattern": "implement_evaluate_repair_integrate_gate",
            "entry_step": "implement",
            "terminal_success_step": "integration-gate-keeper",
            "terminal_error_step": "unexpected-error",
            "feedback_routes": [
                {
                    "from": "implementation-evaluator",
                    "when": "/evaluation/found_gap == true",
                    "to": "developer"
                },
                {
                    "from": "implementation-evaluator",
                    "when": "/evaluation/found_issues == true",
                    "to": "troubleshooter"
                },
                {
                    "from": "integration-tester",
                    "when": "/integration/no_gap == true",
                    "to": "integration-gate-keeper"
                }
            ],
            "error_routes": [
                {
                    "from": ["implement", "developer", "troubleshooter", "integration-tester"],
                    "to": "unexpected-error"
                }
            ]
        },
        "steps": [
            {
                "key": "implement",
                "type": "agent",
                "start": true,
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "implementer",
                "input": {
                    "objective": objective,
                    "prompt": format!("Implement the requested milestone and emit structured evidence for this objective: {objective}"),
                    "validation_strategy": {
                        "artifact": "implementation_result",
                        "expected_paths": ["/implementation/summary", "/implementation/evidence_refs"]
                    }
                }
            },
            {
                "key": "implementation-evaluator",
                "type": "agent",
                "depends_on": ["implement"],
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "implementation_evaluator",
                "input": {
                    "objective": objective,
                    "prompt": "Evaluate the implementation evidence. Return /evaluation/found_gap, /evaluation/found_issues, and cited reasons.",
                    "validation_strategy": {
                        "artifact": "implementation_evaluation",
                        "required_paths": ["/evaluation/found_gap", "/evaluation/found_issues"],
                        "vote": true
                    }
                }
            },
            {
                "key": "developer",
                "type": "agent",
                "depends_on": ["implementation-evaluator"],
                "condition": {
                    "source_step": "implementation-evaluator",
                    "path": "/evaluation/found_gap",
                    "equals": true
                },
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "developer",
                "input": {
                    "objective": objective,
                    "prompt": "Close evaluator-identified implementation gaps and emit patch evidence.",
                    "validation_strategy": {
                        "artifact": "gap_fix_report",
                        "vote": true
                    }
                }
            },
            {
                "key": "troubleshooter",
                "type": "agent",
                "depends_on": ["implementation-evaluator"],
                "condition": {
                    "source_step": "implementation-evaluator",
                    "path": "/evaluation/found_issues",
                    "equals": true
                },
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "troubleshooter",
                "input": {
                    "objective": objective,
                    "prompt": "Investigate evaluator-reported issues, separate code defects from environment/tooling failures, and emit findings.",
                    "validation_strategy": {
                        "artifact": "troubleshooting_report",
                        "vote": true
                    }
                }
            },
            {
                "key": "integration-tester",
                "type": "agent",
                "depends_on": ["developer", "troubleshooter"],
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "integration_tester",
                "input": {
                    "objective": objective,
                    "prompt": "Run integration validation over the implementation, developer fixes, and troubleshooting evidence. Return /integration/no_gap.",
                    "validation_strategy": {
                        "artifact": "integration_test_report",
                        "required_paths": ["/integration/no_gap"],
                        "vote": true
                    }
                }
            },
            {
                "key": "integration-gate-keeper",
                "type": "agent",
                "depends_on": ["integration-tester"],
                "condition": {
                    "source_step": "integration-tester",
                    "path": "/integration/no_gap",
                    "equals": true
                },
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "integration_gate_keeper",
                "input": {
                    "objective": objective,
                    "prompt": "Decide whether the mission is complete. Only pass with explicit evidence and no remaining integration gap.",
                    "validation_strategy": {
                        "artifact": "integration_gate_decision",
                        "vote": true
                    }
                }
            },
            {
                "key": "unexpected-error",
                "type": "agent",
                "on_failure_of": ["implement", "developer", "troubleshooter", "integration-tester"],
                "dynamic_workflow_plan_id": plan.id,
                "agent_role": "unexpected_error_reporter",
                "input": {
                    "objective": objective,
                    "prompt": "Report environmental errors, insufficient permissions, missing tools, or other non-code blockers.",
                    "validation_strategy": {
                        "artifact": "unexpected_error_report",
                        "vote": false
                    }
                }
            }
        ]
    }))
}

pub(crate) fn dynamic_phase_dependencies(phase: &Value) -> Result<Vec<String>, AppError> {
    let Some(value) = phase.get("depends_on").or_else(|| phase.get("after")) else {
        return Ok(Vec::new());
    };
    match value {
        Value::String(item) => Ok(normalize_optional_text(workflow_slug(item))
            .into_iter()
            .collect()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(workflow_slug)
                    .and_then(normalize_optional_text)
                    .ok_or_else(|| {
                        AppError::bad_request(
                            "dynamic workflow phase dependencies must be non-empty strings",
                        )
                    })
            })
            .collect(),
        _ => Err(AppError::bad_request(
            "dynamic workflow phase dependencies must be a string or array",
        )),
    }
}
