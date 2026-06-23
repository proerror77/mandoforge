use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AgentReleaseAutomationRun, ApprovalEscalationDueRun, CodexAppServerStalePollRun,
    CostAlertDelivery, McpServerRolloutDueRun, McpServerScheduledHealthRun,
    OntologyReleaseWorkflowTriggerDrain, PolicyScheduledRolloutRun, ProviderPolicyGateRun,
    RemoteComputerReclaimRun, RemoteComputerSidecarSupervisionRun, SemanticAgingPolicySweep,
    SemanticSynthesisScheduleSweep, UsageFinanceExportDelivery,
    WorkflowScheduledStepActivationSweep,
};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SchedulerDueRun {
    pub(crate) run_id: Uuid,
    pub(crate) idempotency_key: Option<String>,
    pub(crate) owner: String,
    pub(crate) run_window_start: DateTime<Utc>,
    pub(crate) run_window_end: DateTime<Utc>,
    pub(crate) retry_policy: SchedulerRetryPolicy,
    pub(crate) replayed: bool,
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) team_count: usize,
    pub(crate) actions: Vec<String>,
    #[serde(default)]
    pub(crate) task_errors: Vec<SchedulerTaskError>,
    pub(crate) provider_policy_gate: Option<ProviderPolicyGateRun>,
    pub(crate) policy_rollout: PolicyScheduledRolloutRun,
    pub(crate) approval_escalations: ApprovalEscalationDueRun,
    pub(crate) agent_releases: AgentReleaseAutomationRun,
    pub(crate) workflow_scheduled_steps: Option<WorkflowScheduledStepActivationSweep>,
    pub(crate) semantic_synthesis_schedules: Option<SemanticSynthesisScheduleSweep>,
    #[serde(default)]
    pub(crate) semantic_aging_policies: Option<SemanticAgingPolicySweep>,
    #[serde(default)]
    pub(crate) ontology_release_workflow_triggers: Option<OntologyReleaseWorkflowTriggerDrain>,
    pub(crate) mcp_health_runs: Vec<McpServerScheduledHealthRun>,
    pub(crate) mcp_rollout_runs: Vec<McpServerRolloutDueRun>,
    pub(crate) codex_app_server_stale_polls: CodexAppServerStalePollRun,
    pub(crate) cost_alert_delivery: Option<CostAlertDelivery>,
    pub(crate) usage_finance_export: UsageFinanceExportDelivery,
    pub(crate) remote_computer_reclaim: RemoteComputerReclaimRun,
    pub(crate) remote_computer_sidecar_supervision: RemoteComputerSidecarSupervisionRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerTaskError {
    pub(crate) task: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerRetryPolicy {
    pub(crate) max_attempts: u32,
    pub(crate) backoff_seconds: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SchedulerRunDueRequest {
    #[serde(default)]
    pub(crate) idempotency_key: Option<String>,
    #[serde(default)]
    pub(crate) owner: Option<String>,
    #[serde(default)]
    pub(crate) run_window_start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) run_window_end: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) retry_policy: Option<SchedulerRetryPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerDuePlan {
    pub(crate) status: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) team_count: usize,
    pub(crate) item_count: usize,
    pub(crate) actionable_count: usize,
    pub(crate) actions: Vec<SchedulerDuePlanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerDuePlanItem {
    pub(crate) area: String,
    pub(crate) action: String,
    pub(crate) mode: String,
    pub(crate) status: String,
    pub(crate) due_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) target_count: usize,
    pub(crate) severity: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerOrchestrationSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) plan: SchedulerDuePlan,
    pub(crate) deployment_readiness: SchedulerDeploymentReadiness,
    pub(crate) recent_run_count: usize,
    pub(crate) last_run_at: Option<DateTime<Utc>>,
    pub(crate) last_run_status: Option<String>,
    pub(crate) last_run_action_count: usize,
    pub(crate) recent_runs: Vec<SchedulerRunHistoryItem>,
    pub(crate) attention_items: Vec<SchedulerAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerRunHistoryItem {
    pub(crate) audit_log_id: Uuid,
    pub(crate) run_id: Option<Uuid>,
    pub(crate) idempotency_key: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) status: String,
    pub(crate) team_count: usize,
    pub(crate) action_count: usize,
    pub(crate) actions: Vec<String>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerAttentionItem {
    pub(crate) severity: String,
    pub(crate) kind: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) scheduler_manifest_present: bool,
    pub(crate) service_account_manifest_present: bool,
    pub(crate) service_account_name: Option<String>,
    pub(crate) automount_service_account_token_disabled: bool,
    pub(crate) subject_from_secret: bool,
    pub(crate) roles_from_secret: bool,
    pub(crate) token_from_secret: bool,
    pub(crate) token_header_present: bool,
    pub(crate) hardcoded_admin_headers_absent: bool,
    pub(crate) shared_token_runtime_configured: bool,
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
pub(crate) struct SchedulerDeploymentValidationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) readiness_status: String,
    pub(crate) blocking_reasons: Vec<String>,
}
