use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub(crate) const ONTOLOGY_RELEASE_CATALOG_SCHEMA: &str = "mandoforge.ontology.release_catalog.v1";
pub(crate) const ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE: &str = "active";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OntologySdkCatalogProperty {
    pub(crate) stable_key: String,
    pub(crate) source_name: String,
    pub(crate) api_name: String,
    pub(crate) value_type: String,
    pub(crate) nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OntologySdkCatalogObject {
    pub(crate) stable_key: String,
    pub(crate) api_name: String,
    pub(crate) object_type: String,
    #[serde(default)]
    pub(crate) properties: Vec<OntologySdkCatalogProperty>,
    #[serde(default)]
    pub(crate) primary_key_api_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OntologySdkCatalogRelation {
    pub(crate) stable_key: String,
    pub(crate) api_name: String,
    pub(crate) from_object_api_name: String,
    pub(crate) relation_type: String,
    pub(crate) to_object_api_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OntologySdkCatalogAction {
    pub(crate) stable_key: String,
    pub(crate) api_name: String,
    #[serde(default)]
    pub(crate) runtime_name: String,
    pub(crate) contract_digest: String,
    pub(crate) execution_mode: String,
    pub(crate) target_object_api_name: String,
    pub(crate) input_schema: Value,
    pub(crate) approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OntologyReleaseCatalogV1 {
    pub(crate) schema: String,
    pub(crate) domain_scope: String,
    #[serde(alias = "object_types")]
    pub(crate) objects: Vec<OntologySdkCatalogObject>,
    #[serde(alias = "relation_types")]
    pub(crate) relations: Vec<OntologySdkCatalogRelation>,
    pub(crate) actions: Vec<OntologySdkCatalogAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct OntologySdkSubsetManifest {
    #[serde(default, alias = "object_types")]
    pub(crate) objects: Vec<String>,
    #[serde(default, alias = "relation_types")]
    pub(crate) relations: Vec<String>,
    #[serde(default)]
    pub(crate) actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreateOntologySdkApplicationRequest {
    pub(crate) ontology_release_id: Uuid,
    #[serde(alias = "subset_manifest", alias = "manifest")]
    pub(crate) subset: OntologySdkSubsetManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySdkApplication {
    pub(crate) id: Uuid,
    pub(crate) tenant_id: Uuid,
    pub(crate) subject: String,
    pub(crate) ontology_release_id: Uuid,
    pub(crate) release_version: String,
    pub(crate) domain_scope: String,
    pub(crate) catalog_digest: String,
    pub(crate) subset_manifest: OntologySdkSubsetManifest,
    pub(crate) subset_digest: String,
    pub(crate) status: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OntologySdkApplicationManifest {
    pub(crate) application_id: Uuid,
    pub(crate) ontology_release_id: Uuid,
    pub(crate) release_version: String,
    pub(crate) domain_scope: String,
    pub(crate) catalog_digest: String,
    pub(crate) subset_manifest: OntologySdkSubsetManifest,
    pub(crate) subset_digest: String,
    pub(crate) status: String,
    pub(crate) resolved_catalog: OntologyReleaseCatalogV1,
}
