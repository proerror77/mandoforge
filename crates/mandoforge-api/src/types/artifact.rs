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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexArtifactSyncRequest {
    pub(crate) session_id: Uuid,
    #[serde(default)]
    pub(crate) turn_id: Option<String>,
    #[serde(default)]
    pub(crate) command_id: Option<String>,
    pub(crate) artifacts: Vec<CodexArtifactInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexArtifactInput {
    pub(crate) name: String,
    #[serde(default = "default_artifact_type")]
    pub(crate) artifact_type: String,
    #[serde(default)]
    pub(crate) path: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) content: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexArtifactSyncResponse {
    pub(crate) session_id: Uuid,
    pub(crate) turn_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) artifact_count: usize,
    pub(crate) artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RemoteComputerArtifactSyncRequest {
    pub(crate) session_id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    #[serde(default)]
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) artifacts: Vec<CodexArtifactInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RemoteComputerArtifactDiscoverRequest {
    pub(crate) session_id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    #[serde(default)]
    pub(crate) assignment_id: Option<Uuid>,
    #[serde(default = "default_remote_computer_artifact_dir")]
    pub(crate) artifact_dir: String,
    #[serde(default = "default_remote_computer_artifact_discovery_limit")]
    pub(crate) max_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerArtifactSyncResponse {
    pub(crate) session_id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) artifact_count: usize,
    pub(crate) artifacts: Vec<Artifact>,
}

fn default_artifact_type() -> String {
    "json".to_string()
}

fn default_remote_computer_artifact_dir() -> String {
    "artifacts".to_string()
}

fn default_remote_computer_artifact_discovery_limit() -> usize {
    50
}
