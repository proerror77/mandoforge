use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const ADMIN_TOKEN_KEY: &str = "mandoforge.adminToken";

pub const DEFAULT_ONTOLOGY_DOMAIN_SCOPE: &str = "commerce";
pub const DEFAULT_ONTOLOGY_WORKFLOW_SCOPE: &str = "fast-onboarding";
pub const DEFAULT_ONTOLOGY_MEMORY_SCOPE: &str = "enterprise-ontology";
pub const DEFAULT_ONTOLOGY_OBJECTIVE: &str =
    "Preview an agent-ready ontology proposal from enterprise source context.";
pub const DEFAULT_DEPLOYMENT_TARGET: &str = "production";
pub const ONTOLOGY_DOMAIN_SCOPE_KEY: &str = "mandoforge.ontology.domainScope";
pub const ONTOLOGY_WORKFLOW_SCOPE_KEY: &str = "mandoforge.ontology.workflowScope";
pub const ONTOLOGY_MEMORY_SCOPE_KEY: &str = "mandoforge.ontology.memoryScope";
pub const ONTOLOGY_OBJECTIVE_KEY: &str = "mandoforge.ontology.objective";
pub const DEPLOYMENT_TARGET_KEY: &str = "mandoforge.deployment.target";

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

impl Agent {
    pub(crate) fn is_runnable(&self) -> bool {
        self.release_state == "active"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub release_state: String,
    #[serde(default)]
    pub worker_queue_binding: Value,
}

impl Environment {
    pub(crate) fn is_runnable_for_release(&self, release_environment: Option<&str>) -> bool {
        self.status == "enabled"
            && self.release_state == "active"
            && release_environment.is_none_or(|expected| {
                self.worker_queue_binding["release_environment"]
                    .as_str()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
            })
    }
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
    pub current_boundary: String,
    #[serde(default)]
    pub readiness_endpoints: Vec<String>,
    #[serde(default)]
    pub evidence_scripts: Vec<String>,
    #[serde(default)]
    pub required_evidence: Vec<String>,
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
pub struct OntologyOnboardingRun {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source_mode: String,
    #[serde(default)]
    pub dataset_count: usize,
    #[serde(default)]
    pub profile_count: usize,
    #[serde(default)]
    pub proposal_count: usize,
    #[serde(default)]
    pub approved_count: usize,
    #[serde(default)]
    pub materialized_count: usize,
    #[serde(default)]
    pub proposals: Vec<OntologyOnboardingProposal>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingProposal {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub proposal_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source_mapping: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub review_status: String,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub content: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingToolSpec {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target_object: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub approval_required: bool,
    #[serde(default)]
    pub transaction_profile: String,
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub read_write_risk: String,
    #[serde(default)]
    pub source_refs: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingToolSpecResponse {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub tool_specs: Vec<OntologyOnboardingToolSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyRelease {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub domain_scope: String,
    #[serde(default)]
    pub source_run_id: Option<String>,
    #[serde(default)]
    pub parent_release_id: Option<String>,
    #[serde(default)]
    pub rollback_target_release_id: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub release_class: String,
    #[serde(default)]
    pub object_count: i32,
    #[serde(default)]
    pub relation_count: i32,
    #[serde(default)]
    pub action_count: i32,
    #[serde(default)]
    pub migration_policy: Value,
    #[serde(default)]
    pub gate_result: Value,
    #[serde(default)]
    pub promoted_by: Option<String>,
    #[serde(default)]
    pub promoted_at: Option<String>,
    #[serde(default)]
    pub rolled_back_by: Option<String>,
    #[serde(default)]
    pub rolled_back_at: Option<String>,
    #[serde(default)]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct OntologyReviewGraph {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub nodes: Vec<OntologyReviewGraphNode>,
    #[serde(default)]
    pub edges: Vec<OntologyReviewGraphEdge>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub omitted_node_count: usize,
    #[serde(default)]
    pub omitted_edge_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct OntologyReviewGraphNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub node_type: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub source_proposal_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct OntologyReviewGraphEdge {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub edge_type: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub source_proposal_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ConfidenceCalibrationResponse {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub record_count: usize,
    #[serde(default)]
    pub records: Vec<ConfidenceCalibrationRecord>,
    #[serde(default)]
    pub buckets: Vec<ConfidenceCalibrationBucket>,
    #[serde(default)]
    pub threshold_policy: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ConfidenceCalibrationRecord {
    #[serde(default)]
    pub proposal_type: String,
    #[serde(default)]
    pub proposal_name: String,
    #[serde(default)]
    pub model_confidence: f64,
    #[serde(default)]
    pub deterministic_validator_score: f64,
    #[serde(default)]
    pub retrieval_similarity_score: Option<f64>,
    #[serde(default)]
    pub source_quality_score: f64,
    #[serde(default)]
    pub reviewer_status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ConfidenceCalibrationBucket {
    #[serde(default)]
    pub proposal_type: String,
    #[serde(default)]
    pub reviewer_status: String,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub average_model_confidence: f64,
    #[serde(default)]
    pub average_validator_score: f64,
    #[serde(default)]
    pub average_source_quality_score: f64,
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
    #[serde(default)]
    pub manifest_summary: Value,
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
    pub in_flight: bool,
}

impl<T: Default> Default for ApiState<T> {
    fn default() -> Self {
        Self {
            data: T::default(),
            status: LoadStatus::Idle,
            error: None,
            updated_at_ms: 0.0,
            in_flight: false,
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
    let mut request = Request::get(path).header("content-type", "application/json");
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
    let mut request = Request::post(path).header("content-type", "application/json");
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OntologyBuilderConfig {
    pub domain_scope: String,
    pub workflow_scope: String,
    pub memory_scope: String,
    pub objective: String,
}

impl Default for OntologyBuilderConfig {
    fn default() -> Self {
        Self {
            domain_scope: DEFAULT_ONTOLOGY_DOMAIN_SCOPE.to_string(),
            workflow_scope: DEFAULT_ONTOLOGY_WORKFLOW_SCOPE.to_string(),
            memory_scope: DEFAULT_ONTOLOGY_MEMORY_SCOPE.to_string(),
            objective: DEFAULT_ONTOLOGY_OBJECTIVE.to_string(),
        }
    }
}

pub fn ontology_builder_body(source_text: &str, config: &OntologyBuilderConfig) -> Value {
    json!({
        "domain_scope": config.domain_scope,
        "workflow_scope": config.workflow_scope,
        "memory_scope": config.memory_scope,
        "objective": config.objective,
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
