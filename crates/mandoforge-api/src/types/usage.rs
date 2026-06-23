use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageSummary {
    pub(crate) session_count: usize,
    pub(crate) event_count: usize,
    pub(crate) provider_request_count: usize,
    pub(crate) provider_response_count: usize,
    pub(crate) tool_call_count: usize,
    pub(crate) tool_success_count: usize,
    pub(crate) tool_failed_count: usize,
    pub(crate) approval_count: usize,
    pub(crate) prompt_tokens: i64,
    pub(crate) completion_tokens: i64,
    pub(crate) total_tokens: i64,
    pub(crate) total_tool_duration_ms: i64,
    pub(crate) estimated_provider_cost_cents: f64,
    pub(crate) by_provider: HashMap<String, ProviderUsageSummary>,
    pub(crate) by_tool: HashMap<String, ToolUsageSummary>,
    pub(crate) provider_budgets: Vec<ProviderBudgetStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageTrendSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) rollup_count: usize,
    pub(crate) comparison_basis: String,
    pub(crate) current_cost_cents: f64,
    pub(crate) current_total_tokens: i64,
    pub(crate) current_tool_calls: i64,
    pub(crate) latest_period: Option<UsageTrendPeriod>,
    pub(crate) previous_period: Option<UsageTrendPeriod>,
    pub(crate) cost_delta_cents: Option<f64>,
    pub(crate) cost_delta_percent: Option<f64>,
    pub(crate) token_delta: Option<i64>,
    pub(crate) token_delta_percent: Option<f64>,
    pub(crate) tool_call_delta: Option<i64>,
    pub(crate) tool_call_delta_percent: Option<f64>,
    pub(crate) top_provider_by_cost: Option<UsageTrendProvider>,
    pub(crate) budget_pressure: UsageBudgetPressure,
    pub(crate) forecast: UsageForecastSummary,
    pub(crate) recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageTrendPeriod {
    pub(crate) period_start: DateTime<Utc>,
    pub(crate) period_end: DateTime<Utc>,
    pub(crate) cost_cents: f64,
    pub(crate) total_tokens: i64,
    pub(crate) tool_calls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageTrendProvider {
    pub(crate) provider_name: String,
    pub(crate) estimated_cost_cents: f64,
    pub(crate) total_tokens: i64,
    pub(crate) request_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageBudgetPressure {
    pub(crate) total_budgeted_providers: usize,
    pub(crate) pressure_count: usize,
    pub(crate) warning_count: usize,
    pub(crate) critical_count: usize,
    pub(crate) highest_status: String,
    pub(crate) highest_used_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageForecastSummary {
    pub(crate) basis: String,
    pub(crate) horizons: Vec<UsageForecastHorizon>,
    pub(crate) provider_budget_exhaustion: Vec<ProviderBudgetExhaustionForecast>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageForecastHorizon {
    pub(crate) days: i64,
    pub(crate) projected_cost_cents: f64,
    pub(crate) projected_tokens: i64,
    pub(crate) projected_tool_calls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderBudgetExhaustionForecast {
    pub(crate) provider_name: String,
    pub(crate) status: String,
    pub(crate) current_daily_cost_cents: f64,
    pub(crate) daily_cost_limit_cents: f64,
    pub(crate) projected_days_to_limit: Option<f64>,
    pub(crate) projected_exhaustion_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ProviderUsageSummary {
    pub(crate) request_count: usize,
    pub(crate) response_count: usize,
    pub(crate) prompt_tokens: i64,
    pub(crate) completion_tokens: i64,
    pub(crate) total_tokens: i64,
    pub(crate) token_cost_cents: f64,
    pub(crate) estimated_cost_cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ToolUsageSummary {
    pub(crate) call_count: usize,
    pub(crate) success_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) total_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderBudgetStatus {
    pub(crate) provider_name: String,
    pub(crate) status: String,
    pub(crate) window_hours: i64,
    pub(crate) request_count: i64,
    pub(crate) daily_request_limit: Option<i64>,
    pub(crate) request_budget_used_percent: Option<f64>,
    pub(crate) estimated_cost_cents: f64,
    pub(crate) projected_daily_cost_cents: f64,
    pub(crate) daily_cost_limit_cents: Option<f64>,
    pub(crate) cost_budget_used_percent: Option<f64>,
    pub(crate) messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlert {
    pub(crate) provider_name: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) messages: Vec<String>,
    pub(crate) window_hours: i64,
    pub(crate) request_budget_used_percent: Option<f64>,
    pub(crate) cost_budget_used_percent: Option<f64>,
    pub(crate) estimated_cost_cents: f64,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlertSummary {
    pub(crate) webhook_configured: bool,
    pub(crate) min_status: String,
    pub(crate) alerts: Vec<CostAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlertDelivery {
    pub(crate) status: String,
    pub(crate) delivered: bool,
    pub(crate) channel: String,
    pub(crate) webhook_configured: bool,
    pub(crate) alerts: Vec<CostAlert>,
    pub(crate) route_deliveries: Vec<CostAlertRouteDelivery>,
    pub(crate) delivered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlertRouteDelivery {
    pub(crate) route_id: Option<Uuid>,
    pub(crate) route_name: String,
    pub(crate) channel: String,
    pub(crate) status: String,
    pub(crate) delivered: bool,
    pub(crate) matched_alert_count: usize,
    pub(crate) target: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CostAlertSmtpConfig {
    pub(crate) addr: String,
    pub(crate) from: String,
    pub(crate) helo_domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlertRoute {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) channel: String,
    pub(crate) target: Option<String>,
    pub(crate) severity_filter: String,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCostAlertRoute {
    pub(crate) name: String,
    pub(crate) channel: String,
    #[serde(default)]
    pub(crate) target: Option<String>,
    #[serde(default = "crate::default_cost_alert_severity_filter")]
    pub(crate) severity_filter: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AcknowledgeCostAlertRequest {
    pub(crate) provider_name: String,
    pub(crate) severity: String,
    #[serde(default)]
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CostAlertAcknowledgement {
    pub(crate) provider_name: String,
    pub(crate) severity: String,
    pub(crate) acknowledged_by: String,
    pub(crate) comment: Option<String>,
    pub(crate) acknowledged_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceExportDelivery {
    pub(crate) status: String,
    pub(crate) delivered: bool,
    pub(crate) channel: String,
    pub(crate) scheduled: bool,
    pub(crate) target_configured: bool,
    pub(crate) delivery_id: Uuid,
    pub(crate) file_name: String,
    pub(crate) bytes: usize,
    pub(crate) export_bytes: usize,
    pub(crate) record_count: usize,
    pub(crate) provider_count: usize,
    pub(crate) budget_pressure_count: usize,
    pub(crate) rollup_count: usize,
    pub(crate) delivered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceDashboardSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) current_cost_cents: f64,
    pub(crate) current_total_tokens: i64,
    pub(crate) current_tool_calls: i64,
    pub(crate) comparison_basis: String,
    pub(crate) budget_pressure_status: String,
    pub(crate) budget_pressure_count: usize,
    pub(crate) critical_budget_count: usize,
    pub(crate) warning_budget_count: usize,
    pub(crate) alert_count: usize,
    pub(crate) critical_alert_count: usize,
    pub(crate) warning_alert_count: usize,
    pub(crate) alert_route_count: usize,
    pub(crate) active_alert_route_count: usize,
    pub(crate) rollup_count: usize,
    pub(crate) latest_rollup_at: Option<DateTime<Utc>>,
    pub(crate) latest_rollup_age_hours: Option<i64>,
    pub(crate) finance_export_target_configured: bool,
    pub(crate) finance_export_schedule_enabled: bool,
    pub(crate) forecast_7d_cost_cents: Option<f64>,
    pub(crate) forecast_30d_cost_cents: Option<f64>,
    pub(crate) top_provider_by_cost: Option<UsageTrendProvider>,
    pub(crate) recommendations: Vec<String>,
    pub(crate) attention_items: Vec<UsageFinanceAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) provider_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceOperationsSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) readiness_score: i64,
    pub(crate) open_alert_count: usize,
    pub(crate) acknowledged_alert_count: usize,
    pub(crate) unacknowledged_alert_count: usize,
    pub(crate) active_alert_route_count: usize,
    pub(crate) rollup_status: String,
    pub(crate) export_status: String,
    pub(crate) alert_delivery_status: String,
    pub(crate) last_finance_export: Option<UsageFinanceOperationAudit>,
    pub(crate) last_alert_delivery: Option<UsageFinanceOperationAudit>,
    pub(crate) last_alert_acknowledgement: Option<UsageFinanceOperationAudit>,
    pub(crate) last_accounting_reconciliation: Option<UsageFinanceOperationAudit>,
    pub(crate) production_close: UsageFinanceProductionCloseReadiness,
    pub(crate) runbook_actions: Vec<String>,
    pub(crate) attention_items: Vec<UsageFinanceAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceProductionCloseReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) rollup_fresh: bool,
    pub(crate) export_target_configured: bool,
    pub(crate) export_recent: bool,
    pub(crate) alert_delivery_ready: bool,
    pub(crate) critical_alerts_acknowledged: bool,
    pub(crate) failed_delivery_evidence: bool,
    pub(crate) close_controller_required: bool,
    pub(crate) close_controller_configured: bool,
    pub(crate) latest_close_controller_status: Option<String>,
    pub(crate) latest_close_controller_age_hours: Option<i64>,
    pub(crate) close_controller_evidence_fresh: bool,
    pub(crate) latest_close_controller_closed: bool,
    pub(crate) reconciliation_controller_required: bool,
    pub(crate) reconciliation_controller_configured: bool,
    pub(crate) latest_reconciliation_status: Option<String>,
    pub(crate) latest_reconciliation_age_hours: Option<i64>,
    pub(crate) reconciliation_evidence_fresh: bool,
    pub(crate) latest_reconciliation_reconciled: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceOperationsRun {
    pub(crate) status: String,
    pub(crate) ran_at: DateTime<Utc>,
    pub(crate) actions: Vec<String>,
    pub(crate) before: UsageFinanceOperationsSummary,
    pub(crate) after: UsageFinanceOperationsSummary,
    pub(crate) rollup_created: Option<UsageRollup>,
    pub(crate) cost_alert_delivery: Option<CostAlertDelivery>,
    pub(crate) finance_export_delivery: Option<UsageFinanceExportDelivery>,
    pub(crate) close_controller_configured: bool,
    pub(crate) close_controller_execution: Value,
    pub(crate) reconciliation_controller_configured: bool,
    pub(crate) reconciliation_controller_execution: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageFinanceOperationAudit {
    pub(crate) action: String,
    pub(crate) status: String,
    pub(crate) subject: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UsageRollup {
    pub(crate) id: Uuid,
    pub(crate) period_start: DateTime<Utc>,
    pub(crate) period_end: DateTime<Utc>,
    pub(crate) summary: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateUsageRollup {
    #[serde(default)]
    pub(crate) period_start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) period_end: Option<DateTime<Utc>>,
}
