use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::SemanticObject;

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

#[derive(Debug, Deserialize)]
pub(crate) struct ValidateWorkflowPack {
    pub(crate) manifest_path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InstallWorkflowPack {
    pub(crate) manifest_path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackConfigWizardPlanRequest {
    pub(crate) manifest_path: String,
    #[serde(default)]
    pub(crate) domain_scope: Option<String>,
    #[serde(default)]
    pub(crate) target_environment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkflowPackOnboardingProfileInput {
    pub(crate) id: String,
    pub(crate) content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkflowPackOnboardingConnectorInput {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) available_permissions: Vec<String>,
    #[serde(default)]
    pub(crate) provenance_attested: bool,
    #[serde(default)]
    pub(crate) tenant_id: Option<String>,
    #[serde(default)]
    pub(crate) workspace_id: Option<String>,
    #[serde(default)]
    pub(crate) treats_results_as_data: bool,
    #[serde(default)]
    pub(crate) writes_enabled: bool,
    #[serde(default)]
    pub(crate) write_approval_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackOnboardingAssessmentRequest {
    #[serde(default)]
    pub(crate) profiles: Vec<WorkflowPackOnboardingProfileInput>,
    #[serde(default)]
    pub(crate) connectors: Vec<WorkflowPackOnboardingConnectorInput>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackProfileAssetSaveRequest {
    pub(crate) profiles: Vec<WorkflowPackOnboardingProfileInput>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackConnectorQualitySample {
    pub(crate) object_id: String,
    pub(crate) retrieved_at: DateTime<Utc>,
    #[serde(default)]
    pub(crate) citation_url: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) metadata: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) content: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackConnectorQualityInput {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) samples: Vec<WorkflowPackConnectorQualitySample>,
    #[serde(default)]
    pub(crate) team_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) server_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) tool_name: Option<String>,
    #[serde(default)]
    pub(crate) tenant_binding: Option<WorkflowPackConnectorTenantBindingInput>,
    #[serde(default)]
    pub(crate) secret_refs: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) operation_statuses: Vec<WorkflowPackConnectorOperationStatusInput>,
    #[serde(default)]
    pub(crate) lane_impacts: BTreeMap<String, WorkflowPackConnectorLaneImpactInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackConnectorQualityAssessmentRequest {
    #[serde(default)]
    pub(crate) connectors: Vec<WorkflowPackConnectorQualityInput>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WorkflowPackConnectorTenantBindingInput {
    #[serde(default)]
    pub(crate) tenant_id: Option<String>,
    #[serde(default)]
    pub(crate) workspace_id: Option<String>,
    #[serde(default)]
    pub(crate) shop_id: Option<String>,
    #[serde(default)]
    pub(crate) seller_nick: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkflowPackConnectorOperationStatusInput {
    pub(crate) operation_id: String,
    #[serde(default)]
    pub(crate) api_name: Option<String>,
    #[serde(default)]
    pub(crate) permission: Option<String>,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) last_probe_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) sample_count: Option<usize>,
    #[serde(default)]
    pub(crate) error_class: Option<String>,
    #[serde(default)]
    pub(crate) evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorLaneImpact {
    pub(crate) status: String,
    pub(crate) enabled_workflows: Vec<String>,
    pub(crate) blocked_workflows: Vec<String>,
    pub(crate) degraded_reason: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct WorkflowPackConnectorLaneImpactInput {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) enabled_workflows: Vec<String>,
    #[serde(default)]
    pub(crate) blocked_workflows: Vec<String>,
    #[serde(default)]
    pub(crate) degraded_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorSecretRefStatus {
    pub(crate) alias: String,
    pub(crate) reference: String,
    pub(crate) status: String,
    pub(crate) catalog_ref: Option<String>,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorOperationStatus {
    pub(crate) operation_id: String,
    pub(crate) api_name: Option<String>,
    pub(crate) permission: Option<String>,
    pub(crate) operation_type: Option<String>,
    pub(crate) status: String,
    pub(crate) last_probe_at: Option<DateTime<Utc>>,
    pub(crate) sample_count: Option<usize>,
    pub(crate) error_class: Option<String>,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorAssessment {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorQualityResult {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) sample_count: usize,
    pub(crate) passing_sample_count: usize,
    pub(crate) bound_team_id: Option<Uuid>,
    pub(crate) bound_server_id: Option<Uuid>,
    pub(crate) bound_server_name: Option<String>,
    pub(crate) bound_server_health_status: Option<String>,
    pub(crate) bound_tool_name: Option<String>,
    pub(crate) tenant_binding_status: Option<String>,
    pub(crate) credential_status: Option<String>,
    pub(crate) secret_ref_statuses: Vec<WorkflowPackConnectorSecretRefStatus>,
    pub(crate) operation_statuses: Vec<WorkflowPackConnectorOperationStatus>,
    pub(crate) lane_impacts: BTreeMap<String, WorkflowPackConnectorLaneImpact>,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackOnboardingAssessment {
    pub(crate) installation_id: Uuid,
    pub(crate) pack_id: String,
    pub(crate) version: String,
    pub(crate) status: String,
    pub(crate) onboarding_workflow: String,
    pub(crate) onboarding_eval: String,
    pub(crate) required_profile_count: usize,
    pub(crate) profile_schema_count: usize,
    pub(crate) inline_profile_count: usize,
    pub(crate) persisted_profile_count: usize,
    pub(crate) provided_profile_count: usize,
    pub(crate) placeholder_profile_count: usize,
    pub(crate) connector_requirement_count: usize,
    pub(crate) ready_connector_count: usize,
    pub(crate) missing_profiles: Vec<String>,
    pub(crate) placeholder_profiles: Vec<String>,
    pub(crate) connector_blockers: Vec<WorkflowPackConnectorAssessment>,
    pub(crate) blockers: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowPackConnectorQualityAssessment {
    pub(crate) installation_id: Uuid,
    pub(crate) pack_id: String,
    pub(crate) version: String,
    pub(crate) status: String,
    pub(crate) connector_requirement_count: usize,
    pub(crate) ready_connector_count: usize,
    pub(crate) connector_results: Vec<WorkflowPackConnectorQualityResult>,
    pub(crate) blockers: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackUpdateRequest {
    pub(crate) manifest_path: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackStageRequest {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackReleaseRequest {
    pub(crate) eval_gate_status: String,
    pub(crate) release_gate_status: String,
    #[serde(default)]
    pub(crate) environment: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) gate_evidence: Value,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackArchiveRequest {
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowPackRollbackRequest {
    #[serde(default = "crate::empty_json_object")]
    pub(crate) gate_evidence: Value,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkflowPackWorkflowFile {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) entry_agent: Option<String>,
    #[serde(default)]
    pub(crate) trigger_type: Option<String>,
    #[serde(default)]
    pub(crate) input_schema_ref: Option<String>,
    #[serde(default)]
    pub(crate) output_schema_ref: Option<String>,
    #[serde(default)]
    pub(crate) approval_policy_ref: Option<String>,
    #[serde(default)]
    pub(crate) eval_gate_refs: Vec<String>,
    #[serde(default)]
    pub(crate) step_graph: Option<Value>,
    #[serde(default)]
    pub(crate) execution: Value,
    #[serde(default = "crate::default_workflow_execution_strategy")]
    pub(crate) execution_strategy: String,
    #[serde(default)]
    pub(crate) runtime_adapter: Option<String>,
    #[serde(default)]
    pub(crate) runtime_mode: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) runtime_capability_contract: Value,
    #[serde(default = "crate::default_event_ingestion_policy")]
    pub(crate) event_ingestion_policy: String,
    #[serde(default)]
    pub(crate) steps: Vec<Value>,
    #[serde(default)]
    pub(crate) approval: Value,
    #[serde(default)]
    pub(crate) output: Value,
    #[serde(default)]
    pub(crate) outputs: Vec<String>,
    #[serde(default)]
    pub(crate) handoff_rules: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_synthesis_schedule: Value,
}

pub(crate) struct WorkflowPackOntologyProjection {
    pub(crate) object_count: usize,
    pub(crate) link_count: usize,
    pub(crate) relation_type_count: usize,
    pub(crate) object_type_objects: BTreeMap<String, SemanticObject>,
}

pub(crate) struct WorkflowPackActionProjection {
    pub(crate) object_count: usize,
    pub(crate) link_count: usize,
    pub(crate) action_type_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkflowPackConnectorOperationContract {
    pub(crate) api_name: Option<String>,
    pub(crate) permission: Option<String>,
    pub(crate) operation_type: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkflowPackConnectorLaneRequirement {
    pub(crate) lane_id: String,
    pub(crate) required_operations: BTreeSet<String>,
    pub(crate) optional_operations: BTreeSet<String>,
    pub(crate) controlled_write_operations: BTreeSet<String>,
}
