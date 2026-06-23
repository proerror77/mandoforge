use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderAccess {
    pub(crate) id: Uuid,
    pub(crate) team_id: Uuid,
    pub(crate) provider_name: String,
    pub(crate) model_allowlist: Vec<String>,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateProviderAccess {
    pub(crate) provider_name: String,
    #[serde(default)]
    pub(crate) model_allowlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProviderAccess {
    pub(crate) provider_name: String,
    #[serde(default)]
    pub(crate) model_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderRecord {
    pub(crate) id: Uuid,
    pub(crate) provider_type: String,
    pub(crate) name: String,
    pub(crate) base_url: Option<String>,
    pub(crate) default_model: Option<String>,
    pub(crate) config: Value,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateProviderRecord {
    pub(crate) provider_type: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) default_model: Option<String>,
    #[serde(default)]
    pub(crate) config: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProviderStatus {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) emergency: bool,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestProviderStatusApproval {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) approver_subject: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DecideProviderStatusApproval {
    #[serde(default)]
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderStatusApprovalResponse {
    pub(crate) provider: ProviderRecord,
    pub(crate) approval: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderGovernanceSummary {
    pub(crate) provider_count: usize,
    pub(crate) by_status: BTreeMap<String, usize>,
    pub(crate) by_type: BTreeMap<String, usize>,
    pub(crate) pending_status_approval_count: usize,
    pub(crate) last_status_approval_count: usize,
    pub(crate) emergency_lifecycle_count: usize,
    pub(crate) credential_ref_count: usize,
    pub(crate) env_key_count: usize,
    pub(crate) missing_credential_count: usize,
    pub(crate) budgeted_provider_count: usize,
    pub(crate) active_provider_count: usize,
    pub(crate) inactive_provider_count: usize,
    pub(crate) deployment_readiness: ProviderDeploymentReadiness,
    pub(crate) attention_items: Vec<ProviderGovernanceAttentionItem>,
    pub(crate) generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderGovernanceAttentionItem {
    pub(crate) provider_id: Uuid,
    pub(crate) provider_name: String,
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ProviderDeploymentValidationRun {
    pub(crate) status: String,
    pub(crate) provider_count: usize,
    pub(crate) healthy_count: usize,
    pub(crate) unhealthy_count: usize,
    pub(crate) results: Vec<ProviderHealth>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) provider_count: usize,
    pub(crate) healthy_count: usize,
    pub(crate) unhealthy_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) controller_execution_count: usize,
    pub(crate) controller_failed_count: usize,
    pub(crate) deployment_validated: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateReport {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) provider_count: usize,
    pub(crate) passed_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) warning_count: usize,
    pub(crate) checks: Vec<ProviderPolicyGateCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateRunResponse {
    pub(crate) run: ProviderPolicyGateRun,
    pub(crate) report: ProviderPolicyGateReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateRunSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) run_count: usize,
    pub(crate) passed_run_count: usize,
    pub(crate) failed_run_count: usize,
    pub(crate) warning_run_count: usize,
    pub(crate) latest_run: Option<ProviderPolicyGateRun>,
    pub(crate) recent_runs: Vec<ProviderPolicyGateRun>,
    pub(crate) production_enforcement: ProviderPolicyGateEnforcement,
    pub(crate) attention_items: Vec<ProviderPolicyGateRunAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateEnforcement {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) required_fresh_hours: i64,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) latest_run_age_hours: Option<i64>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateRun {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) subject: Option<String>,
    pub(crate) provider_count: usize,
    pub(crate) passed_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) warning_count: usize,
    pub(crate) failed_provider_names: Vec<String>,
    pub(crate) warning_provider_names: Vec<String>,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateRunAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunProviderProductionRollout {
    #[serde(default)]
    pub(crate) environment: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) provider_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunProviderProductionRollback {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderProductionRolloutRun {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) environment: String,
    pub(crate) reason: Option<String>,
    pub(crate) provider_count: usize,
    pub(crate) provider_ids: Vec<Uuid>,
    pub(crate) enforcement: ProviderPolicyGateEnforcement,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) message: String,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderProductionRollbackRun {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) environment: String,
    pub(crate) reason: Option<String>,
    pub(crate) provider_count: usize,
    pub(crate) provider_ids: Vec<Uuid>,
    pub(crate) source_rollout_id: Option<Uuid>,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) message: String,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProviderPolicyGateCheck {
    pub(crate) provider_id: Uuid,
    pub(crate) provider_name: String,
    pub(crate) provider_type: String,
    pub(crate) status: String,
    pub(crate) gate_status: String,
    pub(crate) blockers: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) recommendations: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RotateProviderApiKeyRef {
    pub(crate) api_key_ref: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ProviderHealth {
    pub(crate) provider_id: Uuid,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) healthy: bool,
    pub(crate) issues: Vec<String>,
    pub(crate) checks: Value,
    pub(crate) checked_at: DateTime<Utc>,
}
