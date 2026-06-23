use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn decide_approval(
    state: AppState,
    approval_id: Uuid,
    status: &str,
    decider_subject: Option<String>,
) -> Result<Json<Approval>, AppError> {
    let approval = state.get_approval(approval_id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be decided",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, approval_id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
    let updated = state.decide_approval(approval_id, status).await?;
    if status == "approved" {
        maybe_issue_approval_commit_token(&state, &updated, decider_subject.as_deref()).await?;
    }
    let decision_result = match status {
        "rejected" => record_rejected_approval_tool_result(&state, &updated).await?,
        _ => None,
    };
    let decision_event = state
        .append_event(
            "user",
            Some(approval_id),
            updated.session_id,
            &format!("approval.{status}"),
            json!({"approval_id": approval_id, "decision": status}),
        )
        .await?;
    if status == "approved" {
        let outcome = state
            .execution_worker
            .execute_approved_tool(&state, &updated)
            .await?;
        match outcome {
            ExecutionWorkerOutcome::Completed { job } => {
                if job.is_none() {
                    project_session_event_to_loop(&state, &decision_event).await?;
                }
            }
            ExecutionWorkerOutcome::Queued => {
                set_managed_session_status(
                    &state,
                    updated.session_id,
                    SessionStatus::Running,
                    "approved execution queued for worker",
                )
                .await?;
            }
        }
    } else if status == "rejected" {
        project_session_event_to_loop(&state, &decision_event).await?;
    }
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "user",
            Some(approval_id),
            &format!("approval.{status}"),
            "approval",
            Some(approval_id),
            json!({
                "tool_call_id": updated.tool_call_id,
                "decision": status,
                "tool_result_status": decision_result.map(|tool_call| tool_call.status),
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn maybe_issue_approval_commit_token(
    state: &AppState,
    approval: &Approval,
    approver_subject: Option<&str>,
) -> Result<Option<ApprovalCommitToken>, AppError> {
    let Some(tool_call_id) = approval.tool_call_id else {
        return Ok(None);
    };
    let tool_call = state.get_tool_call(tool_call_id).await?;
    let Some(task_grant_id) = tool_call.task_grant_id else {
        return Ok(None);
    };
    let grant = state.get_task_grant(task_grant_id).await?;
    if !task_grant_requires_approval_commit_token(&grant, &tool_call.tool_name) {
        return Ok(None);
    }
    if state
        .approval_commit_token_for_approval(approval.id)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    let binding = approval_commit_binding_for_args(&tool_call.tool_name, &tool_call.args)?;
    if tool_call.normalized_args_hash.as_deref() != Some(binding.normalized_args_hash.as_str())
        || tool_call.target_binding != binding.target_binding
    {
        return Err(AppError::forbidden(
            "approval commit binding does not match current tool call args",
        ));
    }
    let created_at = Utc::now();
    let token = state
        .create_approval_commit_token(ApprovalCommitToken {
            id: Uuid::new_v4(),
            approval_id: approval.id,
            tool_call_id: tool_call.id,
            task_grant_id,
            session_id: approval.session_id,
            tool_name: tool_call.tool_name.clone(),
            normalized_args_hash: binding.normalized_args_hash,
            target_binding: binding.target_binding,
            approver_subject: approver_subject.unwrap_or("unknown").to_string(),
            status: "issued".to_string(),
            expires_at: approval
                .expires_at
                .unwrap_or_else(|| created_at + ChronoDuration::minutes(15)),
            consumed_at: None,
            created_at,
        })
        .await?;
    state
        .append_event(
            "system",
            Some(token.id),
            approval.session_id,
            "approval_commit_token.issued",
            json!({
                "approval_commit_token_id": token.id,
                "approval_id": approval.id,
                "tool_call_id": token.tool_call_id,
                "task_grant_id": token.task_grant_id,
                "tool": token.tool_name,
                "normalized_args_hash": token.normalized_args_hash,
                "target_binding": token.target_binding,
                "expires_at": token.expires_at
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "system",
            Some(token.id),
            "approval_commit_token.issued",
            "approval_commit_token",
            Some(token.id),
            json!({
                "approval_id": approval.id,
                "tool_call_id": token.tool_call_id,
                "task_grant_id": token.task_grant_id,
                "tool": token.tool_name,
                "normalized_args_hash": token.normalized_args_hash,
                "target_binding": token.target_binding
            }),
        ))
        .await?;
    Ok(Some(token))
}

pub(crate) async fn consume_valid_approval_commit_token_for_tool_call(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<ApprovalCommitToken, AppError> {
    let token = state
        .approval_commit_token_for_approval(approval.id)
        .await?
        .ok_or_else(|| AppError::forbidden("approval commit token is required"))?;
    if token.status != "issued" {
        return Err(AppError::forbidden("approval commit token is not issued"));
    }
    if token.expires_at <= Utc::now() {
        return Err(AppError::forbidden("approval commit token is expired"));
    }
    if token.tool_call_id != tool_call.id
        || token.session_id != approval.session_id
        || token.tool_name != tool_call.tool_name
    {
        return Err(AppError::forbidden(
            "approval commit token does not match tool call",
        ));
    }
    let Some(task_grant_id) = tool_call.task_grant_id else {
        return Err(AppError::forbidden(
            "approval commit token requires task grant binding",
        ));
    };
    if token.task_grant_id != task_grant_id {
        return Err(AppError::forbidden(
            "approval commit token task grant does not match tool call",
        ));
    }
    let grant = state.get_task_grant(task_grant_id).await?;
    if grant.status != "active" {
        return Err(AppError::forbidden("task grant is not active"));
    }
    if grant
        .expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        return Err(AppError::forbidden("task grant is expired"));
    }
    if !task_grant_requires_approval_commit_token(&grant, &tool_call.tool_name) {
        return Err(AppError::forbidden(
            "approval commit token is only valid for commit_write effects",
        ));
    }
    if let Some(reason) =
        task_grant_connector_invocation_denial(&grant, &tool_call.tool_name, &tool_call.args)?
    {
        return Err(AppError::forbidden(reason));
    }
    let binding = approval_commit_binding_for_args(&tool_call.tool_name, &tool_call.args)?;
    if token.normalized_args_hash != binding.normalized_args_hash
        || token.target_binding != binding.target_binding
    {
        return Err(AppError::forbidden(
            "approval commit token digest does not match current tool call args",
        ));
    }
    let consumed = state.consume_approval_commit_token(token.id).await?;
    state
        .append_event(
            "system",
            Some(consumed.id),
            approval.session_id,
            "approval_commit_token.consumed",
            json!({
                "approval_commit_token_id": consumed.id,
                "approval_id": approval.id,
                "tool_call_id": tool_call.id,
                "task_grant_id": consumed.task_grant_id,
                "tool": consumed.tool_name,
                "normalized_args_hash": consumed.normalized_args_hash
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "worker",
            Some(consumed.id),
            "approval_commit_token.consumed",
            "approval_commit_token",
            Some(consumed.id),
            json!({
                "approval_id": approval.id,
                "tool_call_id": tool_call.id,
                "task_grant_id": consumed.task_grant_id,
                "tool": consumed.tool_name,
                "normalized_args_hash": consumed.normalized_args_hash
            }),
        ))
        .await?;
    Ok(consumed)
}

async fn record_rejected_approval_tool_result(
    state: &AppState,
    approval: &Approval,
) -> Result<Option<ToolCall>, AppError> {
    let Some(tool_call_id) = approval.tool_call_id else {
        return Ok(None);
    };
    let tool_call = state.get_tool_call(tool_call_id).await?;
    let result = json!({
        "status": "denied",
        "approval": "rejected",
        "approval_id": approval.id,
        "reason": approval.reason,
    });
    let tool_result_event = state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "content": result,
            }),
        )
        .await?;
    project_session_event_to_loop(state, &tool_result_event).await?;
    state
        .append_event(
            "agent",
            Some(tool_call.id),
            approval.session_id,
            "agent.tool_result",
            json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "status": "denied",
                "content": result,
            }),
        )
        .await?;
    let updated = state
        .update_tool_call_status(tool_call.id, "denied", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.denied",
            "tool_call",
            Some(tool_call.id),
            json!({
                "tool": tool_call.tool_name,
                "approval_id": approval.id,
                "decision": "rejected",
                "status": "denied",
            }),
        ))
        .await?;
    Ok(Some(updated))
}

pub(crate) fn approval_is_expired(approval: &Approval) -> bool {
    approval_is_expired_at(approval, Utc::now())
}

pub(crate) fn approval_is_expired_at(approval: &Approval, now: DateTime<Utc>) -> bool {
    approval.status == "pending"
        && approval
            .expires_at
            .is_some_and(|expires_at| expires_at <= now)
}

pub(crate) fn next_due_escalation_rule(
    approval: &Approval,
    rules: &[ApprovalEscalationRule],
    now: DateTime<Utc>,
) -> Option<ApprovalEscalationRule> {
    let previous_rule_id = approval
        .evidence
        .get("escalation")
        .and_then(|value| value.get("rule_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let previous_order = previous_rule_id.and_then(|rule_id| {
        rules
            .iter()
            .find(|rule| rule.id == rule_id)
            .map(|rule| rule.order_index)
    });
    let age_seconds = now
        .signed_duration_since(approval.created_at)
        .num_seconds()
        .max(0) as i32;
    rules
        .iter()
        .filter(|rule| rule.status == "active")
        .filter(|rule| rule.risk_level == approval.risk_level)
        .filter(|rule| previous_order.map_or(true, |order| rule.order_index > order))
        .filter(|rule| age_seconds >= rule.after_seconds)
        .min_by_key(|rule| (rule.order_index, rule.created_at))
        .cloned()
}

pub(crate) async fn expire_approval_record(
    state: &AppState,
    approval_id: Uuid,
) -> Result<Approval, AppError> {
    let approval = state.get_approval(approval_id).await?;
    if approval.status != "pending" {
        return Ok(approval);
    }
    let updated = state.decide_approval(approval_id, "expired").await?;
    state
        .append_event(
            "system",
            Some(approval_id),
            updated.session_id,
            "approval.expired",
            json!({"approval_id": approval_id, "decision": "expired", "expires_at": updated.expires_at}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "system",
            Some(approval_id),
            "approval.expired",
            "approval",
            Some(approval_id),
            json!({"tool_call_id": updated.tool_call_id, "decision": "expired", "expires_at": updated.expires_at}),
        ))
        .await?;
    Ok(updated)
}

pub(crate) fn approval_expires_at(
    created_at: DateTime<Utc>,
    expires_in_seconds: Option<&Value>,
) -> Option<DateTime<Utc>> {
    let seconds = expires_in_seconds
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0)
        .unwrap_or(86_400);
    Some(created_at + chrono::Duration::seconds(seconds))
}

pub(crate) fn validate_approval_group_input(
    mut input: CreateApprovalGroup,
) -> Result<CreateApprovalGroup, AppError> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request("approval group name is required"));
    }
    input.subjects = input
        .subjects
        .into_iter()
        .map(|subject| subject.trim().to_string())
        .filter(|subject| !subject.is_empty())
        .collect();
    input.subjects.sort();
    input.subjects.dedup();
    if input.subjects.is_empty() {
        return Err(AppError::bad_request(
            "approval group requires at least one subject",
        ));
    }
    Ok(input)
}

pub(crate) fn validate_approval_escalation_rule_input(
    mut input: CreateApprovalEscalationRule,
) -> Result<CreateApprovalEscalationRule, AppError> {
    input.name = input.name.trim().to_string();
    input.risk_level = input.risk_level.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request(
            "approval escalation rule name is required",
        ));
    }
    if input.risk_level.is_empty() {
        return Err(AppError::bad_request(
            "approval escalation rule risk_level is required",
        ));
    }
    input.order_index = input.order_index.max(0);
    input.after_seconds = input.after_seconds.max(0);
    Ok(input)
}

pub(crate) fn default_approval_notification_risk_filter() -> String {
    "all".to_string()
}

pub(crate) fn default_approval_notification_max_attempts() -> i32 {
    1
}

pub(crate) fn validate_approval_notification_channel_policy_input(
    mut input: CreateApprovalNotificationChannelPolicy,
) -> Result<CreateApprovalNotificationChannelPolicy, AppError> {
    input.name = input.name.trim().to_string();
    input.channel = input.channel.trim().to_string();
    input.target_env = input.target_env.and_then(|target_env| {
        let trimmed = target_env.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    input.risk_filter = input.risk_filter.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request(
            "approval notification channel policy name is required",
        ));
    }
    if !matches!(input.channel.as_str(), "webhook" | "slack" | "email_relay") {
        return Err(AppError::bad_request(
            "approval notification channel policy channel must be webhook, slack, or email_relay",
        ));
    }
    if !matches!(
        input.risk_filter.as_str(),
        "all" | "low" | "medium" | "high" | "critical"
    ) {
        return Err(AppError::bad_request(
            "approval notification channel policy risk_filter must be all, low, medium, high, or critical",
        ));
    }
    input.max_attempts = input.max_attempts.clamp(1, 5);
    input.backoff_seconds = input.backoff_seconds.clamp(0, 60);
    Ok(input)
}

pub(crate) fn validate_cost_alert_route_input(
    mut input: CreateCostAlertRoute,
) -> Result<CreateCostAlertRoute, AppError> {
    input.name = input.name.trim().to_string();
    input.channel = input.channel.trim().to_string();
    input.target = input.target.and_then(|target| {
        let trimmed = target.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    input.severity_filter = input.severity_filter.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request("cost alert route name is required"));
    }
    if !matches!(input.channel.as_str(), "webhook" | "slack" | "email") {
        return Err(AppError::bad_request(
            "cost alert route channel must be webhook, slack, or email",
        ));
    }
    if !matches!(input.severity_filter.as_str(), "warning" | "critical") {
        return Err(AppError::bad_request(
            "cost alert route severity_filter must be warning or critical",
        ));
    }
    Ok(input)
}

pub(crate) fn required_trimmed(value: &str, field_name: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::bad_request(format!("{field_name} is required")));
    }
    Ok(value.to_string())
}

pub(crate) fn optional_trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn merge_approval_evidence(target: &mut Value, patch: Value) {
    if !target.is_object() {
        *target = json!({"details": target.clone()});
    }
    let Some(target_map) = target.as_object_mut() else {
        return;
    };
    if let Value::Object(patch_map) = patch {
        for (key, value) in patch_map {
            target_map.insert(key, value);
        }
    }
}

pub(crate) async fn execute_due_approval_escalations(
    state: &AppState,
) -> Result<ApprovalEscalationDueRun, AppError> {
    let checked_at = Utc::now();
    let mut expired_count = 0;
    let mut escalated_count = 0;
    let mut skipped_count = 0;
    let mut notification_deliveries = Vec::new();
    let rules = state.list_approval_escalation_rules().await?;
    for approval in state
        .list_approvals()
        .await?
        .into_iter()
        .filter(|approval| approval.status == "pending")
    {
        if approval_is_expired_at(&approval, checked_at) {
            expire_approval_record(state, approval.id).await?;
            expired_count += 1;
            continue;
        }
        let Some(rule) = next_due_escalation_rule(&approval, &rules, checked_at) else {
            skipped_count += 1;
            continue;
        };
        let group = state.get_approval_group(rule.group_id).await?;
        if group.status != "active" {
            skipped_count += 1;
            continue;
        }
        let updated = escalate_approval_record(
            state,
            &approval,
            &group,
            Some(rule.id),
            format!("Scheduled escalation after {} seconds", rule.after_seconds),
            "system".to_string(),
            "system",
            None,
        )
        .await?;
        escalated_count += 1;
        notification_deliveries
            .push(deliver_approval_notification(state, &updated, checked_at).await?);
    }
    let run = ApprovalEscalationDueRun {
        status: "completed".to_string(),
        checked_at,
        expired_count,
        escalated_count,
        skipped_count,
        notification_deliveries,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "approval.escalation_due_run",
            "approval_escalation_rules",
            None,
            serde_json::to_value(&run)?,
        ))
        .await?;
    Ok(run)
}

pub(crate) async fn escalate_approval_record(
    state: &AppState,
    approval: &Approval,
    group: &ApprovalGroup,
    rule_id: Option<Uuid>,
    reason: String,
    escalated_by: String,
    actor_type: &str,
    actor_id: Option<Uuid>,
) -> Result<Approval, AppError> {
    let escalated_at = Utc::now();
    let mut evidence = approval.evidence.clone();
    merge_approval_evidence(
        &mut evidence,
        json!({
            "approver_group_id": group.id,
            "approver_group_name": group.name,
            "escalation": {
                "rule_id": rule_id,
                "group_id": group.id,
                "reason": reason,
                "escalated_by": escalated_by,
                "escalated_at": escalated_at
            }
        }),
    );
    let updated = state
        .update_approval_evidence(approval.id, evidence)
        .await?;
    state
        .append_event(
            actor_type,
            actor_id,
            updated.session_id,
            "approval.escalated",
            json!({
                "approval_id": approval.id,
                "group_id": group.id,
                "group_name": group.name,
                "rule_id": rule_id,
                "escalated_at": escalated_at
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            actor_type,
            actor_id,
            "approval.escalated",
            "approval",
            Some(approval.id),
            json!({
                "group_id": group.id,
                "group_name": group.name,
                "rule_id": rule_id,
                "subject_count": group.subjects.len()
            }),
        ))
        .await?;
    Ok(updated)
}

pub(crate) async fn execute_approval_notification_delivery_run(
    state: &AppState,
    subject: Option<String>,
    ran_at: DateTime<Utc>,
    max_deliveries: usize,
) -> Result<ApprovalNotificationDeliveryRun, AppError> {
    let approvals = state.list_approvals().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let recently_notified = recently_notified_approval_ids(&audit_logs, ran_at);
    let candidates = approvals
        .into_iter()
        .filter(|approval| approval.status == "pending")
        .collect::<Vec<_>>();
    let candidate_count = candidates.len();
    let mut deliveries = Vec::new();
    let mut failures = Vec::new();
    let mut delivered_count = 0usize;
    let mut reserved_count = 0usize;
    let mut skipped_count = candidate_count.saturating_sub(max_deliveries);

    for approval in candidates.into_iter().take(max_deliveries) {
        if approval_is_expired_at(&approval, ran_at) {
            expire_approval_record(state, approval.id).await?;
            skipped_count += 1;
            continue;
        }
        if recently_notified.contains(&approval.id) {
            skipped_count += 1;
            continue;
        }
        match deliver_approval_notification(state, &approval, ran_at).await {
            Ok(delivery) => {
                if delivery.delivered {
                    delivered_count += 1;
                } else {
                    reserved_count += 1;
                }
                deliveries.push(delivery);
            }
            Err(error) => failures.push(ApprovalNotificationDeliveryFailure {
                approval_id: approval.id,
                error: error.message,
            }),
        }
    }
    let failed_count = failures.len();
    let status = if failed_count > 0 && delivered_count > 0 {
        "partial_failure"
    } else if failed_count > 0 {
        "failed"
    } else if delivered_count > 0 {
        "delivered"
    } else if reserved_count > 0 {
        "reserved"
    } else {
        "no_pending"
    }
    .to_string();
    let run = ApprovalNotificationDeliveryRun {
        status,
        subject,
        candidate_count,
        delivered_count,
        reserved_count,
        failed_count,
        skipped_count,
        deliveries,
        failures,
        ran_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.notification_delivery_run",
            "approval_notifications",
            None,
            serde_json::to_value(&run)?,
        ))
        .await?;
    Ok(run)
}

pub(crate) fn recently_notified_approval_ids(
    audit_logs: &[AuditLog],
    now: DateTime<Utc>,
) -> HashSet<Uuid> {
    let cutoff = now - chrono::Duration::hours(24);
    audit_logs
        .iter()
        .filter(|log| log.action == "approval.notification_delivered" && log.created_at >= cutoff)
        .filter_map(|log| log.resource_id)
        .collect()
}

pub(crate) fn build_approval_notification_delivery_run_summary(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    routing: &ApprovalNotificationRoutingSummary,
) -> ApprovalNotificationDeliveryRunSummary {
    let mut recent_runs: Vec<_> = audit_logs
        .iter()
        .filter_map(approval_notification_delivery_run_from_audit_log)
        .collect();
    recent_runs.sort_by(|left, right| right.ran_at.cmp(&left.ran_at));
    let run_count = recent_runs.len();
    let delivered_run_count = recent_runs
        .iter()
        .filter(|run| run.delivered_count > 0 && run.failed_count == 0)
        .count();
    let reserved_run_count = recent_runs
        .iter()
        .filter(|run| run.status == "reserved")
        .count();
    let failed_run_count = recent_runs
        .iter()
        .filter(|run| run.failed_count > 0 || run.status == "failed")
        .count();
    let latest_run = recent_runs.first().cloned();
    let mut attention_items = Vec::new();
    match latest_run.as_ref() {
        Some(run) if run.failed_count > 0 => {
            attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
                kind: "latest_delivery_failed".to_string(),
                severity: "critical".to_string(),
                message: format!(
                    "latest approval notification run failed for {} approval(s)",
                    run.failed_count
                ),
            });
        }
        Some(run) if run.status == "reserved" => {
            attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
                kind: "latest_delivery_reserved".to_string(),
                severity: "warning".to_string(),
                message:
                    "latest approval notification run found pending approvals but no delivery channel or target was ready".to_string(),
            });
        }
        Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => {
            attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
                kind: "stale_delivery_run".to_string(),
                severity: "warning".to_string(),
                message: "approval notifications have not been run in the last 24 hours"
                    .to_string(),
            });
        }
        None => {
            attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
                kind: "missing_delivery_run".to_string(),
                severity: "warning".to_string(),
                message: "approval notification delivery has not been run yet".to_string(),
            });
        }
        _ => {}
    }
    let production_ops = build_approval_notification_production_ops_readiness(
        latest_run.as_ref(),
        routing,
        audit_logs,
        generated_at,
        approval_notification_ops_controller_required(&|key| std::env::var(key).ok()),
        approval_notification_ops_controller_configured(&|key| std::env::var(key).ok()),
    );
    if production_ops.production_blocked {
        attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
            kind: "approval_notification_production_blocked".to_string(),
            severity: if production_ops.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_ops.message.clone(),
        });
    }
    let lookup = |key: &str| std::env::var(key).ok();
    let deployment_readiness = build_approval_notification_deployment_readiness(
        audit_logs,
        routing,
        generated_at,
        approval_notification_deployment_controller_required(&lookup),
        approval_notification_deployment_controller_configured(&lookup),
    );
    if deployment_readiness.production_blocked {
        attention_items.push(ApprovalNotificationDeliveryRunAttentionItem {
            kind: "approval_notification_deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
        });
    }
    recent_runs.truncate(10);
    ApprovalNotificationDeliveryRunSummary {
        generated_at,
        run_count,
        delivered_run_count,
        reserved_run_count,
        failed_run_count,
        latest_run,
        recent_runs,
        production_ops,
        deployment_readiness,
        attention_items,
    }
}

pub(crate) fn build_approval_notification_production_ops_readiness(
    latest_run: Option<&ApprovalNotificationDeliveryRunRecord>,
    routing: &ApprovalNotificationRoutingSummary,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> ApprovalNotificationProductionOpsReadiness {
    let latest_run_age_hours = latest_run.map(|run| (generated_at - run.ran_at).num_hours());
    let latest_run_status = latest_run.map(|run| run.status.clone());
    let latest_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "approval.notification_ops_validation_run")
        .max_by_key(|log| log.created_at);
    let latest_controller_status = latest_controller_log
        .and_then(|log| {
            log.details["controller_execution"]["status"]
                .as_str()
                .or_else(|| log.details["status"].as_str())
        })
        .map(str::to_string);
    let latest_controller_age_hours =
        latest_controller_log.map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();
    if routing.channel_count == 0 {
        blocking_reasons.push("no delivery channel is configured".to_string());
    }
    if routing.unroutable_pending_count > 0 {
        blocking_reasons.push("pending approvals are unroutable".to_string());
    }
    match latest_run {
        None => blocking_reasons.push("delivery run has not been recorded".to_string()),
        Some(run) if run.failed_count > 0 || run.status == "failed" => {
            blocking_reasons.push("latest delivery run failed".to_string());
        }
        Some(run) if run.status == "reserved" => {
            blocking_reasons.push("latest delivery run was reserved".to_string());
        }
        Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => {
            blocking_reasons.push("latest delivery run is stale".to_string());
        }
        Some(_) => {}
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "approval notification ops controller is required but not configured".to_string(),
        );
    }
    if controller_required && !latest_controller_validated {
        blocking_reasons.push(
            "approval notification ops controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("approval notification ops controller evidence is stale".to_string());
    }
    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    };
    let message = if production_blocked {
        format!(
            "approval notification production ops are blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "approval notification routing, latest delivery run, and required ops controller evidence are ready".to_string()
    };
    ApprovalNotificationProductionOpsReadiness {
        status: status.to_string(),
        production_blocked,
        latest_run_status,
        latest_run_age_hours,
        routing_status: routing.status.clone(),
        channel_count: routing.channel_count,
        unroutable_pending_count: routing.unroutable_pending_count,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        message,
        blocking_reasons,
    }
}

pub(crate) fn build_approval_notification_deployment_readiness(
    audit_logs: &[AuditLog],
    routing: &ApprovalNotificationRoutingSummary,
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> ApprovalNotificationDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "approval.notification_deployment_validation_run")
        .max_by_key(|log| log.created_at);
    let controller_validation_logs = audit_logs
        .iter()
        .filter(|log| {
            log.action == "approval.notification_deployment_validation_run"
                && log
                    .details
                    .get("controller_execution")
                    .and_then(|execution| execution.get("attempted"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let controller_execution_count = controller_validation_logs.len();
    let controller_failed_count = controller_validation_logs
        .iter()
        .filter(|log| {
            log.details
                .get("controller_execution")
                .and_then(|execution| execution.get("status"))
                .and_then(Value::as_str)
                .is_some_and(|status| status != "validated")
        })
        .count();
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_age_hours =
        latest_validation_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let pending_approval_count = latest_validation
        .map(|log| json_usize(&log.details, "pending_approval_count"))
        .unwrap_or(routing.pending_approval_count);
    let routable_pending_count = latest_validation
        .map(|log| json_usize(&log.details, "routable_pending_count"))
        .unwrap_or(routing.routable_pending_count);
    let unroutable_pending_count = latest_validation
        .map(|log| json_usize(&log.details, "unroutable_pending_count"))
        .unwrap_or(routing.unroutable_pending_count);
    let channel_count = latest_validation
        .map(|log| json_usize(&log.details, "channel_count"))
        .unwrap_or(routing.channel_count);
    let persisted_policy_count = latest_validation
        .map(|log| json_usize(&log.details, "persisted_policy_count"))
        .unwrap_or(routing.persisted_policy_count);
    let active_policy_count = latest_validation
        .map(|log| json_usize(&log.details, "active_policy_count"))
        .unwrap_or(routing.active_policy_count);
    let latest_controller_status = latest_validation
        .and_then(|log| log.details.get("controller_execution"))
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_age_hours = latest_validation
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();

    if latest_validation.is_none() {
        blocking_reasons
            .push("approval notification deployment validation has not run".to_string());
    }
    if channel_count == 0 {
        blocking_reasons.push(
            "approval notification deployment validation found no delivery channels".to_string(),
        );
    }
    if active_policy_count == 0 {
        blocking_reasons.push(
            "approval notification deployment validation found no active channel policies"
                .to_string(),
        );
    }
    if unroutable_pending_count > 0 {
        blocking_reasons.push(
            "approval notification deployment validation found unroutable pending approvals"
                .to_string(),
        );
    }
    if latest_validation_status
        .as_deref()
        .is_some_and(|status| status != "healthy")
    {
        blocking_reasons
            .push("latest approval notification deployment validation was not healthy".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons
            .push("approval notification deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "approval notification deployment controller is required but not configured"
                .to_string(),
        );
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "approval notification deployment controller evidence is missing or not validated"
                .to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons
            .push("approval notification deployment controller evidence is stale".to_string());
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
            "Approval notification deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Approval notification deployment has a recent healthy validation run".to_string()
    };

    ApprovalNotificationDeploymentReadiness {
        status,
        production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        pending_approval_count,
        routable_pending_count,
        unroutable_pending_count,
        channel_count,
        persisted_policy_count,
        active_policy_count,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        controller_execution_count,
        controller_failed_count,
        deployment_validated: !production_blocked,
        blocking_reasons,
        message,
    }
}

pub(crate) fn approval_notification_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn approval_notification_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn approval_notification_ops_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn approval_notification_ops_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_approval_notification_ops_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    checked_at: DateTime<Utc>,
    routing: &ApprovalNotificationRoutingSummary,
    delivery_summary: &ApprovalNotificationDeliveryRunSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.approval_notification_ops",
        "subject": subject,
        "checked_at": checked_at,
        "routing": {
            "status": routing.status,
            "channel_count": routing.channel_count,
            "active_policy_count": routing.active_policy_count,
            "pending_approval_count": routing.pending_approval_count,
            "routable_pending_count": routing.routable_pending_count,
            "unroutable_pending_count": routing.unroutable_pending_count,
            "approval_group_count": routing.approval_group_count,
            "escalation_rule_count": routing.escalation_rule_count,
        },
        "delivery": {
            "run_count": delivery_summary.run_count,
            "delivered_run_count": delivery_summary.delivered_run_count,
            "failed_run_count": delivery_summary.failed_run_count,
            "latest_run_status": delivery_summary.latest_run.as_ref().map(|run| run.status.clone()),
            "latest_run_at": delivery_summary.latest_run.as_ref().map(|run| run.ran_at),
            "production_ops": delivery_summary.production_ops,
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
            "approval notification ops controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(controller_status, "validated" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "ops_id": body.get("ops_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "checks": body.get("checks").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn execute_approval_notification_deployment_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    routing: &ApprovalNotificationRoutingSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.approval_notification_deployment",
        "subject": subject,
        "checked_at": checked_at,
        "routing": {
            "status": routing.status,
            "pending_approval_count": routing.pending_approval_count,
            "routable_pending_count": routing.routable_pending_count,
            "unroutable_pending_count": routing.unroutable_pending_count,
            "channel_count": routing.channel_count,
            "persisted_policy_count": routing.persisted_policy_count,
            "active_policy_count": routing.active_policy_count,
            "webhook_configured": routing.webhook_configured,
            "slack_configured": routing.slack_configured,
            "email_relay_configured": routing.email_relay_configured,
        }
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
            "approval notification deployment controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(
        controller_status,
        "validated" | "deployed" | "healthy" | "success" | "ok"
    );
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn approval_notification_delivery_run_from_audit_log(
    log: &AuditLog,
) -> Option<ApprovalNotificationDeliveryRunRecord> {
    if log.action != "approval.notification_delivery_run" {
        return None;
    }
    Some(ApprovalNotificationDeliveryRunRecord {
        id: log.id,
        status: log
            .details
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        subject: log
            .details
            .get("subject")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        candidate_count: json_usize(&log.details, "candidate_count"),
        delivered_count: json_usize(&log.details, "delivered_count"),
        reserved_count: json_usize(&log.details, "reserved_count"),
        failed_count: json_usize(&log.details, "failed_count"),
        skipped_count: json_usize(&log.details, "skipped_count"),
        ran_at: log.created_at,
    })
}

pub(crate) async fn build_approval_notification_routing_summary(
    approvals: Vec<Approval>,
    approval_groups: Vec<ApprovalGroup>,
    escalation_rules: Vec<ApprovalEscalationRule>,
    channel_policies: Vec<ApprovalNotificationChannelPolicy>,
    webhook_configured: bool,
    slack_configured: bool,
    email_relay_configured: bool,
) -> ApprovalNotificationRoutingSummary {
    let generated_at = Utc::now();
    let active_channel_policies: Vec<_> = channel_policies
        .iter()
        .filter(|policy| policy.status == "active")
        .collect();
    let configured_env_channel_count =
        [webhook_configured, slack_configured, email_relay_configured]
            .into_iter()
            .filter(|configured| *configured)
            .count();
    let channel_count = if active_channel_policies.is_empty() {
        configured_env_channel_count
    } else {
        active_channel_policies
            .iter()
            .filter(|policy| {
                approval_notification_policy_has_target(
                    policy,
                    webhook_configured,
                    slack_configured,
                    email_relay_configured,
                )
            })
            .count()
    };
    let group_by_id: HashMap<Uuid, ApprovalGroup> = approval_groups
        .iter()
        .cloned()
        .map(|group| (group.id, group))
        .collect();
    let pending: Vec<_> = approvals
        .iter()
        .filter(|approval| approval.status == "pending")
        .collect();
    let mut delegated_pending_count = 0;
    let mut group_pending_count = 0;
    let mut routable_pending_count = 0;
    let mut unroutable_pending_count = 0;
    let mut attention_items = Vec::new();

    if active_channel_policies.is_empty() {
        attention_items.push(ApprovalNotificationRoutingAttention {
            kind: "missing_persisted_channel_policy".to_string(),
            severity: "warning".to_string(),
            message:
                "no persisted approval notification channel policy is configured; env fallback is reserved"
                    .to_string(),
            approval_id: None,
        });
    }
    if channel_count == 0 {
        attention_items.push(ApprovalNotificationRoutingAttention {
            kind: "missing_channel".to_string(),
            severity: "critical".to_string(),
            message: "no approval notification channel is configured".to_string(),
            approval_id: None,
        });
    }
    if approval_groups.is_empty() {
        attention_items.push(ApprovalNotificationRoutingAttention {
            kind: "missing_approval_group".to_string(),
            severity: "warning".to_string(),
            message: "no approval groups are configured for escalation fan-out".to_string(),
            approval_id: None,
        });
    }
    if escalation_rules.is_empty() {
        attention_items.push(ApprovalNotificationRoutingAttention {
            kind: "missing_escalation_rule".to_string(),
            severity: "warning".to_string(),
            message: "no approval escalation rules are configured".to_string(),
            approval_id: None,
        });
    }

    for approval in &pending {
        if delegated_approver_subject(approval).is_some() {
            delegated_pending_count += 1;
        }
        let group = delegated_approver_group_id(approval).and_then(|id| group_by_id.get(&id));
        if group.is_some() {
            group_pending_count += 1;
        }
        let targets = approval_notification_targets(approval, group);
        let channel_ready = approval_notification_channel_ready_for_approval(
            approval,
            &active_channel_policies,
            webhook_configured,
            slack_configured,
            email_relay_configured,
        );
        if channel_ready && !targets.is_empty() {
            routable_pending_count += 1;
        } else {
            unroutable_pending_count += 1;
            attention_items.push(ApprovalNotificationRoutingAttention {
                kind: if targets.is_empty() {
                    "missing_target"
                } else if active_channel_policies.is_empty() {
                    "missing_channel"
                } else {
                    "missing_matching_channel_policy"
                }
                .to_string(),
                severity: "critical".to_string(),
                message: if targets.is_empty() {
                    "pending approval has no delegated approver or approval group targets"
                        .to_string()
                } else if !active_channel_policies.is_empty() {
                    "pending approval has targets but no active matching notification channel policy"
                        .to_string()
                } else {
                    "pending approval has targets but no configured notification channel"
                        .to_string()
                },
                approval_id: Some(approval.id),
            });
        }
    }

    let status = if unroutable_pending_count > 0 || channel_count == 0 {
        "action_required"
    } else if attention_items
        .iter()
        .any(|item| item.severity == "warning")
    {
        "warning"
    } else {
        "ready"
    }
    .to_string();

    ApprovalNotificationRoutingSummary {
        status,
        generated_at,
        webhook_configured,
        slack_configured,
        email_relay_configured,
        channel_count,
        persisted_policy_count: channel_policies.len(),
        active_policy_count: active_channel_policies.len(),
        channel_policies,
        pending_approval_count: pending.len(),
        delegated_pending_count,
        group_pending_count,
        routable_pending_count,
        unroutable_pending_count,
        approval_group_count: approval_groups.len(),
        escalation_rule_count: escalation_rules.len(),
        attention_items,
    }
}

pub(crate) async fn deliver_approval_notification(
    state: &AppState,
    approval: &Approval,
    delivered_at: DateTime<Utc>,
) -> Result<ApprovalNotificationDelivery, AppError> {
    let approval_group_id = delegated_approver_group_id(approval);
    let approval_group = if let Some(group_id) = approval_group_id {
        state.get_approval_group(group_id).await.ok()
    } else {
        None
    };
    let target_subjects = approval_notification_targets(approval, approval_group.as_ref());
    let group_name = approval_group.as_ref().map(|group| group.name.clone());
    if target_subjects.is_empty() {
        return Ok(ApprovalNotificationDelivery {
            status: "reserved".to_string(),
            delivered: false,
            channel: "none".to_string(),
            webhook_configured: state.approval_webhook_url.is_some(),
            approval_id: approval.id,
            target_count: 0,
            target_subjects,
            group_id: approval_group_id,
            group_name,
            channel_deliveries: vec![],
            delivered_at,
        });
    }
    let channel_configs = approval_notification_channel_configs(state, approval).await?;
    if channel_configs.is_empty() {
        return Ok(ApprovalNotificationDelivery {
            status: "reserved".to_string(),
            delivered: false,
            channel: "webhook".to_string(),
            webhook_configured: false,
            approval_id: approval.id,
            target_count: target_subjects.len(),
            target_subjects,
            group_id: approval_group_id,
            group_name,
            channel_deliveries: vec![],
            delivered_at,
        });
    };
    let mut channel_deliveries = Vec::new();
    for config in &channel_configs {
        channel_deliveries.push(
            deliver_approval_notification_channel(
                config,
                approval,
                approval_group.as_ref(),
                &target_subjects,
                delivered_at,
            )
            .await?,
        );
    }
    let delivered = channel_deliveries.iter().any(|delivery| delivery.delivered);
    let channel = if channel_deliveries.len() == 1 {
        channel_deliveries
            .first()
            .map(|delivery| delivery.channel.clone())
            .unwrap_or_else(|| "webhook".to_string())
    } else {
        "multi".to_string()
    };
    let delivery = ApprovalNotificationDelivery {
        status: if delivered { "delivered" } else { "reserved" }.to_string(),
        delivered,
        channel,
        webhook_configured: state.approval_webhook_url.is_some(),
        approval_id: approval.id,
        target_count: target_subjects.len(),
        target_subjects,
        group_id: approval_group_id,
        group_name,
        channel_deliveries,
        delivered_at,
    };
    if delivery.delivered {
        state
            .append_audit_log(new_audit_log(
                Some(approval.session_id),
                "system",
                Some(approval.id),
                "approval.notification_delivered",
                "approval",
                Some(approval.id),
                serde_json::to_value(&delivery)?,
            ))
            .await?;
    }
    Ok(delivery)
}

struct ApprovalNotificationChannelConfig {
    channel: String,
    policy_id: Option<Uuid>,
    policy_name: Option<String>,
    url: Option<String>,
    max_attempts: usize,
    backoff_seconds: u64,
}

async fn approval_notification_channel_configs(
    state: &AppState,
    approval: &Approval,
) -> Result<Vec<ApprovalNotificationChannelConfig>, AppError> {
    let active_policies: Vec<_> = state
        .list_approval_notification_channel_policies()
        .await?
        .into_iter()
        .filter(|policy| policy.status == "active")
        .collect();
    if !active_policies.is_empty() {
        return Ok(active_policies
            .into_iter()
            .filter(|policy| approval_notification_policy_matches(policy, approval))
            .map(|policy| ApprovalNotificationChannelConfig {
                channel: policy.channel.clone(),
                policy_id: Some(policy.id),
                policy_name: Some(policy.name.clone()),
                url: approval_notification_policy_url(state, &policy),
                max_attempts: policy.max_attempts.max(1) as usize,
                backoff_seconds: policy.backoff_seconds.max(0) as u64,
            })
            .collect());
    }
    let mut configs = Vec::new();
    if let Some(url) = state.approval_webhook_url.clone() {
        configs.push(ApprovalNotificationChannelConfig {
            channel: "webhook".to_string(),
            policy_id: None,
            policy_name: None,
            url: Some(url),
            max_attempts: 1,
            backoff_seconds: 0,
        });
    }
    if let Some(url) = approval_slack_webhook_url_from_env() {
        configs.push(ApprovalNotificationChannelConfig {
            channel: "slack".to_string(),
            policy_id: None,
            policy_name: None,
            url: Some(url),
            max_attempts: 1,
            backoff_seconds: 0,
        });
    }
    if let Some(url) = approval_email_relay_url_from_env() {
        configs.push(ApprovalNotificationChannelConfig {
            channel: "email_relay".to_string(),
            policy_id: None,
            policy_name: None,
            url: Some(url),
            max_attempts: 1,
            backoff_seconds: 0,
        });
    }
    Ok(configs)
}

pub(crate) fn approval_notification_policy_has_target(
    policy: &ApprovalNotificationChannelPolicy,
    webhook_configured: bool,
    slack_configured: bool,
    email_relay_configured: bool,
) -> bool {
    if let Some(target_env) = policy.target_env.as_ref() {
        return std::env::var(target_env)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
    }
    match policy.channel.as_str() {
        "webhook" => webhook_configured,
        "slack" => slack_configured,
        "email_relay" => email_relay_configured,
        _ => false,
    }
}

pub(crate) fn approval_notification_channel_ready_for_approval(
    approval: &Approval,
    active_channel_policies: &[&ApprovalNotificationChannelPolicy],
    webhook_configured: bool,
    slack_configured: bool,
    email_relay_configured: bool,
) -> bool {
    if active_channel_policies.is_empty() {
        return webhook_configured || slack_configured || email_relay_configured;
    }
    active_channel_policies.iter().any(|policy| {
        approval_notification_policy_matches(policy, approval)
            && approval_notification_policy_has_target(
                policy,
                webhook_configured,
                slack_configured,
                email_relay_configured,
            )
    })
}

pub(crate) fn approval_notification_policy_url(
    state: &AppState,
    policy: &ApprovalNotificationChannelPolicy,
) -> Option<String> {
    if let Some(target_env) = policy.target_env.as_ref() {
        return std::env::var(target_env)
            .ok()
            .and_then(|value| (!value.trim().is_empty()).then(|| value));
    }
    match policy.channel.as_str() {
        "webhook" => state.approval_webhook_url.clone(),
        "slack" => approval_slack_webhook_url_from_env(),
        "email_relay" => approval_email_relay_url_from_env(),
        _ => None,
    }
}

pub(crate) fn approval_notification_policy_matches(
    policy: &ApprovalNotificationChannelPolicy,
    approval: &Approval,
) -> bool {
    policy.risk_filter == "all" || policy.risk_filter == approval.risk_level
}

async fn deliver_approval_notification_channel(
    config: &ApprovalNotificationChannelConfig,
    approval: &Approval,
    approval_group: Option<&ApprovalGroup>,
    target_subjects: &[String],
    delivered_at: DateTime<Utc>,
) -> Result<ApprovalNotificationChannelDelivery, AppError> {
    let Some(url) = config.url.as_ref() else {
        return Ok(ApprovalNotificationChannelDelivery {
            channel: config.channel.clone(),
            policy_id: config.policy_id,
            policy_name: config.policy_name.clone(),
            status: "reserved".to_string(),
            delivered: false,
            target_configured: false,
            attempt_count: 0,
            max_attempts: config.max_attempts,
            last_error: None,
        });
    };
    let payload = match config.channel.as_str() {
        "slack" => slack_approval_notification_payload(
            approval,
            approval_group,
            target_subjects,
            delivered_at,
        ),
        "email_relay" => email_approval_notification_payload(
            approval,
            approval_group,
            target_subjects,
            delivered_at,
        ),
        _ => json!({
            "type": "mandoforge.approval_requested",
            "approval": approval,
            "approval_group": approval_group,
            "target_subjects": target_subjects,
            "target_count": target_subjects.len(),
            "delivered_at": delivered_at,
        }),
    };
    let client = reqwest::Client::new();
    let mut last_error = None;
    for attempt in 1..=config.max_attempts {
        match tokio::time::timeout(
            Duration::from_secs(10),
            client.post(url).json(&payload).send(),
        )
        .await
        {
            Ok(Ok(response)) if response.status().is_success() => {
                return Ok(ApprovalNotificationChannelDelivery {
                    channel: config.channel.to_string(),
                    policy_id: config.policy_id,
                    policy_name: config.policy_name.clone(),
                    status: "delivered".to_string(),
                    delivered: true,
                    target_configured: true,
                    attempt_count: attempt,
                    max_attempts: config.max_attempts,
                    last_error: None,
                });
            }
            Ok(Ok(response)) => {
                last_error = Some(format!(
                    "approval {} notification returned status {}",
                    config.channel,
                    response.status()
                ));
            }
            Ok(Err(error)) => {
                last_error = Some(format!(
                    "approval {} notification request failed: {error}",
                    config.channel
                ));
            }
            Err(_) => {
                last_error = Some(format!(
                    "approval {} notification timed out after 10 seconds",
                    config.channel
                ));
            }
        }
        if attempt < config.max_attempts && config.backoff_seconds > 0 {
            tokio::time::sleep(Duration::from_secs(config.backoff_seconds)).await;
        }
    }
    Err(AppError::bad_request(last_error.unwrap_or_else(|| {
        format!("approval {} notification failed", config.channel)
    })))
}

pub(crate) fn slack_approval_notification_payload(
    approval: &Approval,
    approval_group: Option<&ApprovalGroup>,
    target_subjects: &[String],
    delivered_at: DateTime<Utc>,
) -> Value {
    let reason = approval.reason.clone();
    let group_name = approval_group
        .map(|group| group.name.clone())
        .unwrap_or_else(|| "direct".to_string());
    json!({
        "text": format!("MandoForge approval requested: {} ({})", approval.risk_level, reason),
        "blocks": [
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Approval requested*\nRisk: `{}`\nReason: {}\nTargets: {}", approval.risk_level, reason, target_subjects.join(", "))
                }
            },
            {
                "type": "context",
                "elements": [
                    {
                        "type": "mrkdwn",
                        "text": format!("approval `{}` · group `{}` · delivered {}", approval.id, group_name, delivered_at)
                    }
                ]
            }
        ]
    })
}

pub(crate) fn email_approval_notification_payload(
    approval: &Approval,
    approval_group: Option<&ApprovalGroup>,
    target_subjects: &[String],
    delivered_at: DateTime<Utc>,
) -> Value {
    let reason = approval.reason.clone();
    json!({
        "type": "mandoforge.approval_email",
        "to_subjects": target_subjects,
        "subject": format!("MandoForge approval requested: {}", approval.risk_level),
        "text": format!(
            "Approval {} requires review.\nRisk: {}\nReason: {}\nGroup: {}\nDelivered: {}",
            approval.id,
            approval.risk_level,
            reason,
            approval_group.map(|group| group.name.as_str()).unwrap_or("direct"),
            delivered_at
        ),
        "approval_id": approval.id,
        "session_id": approval.session_id,
        "risk_level": approval.risk_level,
        "delivered_at": delivered_at,
    })
}

pub(crate) fn approval_notification_targets(
    approval: &Approval,
    group: Option<&ApprovalGroup>,
) -> Vec<String> {
    if let Some(group) = group {
        return group.subjects.clone();
    }
    delegated_approver_subject(approval)
        .map(|subject| vec![subject.to_string()])
        .unwrap_or_default()
}

pub(crate) async fn authorize_approval_decision(
    state: &AppState,
    headers: &HeaderMap,
    approval_id: Uuid,
) -> Result<Principal, AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::ApprovalsDecide,
        resource_type: "approval".to_string(),
        resource_id: Some(approval_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let approval = state.get_approval(approval_id).await?;
    let session_request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsRead,
        resource_type: "session".to_string(),
        resource_id: Some(approval.session_id),
    };
    enforce_resource_scope(state, &principal, &session_request).await?;
    enforce_delegated_approver(state, &principal, &approval).await?;
    Ok(principal)
}

pub(crate) async fn enforce_delegated_approver(
    state: &AppState,
    principal: &Principal,
    approval: &Approval,
) -> Result<(), AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(());
    }
    if let Some(approver_subject) = delegated_approver_subject(approval) {
        if principal.subject_id == approver_subject {
            return Ok(());
        }
        return Err(AppError::forbidden(format!(
            "approval is delegated to {approver_subject}"
        )));
    }
    if let Some(group_id) = delegated_approver_group_id(approval) {
        let group = state.get_approval_group(group_id).await?;
        if group
            .subjects
            .iter()
            .any(|subject| subject == &principal.subject_id)
        {
            return Ok(());
        }
        return Err(AppError::forbidden(format!(
            "approval is delegated to approval group {}",
            group.name
        )));
    }
    if approval.risk_level == "high" {
        return Err(AppError::forbidden(
            "high-risk approvals require an explicit approver subject or approval group",
        ));
    }
    Ok(())
}

pub(crate) fn delegated_approver_subject(approval: &Approval) -> Option<&str> {
    approval
        .evidence
        .get("approver_subject")
        .or_else(|| approval.evidence.get("delegated_approver"))
        .or_else(|| {
            approval
                .evidence
                .get("args")
                .and_then(|args| args.get("approver_subject"))
        })
        .or_else(|| {
            approval
                .evidence
                .get("args")
                .and_then(|args| args.get("delegated_approver"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn delegated_approver_group_id(approval: &Approval) -> Option<Uuid> {
    approval
        .evidence
        .get("approver_group_id")
        .or_else(|| approval.evidence.get("delegated_approver_group_id"))
        .or_else(|| {
            approval
                .evidence
                .get("args")
                .and_then(|args| args.get("approver_group_id"))
        })
        .or_else(|| {
            approval
                .evidence
                .get("args")
                .and_then(|args| args.get("delegated_approver_group_id"))
        })
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
}
