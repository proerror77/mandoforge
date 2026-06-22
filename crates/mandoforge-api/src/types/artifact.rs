use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Artifact {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) artifact_type: String,
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) content: Value,
    pub(crate) created_at: DateTime<Utc>,
}
