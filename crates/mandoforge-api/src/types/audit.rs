use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuditLog {
    pub(crate) id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) actor_type: String,
    pub(crate) actor_id: Option<Uuid>,
    pub(crate) action: String,
    pub(crate) resource_type: String,
    pub(crate) resource_id: Option<Uuid>,
    pub(crate) details: Value,
    pub(crate) created_at: DateTime<Utc>,
}
