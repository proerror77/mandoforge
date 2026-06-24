use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::remote_computer_runner::{
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarSupervisionRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) active_remote_computer_count: usize,
    pub(crate) heartbeat_count: usize,
    pub(crate) missing_heartbeat_count: usize,
    pub(crate) stale_heartbeat_count: usize,
    pub(crate) stale_after_seconds: i64,
    pub(crate) actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerReadinessReport {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) readiness_score: i64,
    pub(crate) pod_template: RemoteComputerManifestReadiness,
    pub(crate) service_account: RemoteComputerManifestReadiness,
    pub(crate) state_filesystem: RemoteComputerStateFilesystemReadiness,
    pub(crate) production_state_sync: RemoteComputerProductionStateSyncReadiness,
    pub(crate) network_policy: RemoteComputerManifestReadiness,
    pub(crate) autoscaling: RemoteComputerAutoscalingReadiness,
    pub(crate) warm_pool: RemoteComputerWarmPoolReadiness,
    pub(crate) artifact_discovery_sidecar: RemoteComputerManifestReadiness,
    pub(crate) artifact_discovery_sidecar_config:
        RemoteComputerArtifactDiscoverySidecarConfigReadiness,
    pub(crate) sidecar_supervision: RemoteComputerSidecarSupervisionReadiness,
    pub(crate) sidecar_recovery: RemoteComputerSidecarRecoveryReadiness,
    pub(crate) runner: RemoteComputerRunnerReadiness,
    pub(crate) execution_transport: RemoteComputerExecutionTransportReadiness,
    pub(crate) event_types: Vec<String>,
    pub(crate) attention_items: Vec<RemoteComputerAttentionItem>,
    pub(crate) runbook_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerProductionStateSyncReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) distributed_filesystem_configured: bool,
    pub(crate) production_profile_present: bool,
    pub(crate) state_contract_present: bool,
    pub(crate) lock_manager_configured: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) conflict_policy: String,
    pub(crate) provider: String,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerStateSyncValidationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerManifestReadiness {
    pub(crate) present: bool,
    pub(crate) path: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerStateFilesystemReadiness {
    pub(crate) pvc_present: bool,
    pub(crate) pvc_path: String,
    pub(crate) access_mode: String,
    pub(crate) mount_path: String,
    pub(crate) state_contract_present: bool,
    pub(crate) state_contract_path: String,
    pub(crate) state_layout_paths: Vec<String>,
    pub(crate) conflict_policy: String,
    pub(crate) lock_manager_configured: bool,
    pub(crate) sync_contract_status: String,
    pub(crate) distributed_filesystem_configured: bool,
    pub(crate) provider: String,
    pub(crate) provider_configured_by_env: bool,
    pub(crate) provider_manifest_present: bool,
    pub(crate) provider_manifest_path: String,
    pub(crate) production_profile_present: bool,
    pub(crate) production_profile_path: String,
    pub(crate) production_claim_name: String,
    pub(crate) supported_providers: Vec<String>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerAutoscalingReadiness {
    pub(crate) worker_hpa_present: bool,
    pub(crate) keda_manifest_present: bool,
    pub(crate) remote_pool_scaled_object_present: bool,
    pub(crate) remote_pool_scaled_object_path: String,
    pub(crate) queue_depth_scaling_present: bool,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerWarmPoolReadiness {
    pub(crate) configured: bool,
    pub(crate) manifest_present: bool,
    pub(crate) manifest_path: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerArtifactDiscoverySidecarConfigReadiness {
    pub(crate) status: String,
    pub(crate) expected_api_url: String,
    pub(crate) pod_template_api_url_configured: bool,
    pub(crate) warm_pool_api_url_configured: bool,
    pub(crate) configmap_default_api_url_configured: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarSupervisionReadiness {
    pub(crate) status: String,
    pub(crate) heartbeat_count: usize,
    pub(crate) active_remote_computer_count: usize,
    pub(crate) missing_heartbeat_count: usize,
    pub(crate) stale_heartbeat_count: usize,
    pub(crate) stale_after_seconds: i64,
    pub(crate) latest_observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarRecoveryReadiness {
    pub(crate) status: String,
    pub(crate) replacement_enabled: bool,
    pub(crate) validation_controller_required: bool,
    pub(crate) validation_controller_configured: bool,
    pub(crate) runner_configured: bool,
    pub(crate) runner_live_mutation_enabled: bool,
    pub(crate) unhealthy_count: usize,
    pub(crate) replaceable_pod_count: usize,
    pub(crate) blocked_reason: Option<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarRecoveryTarget {
    pub(crate) remote_computer_id: Uuid,
    pub(crate) name: String,
    pub(crate) pod_name: Option<String>,
    pub(crate) reason: String,
    pub(crate) latest_observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarRecoveryRun {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) replacement_enabled: bool,
    pub(crate) validation_controller_required: bool,
    pub(crate) validation_controller_configured: bool,
    pub(crate) runner_status: String,
    pub(crate) unhealthy_count: usize,
    pub(crate) planned_replacement_count: usize,
    pub(crate) attempted_replacement_count: usize,
    pub(crate) blocked_replacement_count: usize,
    pub(crate) targets: Vec<RemoteComputerSidecarRecoveryTarget>,
    pub(crate) runner_responses: Vec<RemoteComputerRunnerDryRunResponse>,
    pub(crate) validation_result: Value,
    pub(crate) execution_enabled: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerExecutionTransportReadiness {
    pub(crate) mode: String,
    pub(crate) requested_execution_enabled: bool,
    pub(crate) execution_enabled: bool,
    pub(crate) status: String,
    pub(crate) assignment_count: usize,
    pub(crate) active_assignment_count: usize,
    pub(crate) supported_operations: Vec<String>,
    pub(crate) required_implementation: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputer {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) profile: String,
    pub(crate) status: String,
    pub(crate) namespace: String,
    pub(crate) pod_name: Option<String>,
    pub(crate) workspace_path: String,
    pub(crate) state_mount_path: String,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerLease {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) heartbeat_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerAttachment {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) lease_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) status: String,
    pub(crate) attached_by: Option<String>,
    pub(crate) stale_after: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerJobAssignment {
    pub(crate) id: Uuid,
    pub(crate) execution_job_id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) lease_id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) status: String,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerStateLock {
    pub(crate) id: Uuid,
    pub(crate) lock_key: String,
    pub(crate) status: String,
    pub(crate) remote_computer_id: Option<Uuid>,
    pub(crate) lease_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) owner: Option<String>,
    pub(crate) expires_at: Option<DateTime<Utc>>,
    pub(crate) released_at: Option<DateTime<Utc>>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerSidecarHeartbeat {
    pub(crate) id: Uuid,
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) sidecar_name: String,
    pub(crate) status: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) metadata: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputer {
    #[serde(default)]
    pub(crate) id: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) profile: Option<String>,
    pub(crate) namespace: Option<String>,
    pub(crate) pod_name: Option<String>,
    pub(crate) workspace_path: Option<String>,
    pub(crate) state_mount_path: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerLease {
    pub(crate) session_id: Option<Uuid>,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpdateRemoteComputerLease {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerAttachment {
    pub(crate) session_id: Uuid,
    pub(crate) attached_by: Option<String>,
    pub(crate) stale_after_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerJobAssignment {
    pub(crate) lease_id: Uuid,
    pub(crate) assigned_by: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerStateLock {
    pub(crate) lock_key: String,
    pub(crate) remote_computer_id: Option<Uuid>,
    pub(crate) lease_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) owner: Option<String>,
    pub(crate) lease_seconds: Option<i64>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseRemoteComputerStateLock {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreateRemoteComputerSidecarHeartbeat {
    pub(crate) remote_computer_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) assignment_id: Option<Uuid>,
    pub(crate) sidecar_name: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpdateRemoteComputerAttachment {
    pub(crate) reason: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerReclaimRun {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) stale_attachment_count: usize,
    pub(crate) reclaimed_attachment_count: usize,
    pub(crate) expired_lease_count: usize,
    pub(crate) reclaimed_lease_count: usize,
    pub(crate) attachments: Vec<RemoteComputerAttachment>,
    pub(crate) leases: Vec<RemoteComputerLease>,
    pub(crate) execution_enabled: bool,
}
