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
