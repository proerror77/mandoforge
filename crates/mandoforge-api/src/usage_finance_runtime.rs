use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn build_usage_finance_dashboard_summary(
    state: &AppState,
) -> Result<UsageFinanceDashboardSummary, AppError> {
    let generated_at = Utc::now();
    let usage = build_usage_summary(state).await?;
    let rollups = state.list_usage_rollups().await?;
    let alerts = build_cost_alerts(&usage.provider_budgets, generated_at);
    let trend = build_usage_trend_from_parts(usage, &rollups, generated_at);
    let alert_routes = state.list_cost_alert_routes().await?;
    Ok(build_usage_finance_dashboard_summary_from_parts(
        trend,
        &rollups,
        &alert_routes,
        &alerts,
        usage_finance_export_webhook_url().is_some(),
        usage_finance_export_schedule_enabled(),
        generated_at,
    ))
}

pub(crate) async fn build_usage_finance_operations_summary(
    state: &AppState,
) -> Result<UsageFinanceOperationsSummary, AppError> {
    let generated_at = Utc::now();
    let usage = build_usage_summary(state).await?;
    let rollups = state.list_usage_rollups().await?;
    let alerts = build_cost_alerts(&usage.provider_budgets, generated_at);
    let trend = build_usage_trend_from_parts(usage, &rollups, generated_at);
    let alert_routes = state.list_cost_alert_routes().await?;
    let dashboard = build_usage_finance_dashboard_summary_from_parts(
        trend,
        &rollups,
        &alert_routes,
        &alerts,
        usage_finance_export_webhook_url().is_some(),
        usage_finance_export_schedule_enabled(),
        generated_at,
    );
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(build_usage_finance_operations_summary_from_parts(
        dashboard,
        &alerts,
        &audit_logs,
        generated_at,
    ))
}

pub(crate) async fn execute_usage_finance_operations(
    state: &AppState,
    subject: Option<&str>,
) -> Result<UsageFinanceOperationsRun, AppError> {
    execute_usage_finance_operations_with_lookup(state, subject, |key| std::env::var(key).ok())
        .await
}

pub(crate) async fn execute_usage_finance_operations_with_lookup<F>(
    state: &AppState,
    subject: Option<&str>,
    lookup: F,
) -> Result<UsageFinanceOperationsRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let ran_at = Utc::now();
    let before = build_usage_finance_operations_summary(state).await?;
    let mut actions = Vec::new();
    let mut rollup_created = None;
    let mut cost_alert_delivery = None;
    let mut finance_export_delivery = None;
    let close_controller_configured = usage_finance_close_controller_configured(&lookup);
    let reconciliation_controller_configured =
        usage_finance_reconciliation_controller_configured(&lookup);

    if before.rollup_status != "fresh" {
        let period_end = ran_at;
        let period_start = period_end - chrono::Duration::hours(24);
        let summary = serde_json::to_value(build_usage_summary(state).await?)?;
        let rollup = state
            .create_usage_rollup(period_start, period_end, summary)
            .await?;
        actions.push("usage_rollup_created".to_string());
        rollup_created = Some(rollup);
    }

    if before.open_alert_count > 0
        && before.active_alert_route_count > 0
        && before.alert_delivery_status != "delivered"
    {
        let delivery = execute_cost_alert_delivery(state, ran_at).await?;
        actions.push("cost_alert_delivery_processed".to_string());
        cost_alert_delivery = Some(delivery);
    }

    if before.export_status != "target_missing"
        && before
            .last_finance_export
            .as_ref()
            .is_none_or(|audit| (ran_at - audit.created_at).num_hours() >= 20)
    {
        let delivery = execute_usage_finance_export_delivery(state, false, "user", subject).await?;
        actions.push("usage_finance_export_processed".to_string());
        finance_export_delivery = Some(delivery);
    }

    let after = build_usage_finance_operations_summary(state).await?;
    let mut close_controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if close_controller_configured {
            "production_close_gate_not_ready"
        } else {
            "close_controller_not_configured"
        }
    });
    if close_controller_configured
        && finance_close_controller_prerequisites_ready(&after.production_close)
    {
        match execute_usage_finance_close_controller(
            &lookup,
            subject,
            ran_at,
            &before,
            &after,
            rollup_created.is_some(),
            cost_alert_delivery.as_ref(),
            finance_export_delivery.as_ref(),
        )
        .await
        {
            Ok(execution) => {
                let execution_status = required_controller_status(&execution)?.to_string();
                close_controller_execution = execution;
                actions.push("usage_finance_close_controller_executed".to_string());
                if execution_status != "closed" {
                    actions.push("usage_finance_close_controller_attention".to_string());
                }
            }
            Err(error) => {
                close_controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
                actions.push("usage_finance_close_controller_failed".to_string());
            }
        }
    }
    let reconciliation_controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if reconciliation_controller_configured {
            "use_reconcile_endpoint_for_accounting_system_validation"
        } else {
            "reconciliation_controller_not_configured"
        }
    });
    let run = UsageFinanceOperationsRun {
        status: if actions.is_empty() {
            "no_action".to_string()
        } else if close_controller_execution
            .get("status")
            .and_then(Value::as_str)
            == Some("failed")
        {
            "attention".to_string()
        } else {
            "completed".to_string()
        },
        ran_at,
        actions,
        before,
        after,
        rollup_created,
        cost_alert_delivery,
        finance_export_delivery,
        close_controller_configured,
        close_controller_execution,
        reconciliation_controller_configured,
        reconciliation_controller_execution,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.finance_operations_run",
            "usage_finance_operations",
            None,
            json!({
                "subject": subject,
                "status": run.status,
                "actions": run.actions,
                "rollup_created": run.rollup_created.is_some(),
                "cost_alert_delivery_status": run.cost_alert_delivery.as_ref().map(|delivery| delivery.status.clone()),
                "finance_export_delivery_status": run.finance_export_delivery.as_ref().map(|delivery| delivery.status.clone()),
                "close_controller_configured": run.close_controller_configured,
                "close_controller_execution": run.close_controller_execution,
                "reconciliation_controller_configured": run.reconciliation_controller_configured,
                "reconciliation_controller_execution": run.reconciliation_controller_execution,
                "before_status": run.before.status,
                "after_status": run.after.status,
                "ran_at": run.ran_at,
            }),
        ))
        .await?;
    Ok(run)
}

pub(crate) fn finance_close_controller_prerequisites_ready(
    production_close: &UsageFinanceProductionCloseReadiness,
) -> bool {
    if !production_close.production_blocked {
        return true;
    }
    let allowed_bootstrap_reasons = [
        "finance close controller has no recent closed evidence",
        "finance close controller evidence is stale",
        "finance reconciliation controller has no recent reconciled evidence",
        "finance reconciliation controller evidence is stale",
    ];
    !production_close.blocking_reasons.is_empty()
        && production_close.blocking_reasons.iter().all(|reason| {
            allowed_bootstrap_reasons
                .iter()
                .any(|allowed| reason == allowed)
        })
}

pub(crate) fn usage_finance_close_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn usage_finance_close_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn usage_finance_reconciliation_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn usage_finance_reconciliation_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) async fn execute_usage_finance_close_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    ran_at: DateTime<Utc>,
    before: &UsageFinanceOperationsSummary,
    after: &UsageFinanceOperationsSummary,
    rollup_created: bool,
    cost_alert_delivery: Option<&CostAlertDelivery>,
    finance_export_delivery: Option<&UsageFinanceExportDelivery>,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_FINANCE_CLOSE_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_FINANCE_CLOSE_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.finance_close",
        "subject": subject,
        "ran_at": ran_at,
        "before": {
            "status": before.status,
            "readiness_score": before.readiness_score,
            "production_close": before.production_close,
        },
        "after": {
            "status": after.status,
            "readiness_score": after.readiness_score,
            "production_close": after.production_close,
        },
        "rollup_created": rollup_created,
        "cost_alert_delivery_status": cost_alert_delivery.map(|delivery| delivery.status.clone()),
        "finance_export_delivery_status": finance_export_delivery.map(|delivery| delivery.status.clone()),
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (http_status, body) =
        controller_response_json(response, "finance close controller").await?;
    let controller_status = required_controller_status(&body)?;
    let closed = matches!(controller_status, "closed" | "success" | "ok" | "validated");
    Ok(json!({
        "attempted": true,
        "status": if closed { "closed" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "close_id": body.get("close_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn execute_usage_finance_reconciliation_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    ran_at: DateTime<Utc>,
    summary: &UsageFinanceOperationsSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_FINANCE_RECONCILIATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.finance_reconciliation",
        "subject": subject,
        "ran_at": ran_at,
        "summary": {
            "status": summary.status,
            "readiness_score": summary.readiness_score,
            "open_alert_count": summary.open_alert_count,
            "unacknowledged_alert_count": summary.unacknowledged_alert_count,
            "rollup_status": summary.rollup_status,
            "export_status": summary.export_status,
            "alert_delivery_status": summary.alert_delivery_status,
            "last_finance_export": summary.last_finance_export,
            "last_alert_delivery": summary.last_alert_delivery,
            "last_alert_acknowledgement": summary.last_alert_acknowledgement,
            "production_close": summary.production_close,
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
    let (http_status, body) =
        controller_response_json(response, "finance reconciliation controller").await?;
    let controller_status = required_controller_status(&body)?;
    let reconciled = matches!(
        controller_status,
        "reconciled" | "success" | "ok" | "validated"
    );
    Ok(json!({
        "attempted": true,
        "status": if reconciled { "reconciled" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "reconciliation_id": body.get("reconciliation_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "checks": body.get("checks").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn build_usage_finance_dashboard_summary_from_parts(
    trend: UsageTrendSummary,
    rollups: &[UsageRollup],
    alert_routes: &[CostAlertRoute],
    alerts: &[CostAlert],
    finance_export_target_configured: bool,
    finance_export_schedule_enabled: bool,
    generated_at: DateTime<Utc>,
) -> UsageFinanceDashboardSummary {
    let latest_rollup_at = rollups
        .iter()
        .map(|rollup| rollup.period_end)
        .max()
        .or_else(|| rollups.iter().map(|rollup| rollup.created_at).max());
    let latest_rollup_age_hours =
        latest_rollup_at.map(|latest| (generated_at - latest).num_hours().max(0));
    let active_alert_route_count = alert_routes
        .iter()
        .filter(|route| route.status == "active")
        .count();
    let critical_alert_count = alerts
        .iter()
        .filter(|alert| alert.severity == "critical")
        .count();
    let warning_alert_count = alerts
        .iter()
        .filter(|alert| alert.severity == "warning")
        .count();
    let forecast_7d_cost_cents = trend
        .forecast
        .horizons
        .iter()
        .find(|horizon| horizon.days == 7)
        .map(|horizon| horizon.projected_cost_cents);
    let forecast_30d_cost_cents = trend
        .forecast
        .horizons
        .iter()
        .find(|horizon| horizon.days == 30)
        .map(|horizon| horizon.projected_cost_cents);
    let mut attention_items = Vec::new();

    for alert in alerts {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "budget_alert".to_string(),
            severity: alert.severity.clone(),
            message: alert.message.clone(),
            provider_name: Some(alert.provider_name.clone()),
        });
    }
    if alerts.is_empty() && trend.budget_pressure.pressure_count > 0 {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "budget_pressure_unrouted".to_string(),
            severity: trend.budget_pressure.highest_status.clone(),
            message: "budget pressure exists but no matching cost alert was generated".to_string(),
            provider_name: None,
        });
    }
    if !alerts.is_empty() && active_alert_route_count == 0 {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "missing_alert_route".to_string(),
            severity: "critical".to_string(),
            message: "cost alerts exist but no active alert route is configured".to_string(),
            provider_name: None,
        });
    }
    if rollups.is_empty() {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "missing_usage_rollup".to_string(),
            severity: "warning".to_string(),
            message: "no persisted usage rollup exists for finance comparison".to_string(),
            provider_name: None,
        });
    } else if latest_rollup_age_hours.is_some_and(|hours| hours >= 30) {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "stale_usage_rollup".to_string(),
            severity: "warning".to_string(),
            message: "latest usage rollup is older than 30 hours".to_string(),
            provider_name: None,
        });
    }
    if !finance_export_target_configured {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "finance_export_target_missing".to_string(),
            severity: "warning".to_string(),
            message: "finance export webhook target is not configured".to_string(),
            provider_name: None,
        });
    }
    if trend
        .cost_delta_percent
        .is_some_and(|cost_delta_percent| cost_delta_percent >= 25.0)
    {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "cost_growth".to_string(),
            severity: "warning".to_string(),
            message: "provider cost increased by at least 25 percent versus comparison period"
                .to_string(),
            provider_name: trend
                .top_provider_by_cost
                .as_ref()
                .map(|provider| provider.provider_name.clone()),
        });
    }

    UsageFinanceDashboardSummary {
        generated_at,
        current_cost_cents: trend.current_cost_cents,
        current_total_tokens: trend.current_total_tokens,
        current_tool_calls: trend.current_tool_calls,
        comparison_basis: trend.comparison_basis,
        budget_pressure_status: trend.budget_pressure.highest_status,
        budget_pressure_count: trend.budget_pressure.pressure_count,
        critical_budget_count: trend.budget_pressure.critical_count,
        warning_budget_count: trend.budget_pressure.warning_count,
        alert_count: alerts.len(),
        critical_alert_count,
        warning_alert_count,
        alert_route_count: alert_routes.len(),
        active_alert_route_count,
        rollup_count: rollups.len(),
        latest_rollup_at,
        latest_rollup_age_hours,
        finance_export_target_configured,
        finance_export_schedule_enabled,
        forecast_7d_cost_cents,
        forecast_30d_cost_cents,
        top_provider_by_cost: trend.top_provider_by_cost,
        recommendations: trend.recommendations,
        attention_items,
    }
}

pub(crate) fn build_usage_finance_operations_summary_from_parts(
    dashboard: UsageFinanceDashboardSummary,
    alerts: &[CostAlert],
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> UsageFinanceOperationsSummary {
    let acknowledgement_cutoff = generated_at - chrono::Duration::hours(24);
    let acknowledged_alert_count = alerts
        .iter()
        .filter(|alert| {
            audit_logs.iter().any(|log| {
                log.action == "usage.alert_acknowledged"
                    && log.created_at >= acknowledgement_cutoff
                    && log.details.get("provider_name").and_then(Value::as_str)
                        == Some(alert.provider_name.as_str())
                    && log.details.get("severity").and_then(Value::as_str)
                        == Some(alert.severity.as_str())
            })
        })
        .count();
    let unacknowledged_alert_count = alerts.len().saturating_sub(acknowledged_alert_count);
    let last_finance_export =
        latest_usage_finance_audit(audit_logs, "usage.finance_export_delivered");
    let last_alert_delivery = latest_usage_finance_audit(audit_logs, "usage.cost_alerts_delivered");
    let last_alert_acknowledgement =
        latest_usage_finance_audit(audit_logs, "usage.alert_acknowledged");
    let last_accounting_reconciliation =
        latest_usage_finance_audit(audit_logs, "usage.finance_reconciliation_run");
    let mut attention_items = dashboard.attention_items.clone();
    let mut runbook_actions = dashboard.recommendations.clone();

    if unacknowledged_alert_count > 0 {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "cost_alert_acknowledgement_missing".to_string(),
            severity: "warning".to_string(),
            message: format!(
                "{unacknowledged_alert_count} current budget alert(s) have no recent acknowledgement"
            ),
            provider_name: None,
        });
        runbook_actions.push("acknowledge_or_escalate_cost_alerts".to_string());
    }
    if dashboard.finance_export_schedule_enabled && !dashboard.finance_export_target_configured {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "scheduled_finance_export_blocked".to_string(),
            severity: "critical".to_string(),
            message: "scheduled finance export is enabled but no delivery target is configured"
                .to_string(),
            provider_name: None,
        });
        runbook_actions.push("configure_finance_export_webhook".to_string());
    }
    if dashboard.finance_export_schedule_enabled
        && last_finance_export
            .as_ref()
            .is_none_or(|audit| (generated_at - audit.created_at).num_hours() >= 30)
    {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "finance_export_not_recent".to_string(),
            severity: "warning".to_string(),
            message: "scheduled finance export has no delivery audit in the last 30 hours"
                .to_string(),
            provider_name: None,
        });
        runbook_actions.push("run_or_debug_scheduled_finance_export".to_string());
    }
    if !alerts.is_empty() && last_alert_delivery.is_none() {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "cost_alert_delivery_missing".to_string(),
            severity: "warning".to_string(),
            message: "current budget alerts have not been delivered through an alert route"
                .to_string(),
            provider_name: None,
        });
        runbook_actions.push("deliver_cost_alerts".to_string());
    }
    if dashboard.rollup_count == 0 {
        runbook_actions.push("create_daily_usage_rollup".to_string());
    }

    let rollup_status = if dashboard.rollup_count == 0 {
        "missing"
    } else if dashboard
        .latest_rollup_age_hours
        .is_some_and(|hours| hours >= 30)
    {
        "stale"
    } else {
        "fresh"
    }
    .to_string();
    let export_status = if !dashboard.finance_export_target_configured {
        "target_missing"
    } else if dashboard.finance_export_schedule_enabled {
        "scheduled"
    } else {
        "manual_ready"
    }
    .to_string();
    let alert_delivery_status = if alerts.is_empty() {
        "no_alerts"
    } else if dashboard.active_alert_route_count == 0 {
        "route_missing"
    } else if last_alert_delivery.is_none() {
        "pending_delivery"
    } else {
        "delivered"
    }
    .to_string();
    let production_close = build_usage_finance_production_close_readiness(
        &dashboard,
        alerts,
        audit_logs,
        &rollup_status,
        &alert_delivery_status,
        last_finance_export.as_ref(),
        last_alert_delivery.as_ref(),
        generated_at,
        usage_finance_close_controller_required(&|key| std::env::var(key).ok()),
        usage_finance_close_controller_configured(&|key| std::env::var(key).ok()),
        usage_finance_reconciliation_controller_required(&|key| std::env::var(key).ok()),
        usage_finance_reconciliation_controller_configured(&|key| std::env::var(key).ok()),
    );
    if production_close.production_blocked {
        attention_items.push(UsageFinanceAttentionItem {
            kind: "finance_production_close_blocked".to_string(),
            severity: "critical".to_string(),
            message: production_close.message.clone(),
            provider_name: None,
        });
        runbook_actions.push("resolve_finance_production_close_gate".to_string());
    }
    dedupe_strings(&mut runbook_actions);
    let critical_attention_count = attention_items
        .iter()
        .filter(|item| item.severity == "critical")
        .count();
    let warning_attention_count = attention_items
        .iter()
        .filter(|item| item.severity == "warning")
        .count();
    let status = if critical_attention_count > 0 {
        "critical"
    } else if warning_attention_count > 0 {
        "attention"
    } else {
        "ready"
    }
    .to_string();
    let readiness_score = (100_i64
        - (critical_attention_count as i64 * 25)
        - (warning_attention_count as i64 * 10)
        - (unacknowledged_alert_count as i64 * 5))
        .clamp(0, 100);

    UsageFinanceOperationsSummary {
        generated_at,
        status,
        readiness_score,
        open_alert_count: alerts.len(),
        acknowledged_alert_count,
        unacknowledged_alert_count,
        active_alert_route_count: dashboard.active_alert_route_count,
        rollup_status,
        export_status,
        alert_delivery_status,
        last_finance_export,
        last_alert_delivery,
        last_alert_acknowledgement,
        last_accounting_reconciliation,
        production_close,
        runbook_actions,
        attention_items,
    }
}

pub(crate) fn build_usage_finance_production_close_readiness(
    dashboard: &UsageFinanceDashboardSummary,
    alerts: &[CostAlert],
    audit_logs: &[AuditLog],
    rollup_status: &str,
    alert_delivery_status: &str,
    last_finance_export: Option<&UsageFinanceOperationAudit>,
    last_alert_delivery: Option<&UsageFinanceOperationAudit>,
    generated_at: DateTime<Utc>,
    close_controller_required: bool,
    close_controller_configured: bool,
    reconciliation_controller_required: bool,
    reconciliation_controller_configured: bool,
) -> UsageFinanceProductionCloseReadiness {
    let acknowledgement_cutoff = generated_at - chrono::Duration::hours(24);
    let critical_alerts_acknowledged = alerts
        .iter()
        .filter(|alert| alert.severity == "critical")
        .all(|alert| {
            audit_logs.iter().any(|log| {
                log.action == "usage.alert_acknowledged"
                    && log.created_at >= acknowledgement_cutoff
                    && log.details.get("provider_name").and_then(Value::as_str)
                        == Some(alert.provider_name.as_str())
                    && log.details.get("severity").and_then(Value::as_str)
                        == Some(alert.severity.as_str())
            })
        });
    let export_recent = last_finance_export.as_ref().is_some_and(|audit| {
        audit.status == "delivered" && (generated_at - audit.created_at).num_hours() < 30
    });
    let alert_delivery_ready = alerts.is_empty()
        || (alert_delivery_status == "delivered"
            && last_alert_delivery
                .as_ref()
                .is_some_and(|audit| audit.status != "failed"));
    let failed_delivery_evidence = last_finance_export
        .as_ref()
        .is_some_and(|audit| audit.status == "failed")
        || last_alert_delivery
            .as_ref()
            .is_some_and(|audit| audit.status == "failed");
    let latest_close_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "usage.finance_operations_run")
        .max_by_key(|log| log.created_at);
    let latest_close_controller_status = latest_close_controller_log
        .and_then(|log| log.details["close_controller_execution"]["status"].as_str())
        .map(str::to_string);
    let latest_close_controller_age_hours = latest_close_controller_log
        .filter(|_| latest_close_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let close_controller_evidence_fresh =
        latest_close_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_close_controller_closed = latest_close_controller_status
        .as_deref()
        .map(|status| status == "closed")
        .unwrap_or(false);
    let latest_reconciliation_log = audit_logs
        .iter()
        .filter(|log| log.action == "usage.finance_reconciliation_run")
        .max_by_key(|log| log.created_at);
    let latest_reconciliation_status = latest_reconciliation_log
        .and_then(|log| {
            log.details["reconciliation_controller_execution"]["status"]
                .as_str()
                .or_else(|| log.details["status"].as_str())
        })
        .map(str::to_string);
    let latest_reconciliation_age_hours = latest_reconciliation_log
        .filter(|_| latest_reconciliation_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let reconciliation_evidence_fresh =
        latest_reconciliation_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_reconciliation_reconciled = latest_reconciliation_status
        .as_deref()
        .map(|status| status == "reconciled")
        .unwrap_or(false);
    let rollup_fresh = rollup_status == "fresh";
    let mut blocking_reasons = Vec::new();

    if !rollup_fresh {
        blocking_reasons.push("usage rollup is missing or stale".to_string());
    }
    if !dashboard.finance_export_target_configured {
        blocking_reasons.push("finance export target is not configured".to_string());
    }
    if !export_recent {
        blocking_reasons.push("finance export has no recent delivered audit evidence".to_string());
    }
    if !alert_delivery_ready {
        blocking_reasons.push("current cost alerts have not been delivered".to_string());
    }
    if !critical_alerts_acknowledged {
        blocking_reasons.push("critical cost alerts have not been acknowledged".to_string());
    }
    if failed_delivery_evidence {
        blocking_reasons.push("failed finance or alert delivery evidence is present".to_string());
    }
    if close_controller_required && !close_controller_configured {
        blocking_reasons
            .push("finance close controller is required but not configured".to_string());
    }
    if close_controller_required && !latest_close_controller_closed {
        blocking_reasons.push("finance close controller has no recent closed evidence".to_string());
    }
    if close_controller_required
        && latest_close_controller_closed
        && !close_controller_evidence_fresh
    {
        blocking_reasons.push("finance close controller evidence is stale".to_string());
    }
    if reconciliation_controller_required && !reconciliation_controller_configured {
        blocking_reasons
            .push("finance reconciliation controller is required but not configured".to_string());
    }
    if reconciliation_controller_required && !latest_reconciliation_reconciled {
        blocking_reasons.push(
            "finance reconciliation controller has no recent reconciled evidence".to_string(),
        );
    }
    if reconciliation_controller_required
        && latest_reconciliation_reconciled
        && !reconciliation_evidence_fresh
    {
        blocking_reasons.push("finance reconciliation controller evidence is stale".to_string());
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
            "Finance production close is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Finance production close has fresh rollup, recent export evidence, delivered alerts, acknowledged critical alerts, and required reconciliation evidence".to_string()
    };

    UsageFinanceProductionCloseReadiness {
        status,
        production_blocked,
        rollup_fresh,
        export_target_configured: dashboard.finance_export_target_configured,
        export_recent,
        alert_delivery_ready,
        critical_alerts_acknowledged,
        failed_delivery_evidence,
        close_controller_required,
        close_controller_configured,
        latest_close_controller_status,
        latest_close_controller_age_hours,
        close_controller_evidence_fresh,
        latest_close_controller_closed,
        reconciliation_controller_required,
        reconciliation_controller_configured,
        latest_reconciliation_status,
        latest_reconciliation_age_hours,
        reconciliation_evidence_fresh,
        latest_reconciliation_reconciled,
        blocking_reasons,
        message,
    }
}

pub(crate) fn latest_usage_finance_audit(
    audit_logs: &[AuditLog],
    action: &str,
) -> Option<UsageFinanceOperationAudit> {
    audit_logs
        .iter()
        .filter(|log| log.action == action)
        .max_by_key(|log| log.created_at)
        .map(|log| UsageFinanceOperationAudit {
            action: log.action.clone(),
            status: log
                .details
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("recorded")
                .to_string(),
            subject: log
                .details
                .get("subject")
                .and_then(Value::as_str)
                .or_else(|| log.details.get("acknowledged_by").and_then(Value::as_str))
                .map(ToString::to_string),
            created_at: log.created_at,
        })
}

pub(crate) fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

pub(crate) async fn execute_usage_finance_export_delivery(
    state: &AppState,
    scheduled: bool,
    actor_type: &str,
    subject: Option<&str>,
) -> Result<UsageFinanceExportDelivery, AppError> {
    let delivered_at = Utc::now();
    let delivery_id = Uuid::new_v4();
    let file_name = "mandoforge-usage-export.csv".to_string();
    if scheduled && !usage_finance_export_schedule_enabled() {
        return Ok(UsageFinanceExportDelivery {
            status: "disabled".to_string(),
            delivered: false,
            channel: "webhook".to_string(),
            scheduled,
            target_configured: usage_finance_export_webhook_url().is_some(),
            delivery_id,
            file_name,
            bytes: 0,
            export_bytes: 0,
            record_count: 0,
            provider_count: 0,
            budget_pressure_count: 0,
            rollup_count: 0,
            delivered_at,
        });
    }

    let summary = build_usage_summary(state).await?;
    let trend = build_usage_trend_summary(state).await?;
    let csv = build_usage_finance_csv(&summary, &trend);
    let webhook_url = usage_finance_export_webhook_url();
    let mut delivery = UsageFinanceExportDelivery {
        status: if webhook_url.is_some() {
            "pending".to_string()
        } else {
            "reserved".to_string()
        },
        delivered: false,
        channel: "webhook".to_string(),
        scheduled,
        target_configured: webhook_url.is_some(),
        delivery_id,
        file_name: file_name.clone(),
        bytes: csv.len(),
        export_bytes: csv.len(),
        record_count: summary.by_provider.len() + trend.rollup_count,
        provider_count: summary.by_provider.len(),
        budget_pressure_count: trend.budget_pressure.pressure_count,
        rollup_count: trend.rollup_count,
        delivered_at,
    };

    if let Some(webhook_url) = webhook_url {
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            reqwest::Client::new()
                .post(&webhook_url)
                .json(&json!({
                    "type": "mandoforge.usage_finance_export",
                    "delivery_id": delivery.delivery_id,
                    "filename": delivery.file_name,
                    "file_name": delivery.file_name,
                    "export_bytes": delivery.export_bytes,
                    "byte_count": delivery.export_bytes,
                    "record_count": delivery.record_count,
                    "csv": csv,
                    "scheduled": scheduled,
                    "provider_count": summary.by_provider.len(),
                    "budget_pressure_count": trend.budget_pressure.pressure_count,
                    "rollup_count": trend.rollup_count,
                    "delivered_at": delivered_at,
                }))
                .send(),
        )
        .await??;
        if !response.status().is_success() {
            return Err(AppError::bad_request(format!(
                "usage finance export webhook returned status {}",
                response.status()
            )));
        }
        delivery.status = "delivered".to_string();
        delivery.delivered = true;
    }

    state
        .append_audit_log(new_audit_log(
            None,
            actor_type,
            None,
            "usage.finance_export_delivered",
            "usage_export",
            None,
            json!({
                "subject": subject,
                "status": delivery.status,
                "delivered": delivery.delivered,
                "scheduled": scheduled,
                "target_configured": delivery.target_configured,
                "delivery_id": delivery.delivery_id,
                "file_name": delivery.file_name,
                "bytes": delivery.bytes,
                "export_bytes": delivery.export_bytes,
                "record_count": delivery.record_count,
                "provider_count": delivery.provider_count,
                "budget_pressure_count": delivery.budget_pressure_count,
                "rollup_count": delivery.rollup_count,
                "delivered_at": delivery.delivered_at,
            }),
        ))
        .await?;
    Ok(delivery)
}

pub(crate) fn usage_finance_export_schedule_enabled() -> bool {
    std::env::var("MANDOFORGE_USAGE_EXPORT_SCHEDULE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "enabled"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn usage_finance_export_webhook_url() -> Option<String> {
    std::env::var("MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL")
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
}
