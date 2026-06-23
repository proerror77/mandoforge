use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::policy::{PolicyConfig, ToolPolicyDecision};

#[derive(Debug, Clone)]
pub(crate) struct PolicyRuntime {
    pub(crate) active_revision_id: Option<Uuid>,
    pub(crate) active: PolicyConfig,
    pub(crate) staged: Option<StagedPolicyRuntime>,
}

#[derive(Debug, Clone)]
pub(crate) struct StagedPolicyRuntime {
    #[allow(dead_code)]
    pub(crate) revision_id: Uuid,
    pub(crate) rollout_percent: u8,
    pub(crate) policy: PolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRuntimeStatus {
    pub(crate) active_revision_id: Option<Uuid>,
    pub(crate) staged_revision_id: Option<Uuid>,
    pub(crate) staged_rollout_percent: Option<u8>,
    pub(crate) rollout_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRollbackResult {
    pub(crate) rolled_back_from_revision_id: Uuid,
    pub(crate) active_revision_id: Uuid,
    pub(crate) active_revision: PolicyRevision,
    pub(crate) rolled_back_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyScheduledRolloutRun {
    pub(crate) status: String,
    pub(crate) activated_revision_id: Option<Uuid>,
    pub(crate) activated_revision: Option<PolicyRevision>,
    pub(crate) controller_id: Option<String>,
    pub(crate) policy_store_id: Option<String>,
    pub(crate) deployment_id: Option<String>,
    pub(crate) scanned_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) scanned_revisions: Vec<PolicyScheduledRolloutScanDetail>,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyScheduledRolloutScanDetail {
    pub(crate) policy_id: String,
    pub(crate) policy_name: String,
    pub(crate) revision_id: Uuid,
    pub(crate) controller_id: Option<String>,
    pub(crate) policy_store_id: Option<String>,
    pub(crate) deployment_id: Option<String>,
    pub(crate) status: String,
    pub(crate) audit_id: Uuid,
    pub(crate) scanned_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct PolicyRolloutControllerBinding {
    pub(crate) controller_id: String,
    pub(crate) policy_store_id: String,
    pub(crate) deployment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRolloutOrchestrationReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) rollout_active: bool,
    pub(crate) active_revision_id: Option<Uuid>,
    pub(crate) staged_revision_id: Option<Uuid>,
    pub(crate) latest_due_run_at: Option<DateTime<Utc>>,
    pub(crate) latest_due_run_status: Option<String>,
    pub(crate) latest_due_run_age_hours: Option<i64>,
    pub(crate) due_run_fresh: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) latest_controller_production_target: bool,
    pub(crate) latest_controller_target_kind: Option<String>,
    pub(crate) latest_controller_environment: Option<String>,
    pub(crate) latest_controller_id: Option<String>,
    pub(crate) latest_controller_rollout_scope: Option<String>,
    pub(crate) latest_controller_production_policy_store: Option<bool>,
    pub(crate) latest_controller_rollback_supported: Option<bool>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRolloutOrchestrationValidationRun {
    pub(crate) status: String,
    pub(crate) rollout_active: bool,
    pub(crate) active_revision_id: Option<Uuid>,
    pub(crate) staged_revision_id: Option<Uuid>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SimulatePolicy {
    pub(crate) tool_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TestPolicyRequest {
    pub(crate) tool_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PolicyTestResult {
    pub(crate) decisions: Vec<ToolPolicyDecision>,
    pub(crate) tested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyDiffChange {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) current: Value,
    pub(crate) proposed: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRevisionDiff {
    pub(crate) revision_id: Uuid,
    pub(crate) changes: Vec<PolicyDiffChange>,
    pub(crate) generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRevisionGate {
    pub(crate) revision_id: Uuid,
    pub(crate) status: String,
    pub(crate) suite_source: String,
    pub(crate) rollout_percent: u8,
    pub(crate) activation_window: Option<PolicyActivationWindow>,
    pub(crate) cases: Vec<PolicyGateCaseResult>,
    pub(crate) diff: PolicyRevisionDiff,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyActivationWindow {
    pub(crate) activate_after: Option<DateTime<Utc>>,
    pub(crate) activate_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct PolicyRevisionGateRequest {
    #[serde(default)]
    pub(crate) cases: Vec<PolicyGateCaseInput>,
    #[serde(default)]
    pub(crate) rollout_percent: Option<u8>,
    #[serde(default)]
    pub(crate) activate_after: Option<String>,
    #[serde(default)]
    pub(crate) activate_before: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PolicyGateCaseInput {
    pub(crate) tool_name: String,
    pub(crate) expected_decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyGateCaseResult {
    pub(crate) tool_name: String,
    pub(crate) expected_decision: String,
    pub(crate) actual_decision: String,
    pub(crate) passed: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PolicyRevision {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) body: Value,
    pub(crate) status: String,
    pub(crate) created_by: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) activated_at: Option<DateTime<Utc>>,
    pub(crate) gate_status: Option<String>,
    pub(crate) gate_result: Value,
    pub(crate) gated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePolicyRevision {
    pub(crate) name: String,
    pub(crate) body: Value,
}
