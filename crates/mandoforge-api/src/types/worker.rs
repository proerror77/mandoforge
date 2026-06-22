use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerReadinessReport {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) readiness_score: i64,
    pub(crate) queue_backend: WorkerQueueBackendReadiness,
    pub(crate) worker_mode: WorkerModeReadiness,
    pub(crate) job_summary: WorkerJobSummary,
    pub(crate) lease_summary: WorkerLeaseSummary,
    pub(crate) k8s: WorkerK8sReadiness,
    pub(crate) autoscaling: WorkerAutoscalingReadiness,
    pub(crate) load_validation: WorkerLoadValidationEvidence,
    pub(crate) production_ops: WorkerProductionOpsReadiness,
    pub(crate) attention_items: Vec<WorkerReadinessAttentionItem>,
    pub(crate) runbook_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) durable_queue: bool,
    pub(crate) queue_worker_mode: bool,
    pub(crate) hardened_worker_pod: bool,
    pub(crate) queue_depth_autoscaling: bool,
    pub(crate) load_validated: bool,
    pub(crate) isolated_worker_pool_configured: bool,
    pub(crate) no_failed_jobs: bool,
    pub(crate) no_stale_leases: bool,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerLoadValidationEvidence {
    pub(crate) status: String,
    pub(crate) latest_run_at: Option<DateTime<Utc>>,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) load_validated: bool,
    pub(crate) isolated_worker_pool_configured: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) required_profile: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerLoadValidationRun {
    pub(crate) status: String,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) queue_backend: String,
    pub(crate) worker_mode: String,
    pub(crate) autoscaling_status: String,
    pub(crate) autoscaling: WorkerAutoscalingReadiness,
    pub(crate) load_validated: bool,
    pub(crate) isolated_worker_pool_configured: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerQueueBackendReadiness {
    pub(crate) kind: String,
    pub(crate) durable: bool,
    pub(crate) broker_handoff: bool,
    pub(crate) jetstream_enabled: bool,
    pub(crate) semantics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerModeReadiness {
    pub(crate) mode: String,
    pub(crate) external_worker_required: bool,
    pub(crate) api_inline_execution: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerJobSummary {
    pub(crate) total_jobs: usize,
    pub(crate) queued_jobs: usize,
    pub(crate) running_jobs: usize,
    pub(crate) completed_jobs: usize,
    pub(crate) failed_jobs: usize,
    pub(crate) retryable_jobs: usize,
    pub(crate) oldest_queued_job_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerLeaseSummary {
    pub(crate) running_jobs: usize,
    pub(crate) leased_jobs: usize,
    pub(crate) stale_leases: usize,
    pub(crate) oldest_stale_lease_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerK8sReadiness {
    pub(crate) worker_manifest_present: bool,
    pub(crate) worker_manifest_path: String,
    pub(crate) service_account_name: Option<String>,
    pub(crate) service_account_manifest_present: bool,
    pub(crate) service_account_manifest_path: String,
    pub(crate) automount_service_account_token_disabled: bool,
    pub(crate) pod_run_as_non_root: bool,
    pub(crate) seccomp_runtime_default: bool,
    pub(crate) container_allow_privilege_escalation_disabled: bool,
    pub(crate) container_read_only_root_filesystem: bool,
    pub(crate) container_drops_all_capabilities: bool,
    pub(crate) resources_requests_configured: bool,
    pub(crate) resources_limits_configured: bool,
    pub(crate) network_policy_present: bool,
    pub(crate) network_policy_path: String,
    pub(crate) hardening_status: String,
    pub(crate) scheduler_manifest_present: bool,
    pub(crate) scheduler_manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerAutoscalingReadiness {
    pub(crate) autoscaling_manifest_present: bool,
    pub(crate) autoscaling_manifest_paths: Vec<String>,
    pub(crate) configured_min_replicas: Option<i64>,
    pub(crate) configured_max_replicas: Option<i64>,
    pub(crate) scale_target_refs: Vec<String>,
    pub(crate) trigger_types: Vec<String>,
    pub(crate) queue_depth_scaling_present: bool,
    pub(crate) validation_status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct K8sAutoscalingManifest {
    pub(crate) kind: Option<String>,
    pub(crate) spec: Option<K8sAutoscalingSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct K8sAutoscalingSpec {
    pub(crate) scale_target_ref: Option<K8sScaleTargetRef>,
    pub(crate) min_replicas: Option<i64>,
    pub(crate) max_replicas: Option<i64>,
    pub(crate) min_replica_count: Option<i64>,
    pub(crate) max_replica_count: Option<i64>,
    pub(crate) triggers: Option<Vec<K8sAutoscalingTrigger>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct K8sScaleTargetRef {
    pub(crate) kind: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct K8sAutoscalingTrigger {
    #[serde(rename = "type")]
    pub(crate) trigger_type: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkerReadinessAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
}
