use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub(crate) struct ModifyApproval {
    pub(crate) args: Value,
    #[serde(default)]
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Approval {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) tool_call_id: Option<Uuid>,
    pub(crate) action: String,
    pub(crate) risk_level: String,
    pub(crate) reason: String,
    pub(crate) evidence: Value,
    pub(crate) decision_payload: Value,
    pub(crate) status: String,
    pub(crate) expires_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalCommitToken {
    pub(crate) id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) task_grant_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) tool_name: String,
    pub(crate) normalized_args_hash: String,
    pub(crate) target_binding: Value,
    pub(crate) approver_subject: String,
    pub(crate) status: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalGroup {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) subjects: Vec<String>,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateApprovalGroup {
    pub(crate) name: String,
    pub(crate) subjects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalEscalationRule {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) risk_level: String,
    pub(crate) group_id: Uuid,
    pub(crate) order_index: i32,
    pub(crate) after_seconds: i32,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateApprovalEscalationRule {
    pub(crate) name: String,
    pub(crate) risk_level: String,
    pub(crate) group_id: Uuid,
    #[serde(default)]
    pub(crate) order_index: i32,
    #[serde(default)]
    pub(crate) after_seconds: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EscalateApproval {
    #[serde(default)]
    pub(crate) group_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDelivery {
    pub(crate) status: String,
    pub(crate) delivered: bool,
    pub(crate) channel: String,
    pub(crate) webhook_configured: bool,
    pub(crate) approval_id: Uuid,
    pub(crate) target_count: usize,
    pub(crate) target_subjects: Vec<String>,
    pub(crate) group_id: Option<Uuid>,
    pub(crate) group_name: Option<String>,
    pub(crate) channel_deliveries: Vec<ApprovalNotificationChannelDelivery>,
    pub(crate) delivered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationChannelDelivery {
    pub(crate) channel: String,
    pub(crate) policy_id: Option<Uuid>,
    pub(crate) policy_name: Option<String>,
    pub(crate) status: String,
    pub(crate) delivered: bool,
    pub(crate) target_configured: bool,
    pub(crate) attempt_count: usize,
    pub(crate) max_attempts: usize,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeliveryRun {
    pub(crate) status: String,
    pub(crate) subject: Option<String>,
    pub(crate) candidate_count: usize,
    pub(crate) delivered_count: usize,
    pub(crate) reserved_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) deliveries: Vec<ApprovalNotificationDelivery>,
    pub(crate) failures: Vec<ApprovalNotificationDeliveryFailure>,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeliveryFailure {
    pub(crate) approval_id: Uuid,
    pub(crate) error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeliveryRunSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) run_count: usize,
    pub(crate) delivered_run_count: usize,
    pub(crate) reserved_run_count: usize,
    pub(crate) failed_run_count: usize,
    pub(crate) latest_run: Option<ApprovalNotificationDeliveryRunRecord>,
    pub(crate) recent_runs: Vec<ApprovalNotificationDeliveryRunRecord>,
    pub(crate) production_ops: ApprovalNotificationProductionOpsReadiness,
    pub(crate) deployment_readiness: ApprovalNotificationDeploymentReadiness,
    pub(crate) attention_items: Vec<ApprovalNotificationDeliveryRunAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) latest_run_age_hours: Option<i64>,
    pub(crate) routing_status: String,
    pub(crate) channel_count: usize,
    pub(crate) unroutable_pending_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) message: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationOpsValidationRun {
    pub(crate) status: String,
    pub(crate) routing_status: String,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeploymentValidationRun {
    pub(crate) status: String,
    pub(crate) pending_approval_count: usize,
    pub(crate) routable_pending_count: usize,
    pub(crate) unroutable_pending_count: usize,
    pub(crate) channel_count: usize,
    pub(crate) persisted_policy_count: usize,
    pub(crate) active_policy_count: usize,
    pub(crate) routing_status: String,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) pending_approval_count: usize,
    pub(crate) routable_pending_count: usize,
    pub(crate) unroutable_pending_count: usize,
    pub(crate) channel_count: usize,
    pub(crate) persisted_policy_count: usize,
    pub(crate) active_policy_count: usize,
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
pub(crate) struct ApprovalNotificationDeliveryRunRecord {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) subject: Option<String>,
    pub(crate) candidate_count: usize,
    pub(crate) delivered_count: usize,
    pub(crate) reserved_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) ran_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationDeliveryRunAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationRoutingSummary {
    pub(crate) status: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) webhook_configured: bool,
    pub(crate) slack_configured: bool,
    pub(crate) email_relay_configured: bool,
    pub(crate) channel_count: usize,
    pub(crate) persisted_policy_count: usize,
    pub(crate) active_policy_count: usize,
    pub(crate) channel_policies: Vec<ApprovalNotificationChannelPolicy>,
    pub(crate) pending_approval_count: usize,
    pub(crate) delegated_pending_count: usize,
    pub(crate) group_pending_count: usize,
    pub(crate) routable_pending_count: usize,
    pub(crate) unroutable_pending_count: usize,
    pub(crate) approval_group_count: usize,
    pub(crate) escalation_rule_count: usize,
    pub(crate) attention_items: Vec<ApprovalNotificationRoutingAttention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationRoutingAttention {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) approval_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalNotificationChannelPolicy {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) channel: String,
    pub(crate) target_env: Option<String>,
    pub(crate) risk_filter: String,
    pub(crate) max_attempts: i32,
    pub(crate) backoff_seconds: i32,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateApprovalNotificationChannelPolicy {
    pub(crate) name: String,
    pub(crate) channel: String,
    #[serde(default)]
    pub(crate) target_env: Option<String>,
    #[serde(default = "crate::default_approval_notification_risk_filter")]
    pub(crate) risk_filter: String,
    #[serde(default = "crate::default_approval_notification_max_attempts")]
    pub(crate) max_attempts: i32,
    #[serde(default)]
    pub(crate) backoff_seconds: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApprovalEscalationDueRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) expired_count: usize,
    pub(crate) escalated_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) notification_deliveries: Vec<ApprovalNotificationDelivery>,
}
