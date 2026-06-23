use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{ApprovalEscalationDueRun, CodexAppServerStalePollRun};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilitySummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) telemetry: ObservabilityTelemetryStatus,
    pub(crate) sessions_by_status: HashMap<String, usize>,
    pub(crate) tool_calls_by_status: HashMap<String, usize>,
    pub(crate) approvals_by_status: HashMap<String, usize>,
    pub(crate) execution_jobs_by_status: HashMap<String, usize>,
    pub(crate) event_categories: HashMap<String, usize>,
    pub(crate) recent_error_events: Vec<ObservabilityErrorEvent>,
    pub(crate) backpressure: ObservabilityBackpressure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityTelemetryStatus {
    pub(crate) service_name: String,
    pub(crate) otlp_enabled: bool,
    pub(crate) sample_ratio: f64,
    pub(crate) endpoint_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityErrorEvent {
    pub(crate) session_id: Uuid,
    pub(crate) event_type: String,
    pub(crate) seq: i64,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityBackpressure {
    pub(crate) status: String,
    pub(crate) queued_jobs: usize,
    pub(crate) running_jobs: usize,
    pub(crate) failed_jobs: usize,
    pub(crate) retryable_jobs: usize,
    pub(crate) pending_approvals: usize,
    pub(crate) waiting_approval_sessions: usize,
    pub(crate) failed_sessions: usize,
    pub(crate) failed_tool_calls: usize,
    pub(crate) oldest_queued_job_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityRemediationRun {
    pub(crate) status: String,
    pub(crate) ran_at: DateTime<Utc>,
    pub(crate) actions: Vec<String>,
    pub(crate) before: ObservabilityBackpressure,
    pub(crate) after: ObservabilityBackpressure,
    pub(crate) approval_escalation_run: Option<ApprovalEscalationDueRun>,
    pub(crate) codex_app_server_stale_polls: Option<CodexAppServerStalePollRun>,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityRemediationPlan {
    pub(crate) status: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) auto_action_count: usize,
    pub(crate) manual_action_count: usize,
    pub(crate) configuration_action_count: usize,
    pub(crate) backpressure: ObservabilityBackpressure,
    pub(crate) actions: Vec<ObservabilityRemediationPlanAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityRemediationPlanAction {
    pub(crate) action: String,
    pub(crate) mode: String,
    pub(crate) severity: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorReadiness {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) service_name: String,
    pub(crate) otlp_enabled: bool,
    pub(crate) endpoint_configured: bool,
    pub(crate) endpoint: Option<String>,
    pub(crate) sample_ratio: f64,
    pub(crate) health_check: ObservabilityCollectorHealthCheck,
    pub(crate) signal_paths: Vec<ObservabilityCollectorSignalPath>,
    pub(crate) production_ops: ObservabilityCollectorProductionOpsReadiness,
    pub(crate) deployment_readiness: ObservabilityCollectorDeploymentReadiness,
    pub(crate) cluster_rollout: ObservabilityCollectorClusterRolloutReadiness,
    pub(crate) remediation_supervision: ObservabilityRemediationSupervisionReadiness,
    pub(crate) attention_items: Vec<ObservabilityCollectorAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) signal_path_count: usize,
    pub(crate) configured_signal_path_count: usize,
    pub(crate) health_checked: bool,
    pub(crate) health_status: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) otlp_enabled: bool,
    pub(crate) endpoint_configured: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) deployment_validated: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) latest_validation_healthy: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) latest_controller_validated: bool,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorClusterRolloutReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_rollout_at: Option<DateTime<Utc>>,
    pub(crate) latest_rollout_status: Option<String>,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) latest_controller_validated: bool,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) deployment_validated: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorClusterRolloutValidationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityRemediationSupervisionReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_run_at: Option<DateTime<Utc>>,
    pub(crate) latest_controller_run_age_hours: Option<i64>,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_remediated: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorHealthCheck {
    pub(crate) status: String,
    pub(crate) checked: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorSignalPath {
    pub(crate) signal: String,
    pub(crate) url: Option<String>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObservabilityCollectorAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}
