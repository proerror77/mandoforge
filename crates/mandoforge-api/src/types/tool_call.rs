use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolCall {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) event_id: Option<Uuid>,
    pub(crate) tool_name: String,
    pub(crate) args: Value,
    pub(crate) task_grant_id: Option<Uuid>,
    pub(crate) normalized_args_hash: Option<String>,
    pub(crate) target_binding: Value,
    pub(crate) status: String,
    pub(crate) risk_level: String,
    pub(crate) policy_decision: Value,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<Value>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}
