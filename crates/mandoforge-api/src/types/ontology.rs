use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyRegistry {
    pub(crate) version: String,
    pub(crate) object_types: Vec<OntologyObjectType>,
    pub(crate) relation_types: Vec<OntologyRelationType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyObjectType {
    pub(crate) name: String,
    pub(crate) description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_level: Option<String>,
    pub(crate) governance_boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyRelationType {
    pub(crate) name: String,
    pub(crate) from_entity_type: String,
    pub(crate) to_entity_type: String,
    pub(crate) description: String,
    pub(crate) governance_boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingField {
    pub(crate) name: String,
    pub(crate) field_type: String,
    pub(crate) sample_values: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingDataset {
    pub(crate) table_name: String,
    pub(crate) source_system: String,
    pub(crate) source_object: String,
    pub(crate) fields: Vec<OntologyOnboardingField>,
    pub(crate) rows: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySourceBundle {
    pub(crate) industry: String,
    pub(crate) source_mode: String,
    pub(crate) tool_namespace: String,
    pub(crate) datasets: Vec<OntologyOnboardingDataset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedPack {
    pub(crate) industry: String,
    pub(crate) domain_scope: String,
    pub(crate) source_mode: String,
    pub(crate) tool_namespace: String,
    pub(crate) objects: Vec<OntologySeedObjectMapping>,
    pub(crate) relations: Vec<OntologySeedRelationMapping>,
    pub(crate) metrics: Vec<OntologySeedMetricMapping>,
    pub(crate) actions: Vec<OntologySeedActionMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedObjectMapping {
    pub(crate) table_name: String,
    pub(crate) object_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedRelationMapping {
    pub(crate) name: String,
    pub(crate) from_object: String,
    pub(crate) relation: String,
    pub(crate) to_object: String,
    pub(crate) source_table: String,
    pub(crate) source_field: String,
    pub(crate) reference_table: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedMetricMapping {
    pub(crate) name: String,
    pub(crate) target_object: String,
    pub(crate) expression: String,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedActionMapping {
    pub(crate) name: String,
    pub(crate) target_object: String,
    pub(crate) approval_required: bool,
    pub(crate) inputs: Value,
    pub(crate) reads: Value,
    pub(crate) effects: Value,
    pub(crate) executor: Value,
    pub(crate) transaction_profile: OntologyActionTransactionProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OntologyActionTransactionProfile {
    ProposalOnly,
    LocalSerializable,
    EventSourced,
    Saga,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyForeignKeyCandidate {
    pub(crate) field: String,
    pub(crate) references_table: String,
    pub(crate) references_field: String,
    pub(crate) join_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyDatasetProfile {
    pub(crate) table_name: String,
    pub(crate) row_count: usize,
    pub(crate) primary_key_candidates: Vec<String>,
    pub(crate) foreign_key_candidates: Vec<OntologyForeignKeyCandidate>,
    pub(crate) enum_candidates: Vec<String>,
    pub(crate) time_dimensions: Vec<String>,
    pub(crate) currency_fields: Vec<String>,
    pub(crate) pii_candidates: Vec<String>,
    pub(crate) field_null_rates: Value,
    pub(crate) field_uniqueness: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingProposalDraft {
    pub(crate) id: Uuid,
    pub(crate) run_id: Uuid,
    pub(crate) proposal_type: String,
    pub(crate) name: String,
    pub(crate) source_mapping: String,
    pub(crate) confidence: f64,
    pub(crate) evidence: Value,
    pub(crate) recommendation: String,
    pub(crate) review_status: String,
    pub(crate) content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingRun {
    pub(crate) id: Uuid,
    pub(crate) status: String,
    pub(crate) source_mode: String,
    pub(crate) dataset_count: usize,
    pub(crate) profile_count: usize,
    pub(crate) proposal_count: usize,
    pub(crate) approved_count: usize,
    pub(crate) materialized_count: usize,
    pub(crate) datasets: Vec<OntologyOnboardingDataset>,
    pub(crate) profiles: Vec<OntologyDatasetProfile>,
    pub(crate) proposals: Vec<OntologyOnboardingProposalDraft>,
    pub(crate) generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewOntologyOnboardingProposalRequest {
    pub(crate) decision: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewOntologyCuratedDatasetRequest {
    pub(crate) decision: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateOntologyOnboardingRunRequest {
    #[serde(default)]
    pub(crate) industry: Option<String>,
    #[serde(default)]
    pub(crate) source_mode: Option<String>,
    #[serde(default)]
    pub(crate) source_payload: Option<crate::ontology_source_adapters::OntologySourcePayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SchemaUnderstandingRequest {
    #[serde(default)]
    pub(crate) run_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) industry: Option<String>,
    #[serde(default)]
    pub(crate) source_mode: Option<String>,
    #[serde(default)]
    pub(crate) max_sample_rows: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PropertyUnderstandingCandidate {
    pub(crate) field_name: String,
    pub(crate) field_type: String,
    pub(crate) semantic_role: String,
    pub(crate) confidence: f64,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaxonomyLayerCandidate {
    pub(crate) layer: usize,
    pub(crate) label: String,
    pub(crate) confidence: f64,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchemaUnderstandingCandidate {
    pub(crate) table_name: String,
    pub(crate) source_system: String,
    pub(crate) source_object: String,
    pub(crate) object_type_candidate: String,
    pub(crate) confidence: f64,
    pub(crate) recommendation: String,
    pub(crate) properties: Vec<PropertyUnderstandingCandidate>,
    pub(crate) taxonomy_layers: Vec<TaxonomyLayerCandidate>,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchemaUnderstandingResponse {
    pub(crate) run_id: Option<Uuid>,
    pub(crate) industry: String,
    pub(crate) source_mode: String,
    pub(crate) domain_scope: String,
    pub(crate) tool_namespace: String,
    pub(crate) candidate_count: usize,
    pub(crate) candidates: Vec<SchemaUnderstandingCandidate>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubgraphProposalRequest {
    #[serde(default)]
    pub(crate) run_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) industry: Option<String>,
    #[serde(default)]
    pub(crate) source_mode: Option<String>,
    #[serde(default)]
    pub(crate) target_object: Option<String>,
    #[serde(default)]
    pub(crate) review_decision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubgraphProposalMember {
    pub(crate) proposal_id: Uuid,
    pub(crate) proposal_type: String,
    pub(crate) name: String,
    pub(crate) role: String,
    pub(crate) confidence: f64,
    pub(crate) review_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubgraphProposalDraft {
    pub(crate) id: String,
    pub(crate) run_id: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) target_object: String,
    pub(crate) review_status: String,
    pub(crate) confidence: f64,
    pub(crate) recommendation: String,
    pub(crate) members: Vec<SubgraphProposalMember>,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubgraphProposalResponse {
    pub(crate) run_id: Option<Uuid>,
    pub(crate) industry: String,
    pub(crate) source_mode: String,
    pub(crate) domain_scope: String,
    pub(crate) tool_namespace: String,
    pub(crate) subgraph_count: usize,
    pub(crate) subgraphs: Vec<SubgraphProposalDraft>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntityResolutionRequest {
    #[serde(default)]
    pub(crate) run_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) candidate_name: Option<String>,
    #[serde(default)]
    pub(crate) candidate_object_type: Option<String>,
    #[serde(default)]
    pub(crate) domain_scope: Option<String>,
    #[serde(default)]
    pub(crate) min_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntityResolutionRetrievalHit {
    pub(crate) object_id: Uuid,
    pub(crate) object_key: String,
    pub(crate) title: String,
    pub(crate) object_type: String,
    pub(crate) domain_scope: String,
    pub(crate) normalized_name: String,
    pub(crate) score: f64,
    pub(crate) match_reasons: Vec<String>,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntityResolutionDecisionDraft {
    pub(crate) is_duplicate: bool,
    pub(crate) canonical_name: String,
    pub(crate) existing_node_uuid: Option<Uuid>,
    pub(crate) confidence: f64,
    pub(crate) decision: String,
    pub(crate) review_required: bool,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntityResolutionCandidate {
    pub(crate) candidate_name: String,
    pub(crate) candidate_object_type: String,
    pub(crate) domain_scope: String,
    pub(crate) normalized_name: String,
    pub(crate) retrieval_hits: Vec<EntityResolutionRetrievalHit>,
    pub(crate) decision: EntityResolutionDecisionDraft,
    pub(crate) evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntityResolutionResponse {
    pub(crate) run_id: Option<Uuid>,
    pub(crate) candidate_count: usize,
    pub(crate) candidates: Vec<EntityResolutionCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfidenceCalibrationRecord {
    pub(crate) id: Uuid,
    pub(crate) run_id: Uuid,
    pub(crate) proposal_id: Uuid,
    pub(crate) proposal_type: String,
    pub(crate) proposal_name: String,
    pub(crate) model_confidence: f64,
    pub(crate) deterministic_validator_score: f64,
    pub(crate) retrieval_similarity_score: Option<f64>,
    pub(crate) source_quality_score: f64,
    pub(crate) reviewer_decision: String,
    pub(crate) reviewer_status: String,
    pub(crate) runtime_outcome: Option<String>,
    pub(crate) evidence: Value,
    pub(crate) recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfidenceCalibrationBucket {
    pub(crate) proposal_type: String,
    pub(crate) reviewer_status: String,
    pub(crate) count: usize,
    pub(crate) average_model_confidence: f64,
    pub(crate) average_validator_score: f64,
    pub(crate) average_source_quality_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfidenceCalibrationResponse {
    pub(crate) run_id: Uuid,
    pub(crate) record_count: usize,
    pub(crate) records: Vec<ConfidenceCalibrationRecord>,
    pub(crate) buckets: Vec<ConfidenceCalibrationBucket>,
    pub(crate) threshold_policy: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExpandSemanticOntologyRequest {
    pub(crate) domain_scope: String,
    #[serde(default)]
    pub(crate) object_types: Vec<String>,
    #[serde(default)]
    pub(crate) relation_types: Vec<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuildSemanticOntologyRequest {
    pub(crate) domain_scope: String,
    #[serde(default)]
    pub(crate) workflow_scope: Option<String>,
    #[serde(default)]
    pub(crate) memory_scope: Option<String>,
    #[serde(default)]
    pub(crate) objective: Option<String>,
    #[serde(default)]
    pub(crate) source_text: Option<String>,
    #[serde(default)]
    pub(crate) source_refs: Vec<String>,
    #[serde(default)]
    pub(crate) evidence_object_ids: Vec<Uuid>,
    #[serde(default)]
    pub(crate) agent_draft: Option<Value>,
    #[serde(default)]
    pub(crate) max_object_types: Option<usize>,
    #[serde(default)]
    pub(crate) max_relation_types: Option<usize>,
    #[serde(default)]
    pub(crate) preview_only: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewOntologyProposalRequest {
    pub(crate) decision: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateOntologyReleaseCandidateRequest {
    #[serde(default)]
    pub(crate) version: Option<String>,
    #[serde(default)]
    pub(crate) migration_policy: Option<Value>,
    #[serde(default)]
    pub(crate) release_class: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct OntologyReleaseListQuery {
    #[serde(default)]
    pub(crate) domain_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyRelease {
    pub(crate) id: Uuid,
    pub(crate) version: String,
    pub(crate) domain_scope: String,
    pub(crate) source_run_id: Option<Uuid>,
    pub(crate) parent_release_id: Option<Uuid>,
    pub(crate) rollback_target_release_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) release_class: String,
    pub(crate) object_count: i32,
    pub(crate) relation_count: i32,
    pub(crate) action_count: i32,
    pub(crate) migration_policy: Value,
    pub(crate) gate_result: Value,
    pub(crate) materialized_object_ids: Value,
    pub(crate) materialized_link_ids: Value,
    pub(crate) evidence_refs: Value,
    pub(crate) promoted_by: Option<String>,
    pub(crate) promoted_at: Option<DateTime<Utc>>,
    pub(crate) rolled_back_by: Option<String>,
    pub(crate) rolled_back_at: Option<DateTime<Utc>>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyReleaseWorkflowTrigger {
    pub(crate) id: Uuid,
    pub(crate) ontology_release_id: Uuid,
    pub(crate) workflow_definition_id: Uuid,
    pub(crate) workflow_run_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) attempt_count: i32,
    pub(crate) claimed_at: Option<DateTime<Utc>>,
    pub(crate) error_message: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

pub(crate) const ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING: &str = "pending";
pub(crate) const ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_TRIGGERED: &str = "triggered";
pub(crate) const ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED: &str = "failed";
pub(crate) const ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED: &str = "skipped";

pub(crate) fn ontology_release_workflow_trigger_status_allowed(status: &str) -> bool {
    matches!(
        status,
        ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING
            | ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_TRIGGERED
            | ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED
            | ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_SKIPPED
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyReleaseWorkflowTriggerDrain {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) retryable_count: usize,
    pub(crate) triggered_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) trigger_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyEngineReadiness {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) registry_version: String,
    pub(crate) required_evidence_class: String,
    pub(crate) object_type_count: usize,
    pub(crate) relation_type_count: usize,
    pub(crate) check_count: usize,
    pub(crate) ready_check_count: usize,
    pub(crate) pilot_ready_check_count: usize,
    pub(crate) blocked_check_count: usize,
    pub(crate) completion_blocked: bool,
    pub(crate) checks: Vec<OntologyEngineReadinessCheck>,
    pub(crate) next_actions: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyEngineReadinessCheck {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) current_evidence_class: String,
    pub(crate) required_evidence_class: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) blockers: Vec<String>,
    pub(crate) next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySeedPackSummary {
    pub(crate) industry: String,
    pub(crate) domain_scope: String,
    pub(crate) source_mode: String,
    pub(crate) tool_namespace: String,
    pub(crate) object_count: usize,
    pub(crate) relation_count: usize,
    pub(crate) metric_count: usize,
    pub(crate) action_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingMaterializationResult {
    pub(crate) run_id: Uuid,
    pub(crate) status: String,
    pub(crate) semantic_object_count: usize,
    pub(crate) semantic_link_count: usize,
    pub(crate) tool_spec_count: usize,
    pub(crate) semantic_object_ids: Vec<Uuid>,
    pub(crate) semantic_link_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingToolSpec {
    pub(crate) id: Uuid,
    pub(crate) run_id: Uuid,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) tool_kind: String,
    pub(crate) target_object: String,
    pub(crate) read_only: bool,
    pub(crate) approval_required: bool,
    pub(crate) input_schema: Value,
    pub(crate) effects: Value,
    pub(crate) policy: Value,
    pub(crate) transaction_profile: OntologyActionTransactionProfile,
    pub(crate) execution_mode: String,
    pub(crate) read_write_risk: String,
    pub(crate) source_refs: Value,
    pub(crate) audit_event: String,
    pub(crate) source_proposal_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyOnboardingToolSpecResponse {
    pub(crate) run_id: Uuid,
    pub(crate) tool_specs: Vec<OntologyOnboardingToolSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyBuilderNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) node_type: String,
    pub(crate) status: String,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyBuilderEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) edge_type: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyBuilderExecutionLevel {
    pub(crate) level: usize,
    pub(crate) node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyBuilderDag {
    pub(crate) run_id: Option<Uuid>,
    pub(crate) mode: String,
    pub(crate) nodes: Vec<OntologyBuilderNode>,
    pub(crate) edges: Vec<OntologyBuilderEdge>,
    pub(crate) execution_levels: Vec<OntologyBuilderExecutionLevel>,
    pub(crate) stale_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CuratedDatasetDraft {
    pub(crate) id: String,
    pub(crate) table_name: String,
    pub(crate) source_system: String,
    pub(crate) object_candidate: Option<String>,
    pub(crate) quality_score: f64,
    pub(crate) review_status: String,
    pub(crate) issues: Vec<String>,
    pub(crate) schema_version: String,
    pub(crate) reviewer_metadata: Value,
    pub(crate) sample_rows: Vec<Value>,
    pub(crate) profile: OntologyDatasetProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyPromptPacket {
    pub(crate) run_id: Uuid,
    pub(crate) industry: String,
    pub(crate) source_mode: String,
    pub(crate) domain_scope: String,
    pub(crate) tool_namespace: String,
    pub(crate) seed_pack: OntologySeedPack,
    pub(crate) curated_datasets: Vec<CuratedDatasetDraft>,
    pub(crate) profiles: Vec<OntologyDatasetProfile>,
    pub(crate) allowed_ontology_triples: Vec<Value>,
    pub(crate) evidence_rules: Vec<String>,
    pub(crate) policy_reminders: Vec<String>,
    pub(crate) proposal_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyReviewGraphNode {
    pub(crate) id: String,
    pub(crate) node_type: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) confidence: f64,
    pub(crate) risk: String,
    pub(crate) evidence: Value,
    pub(crate) source_proposal_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyReviewGraphEdge {
    pub(crate) id: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) edge_type: String,
    pub(crate) status: String,
    pub(crate) confidence: f64,
    pub(crate) risk: String,
    pub(crate) evidence: Value,
    pub(crate) source_proposal_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologyReviewGraph {
    pub(crate) run_id: Uuid,
    pub(crate) nodes: Vec<OntologyReviewGraphNode>,
    pub(crate) edges: Vec<OntologyReviewGraphEdge>,
    pub(crate) truncated: bool,
    pub(crate) omitted_node_count: usize,
    pub(crate) omitted_edge_count: usize,
}
