use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const ADMIN_TOKEN_KEY: &str = "mandoforge.adminToken";

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub agent_role: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skill_ids: Vec<String>,
    #[serde(default)]
    pub workflow_pack_ids: Vec<String>,
    #[serde(default)]
    pub release_state: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub environment_id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Approval {
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkerJob {
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkflowRun {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub workflow_definition_id: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub execution_strategy: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DynamicWorkflowPlan {
    pub id: String,
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub runtime_adapter: String,
    #[serde(default)]
    pub phases: Value,
    #[serde(default)]
    pub materialization: Value,
    #[serde(default)]
    pub analysis: Value,
    #[serde(default)]
    pub plan: Value,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct TaskBoardSnapshot {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub items: Vec<TaskBoardItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct TaskBoardItem {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub assignee_agent_id: Option<String>,
    #[serde(default)]
    pub work_item: Option<WorkItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkItem {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Stage2Readiness {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub readiness_score: Option<f64>,
    #[serde(default)]
    pub categories: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EnterpriseProductReadiness {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub required_evidence_class: String,
    #[serde(default)]
    pub lane_count: usize,
    #[serde(default)]
    pub ready_lane_count: usize,
    #[serde(default)]
    pub pilot_ready_lane_count: usize,
    #[serde(default)]
    pub blocked_lane_count: usize,
    #[serde(default)]
    pub completion_blocked: bool,
    #[serde(default)]
    pub lanes: Vec<EnterpriseProductLane>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct EnterpriseProductLane {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub current_evidence_class: String,
    #[serde(default)]
    pub required_evidence_class: String,
    #[serde(default)]
    pub production_target: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ObservabilitySummary {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub signals: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct CapabilityDiscovery {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub capabilities: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SemanticObject {
    pub id: String,
    #[serde(default)]
    pub object_type: String,
    #[serde(default)]
    pub object_key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub freshness: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub semantic_scopes: Value,
    #[serde(default)]
    pub source_uri: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SemanticGraphSnapshot {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub node_count: usize,
    #[serde(default)]
    pub edge_count: usize,
    #[serde(default)]
    pub partition_count: usize,
    #[serde(default)]
    pub conflicts: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyRegistry {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub object_types: Vec<Value>,
    #[serde(default)]
    pub relation_types: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SemanticReflectionQueue {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub queue: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ContextPacket {
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub version: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RenderedExecutionContext {
    #[serde(default)]
    pub context_packet_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub context_packet_version: i64,
    #[serde(default)]
    pub ontology_scope: Value,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub must_follow: Vec<String>,
    #[serde(default)]
    pub relevant_objects: Vec<RenderedSemanticObject>,
    #[serde(default)]
    pub fetchable_object_ids: Vec<String>,
    #[serde(default)]
    pub omitted: RenderedContextOmissions,
    #[serde(default)]
    pub budget: RenderedContextBudget,
    #[serde(default)]
    pub available_tools: Vec<String>,
    #[serde(default)]
    pub full_content_included: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RenderedSemanticObject {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub object_type: String,
    #[serde(default)]
    pub object_key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub freshness: String,
    #[serde(default)]
    pub source_uri: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RenderedContextOmissions {
    #[serde(default)]
    pub token_budget_exceeded: usize,
    #[serde(default)]
    pub object_limit_exceeded: usize,
    #[serde(default)]
    pub policy_reminders_omitted: usize,
    #[serde(default)]
    pub source_refs_not_rendered: usize,
    #[serde(default)]
    pub full_content_not_rendered: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RenderedContextBudget {
    #[serde(default)]
    pub max_prompt_tokens: usize,
    #[serde(default)]
    pub estimated_tokens_used: usize,
    #[serde(default)]
    pub max_objects: usize,
    #[serde(default)]
    pub max_summary_chars: usize,
    #[serde(default)]
    pub max_policy_reminders: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkflowPackInstallation {
    pub id: String,
    #[serde(default)]
    pub pack_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub manifest: Value,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkflowPackMarketplace {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub packs: Vec<WorkflowPackMarketplacePack>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WorkflowPackMarketplacePack {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub manifest_path: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub validation: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct DeploymentVersion {
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub cargo_package_version: String,
    #[serde(default)]
    pub image_tag: Option<String>,
    #[serde(default)]
    pub git_sha: Option<String>,
    #[serde(default)]
    pub build_time: Option<String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiState<T> {
    pub data: T,
    pub status: LoadStatus,
    pub error: Option<String>,
    pub updated_at_ms: f64,
}

impl<T: Default> Default for ApiState<T> {
    fn default() -> Self {
        Self {
            data: T::default(),
            status: LoadStatus::Idle,
            error: None,
            updated_at_ms: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CreateSessionRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub fn get_admin_token() -> String {
    storage_get(ADMIN_TOKEN_KEY).unwrap_or_default()
}

pub fn set_admin_token(token: &str) {
    let token = token.trim();
    if token.is_empty() {
        storage_delete(ADMIN_TOKEN_KEY);
    } else {
        storage_set(ADMIN_TOKEN_KEY, token);
    }
}

pub async fn api_get<T>(path: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let mut request = Request::get(path)
        .header("content-type", "application/json")
        .header("x-mandoforge-subject", "ui-yew-console")
        .header("x-mandoforge-roles", "admin");
    let token = get_admin_token();
    if !token.is_empty() {
        request = request.header("authorization", &format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("{status}: {body}"));
    }
    response
        .json::<T>()
        .await
        .map_err(|error| format!("json decode failed: {error}"))
}

pub async fn api_post<T, B>(path: &str, body: &B) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
    B: Serialize,
{
    let mut request = Request::post(path)
        .header("content-type", "application/json")
        .header("x-mandoforge-subject", "ui-yew-console")
        .header("x-mandoforge-roles", "admin");
    let token = get_admin_token();
    if !token.is_empty() {
        request = request.header("authorization", &format!("Bearer {token}"));
    }
    let body =
        serde_json::to_string(body).map_err(|error| format!("json encode failed: {error}"))?;
    let response = request
        .body(body)
        .map_err(|error| format!("request body failed: {error:?}"))?
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("{status}: {body}"));
    }
    response
        .json::<T>()
        .await
        .map_err(|error| format!("json decode failed: {error}"))
}

pub fn compile_dynamic_body(objective: &str, total_agents: u32, parallel_agents: u32) -> Value {
    json!({
        "objective": objective,
        "runtime_adapter": "codex_app_server",
        "execution_strategy": "native_dynamic",
        "max_total_agents": total_agents,
        "max_parallel_agents": parallel_agents
    })
}

pub fn ontology_builder_body(source_text: &str) -> Value {
    json!({
        "domain_scope": "legal",
        "workflow_scope": "contract-review",
        "memory_scope": "legal-policy",
        "objective": "Build a first-draft ontology proposal for legal contract review.",
        "source_text": source_text,
        "source_refs": ["gate://semantic-ontology-builder"],
        "max_object_types": 8,
        "max_relation_types": 8,
        "preview_only": true
    })
}

pub fn render_context_body(
    max_prompt_tokens: usize,
    max_objects: usize,
    max_summary_chars: usize,
    max_policy_reminders: usize,
) -> Value {
    json!({
        "max_prompt_tokens": max_prompt_tokens,
        "max_objects": max_objects,
        "max_summary_chars": max_summary_chars,
        "max_policy_reminders": max_policy_reminders,
        "allow_full_content": false,
        "allow_on_demand_fetch": true
    })
}

pub fn create_session_body(
    agent_id: &str,
    environment_id: Option<&str>,
    title: &str,
    message: &str,
) -> CreateSessionRequest {
    let title = if title.trim().is_empty() {
        "Operator task from Agent OS Console"
    } else {
        title.trim()
    };
    let message = if message.trim().is_empty() {
        None
    } else {
        Some(message.trim().to_string())
    };
    CreateSessionRequest {
        agent_id: agent_id.trim().to_string(),
        environment_id: environment_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        title: title.to_string(),
        message,
    }
}

fn storage_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

fn storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

fn storage_delete(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.remove_item(key);
    }
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}
