use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRecord {
    pub(crate) id: Uuid,
    pub(crate) team_id: Uuid,
    pub(crate) name: String,
    pub(crate) transport: String,
    pub(crate) config: Value,
    pub(crate) tool_allowlist: Vec<String>,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerHealth {
    pub(crate) server_id: Uuid,
    pub(crate) team_id: Uuid,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) healthy: bool,
    pub(crate) issues: Vec<String>,
    pub(crate) checks: Value,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerHealthRun {
    pub(crate) team_id: Uuid,
    pub(crate) server_count: usize,
    pub(crate) healthy_count: usize,
    pub(crate) unhealthy_count: usize,
    pub(crate) results: Vec<McpServerHealth>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerScheduledHealthRun {
    pub(crate) team_id: Uuid,
    pub(crate) due_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) healthy_count: usize,
    pub(crate) unhealthy_count: usize,
    pub(crate) results: Vec<McpServerHealth>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerDeploymentValidationRun {
    pub(crate) team_id: Uuid,
    pub(crate) server_count: usize,
    pub(crate) healthy_count: usize,
    pub(crate) unhealthy_count: usize,
    pub(crate) results: Vec<McpServerHealth>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMcpServerRecord {
    pub(crate) name: String,
    #[serde(default = "crate::default_mcp_transport")]
    pub(crate) transport: String,
    #[serde(default)]
    pub(crate) config: Value,
    #[serde(default)]
    pub(crate) tool_allowlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMcpServerRecord {
    #[serde(default)]
    pub(crate) transport: Option<String>,
    #[serde(default)]
    pub(crate) config: Option<Value>,
    #[serde(default)]
    pub(crate) tool_allowlist: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMcpServerStatus {
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestMcpServerRollout {
    #[serde(default)]
    pub(crate) transport: Option<String>,
    #[serde(default)]
    pub(crate) config: Option<Value>,
    #[serde(default)]
    pub(crate) tool_allowlist: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) activate_after: Option<String>,
    #[serde(default)]
    pub(crate) activate_before: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutResponse {
    pub(crate) server: McpServerRecord,
    pub(crate) rollout: Value,
    pub(crate) preflight_health: Option<McpServerHealth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutDueRun {
    pub(crate) team_id: Uuid,
    pub(crate) applied_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) expired_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution_count: usize,
    pub(crate) controller_failed_count: usize,
    pub(crate) results: Vec<Value>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutRunSummary {
    pub(crate) team_id: Uuid,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) run_count: usize,
    pub(crate) processed_run_count: usize,
    pub(crate) failed_run_count: usize,
    pub(crate) latest_run: Option<McpServerRolloutRunRecord>,
    pub(crate) recent_runs: Vec<McpServerRolloutRunRecord>,
    pub(crate) production_ops: McpServerRolloutProductionOpsReadiness,
    pub(crate) production_orchestration: McpServerRolloutProductionOrchestrationReadiness,
    pub(crate) deployment_readiness: McpServerDeploymentReadiness,
    pub(crate) attention_items: Vec<McpServerRolloutRunAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) latest_run_age_hours: Option<i64>,
    pub(crate) pending_rollout_count: usize,
    pub(crate) due_pending_count: usize,
    pub(crate) expired_pending_count: usize,
    pub(crate) failed_preflight_count: usize,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutProductionOrchestrationReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) scheduler_supervision_fresh: bool,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) pending_clear: bool,
    pub(crate) failed_preflight_clear: bool,
    pub(crate) failed_runs_clear: bool,
    pub(crate) manual_apply_required_count: usize,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) server_count: usize,
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
pub(crate) struct McpServerRolloutRunRecord {
    pub(crate) id: Uuid,
    pub(crate) team_id: Uuid,
    pub(crate) status: String,
    pub(crate) applied_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) expired_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution_count: usize,
    pub(crate) controller_failed_count: usize,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutRunAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutSummary {
    pub(crate) team_id: Uuid,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) server_count: usize,
    pub(crate) by_server_status: BTreeMap<String, usize>,
    pub(crate) by_transport: BTreeMap<String, usize>,
    pub(crate) pending_rollout_count: usize,
    pub(crate) manual_pending_count: usize,
    pub(crate) scheduled_pending_count: usize,
    pub(crate) due_pending_count: usize,
    pub(crate) not_due_pending_count: usize,
    pub(crate) expired_pending_count: usize,
    pub(crate) applied_rollout_count: usize,
    pub(crate) rolled_back_rollout_count: usize,
    pub(crate) expired_rollout_count: usize,
    pub(crate) failed_preflight_count: usize,
    pub(crate) attention_items: Vec<McpServerRolloutAttentionItem>,
    pub(crate) latest_rollouts: Vec<McpServerLatestRollout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerRolloutAttentionItem {
    pub(crate) server_id: Uuid,
    pub(crate) name: String,
    pub(crate) server_status: String,
    pub(crate) rollout_id: Option<String>,
    pub(crate) rollout_status: String,
    pub(crate) reason: String,
    pub(crate) requested_by: Option<String>,
    pub(crate) requested_at: Option<DateTime<Utc>>,
    pub(crate) activate_after: Option<DateTime<Utc>>,
    pub(crate) activate_before: Option<DateTime<Utc>>,
    pub(crate) target_keys: Vec<String>,
    pub(crate) preflight_healthy: Option<bool>,
    pub(crate) preflight_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct McpServerLatestRollout {
    pub(crate) server_id: Uuid,
    pub(crate) name: String,
    pub(crate) rollout_id: Option<String>,
    pub(crate) status: String,
    pub(crate) updated_at: Option<DateTime<Utc>>,
    pub(crate) requested_by: Option<String>,
    pub(crate) applied_by: Option<String>,
    pub(crate) rolled_back_by: Option<String>,
}
