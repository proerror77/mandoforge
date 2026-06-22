use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackInstallation {
    pub(crate) id: Uuid,
    pub(crate) pack_id: String,
    pub(crate) kind: String,
    pub(crate) version: String,
    pub(crate) manifest_path: String,
    pub(crate) manifest: Value,
    pub(crate) validation_report: Value,
    pub(crate) status: String,
    pub(crate) eval_gate_status: String,
    pub(crate) release_gate_status: String,
    pub(crate) gate_evidence: Value,
    pub(crate) staged_at: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackProfileAsset {
    pub(crate) id: Uuid,
    pub(crate) installation_id: Uuid,
    pub(crate) profile_id: String,
    pub(crate) content: String,
    pub(crate) version: i32,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackBinding {
    pub(crate) id: Uuid,
    pub(crate) installation_id: Uuid,
    pub(crate) pack_id: String,
    pub(crate) pack_version: String,
    pub(crate) binding_type: String,
    pub(crate) binding_key: String,
    pub(crate) source_path: Option<String>,
    pub(crate) target_kind: String,
    pub(crate) target_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) materialized_payload: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackRuntimeObject {
    pub(crate) id: Uuid,
    pub(crate) installation_id: Uuid,
    pub(crate) binding_id: Uuid,
    pub(crate) pack_id: String,
    pub(crate) pack_version: String,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) runtime_kind: String,
    pub(crate) status: String,
    pub(crate) spec: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}
