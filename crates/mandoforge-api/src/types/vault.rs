use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SecretProviderHealth {
    pub(crate) provider_kind: String,
    pub(crate) healthy: bool,
    pub(crate) status: String,
    pub(crate) issues: Vec<String>,
    pub(crate) checks: Value,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultReadinessReport {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) secret_provider: SecretProviderHealth,
    pub(crate) kms: VaultKmsReadiness,
    pub(crate) production_rotation: VaultProductionRotationReadiness,
    pub(crate) production_recovery: VaultKmsRecoveryReadiness,
    pub(crate) secret_record_count: usize,
    pub(crate) active_secret_record_count: usize,
    pub(crate) provider_ref_count: usize,
    pub(crate) mcp_secret_ref_count: usize,
    pub(crate) eval_judge_secret_ref_count: usize,
    pub(crate) unresolved_ref_count: usize,
    pub(crate) stale_rotation_count: usize,
    pub(crate) checks: Vec<VaultReadinessCheck>,
    pub(crate) attention_items: Vec<VaultReadinessAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultProductionRotationReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) vault_healthy: bool,
    pub(crate) kms_ready: bool,
    pub(crate) unresolved_refs_clear: bool,
    pub(crate) stale_rotations_clear: bool,
    pub(crate) latest_rotation_validated: bool,
    pub(crate) latest_rotation_run_at: Option<DateTime<Utc>>,
    pub(crate) latest_rotation_run_status: Option<String>,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultKmsRecoveryReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_recovery_at: Option<DateTime<Utc>>,
    pub(crate) latest_recovery_status: Option<String>,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) latest_controller_production_backend: bool,
    pub(crate) latest_controller_backend_kind: Option<String>,
    pub(crate) latest_controller_environment: Option<String>,
    pub(crate) latest_controller_backend_id: Option<String>,
    pub(crate) latest_controller_key_id: Option<String>,
    pub(crate) latest_controller_hsm_provider: Option<String>,
    pub(crate) latest_rotation_validated: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultKmsReadiness {
    pub(crate) provider: String,
    pub(crate) status: String,
    pub(crate) configured: bool,
    pub(crate) key_id_configured: bool,
    pub(crate) rotation_policy_configured: bool,
    pub(crate) endpoint_configured: bool,
    pub(crate) validation_mode: String,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultKmsRotationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) kms_provider: String,
    pub(crate) kms_status: String,
    pub(crate) kms_endpoint_configured: bool,
    pub(crate) secret_provider_status: String,
    pub(crate) secret_record_count: usize,
    pub(crate) stale_rotation_count: usize,
    pub(crate) rotated_count: usize,
    pub(crate) catalog_updated_count: usize,
    pub(crate) rotation_details: Vec<VaultKmsRotationDetail>,
    pub(crate) blocked_count: usize,
    pub(crate) actions: Vec<String>,
    pub(crate) external_execution: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultKmsRotationDetail {
    pub(crate) key_id: String,
    pub(crate) rotation_id: String,
    pub(crate) secret_record_id: Uuid,
    pub(crate) status: String,
    pub(crate) catalog_updated: bool,
    pub(crate) audit_id: Uuid,
    pub(crate) rotated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultKmsRecoveryValidationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) kms_provider: String,
    pub(crate) secret_record_count: usize,
    pub(crate) latest_rotation_validated: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultReadinessCheck {
    pub(crate) resource_type: String,
    pub(crate) resource_id: Option<Uuid>,
    pub(crate) resource_name: String,
    pub(crate) status: String,
    pub(crate) secret_refs: Vec<String>,
    pub(crate) blockers: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VaultReadinessAttentionItem {
    pub(crate) resource_type: String,
    pub(crate) resource_id: Option<Uuid>,
    pub(crate) resource_name: String,
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SecretRecord {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) key: String,
    pub(crate) scope_type: String,
    pub(crate) scope_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSecretRecord {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) key: String,
    #[serde(default = "crate::default_secret_scope_type")]
    pub(crate) scope_type: String,
    #[serde(default)]
    pub(crate) scope_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RotateSecretRecord {
    pub(crate) path: String,
    pub(crate) key: String,
    #[serde(default)]
    pub(crate) value: Option<String>,
}
