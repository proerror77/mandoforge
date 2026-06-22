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
