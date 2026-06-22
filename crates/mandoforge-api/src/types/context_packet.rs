use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::semantic::{ContextPacketSemanticObject, RenderedSemanticObject};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextPacket {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Option<Uuid>,
    pub(crate) version: i64,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) task: Value,
    pub(crate) agent: ContextPacketAgent,
    pub(crate) runtime_profile: Option<ContextPacketRuntimeProfile>,
    pub(crate) semantic_scopes: Value,
    pub(crate) tool_policy: Value,
    pub(crate) policy_reminders: Vec<String>,
    pub(crate) freshness_warnings: Vec<String>,
    pub(crate) source_refs: Vec<ContextPacketSourceRef>,
    pub(crate) retrieved_objects: Vec<ContextPacketSemanticObject>,
    pub(crate) replay_summary: Value,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextPacketAgent {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) agent_role: String,
    pub(crate) release_state: String,
    pub(crate) tools: Vec<String>,
    pub(crate) mcp_server_ids: Vec<Uuid>,
    pub(crate) skill_ids: Vec<String>,
    pub(crate) workflow_pack_ids: Vec<String>,
    pub(crate) remote_computer_profile: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextPacketRuntimeProfile {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) runtime_type: String,
    pub(crate) remote_computer_required: bool,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextPacketSourceRef {
    pub(crate) source_type: String,
    pub(crate) source_id: String,
    pub(crate) freshness: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenderContextPacketRequest {
    #[serde(default)]
    pub(crate) max_prompt_tokens: Option<usize>,
    #[serde(default)]
    pub(crate) max_objects: Option<usize>,
    #[serde(default)]
    pub(crate) max_summary_chars: Option<usize>,
    #[serde(default)]
    pub(crate) max_policy_reminders: Option<usize>,
    #[serde(default)]
    pub(crate) allow_full_content: Option<bool>,
    #[serde(default)]
    pub(crate) allow_on_demand_fetch: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RenderedExecutionContext {
    pub(crate) context_packet_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) context_packet_version: i64,
    pub(crate) ontology_scope: Value,
    pub(crate) role: String,
    pub(crate) must_follow: Vec<String>,
    pub(crate) relevant_objects: Vec<RenderedSemanticObject>,
    pub(crate) fetchable_object_ids: Vec<Uuid>,
    pub(crate) omitted: RenderedContextOmissions,
    pub(crate) budget: RenderedContextBudget,
    pub(crate) available_tools: Vec<String>,
    pub(crate) full_content_included: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RenderedContextOmissions {
    pub(crate) token_budget_exceeded: usize,
    pub(crate) object_limit_exceeded: usize,
    pub(crate) policy_reminders_omitted: usize,
    pub(crate) source_refs_not_rendered: usize,
    pub(crate) full_content_not_rendered: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RenderedContextBudget {
    pub(crate) max_prompt_tokens: usize,
    pub(crate) estimated_tokens_used: usize,
    pub(crate) max_objects: usize,
    pub(crate) max_summary_chars: usize,
    pub(crate) max_policy_reminders: usize,
}
