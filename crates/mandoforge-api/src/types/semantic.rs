use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::Artifact;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticSource {
    pub(crate) id: Uuid,
    pub(crate) source_type: String,
    pub(crate) source_uri: String,
    pub(crate) display_name: String,
    pub(crate) owner_type: Option<String>,
    pub(crate) owner_id: Option<Uuid>,
    pub(crate) metadata: Value,
    pub(crate) provenance: Value,
    pub(crate) freshness: Value,
    pub(crate) status: String,
    pub(crate) last_ingested_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSemanticSource {
    pub(crate) source_type: String,
    pub(crate) source_uri: String,
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) owner_type: Option<String>,
    #[serde(default)]
    pub(crate) owner_id: Option<Uuid>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) freshness: Value,
    #[serde(default = "crate::default_semantic_source_status")]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) last_ingested_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSemanticSource {
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    #[serde(default)]
    pub(crate) owner_type: Option<Option<String>>,
    #[serde(default)]
    pub(crate) owner_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub(crate) metadata: Option<Value>,
    #[serde(default)]
    pub(crate) provenance: Option<Value>,
    #[serde(default)]
    pub(crate) freshness: Option<Value>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) last_ingested_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticObject {
    pub(crate) id: Uuid,
    pub(crate) source_id: Option<Uuid>,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) content: Value,
    pub(crate) semantic_scopes: Value,
    pub(crate) source_uri: Option<String>,
    pub(crate) provenance: Value,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSemanticObject {
    #[serde(default)]
    pub(crate) source_id: Option<Uuid>,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) content: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default)]
    pub(crate) source_uri: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::default_semantic_trust_level")]
    pub(crate) trust_level: String,
    #[serde(default = "crate::default_semantic_freshness")]
    pub(crate) freshness: String,
    #[serde(default = "crate::default_semantic_record_status")]
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSemanticObject {
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) summary: Option<String>,
    #[serde(default)]
    pub(crate) content: Option<Value>,
    #[serde(default)]
    pub(crate) semantic_scopes: Option<Value>,
    #[serde(default)]
    pub(crate) source_uri: Option<Option<String>>,
    #[serde(default)]
    pub(crate) provenance: Option<Value>,
    #[serde(default)]
    pub(crate) trust_level: Option<String>,
    #[serde(default)]
    pub(crate) freshness: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticLink {
    pub(crate) id: Uuid,
    pub(crate) from_entity_type: String,
    pub(crate) from_entity_id: String,
    pub(crate) relation_type: String,
    pub(crate) to_entity_type: String,
    pub(crate) to_entity_id: String,
    pub(crate) metadata: Value,
    pub(crate) provenance: Value,
    pub(crate) confidence: f64,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSemanticLink {
    pub(crate) from_entity_type: String,
    pub(crate) from_entity_id: String,
    pub(crate) relation_type: String,
    pub(crate) to_entity_type: String,
    pub(crate) to_entity_id: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::default_semantic_confidence")]
    pub(crate) confidence: f64,
    #[serde(default = "crate::default_semantic_record_status")]
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSemanticLink {
    #[serde(default)]
    pub(crate) metadata: Option<Value>,
    #[serde(default)]
    pub(crate) provenance: Option<Value>,
    #[serde(default)]
    pub(crate) confidence: Option<f64>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunSemanticDreamingRequest {
    pub(crate) session_id: Uuid,
    pub(crate) domain_scope: String,
    pub(crate) workflow_scope: String,
    pub(crate) memory_scope: String,
    pub(crate) goal: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSemanticIngestionBatch {
    pub(crate) source: CreateSemanticSource,
    pub(crate) objects: Vec<SemanticIngestionObjectInput>,
    #[serde(default)]
    pub(crate) links: Vec<SemanticIngestionLinkInput>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticIngestionObjectInput {
    pub(crate) temp_ref: String,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) content: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::default_semantic_trust_level")]
    pub(crate) trust_level: String,
    #[serde(default = "crate::default_semantic_freshness")]
    pub(crate) freshness: String,
    #[serde(default = "crate::default_semantic_record_status")]
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticIngestionLinkInput {
    pub(crate) from_ref: String,
    pub(crate) relation_type: String,
    pub(crate) to_ref: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::default_semantic_confidence")]
    pub(crate) confidence: f64,
    #[serde(default = "crate::default_semantic_record_status")]
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticIngestionBatchResult {
    pub(crate) status: String,
    pub(crate) source: SemanticSource,
    pub(crate) objects: Vec<SemanticObject>,
    pub(crate) object_refs: Vec<SemanticIngestionObjectRef>,
    pub(crate) links: Vec<SemanticLink>,
    pub(crate) ingested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticIngestionObjectRef {
    pub(crate) temp_ref: String,
    pub(crate) semantic_object_id: Uuid,
    pub(crate) object_key: String,
    pub(crate) title: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticProductQuery {
    #[serde(default)]
    pub(crate) q: Option<String>,
    #[serde(default)]
    pub(crate) object_type: Option<String>,
    #[serde(default)]
    pub(crate) domain_scope: Option<String>,
    #[serde(default)]
    pub(crate) workflow_scope: Option<String>,
    #[serde(default)]
    pub(crate) memory_scope: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) trust_level: Option<String>,
    #[serde(default)]
    pub(crate) freshness: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SemanticSearchResponse {
    pub(crate) query: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) result_count: usize,
    pub(crate) results: Vec<SemanticSearchResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SemanticSearchResult {
    pub(crate) object: SemanticObject,
    pub(crate) score: i32,
    pub(crate) matched_fields: Vec<String>,
    pub(crate) partition_key: String,
    pub(crate) provenance: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SemanticGraphSnapshot {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) node_count: usize,
    pub(crate) edge_count: usize,
    pub(crate) partition_count: usize,
    pub(crate) nodes: Vec<SemanticGraphNode>,
    pub(crate) edges: Vec<SemanticGraphEdge>,
    pub(crate) partitions: Vec<SemanticGraphPartition>,
    pub(crate) conflicts: Vec<SemanticGraphConflict>,
    pub(crate) stale_nodes: Vec<SemanticGraphNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticGraphNode {
    pub(crate) id: Uuid,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) status: String,
    pub(crate) partition_key: String,
    pub(crate) semantic_scopes: Value,
    pub(crate) source_uri: Option<String>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SemanticGraphEdge {
    pub(crate) id: Uuid,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) relation_type: String,
    pub(crate) confidence: f64,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticGraphPartition {
    pub(crate) partition_key: String,
    pub(crate) domain_scope: String,
    pub(crate) workflow_scope: String,
    pub(crate) memory_scope: String,
    pub(crate) node_count: usize,
    pub(crate) stale_count: usize,
    pub(crate) unverified_count: usize,
    pub(crate) conflict_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticGraphConflict {
    pub(crate) kind: String,
    pub(crate) object_key: Option<String>,
    pub(crate) relation_id: Option<Uuid>,
    pub(crate) object_ids: Vec<Uuid>,
    pub(crate) partition_key: String,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticGovernanceRunRequest {
    #[serde(default)]
    pub(crate) domain_scope: Option<String>,
    #[serde(default)]
    pub(crate) workflow_scope: Option<String>,
    #[serde(default)]
    pub(crate) memory_scope: Option<String>,
    #[serde(default)]
    pub(crate) archive_stale: bool,
    #[serde(default = "crate::default_true")]
    pub(crate) dry_run: bool,
    #[serde(default = "crate::default_semantic_conflict_strategy")]
    pub(crate) conflict_strategy: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SemanticGovernanceRunResult {
    pub(crate) status: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) dry_run: bool,
    pub(crate) archive_stale: bool,
    pub(crate) conflict_strategy: String,
    pub(crate) archived_count: usize,
    pub(crate) conflict_count: usize,
    pub(crate) stale_count: usize,
    pub(crate) archived_object_ids: Vec<Uuid>,
    pub(crate) conflicts: Vec<SemanticGraphConflict>,
    pub(crate) graph: SemanticGraphSnapshot,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResolveSemanticConflictRequest {
    pub(crate) preferred_object_id: Uuid,
    #[serde(default)]
    pub(crate) archive_object_ids: Vec<Uuid>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ContextPacketSemanticObject {
    pub(crate) id: Uuid,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) source_id: Option<Uuid>,
    pub(crate) source_uri: Option<String>,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) semantic_scopes: Value,
    pub(crate) provenance: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RenderedSemanticObject {
    pub(crate) id: Uuid,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) source_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FetchSemanticObjectRequest {
    pub(crate) context_packet_id: Uuid,
    #[serde(default)]
    pub(crate) include_content: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchSemanticObjectsRequest {
    pub(crate) context_packet_id: Uuid,
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) object_type: Option<String>,
    #[serde(default)]
    pub(crate) max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FetchSemanticObjectResponse {
    pub(crate) context_packet_id: Uuid,
    pub(crate) object: FetchableSemanticObject,
    pub(crate) content_included: bool,
    pub(crate) fetch_policy: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchSemanticObjectsResponse {
    pub(crate) context_packet_id: Uuid,
    pub(crate) boundary: String,
    pub(crate) query: Option<String>,
    pub(crate) object_type: Option<String>,
    pub(crate) results: Vec<RenderedSemanticObject>,
    pub(crate) omitted: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FetchableSemanticObject {
    pub(crate) id: Uuid,
    pub(crate) object_type: String,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) content: Option<Value>,
    pub(crate) semantic_scopes: Value,
    pub(crate) source_uri: Option<String>,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExpandSemanticLinksRequest {
    pub(crate) context_packet_id: Uuid,
    pub(crate) object_id: Uuid,
    #[serde(default)]
    pub(crate) relation_type: Option<String>,
    #[serde(default)]
    pub(crate) max_links: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExpandSemanticLinksResponse {
    pub(crate) context_packet_id: Uuid,
    pub(crate) object_id: Uuid,
    pub(crate) links: Vec<SemanticLink>,
    pub(crate) omitted: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticRetrievalBackendRegistry {
    pub(crate) selected_backend: String,
    pub(crate) effective_backend: String,
    pub(crate) fail_closed: bool,
    pub(crate) object_model_required: bool,
    pub(crate) backends: Vec<SemanticRetrievalBackendStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticRetrievalBackendStatus {
    pub(crate) backend: String,
    pub(crate) backend_type: String,
    pub(crate) status: String,
    pub(crate) selected: bool,
    pub(crate) effective: bool,
    pub(crate) configured: bool,
    pub(crate) required_env_vars: Vec<String>,
    pub(crate) missing_env_vars: Vec<String>,
    pub(crate) object_link_context_packet_required: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryWritebackCandidate {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) candidate_type: String,
    pub(crate) source_event_id: Option<Uuid>,
    pub(crate) source_artifact_id: Option<Uuid>,
    pub(crate) source_approval_id: Option<Uuid>,
    pub(crate) source_handoff_id: Option<Uuid>,
    pub(crate) proposed_object_type: String,
    pub(crate) proposed_object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) content: Value,
    pub(crate) semantic_scopes: Value,
    pub(crate) source_refs: Value,
    pub(crate) provenance: Value,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) status: String,
    pub(crate) reviewer_subject: Option<String>,
    pub(crate) review_reason: Option<String>,
    pub(crate) semantic_object_id: Option<Uuid>,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceSummary {
    pub(crate) status: String,
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) isolation_policy: String,
    pub(crate) semantic_object_count: usize,
    pub(crate) memory_object_count: usize,
    pub(crate) partition_count: usize,
    pub(crate) partitions: Vec<MemoryGovernancePartition>,
    pub(crate) trust_counts: BTreeMap<String, usize>,
    pub(crate) freshness_counts: BTreeMap<String, usize>,
    pub(crate) writeback: MemoryGovernanceWritebackSummary,
    pub(crate) attention_items: Vec<MemoryGovernanceAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernancePartition {
    pub(crate) partition_key: String,
    pub(crate) domain_scope: String,
    pub(crate) workflow_scope: String,
    pub(crate) memory_scope: String,
    pub(crate) object_count: usize,
    pub(crate) memory_object_count: usize,
    pub(crate) human_verified_count: usize,
    pub(crate) unverified_count: usize,
    pub(crate) stale_count: usize,
    pub(crate) shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceWritebackSummary {
    pub(crate) pending_count: usize,
    pub(crate) approved_count: usize,
    pub(crate) rejected_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceAttentionItem {
    pub(crate) severity: String,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) partition_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryGovernancePartitionQuery {
    pub(crate) partition_key: String,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemoryGovernanceWritebackQuery {
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernancePartitionDetail {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) partition: MemoryGovernancePartition,
    pub(crate) object_count: usize,
    pub(crate) pending_writeback_count: usize,
    pub(crate) access_policy: String,
    pub(crate) risk_items: Vec<MemoryGovernanceAttentionItem>,
    pub(crate) objects: Vec<MemoryGovernanceObjectRef>,
    pub(crate) writeback_candidates: Vec<MemoryGovernanceWritebackRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceObjectRef {
    pub(crate) id: Uuid,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) status: String,
    pub(crate) source_uri: Option<String>,
    pub(crate) semantic_scopes: Value,
    pub(crate) provenance: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceWritebackQueue {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status_filter: Option<String>,
    pub(crate) candidate_count: usize,
    pub(crate) pending_count: usize,
    pub(crate) candidates: Vec<MemoryGovernanceWritebackRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MemoryGovernanceWritebackRef {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) candidate_type: String,
    pub(crate) proposed_object_type: String,
    pub(crate) proposed_object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) trust_level: String,
    pub(crate) freshness: String,
    pub(crate) status: String,
    pub(crate) partition_key: String,
    pub(crate) semantic_object_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMemoryWritebackCandidates {
    #[serde(default)]
    pub(crate) include_session_summary: Option<bool>,
    #[serde(default)]
    pub(crate) include_artifacts: Option<bool>,
    #[serde(default)]
    pub(crate) include_handoffs: Option<bool>,
    #[serde(default)]
    pub(crate) include_approvals: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateSemanticSynthesisRun {
    pub(crate) synthesis_type: String,
    pub(crate) goal_attempted: String,
    #[serde(default)]
    pub(crate) context_used: Vec<String>,
    #[serde(default)]
    pub(crate) worked: Vec<String>,
    #[serde(default)]
    pub(crate) failed_or_corrected: Vec<String>,
    #[serde(default)]
    pub(crate) unsafe_assumptions: Vec<String>,
    #[serde(default)]
    pub(crate) durable_memory_candidates: Vec<SemanticSynthesisMemoryCandidateInput>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SemanticSynthesisMemoryCandidateInput {
    #[serde(default = "crate::default_memory_object_type")]
    pub(crate) proposed_object_type: String,
    pub(crate) proposed_object_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) content: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) source_refs: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) provenance: Value,
    #[serde(default = "crate::default_semantic_trust_level")]
    pub(crate) trust_level: String,
    #[serde(default = "crate::default_semantic_freshness")]
    pub(crate) freshness: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticSynthesisRunResult {
    pub(crate) status: String,
    pub(crate) synthesis_type: String,
    pub(crate) session_id: Uuid,
    pub(crate) checkpoint_event_id: Uuid,
    pub(crate) artifact: Artifact,
    pub(crate) candidates: Vec<MemoryWritebackCandidate>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewMemoryWritebackCandidate {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticAgingPolicySweep {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) policy_count: usize,
    pub(crate) due_count: usize,
    pub(crate) archived_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) archived_object_ids: Vec<Uuid>,
    pub(crate) runs: Vec<Value>,
    pub(crate) actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticSynthesisScheduleSweep {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) scheduled_count: usize,
    pub(crate) due_count: usize,
    pub(crate) created_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) runs: Vec<SemanticSynthesisScheduledRun>,
    pub(crate) actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticSynthesisScheduledRun {
    pub(crate) runtime_object_id: Uuid,
    pub(crate) object_key: String,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) synthesis_type: Option<String>,
    pub(crate) status: String,
    pub(crate) artifact_id: Option<Uuid>,
    pub(crate) candidate_count: usize,
    pub(crate) reason: Option<String>,
}
