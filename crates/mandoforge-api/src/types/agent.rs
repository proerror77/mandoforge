use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Agent {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) team_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) runtime_profile_id: Option<Uuid>,
    pub(crate) agent_role: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) system_prompt: String,
    pub(crate) tools: Vec<String>,
    pub(crate) tool_policy: Value,
    pub(crate) mcp_server_ids: Vec<Uuid>,
    pub(crate) skill_ids: Vec<String>,
    pub(crate) workflow_pack_ids: Vec<String>,
    pub(crate) remote_computer_profile: Value,
    pub(crate) semantic_scopes: Value,
    pub(crate) release_state: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentVersion {
    pub(crate) id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) version: i32,
    pub(crate) model: String,
    pub(crate) system_prompt: String,
    pub(crate) tools: Vec<String>,
    pub(crate) tool_names: Vec<String>,
    pub(crate) runtime_config: Value,
    pub(crate) approval_policy: Value,
    pub(crate) mcp_server_ids: Vec<Uuid>,
    pub(crate) skill_ids: Vec<String>,
    pub(crate) workflow_pack_ids: Vec<String>,
    pub(crate) semantic_scopes: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentRuntimeProfile {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) runtime_type: String,
    pub(crate) command: String,
    pub(crate) default_args: Vec<String>,
    pub(crate) env: Value,
    pub(crate) timeout_seconds: Option<i64>,
    pub(crate) remote_computer_required: bool,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Environment {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) environment_type: String,
    pub(crate) runtime_profile_id: Option<Uuid>,
    pub(crate) remote_computer_profile: Value,
    pub(crate) codex_app_server_profile: Value,
    pub(crate) worker_queue_binding: Value,
    pub(crate) state_mounts: Value,
    pub(crate) network_policy: Value,
    pub(crate) vault_requirements: Value,
    pub(crate) mcp_requirements: Value,
    pub(crate) release_state: String,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentRuntimeProfileReleaseGate {
    pub(crate) profile_id: Uuid,
    pub(crate) name: String,
    pub(crate) runtime_type: String,
    pub(crate) command: String,
    pub(crate) status: String,
    pub(crate) release_state: String,
    pub(crate) fail_closed: bool,
    pub(crate) requires_managed_profile: bool,
    pub(crate) runtime_type_supported: bool,
    pub(crate) runtime_allowlisted: bool,
    pub(crate) command_allowlisted: bool,
    pub(crate) remote_computer_required: bool,
    pub(crate) allowed_commands: Vec<String>,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentRuntimeProfile {
    pub(crate) name: String,
    #[serde(default = "crate::default_agent_runtime_type")]
    pub(crate) runtime_type: String,
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) default_args: Vec<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) env: Value,
    #[serde(default)]
    pub(crate) timeout_seconds: Option<i64>,
    #[serde(default)]
    pub(crate) remote_computer_required: bool,
    #[serde(default = "crate::default_enabled_status")]
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAgentRuntimeProfile {
    #[serde(default)]
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) default_args: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) env: Option<Value>,
    #[serde(default)]
    pub(crate) timeout_seconds: Option<Option<i64>>,
    #[serde(default)]
    pub(crate) remote_computer_required: Option<bool>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEnvironment {
    pub(crate) name: String,
    #[serde(default = "crate::default_environment_type")]
    pub(crate) environment_type: String,
    #[serde(default)]
    pub(crate) runtime_profile_id: Option<Uuid>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) remote_computer_profile: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) codex_app_server_profile: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) worker_queue_binding: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) state_mounts: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) network_policy: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) vault_requirements: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) mcp_requirements: Value,
    #[serde(default = "crate::default_agent_release_state")]
    pub(crate) release_state: String,
    #[serde(default = "crate::default_enabled_status")]
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEnvironment {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) environment_type: Option<String>,
    #[serde(default)]
    pub(crate) runtime_profile_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub(crate) remote_computer_profile: Option<Value>,
    #[serde(default)]
    pub(crate) codex_app_server_profile: Option<Value>,
    #[serde(default)]
    pub(crate) worker_queue_binding: Option<Value>,
    #[serde(default)]
    pub(crate) state_mounts: Option<Value>,
    #[serde(default)]
    pub(crate) network_policy: Option<Value>,
    #[serde(default)]
    pub(crate) vault_requirements: Option<Value>,
    #[serde(default)]
    pub(crate) mcp_requirements: Option<Value>,
    #[serde(default)]
    pub(crate) release_state: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgent {
    pub(crate) name: String,
    #[serde(default = "crate::default_agent_kind")]
    pub(crate) kind: String,
    #[serde(default = "crate::default_provider")]
    pub(crate) provider: String,
    #[serde(default = "crate::default_model")]
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) team_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) project_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) runtime_profile_id: Option<Uuid>,
    #[serde(default = "crate::default_agent_role")]
    pub(crate) agent_role: String,
    #[serde(default)]
    pub(crate) system_prompt: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) runtime_config: Value,
    #[serde(default)]
    pub(crate) tools: Vec<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) tool_policy: Value,
    #[serde(default)]
    pub(crate) mcp_server_ids: Vec<Uuid>,
    #[serde(default)]
    pub(crate) skill_ids: Vec<String>,
    #[serde(default)]
    pub(crate) workflow_pack_ids: Vec<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) remote_computer_profile: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default = "crate::default_agent_release_state")]
    pub(crate) release_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentRelease {
    pub(crate) id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Uuid,
    pub(crate) environment: String,
    pub(crate) status: String,
    pub(crate) eval_run_id: Option<Uuid>,
    pub(crate) eval_score: Option<f64>,
    pub(crate) min_score: f64,
    pub(crate) requested_by: Option<String>,
    pub(crate) requested_at: Option<DateTime<Utc>>,
    pub(crate) request_reason: Option<String>,
    pub(crate) approver_subject: Option<String>,
    pub(crate) decision_by: Option<String>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
    pub(crate) decision_reason: Option<String>,
    pub(crate) promoted_by: Option<String>,
    pub(crate) promoted_at: Option<DateTime<Utc>>,
    pub(crate) automation_policy: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentRelease {
    #[serde(default)]
    pub(crate) agent_version_id: Option<Uuid>,
    pub(crate) eval_run_id: Uuid,
    #[serde(default = "crate::default_release_environment")]
    pub(crate) environment: String,
    #[serde(default)]
    pub(crate) min_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestAgentReleasePromotion {
    #[serde(flatten)]
    pub(crate) release: CreateAgentRelease,
    #[serde(default)]
    pub(crate) approver_subject: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) auto_approve: Option<bool>,
    #[serde(default)]
    pub(crate) activate_after: Option<String>,
    #[serde(default)]
    pub(crate) expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RejectAgentReleasePromotion {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AgentReleaseAutomationRun {
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) pending_count: usize,
    pub(crate) promoted_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution_count: usize,
    pub(crate) controller_failed_count: usize,
    pub(crate) results: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseAutomationRunSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) run_count: usize,
    pub(crate) processed_run_count: usize,
    pub(crate) skipped_run_count: usize,
    pub(crate) latest_run: Option<AgentReleaseAutomationRunRecord>,
    pub(crate) recent_runs: Vec<AgentReleaseAutomationRunRecord>,
    pub(crate) production_ops: AgentReleaseProductionOpsReadiness,
    pub(crate) production_orchestration: AgentReleaseProductionOrchestrationReadiness,
    pub(crate) deployment_readiness: AgentReleaseDeploymentReadiness,
    pub(crate) attention_items: Vec<AgentReleaseAutomationRunAttentionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AgentReleaseDeploymentValidationRun {
    pub(crate) status: String,
    pub(crate) release_count: usize,
    pub(crate) pending_count: usize,
    pub(crate) promoted_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) rolled_back_count: usize,
    pub(crate) latest_automation_status: Option<String>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AgentReleaseOrchestrationValidationRun {
    pub(crate) status: String,
    pub(crate) release_count: usize,
    pub(crate) latest_automation_status: Option<String>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) latest_validation_healthy: bool,
    pub(crate) release_count: usize,
    pub(crate) pending_count: usize,
    pub(crate) promoted_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) rolled_back_count: usize,
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
pub(crate) struct AgentReleaseProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) pending_count: usize,
    pub(crate) auto_pending_count: usize,
    pub(crate) manual_pending_count: usize,
    pub(crate) expired_pending_count: usize,
    pub(crate) stale_pending_count: usize,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) latest_run_age_hours: Option<i64>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseProductionOrchestrationReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) automation_supervision_fresh: bool,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) pending_clear: bool,
    pub(crate) expired_clear: bool,
    pub(crate) stale_clear: bool,
    pub(crate) skipped_automation_clear: bool,
    pub(crate) manual_approval_clear: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseAutomationRunRecord {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) pending_count: usize,
    pub(crate) promoted_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseAutomationRunAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseRolloutSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) release_count: usize,
    pub(crate) by_status: BTreeMap<String, usize>,
    pub(crate) by_environment: BTreeMap<String, usize>,
    pub(crate) pending_count: usize,
    pub(crate) promoted_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) rolled_back_count: usize,
    pub(crate) auto_pending_count: usize,
    pub(crate) manual_pending_count: usize,
    pub(crate) expired_pending_count: usize,
    pub(crate) expiring_soon_count: usize,
    pub(crate) stale_pending_count: usize,
    pub(crate) latest_promoted_by_environment: Vec<AgentReleaseLatestPromotion>,
    pub(crate) attention_items: Vec<AgentReleaseAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseLatestPromotion {
    pub(crate) environment: String,
    pub(crate) release_id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Uuid,
    pub(crate) promoted_at: DateTime<Utc>,
    pub(crate) eval_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentReleaseAttentionItem {
    pub(crate) release_id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Uuid,
    pub(crate) environment: String,
    pub(crate) status: String,
    pub(crate) reason: String,
    pub(crate) requested_by: Option<String>,
    pub(crate) approver_subject: Option<String>,
    pub(crate) requested_at: Option<DateTime<Utc>>,
    pub(crate) activate_after: Option<DateTime<Utc>>,
    pub(crate) expires_at: Option<DateTime<Utc>>,
    pub(crate) eval_score: Option<f64>,
    pub(crate) min_score: f64,
}
