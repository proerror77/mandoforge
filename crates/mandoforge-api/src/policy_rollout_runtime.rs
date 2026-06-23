use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn execute_due_policy_rollouts(
    state: &AppState,
    subject: &str,
    actor_type: &str,
) -> Result<PolicyScheduledRolloutRun, AppError> {
    let now = Utc::now();
    let revisions = state.list_policy_revisions().await?;
    let controller_binding =
        latest_policy_rollout_controller_binding(&state.list_audit_logs(None).await?);
    let mut due_revisions = Vec::new();
    let mut skipped_count = 0usize;
    for revision in &revisions {
        if policy_revision_is_due_for_scheduled_activation(revision, now) {
            due_revisions.push(revision.clone());
        } else {
            skipped_count += 1;
        }
    }
    due_revisions.sort_by_key(|revision| {
        policy_revision_activation_window(revision)
            .and_then(|window| window.activate_after)
            .unwrap_or(revision.created_at)
    });

    let mut result = if let Some(revision) = due_revisions.into_iter().next() {
        let activated_revision = activate_policy_revision_for_runtime(state, revision.id).await?;
        PolicyScheduledRolloutRun {
            status: "activated".to_string(),
            activated_revision_id: Some(activated_revision.id),
            activated_revision: Some(activated_revision),
            controller_id: controller_binding
                .as_ref()
                .map(|binding| binding.controller_id.clone()),
            policy_store_id: controller_binding
                .as_ref()
                .map(|binding| binding.policy_store_id.clone()),
            deployment_id: controller_binding
                .as_ref()
                .map(|binding| binding.deployment_id.clone()),
            scanned_count: revisions.len(),
            skipped_count,
            scanned_revisions: Vec::new(),
            checked_at: now,
            reason: "activated the earliest due policy revision".to_string(),
        }
    } else {
        PolicyScheduledRolloutRun {
            status: "noop".to_string(),
            activated_revision_id: None,
            activated_revision: None,
            controller_id: controller_binding
                .as_ref()
                .map(|binding| binding.controller_id.clone()),
            policy_store_id: controller_binding
                .as_ref()
                .map(|binding| binding.policy_store_id.clone()),
            deployment_id: controller_binding
                .as_ref()
                .map(|binding| binding.deployment_id.clone()),
            scanned_count: revisions.len(),
            skipped_count,
            scanned_revisions: Vec::new(),
            checked_at: now,
            reason: "no passed draft policy revision is inside its activation window".to_string(),
        }
    };

    let mut audit_log = new_audit_log(
        None,
        actor_type,
        None,
        "policy.rollout_due_run",
        "policy",
        result.activated_revision_id,
        json!({}),
    );
    result.scanned_revisions = revisions
        .iter()
        .map(|revision| {
            let status = if Some(revision.id) == result.activated_revision_id {
                "activated"
            } else if policy_revision_is_due_for_scheduled_activation(revision, now) {
                "scanned"
            } else {
                "skipped"
            };
            PolicyScheduledRolloutScanDetail {
                policy_id: revision.name.clone(),
                policy_name: revision.name.clone(),
                revision_id: revision.id,
                controller_id: controller_binding
                    .as_ref()
                    .map(|binding| binding.controller_id.clone()),
                policy_store_id: controller_binding
                    .as_ref()
                    .map(|binding| binding.policy_store_id.clone()),
                deployment_id: controller_binding
                    .as_ref()
                    .map(|binding| binding.deployment_id.clone()),
                status: status.to_string(),
                audit_id: audit_log.id,
                scanned_at: now,
            }
        })
        .collect();
    audit_log.details = json!({
        "subject": subject,
        "status": result.status,
        "activated_revision_id": result.activated_revision_id,
        "controller_id": result.controller_id.clone(),
        "policy_store_id": result.policy_store_id.clone(),
        "deployment_id": result.deployment_id.clone(),
        "scanned_count": result.scanned_count,
        "skipped_count": result.skipped_count,
        "scanned_revisions": result.scanned_revisions,
        "checked_at": result.checked_at
    });
    state.append_audit_log(audit_log).await?;
    Ok(result)
}

pub(crate) fn build_policy_rollout_orchestration_readiness(
    runtime: &PolicyRuntimeStatus,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> PolicyRolloutOrchestrationReadiness {
    let latest_due_run = audit_logs
        .iter()
        .filter(|log| log.action == "policy.rollout_due_run")
        .max_by_key(|log| log.created_at);
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "policy.rollout_orchestration_validation_run")
        .max_by_key(|log| log.created_at);
    let latest_due_run_at = latest_due_run.map(|log| log.created_at);
    let latest_due_run_age_hours =
        latest_due_run_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_due_run_status = latest_due_run
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let due_run_fresh = latest_due_run_age_hours.is_some_and(|hours| hours < 24);
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_execution =
        latest_validation.and_then(|log| log.details.get("controller_execution"));
    let latest_controller_status = latest_validation
        .and(latest_controller_execution)
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_target_kind = latest_controller_execution
        .and_then(|execution| execution.get("target_kind"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_environment = latest_controller_execution
        .and_then(|execution| execution.get("environment"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_id = latest_controller_execution
        .and_then(|execution| execution.get("controller_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_rollout_scope = latest_controller_execution
        .and_then(|execution| execution.get("rollout_scope"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_production_policy_store = latest_controller_execution
        .and_then(|execution| execution.get("production_policy_store"))
        .and_then(Value::as_bool);
    let latest_controller_rollback_supported = latest_controller_execution
        .and_then(|execution| execution.get("rollback_supported"))
        .and_then(Value::as_bool);
    let latest_controller_age_hours = latest_validation
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_production_target = latest_controller_execution
        .is_some_and(policy_rollout_orchestration_execution_is_production_target);
    let latest_controller_validated = latest_validation_status.as_deref() == Some("validated")
        && latest_controller_status.as_deref() == Some("validated")
        && latest_controller_production_target;
    let mut blocking_reasons = Vec::new();

    if runtime.active_revision_id.is_none() {
        blocking_reasons.push("no active policy revision is installed".to_string());
    }
    if runtime.rollout_active {
        blocking_reasons.push("staged policy rollout is still active".to_string());
    }
    if latest_due_run.is_none() {
        blocking_reasons.push("policy rollout due-run supervision has not run".to_string());
    }
    if latest_due_run_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("policy rollout due-run supervision is stale".to_string());
    }
    if latest_due_run_status.as_deref() == Some("activated") && runtime.rollout_active {
        blocking_reasons
            .push("activated policy rollout still has staged runtime traffic".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "policy rollout orchestration controller is required but not configured".to_string(),
        );
    }
    if controller_required
        && controller_configured
        && latest_controller_status.as_deref() != Some("validated")
    {
        blocking_reasons.push(
            "policy rollout orchestration controller evidence is missing or not validated"
                .to_string(),
        );
    }
    if controller_required
        && latest_controller_status.as_deref() == Some("validated")
        && !latest_controller_production_target
    {
        blocking_reasons.push(
            "policy rollout orchestration controller did not identify a real production policy controller target"
                .to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons
            .push("policy rollout orchestration controller evidence is stale".to_string());
    }

    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Policy rollout production orchestration is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Policy rollout production orchestration has fresh due-run supervision, no active staged rollout, and required controller evidence".to_string()
    };

    PolicyRolloutOrchestrationReadiness {
        status,
        production_blocked,
        rollout_active: runtime.rollout_active,
        active_revision_id: runtime.active_revision_id,
        staged_revision_id: runtime.staged_revision_id,
        latest_due_run_at,
        latest_due_run_status,
        latest_due_run_age_hours,
        due_run_fresh,
        latest_validation_at,
        latest_validation_status,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        latest_controller_production_target,
        latest_controller_target_kind,
        latest_controller_environment,
        latest_controller_id,
        latest_controller_rollout_scope,
        latest_controller_production_policy_store,
        latest_controller_rollback_supported,
        controller_required,
        controller_configured,
        blocking_reasons,
        message,
    }
}

pub(crate) fn policy_rollout_orchestration_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn policy_rollout_orchestration_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn is_production_policy_rollout_target_kind(value: Option<&str>) -> bool {
    matches!(
        value,
        Some(
            "production_policy_controller"
                | "enterprise_policy_controller"
                | "external_policy_controller"
                | "policy_controller_cluster"
        )
    )
}

pub(crate) fn is_production_policy_rollout_environment(value: Option<&str>) -> bool {
    matches!(value, Some("production" | "prod"))
}

pub(crate) fn is_production_policy_rollout_scope(value: Option<&str>) -> bool {
    matches!(
        value,
        Some("production" | "global" | "enterprise" | "multi_tenant")
    )
}

pub(crate) fn policy_rollout_orchestration_execution_is_production_target(
    execution: &Value,
) -> bool {
    let controller_id_present = execution
        .get("controller_id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    is_production_policy_rollout_target_kind(execution.get("target_kind").and_then(Value::as_str))
        && is_production_policy_rollout_environment(
            execution.get("environment").and_then(Value::as_str),
        )
        && is_production_policy_rollout_scope(
            execution.get("rollout_scope").and_then(Value::as_str),
        )
        && controller_id_present
        && execution
            .get("production_policy_store")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && execution
            .get("rollback_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

pub(crate) fn nonempty_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub(crate) fn latest_policy_rollout_controller_binding(
    audit_logs: &[AuditLog],
) -> Option<PolicyRolloutControllerBinding> {
    let execution = audit_logs
        .iter()
        .filter(|log| log.action == "policy.rollout_orchestration_validation_run")
        .filter(|log| log.details.get("status").and_then(Value::as_str) == Some("validated"))
        .filter_map(|log| {
            log.details
                .get("controller_execution")
                .map(|execution| (log, execution))
        })
        .filter(|(_, execution)| {
            execution.get("status").and_then(Value::as_str) == Some("validated")
                && policy_rollout_orchestration_execution_is_production_target(execution)
        })
        .max_by_key(|(log, _)| log.created_at)
        .map(|(_, execution)| execution)?;

    Some(PolicyRolloutControllerBinding {
        controller_id: nonempty_json_string(execution, "controller_id")?,
        policy_store_id: nonempty_json_string(execution, "policy_store_id")?,
        deployment_id: nonempty_json_string(execution, "deployment_id")?,
    })
}

pub(crate) async fn execute_policy_rollout_orchestration_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    runtime: &PolicyRuntimeStatus,
    readiness: &PolicyRolloutOrchestrationReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.policy_rollout_orchestration_validation",
        "subject": subject,
        "checked_at": checked_at,
        "runtime": {
            "active_revision_id": runtime.active_revision_id,
            "staged_revision_id": runtime.staged_revision_id,
            "staged_rollout_percent": runtime.staged_rollout_percent,
            "rollout_active": runtime.rollout_active,
        },
        "readiness": {
            "status": readiness.status,
            "production_blocked": readiness.production_blocked,
            "latest_due_run_status": readiness.latest_due_run_status,
            "latest_due_run_age_hours": readiness.latest_due_run_age_hours,
            "due_run_fresh": readiness.due_run_fresh,
            "blocking_reasons": readiness.blocking_reasons,
        },
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "policy rollout orchestration controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(provider_status, "validated" | "success" | "ok" | "healthy");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "orchestration_id": body.get("orchestration_id").and_then(Value::as_str),
        "controller_id": body.get("controller_id").and_then(Value::as_str),
        "target_kind": body.get("target_kind").and_then(Value::as_str),
        "environment": body.get("environment").and_then(Value::as_str),
        "rollout_scope": body.get("rollout_scope").and_then(Value::as_str),
        "production_policy_store": body.get("production_policy_store").and_then(Value::as_bool),
        "rollback_supported": body.get("rollback_supported").and_then(Value::as_bool),
        "policy_store_id": body.get("policy_store_id").and_then(Value::as_str),
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn activate_policy_revision_for_runtime(
    state: &AppState,
    id: Uuid,
) -> Result<PolicyRevision, AppError> {
    let pending_revision = state.get_policy_revision(id).await?;
    enforce_policy_activation_window(&pending_revision, Utc::now())?;
    let revision = state.activate_policy_revision(id).await?;
    let activated_policy = serde_json::from_value::<PolicyConfig>(revision.body.clone())
        .map_err(|error| AppError::bad_request(format!("invalid activated policy: {error}")))?;
    let rollout_percent = policy_revision_rollout_percent(&revision);
    state
        .activate_runtime_policy(revision.id, activated_policy, rollout_percent)
        .await;
    Ok(revision)
}

pub(crate) fn enforce_policy_activation_window(
    revision: &PolicyRevision,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let Some(window) = policy_revision_activation_window(revision) else {
        return Ok(());
    };
    if let Some(activate_after) = window.activate_after
        && now < activate_after
    {
        return Err(AppError::bad_request(format!(
            "policy activation window is not open until {}",
            activate_after.to_rfc3339()
        )));
    }
    if let Some(activate_before) = window.activate_before
        && now > activate_before
    {
        return Err(AppError::bad_request(format!(
            "policy activation window closed at {}",
            activate_before.to_rfc3339()
        )));
    }
    Ok(())
}

pub(crate) fn policy_revision_is_due_for_scheduled_activation(
    revision: &PolicyRevision,
    now: DateTime<Utc>,
) -> bool {
    if revision.status != "draft" || revision.gate_status.as_deref() != Some("passed") {
        return false;
    }
    let Some(window) = policy_revision_activation_window(revision) else {
        return false;
    };
    let Some(activate_after) = window.activate_after else {
        return false;
    };
    if now < activate_after {
        return false;
    }
    if let Some(activate_before) = window.activate_before
        && now > activate_before
    {
        return false;
    }
    true
}

pub(crate) fn policy_revision_activation_window(
    revision: &PolicyRevision,
) -> Option<PolicyActivationWindow> {
    let window = revision.gate_result.get("activation_window")?;
    if window.is_null() {
        return None;
    }
    Some(PolicyActivationWindow {
        activate_after: window
            .get("activate_after")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
        activate_before: window
            .get("activate_before")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
    })
}

pub(crate) fn policy_revision_rollout_percent(revision: &PolicyRevision) -> u8 {
    revision
        .gate_result
        .get("rollout_percent")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| *value <= 100)
        .unwrap_or(100)
}

pub(crate) fn validate_policy_revision_input(
    mut input: CreatePolicyRevision,
) -> Result<CreatePolicyRevision, AppError> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request("policy revision name is required"));
    }
    if !input.body.is_object() {
        return Err(AppError::bad_request(
            "policy revision body must be a JSON object",
        ));
    }
    serde_json::from_value::<PolicyConfig>(input.body.clone())
        .map_err(|error| AppError::bad_request(format!("invalid policy body: {error}")))?;
    Ok(input)
}

pub(crate) fn build_policy_revision_diff(
    current_policy: &PolicyConfig,
    revision: &PolicyRevision,
) -> Result<PolicyRevisionDiff, AppError> {
    let current = serde_json::to_value(current_policy)?;
    let mut changes = Vec::new();
    collect_policy_diff("", &current, &revision.body, &mut changes);
    Ok(PolicyRevisionDiff {
        revision_id: revision.id,
        changes,
        generated_at: Utc::now(),
    })
}

pub(crate) fn collect_policy_diff(
    path: &str,
    current: &Value,
    proposed: &Value,
    changes: &mut Vec<PolicyDiffChange>,
) {
    match (current, proposed) {
        (Value::Object(current_map), Value::Object(proposed_map)) => {
            let keys: BTreeSet<_> = current_map.keys().chain(proposed_map.keys()).collect();
            for key in keys {
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                match (current_map.get(key), proposed_map.get(key)) {
                    (Some(current_value), Some(proposed_value)) => {
                        collect_policy_diff(&child_path, current_value, proposed_value, changes);
                    }
                    (Some(current_value), None) => changes.push(PolicyDiffChange {
                        path: child_path,
                        kind: "removed".to_string(),
                        current: current_value.clone(),
                        proposed: Value::Null,
                    }),
                    (None, Some(proposed_value)) => changes.push(PolicyDiffChange {
                        path: child_path,
                        kind: "added".to_string(),
                        current: Value::Null,
                        proposed: proposed_value.clone(),
                    }),
                    (None, None) => {}
                }
            }
        }
        _ if current != proposed => changes.push(PolicyDiffChange {
            path: path.to_string(),
            kind: "changed".to_string(),
            current: current.clone(),
            proposed: proposed.clone(),
        }),
        _ => {}
    }
}

pub(crate) fn build_policy_revision_gate(
    current_policy: &PolicyConfig,
    revision: &PolicyRevision,
    input: PolicyRevisionGateRequest,
) -> Result<PolicyRevisionGate, AppError> {
    let proposed_policy = serde_json::from_value::<PolicyConfig>(revision.body.clone())
        .map_err(|error| AppError::bad_request(format!("invalid policy body: {error}")))?;
    let rollout_percent = input.rollout_percent.unwrap_or(100);
    if rollout_percent > 100 {
        return Err(AppError::bad_request(
            "policy rollout percent must be between 0 and 100",
        ));
    }
    let activation_window = normalize_policy_activation_window(
        input.activate_after.as_deref(),
        input.activate_before.as_deref(),
    )?;
    let (suite_source, suite_cases) = normalize_policy_gate_cases(input.cases)?;
    let cases = suite_cases
        .into_iter()
        .map(|case| {
            let tool_name = case.tool_name;
            let expected_decision = case.expected_decision;
            let decision = proposed_policy.evaluate_tool(&tool_name);
            let passed = decision.decision == expected_decision;
            PolicyGateCaseResult {
                tool_name: tool_name.to_string(),
                expected_decision: expected_decision.to_string(),
                actual_decision: decision.decision.to_string(),
                passed,
                reason: decision.reason,
            }
        })
        .collect::<Vec<_>>();
    let status = if cases.iter().all(|case| case.passed) {
        "passed"
    } else {
        "failed"
    }
    .to_string();
    Ok(PolicyRevisionGate {
        revision_id: revision.id,
        status,
        suite_source,
        rollout_percent,
        activation_window,
        cases,
        diff: build_policy_revision_diff(current_policy, revision)?,
        checked_at: Utc::now(),
    })
}

pub(crate) fn normalize_policy_activation_window(
    activate_after: Option<&str>,
    activate_before: Option<&str>,
) -> Result<Option<PolicyActivationWindow>, AppError> {
    let activate_after = parse_optional_rfc3339("activate_after", activate_after)?;
    let activate_before = parse_optional_rfc3339("activate_before", activate_before)?;
    if let (Some(after), Some(before)) = (activate_after, activate_before)
        && after >= before
    {
        return Err(AppError::bad_request(
            "policy activation window activate_after must be before activate_before",
        ));
    }
    if activate_after.is_none() && activate_before.is_none() {
        return Ok(None);
    }
    Ok(Some(PolicyActivationWindow {
        activate_after,
        activate_before,
    }))
}

pub(crate) fn parse_optional_rfc3339(
    field: &str,
    value: Option<&str>,
) -> Result<Option<DateTime<Utc>>, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|error| AppError::bad_request(format!("{field} must be RFC3339: {error}")))
}

pub(crate) fn normalize_policy_gate_cases(
    cases: Vec<PolicyGateCaseInput>,
) -> Result<(String, Vec<PolicyGateCaseInput>), AppError> {
    let cases = if cases.is_empty() {
        vec![
            PolicyGateCaseInput {
                tool_name: "secret.read".to_string(),
                expected_decision: "denied".to_string(),
            },
            PolicyGateCaseInput {
                tool_name: "shell.exec".to_string(),
                expected_decision: "requires_approval".to_string(),
            },
            PolicyGateCaseInput {
                tool_name: "file.write".to_string(),
                expected_decision: "requires_approval".to_string(),
            },
            PolicyGateCaseInput {
                tool_name: "sql.query".to_string(),
                expected_decision: "allowed".to_string(),
            },
            PolicyGateCaseInput {
                tool_name: "file.read".to_string(),
                expected_decision: "allowed".to_string(),
            },
        ]
    } else {
        cases
    };
    if cases.len() > 50 {
        return Err(AppError::bad_request(
            "policy revision gate supports at most 50 cases",
        ));
    }
    let mut normalized = Vec::with_capacity(cases.len());
    for mut case in cases {
        case.tool_name = case.tool_name.trim().to_string();
        case.expected_decision = case.expected_decision.trim().to_string();
        if case.tool_name.is_empty() || case.expected_decision.is_empty() {
            return Err(AppError::bad_request(
                "policy gate cases require tool_name and expected_decision",
            ));
        }
        match case.expected_decision.as_str() {
            "allowed" | "denied" | "requires_approval" => {}
            _ => {
                return Err(AppError::bad_request(
                    "expected_decision must be allowed, denied, or requires_approval",
                ));
            }
        }
        normalized.push(case);
    }
    let source = if normalized.len() == 5
        && normalized
            .iter()
            .any(|case| case.tool_name == "secret.read")
        && normalized.iter().any(|case| case.tool_name == "shell.exec")
    {
        "default"
    } else {
        "custom"
    };
    Ok((source.to_string(), normalized))
}
