use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentHandoffEvent {
    pub(crate) id: Uuid,
    pub(crate) source_session_id: Uuid,
    pub(crate) source_agent_id: Uuid,
    pub(crate) target_agent_id: Uuid,
    pub(crate) manager_plan_id: Option<Uuid>,
    pub(crate) intent: String,
    pub(crate) payload: Value,
    pub(crate) schema_version: String,
    pub(crate) risk_level: String,
    pub(crate) approval_required: bool,
    pub(crate) semantic_scopes: Value,
    pub(crate) runtime_profile_id: Option<Uuid>,
    pub(crate) remote_computer_required: bool,
    pub(crate) review_status: String,
    pub(crate) human_escalation_status: String,
    pub(crate) status: String,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentHandoffAssignment {
    pub(crate) id: Uuid,
    pub(crate) agent_handoff_event_id: Uuid,
    pub(crate) manager_plan_id: Uuid,
    pub(crate) source_session_id: Uuid,
    pub(crate) specialist_session_id: Uuid,
    pub(crate) source_agent_id: Uuid,
    pub(crate) target_agent_id: Uuid,
    pub(crate) semantic_scopes: Value,
    pub(crate) runtime_profile_id: Option<Uuid>,
    pub(crate) remote_computer_required: bool,
    pub(crate) remote_computer_job_assignment_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ManagerAgentPlan {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) manager_agent_id: Uuid,
    pub(crate) work_item_id: Option<Uuid>,
    pub(crate) specialist_agent_id: Option<Uuid>,
    pub(crate) task_intake: Value,
    pub(crate) decomposition: Value,
    pub(crate) specialist_selection: Value,
    pub(crate) risk_classification: String,
    pub(crate) review: Value,
    pub(crate) status: String,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateManagerAgentPlan {
    #[serde(default)]
    pub(crate) work_item_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) specialist_agent_id: Option<Uuid>,
    pub(crate) task_intake: Value,
    pub(crate) decomposition: Value,
    pub(crate) specialist_selection: Value,
    pub(crate) risk_classification: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) review: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewManagerAgentPlan {
    pub(crate) review: Value,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentHandoffEvent {
    pub(crate) target_agent_id: Uuid,
    #[serde(default)]
    pub(crate) manager_plan_id: Option<Uuid>,
    pub(crate) intent: String,
    pub(crate) payload: Value,
    pub(crate) schema_version: String,
    pub(crate) risk_level: String,
    #[serde(default)]
    pub(crate) approval_required: bool,
    #[serde(default)]
    pub(crate) semantic_scopes: Option<Value>,
    #[serde(default)]
    pub(crate) runtime_profile_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) remote_computer_required: Option<bool>,
    #[serde(default)]
    pub(crate) review_status: Option<String>,
    #[serde(default)]
    pub(crate) human_escalation_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TransitionAgentHandoffEvent {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EscalateAgentHandoffEvent {
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAgentHandoffAssignment {
    #[serde(default)]
    pub(crate) specialist_session_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) remote_computer_job_assignment_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) assigned_by: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AttachAgentHandoffRemoteComputerAssignment {
    pub(crate) remote_computer_job_assignment_id: Uuid,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}
