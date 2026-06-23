use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProjectGitHubBinding {
    pub(crate) id: Uuid,
    pub(crate) repo_full_name: String,
    pub(crate) pack_installation_id: Uuid,
    pub(crate) webhook_secret_ref: String,
    pub(crate) active: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertProjectGitHubBinding {
    pub(crate) repo_full_name: String,
    pub(crate) pack_installation_id: Uuid,
    #[serde(default)]
    pub(crate) webhook_secret_ref: Option<String>,
    #[serde(default)]
    pub(crate) active: Option<bool>,
}
