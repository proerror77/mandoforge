use chrono::Utc;
use serde_json::{Value, json};

use crate::{
    AppError, AppState, ExecutionJobStatus, K8sAutoscalingManifest, WorkerAutoscalingReadiness,
    WorkerJobSummary, WorkerK8sReadiness, WorkerLeaseSummary, WorkerLoadValidationEvidence,
    WorkerModeReadiness, WorkerProductionOpsReadiness, WorkerQueueBackendReadiness,
    WorkerReadinessAttentionItem, WorkerReadinessReport, env_i64, manifest_has_kind_name,
    network_policy_targets_app, project_file_path, read_yaml_manifest_value,
    worker_load_validation_evidence,
};

pub(crate) async fn build_worker_readiness(
    state: &AppState,
) -> Result<WorkerReadinessReport, AppError> {
    let generated_at = Utc::now();
    let queue_backend = worker_queue_backend_readiness(state.execution_queue.backend_kind());
    let jobs = state.execution_queue.list().await?;
    let worker_mode = WorkerModeReadiness {
        mode: state.execution_worker.mode().to_string(),
        external_worker_required: state.execution_worker.mode() == "queue",
        api_inline_execution: state.execution_worker.mode() == "inline",
    };
    let mut queued_jobs = 0usize;
    let mut running_jobs = 0usize;
    let mut completed_jobs = 0usize;
    let mut failed_jobs = 0usize;
    let mut retryable_jobs = 0usize;
    let mut leased_jobs = 0usize;
    let mut stale_leases = 0usize;
    let mut oldest_queued_at = None;
    let mut oldest_stale_lease_at = None;

    for job in &jobs {
        match job.status {
            ExecutionJobStatus::Queued => {
                queued_jobs += 1;
                oldest_queued_at = Some(match oldest_queued_at {
                    Some(oldest) if oldest <= job.enqueued_at => oldest,
                    _ => job.enqueued_at,
                });
            }
            ExecutionJobStatus::Running => running_jobs += 1,
            ExecutionJobStatus::Completed => completed_jobs += 1,
            ExecutionJobStatus::Failed => failed_jobs += 1,
            ExecutionJobStatus::Canceled => {}
        }
        if job.attempt_count > 0
            && job.attempt_count < job.max_attempts
            && job.status != ExecutionJobStatus::Completed
            && job.status != ExecutionJobStatus::Canceled
        {
            retryable_jobs += 1;
        }
        if job.lease_expires_at.is_some() {
            leased_jobs += 1;
        }
        if job.status == ExecutionJobStatus::Running
            && job
                .lease_expires_at
                .is_some_and(|lease_expires_at| lease_expires_at < generated_at)
        {
            stale_leases += 1;
            if let Some(lease_expires_at) = job.lease_expires_at {
                oldest_stale_lease_at = Some(match oldest_stale_lease_at {
                    Some(oldest) if oldest <= lease_expires_at => oldest,
                    _ => lease_expires_at,
                });
            }
        }
    }

    let oldest_queued_job_age_seconds = oldest_queued_at.map(|queued_at| {
        generated_at
            .signed_duration_since(queued_at)
            .num_seconds()
            .max(0)
    });
    let oldest_stale_lease_age_seconds = oldest_stale_lease_at.map(|lease_at| {
        generated_at
            .signed_duration_since(lease_at)
            .num_seconds()
            .max(0)
    });
    let job_summary = WorkerJobSummary {
        total_jobs: jobs.len(),
        queued_jobs,
        running_jobs,
        completed_jobs,
        failed_jobs,
        retryable_jobs,
        oldest_queued_job_age_seconds,
    };
    let lease_summary = WorkerLeaseSummary {
        running_jobs,
        leased_jobs,
        stale_leases,
        oldest_stale_lease_age_seconds,
    };
    let k8s = worker_k8s_readiness_from_manifests();
    let autoscaling = worker_autoscaling_readiness_from_manifests(&[
        "deploy/k8s/worker-hpa.yaml",
        "deploy/k8s/worker-keda.yaml",
        "deploy/k8s/keda.yaml",
        "deploy/k8s/worker-isolated-pool-keda.yaml",
    ]);
    let load_validation = worker_load_validation_evidence(state).await?;
    let production_ops = build_worker_production_ops_readiness(
        &queue_backend,
        &worker_mode,
        &k8s,
        &autoscaling,
        &load_validation,
        failed_jobs,
        stale_leases,
    );
    let mut attention_items = Vec::new();
    if worker_mode.api_inline_execution {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "inline_worker_mode".to_string(),
            severity: "warning".to_string(),
            message: "approved tools still execute in the API process; use MANDOFORGE_EXECUTION_WORKER=queue for production drains".to_string(),
        });
    }
    if queue_backend.kind == "memory" {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "process_local_queue".to_string(),
            severity: "warning".to_string(),
            message: "memory queue is process-local and does not survive API restart".to_string(),
        });
    }
    if queue_backend.kind == "nats" && !queue_backend.jetstream_enabled {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "core_nats_non_jetstream".to_string(),
            severity: "warning".to_string(),
            message:
                "NATS backend is Core NATS broker handoff; JetStream durability is not enabled"
                    .to_string(),
        });
    }
    if queued_jobs > 0 {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "queued_jobs_present".to_string(),
            severity: "warning".to_string(),
            message: format!("{queued_jobs} execution job(s) are waiting for a worker drain"),
        });
    }
    if retryable_jobs > 0 {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "retryable_jobs_present".to_string(),
            severity: "warning".to_string(),
            message: format!(
                "{retryable_jobs} execution job(s) can retry before exhausting attempts"
            ),
        });
    }
    if failed_jobs > 0 {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "failed_jobs_present".to_string(),
            severity: "critical".to_string(),
            message: format!(
                "{failed_jobs} execution job(s) exhausted attempts and require triage"
            ),
        });
    }
    if stale_leases > 0 {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "stale_worker_leases".to_string(),
            severity: "critical".to_string(),
            message: format!("{stale_leases} running execution job lease(s) are expired"),
        });
    }
    if !k8s.worker_manifest_present {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_manifest_missing".to_string(),
            severity: "warning".to_string(),
            message: "deploy/k8s/worker.yaml is not present in this runtime package".to_string(),
        });
    }
    if k8s.worker_manifest_present && k8s.hardening_status != "hardened" {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_hardening_incomplete".to_string(),
            severity: "warning".to_string(),
            message: "worker Deployment is present, but securityContext, ServiceAccount, NetworkPolicy, or resource bounds are incomplete".to_string(),
        });
    }
    if !autoscaling.autoscaling_manifest_present {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_autoscaling_missing".to_string(),
            severity: "warning".to_string(),
            message: "no worker HPA/KEDA manifest is present; autoscaling remains a production gap"
                .to_string(),
        });
    } else if autoscaling.validation_status == "skeleton" {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_autoscaling_skeleton".to_string(),
            severity: "warning".to_string(),
            message: "worker HPA/KEDA manifest is present, but production autoscaling still needs cluster metrics, load validation, and isolation policy".to_string(),
        });
    } else if autoscaling.validation_status == "queue_depth_configured" {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_autoscaling_load_validation_missing".to_string(),
            severity: "warning".to_string(),
            message: "worker KEDA queue-depth scaling is configured, but production load validation and isolation policy remain required".to_string(),
        });
    }
    if !load_validation.load_validated {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_load_validation_missing".to_string(),
            severity: "warning".to_string(),
            message: load_validation.message.clone(),
        });
    }
    if production_ops.production_blocked {
        attention_items.push(WorkerReadinessAttentionItem {
            kind: "worker_production_ops_blocked".to_string(),
            severity: "critical".to_string(),
            message: production_ops.message.clone(),
        });
    }

    let mut runbook_actions = Vec::new();
    if worker_mode.api_inline_execution {
        runbook_actions
            .push("set MANDOFORGE_EXECUTION_WORKER=queue before production pilot".to_string());
    }
    if queued_jobs > 0 || retryable_jobs > 0 {
        runbook_actions.push(
            "run mandoforge-worker or POST /api/execution-jobs/:id/run for claimable jobs"
                .to_string(),
        );
    }
    if stale_leases > 0 {
        runbook_actions.push(
            "restart or reclaim stale worker leases before resuming high-risk actions".to_string(),
        );
    }
    if failed_jobs > 0 {
        runbook_actions.push(
            "inspect failed execution job last_error and related tool/audit logs".to_string(),
        );
    }
    if !autoscaling.autoscaling_manifest_present {
        runbook_actions.push(
            "add HPA/KEDA worker autoscaling before declaring production hardening complete"
                .to_string(),
        );
    } else if autoscaling.validation_status == "skeleton" {
        runbook_actions.push(
            "validate worker HPA/KEDA behavior under load before declaring production autoscaling complete"
                .to_string(),
        );
    } else if autoscaling.validation_status == "queue_depth_configured" {
        runbook_actions.push(
            "run a production-like queue pressure test against worker KEDA scaling before declaring autoscaling validated"
                .to_string(),
        );
    }
    if !load_validation.load_validated {
        runbook_actions.push(
            "run POST /api/execution-jobs/worker-load-validation/run after a queue pressure test and isolated worker-pool check; keep Stage 2 production pilot blocked until it reports validated"
                .to_string(),
        );
    }
    if production_ops.production_blocked {
        runbook_actions.push("resolve_worker_production_ops_gate".to_string());
    }
    if queue_backend.kind == "nats" && !queue_backend.jetstream_enabled {
        runbook_actions.push(
            "replace Core NATS handoff with JetStream before claiming durable NATS queues"
                .to_string(),
        );
    }
    if k8s.worker_manifest_present && k8s.hardening_status != "hardened" {
        runbook_actions.push(
            "complete worker Pod hardening: restricted ServiceAccount, token automount disabled, RuntimeDefault seccomp, no privilege escalation, dropped capabilities, read-only root filesystem, resource bounds, and NetworkPolicy".to_string(),
        );
    }

    let critical_count = attention_items
        .iter()
        .filter(|item| item.severity == "critical")
        .count() as i64;
    let warning_count = attention_items
        .iter()
        .filter(|item| item.severity == "warning")
        .count() as i64;
    let readiness_score = (100_i64 - critical_count * 25 - warning_count * 10).clamp(0, 100);
    let status = if critical_count > 0 {
        "critical"
    } else if warning_count > 0 {
        "attention"
    } else {
        "ready"
    }
    .to_string();

    Ok(WorkerReadinessReport {
        generated_at,
        status,
        readiness_score,
        queue_backend,
        worker_mode,
        job_summary,
        lease_summary,
        k8s,
        autoscaling,
        load_validation,
        production_ops,
        attention_items,
        runbook_actions,
    })
}

pub(crate) fn worker_queue_backend_readiness(kind: &str) -> WorkerQueueBackendReadiness {
    match kind {
        "postgres" => WorkerQueueBackendReadiness {
            kind: "postgres".to_string(),
            durable: true,
            broker_handoff: false,
            jetstream_enabled: false,
            semantics: "Postgres-backed durable execution_jobs table with lease/retry semantics"
                .to_string(),
        },
        "redis" => WorkerQueueBackendReadiness {
            kind: "redis".to_string(),
            durable: true,
            broker_handoff: true,
            jetstream_enabled: false,
            semantics: "Redis Streams broker-backed queue with XADD/XREADGROUP/XACK handoff"
                .to_string(),
        },
        "nats" => WorkerQueueBackendReadiness {
            kind: "nats".to_string(),
            durable: false,
            broker_handoff: true,
            jetstream_enabled: false,
            semantics: "Core NATS queue subscription handoff; not JetStream durable".to_string(),
        },
        "nats_jetstream" => WorkerQueueBackendReadiness {
            kind: "nats_jetstream".to_string(),
            durable: true,
            broker_handoff: true,
            jetstream_enabled: true,
            semantics: "NATS JetStream durable stream with request/reply publish ack, durable pull-consumer drain, explicit ack, and redelivery semantics".to_string(),
        },
        _ => WorkerQueueBackendReadiness {
            kind: "memory".to_string(),
            durable: false,
            broker_handoff: false,
            jetstream_enabled: false,
            semantics: "process-local in-memory queue for local demo and tests".to_string(),
        },
    }
}

pub(crate) fn worker_k8s_readiness_from_manifests() -> WorkerK8sReadiness {
    let worker_manifest_path = "deploy/k8s/worker.yaml";
    let service_account_manifest_path = "deploy/k8s/worker-serviceaccount.yaml";
    let network_policy_path = "deploy/k8s/worker-networkpolicy.yaml";
    let scheduler_manifest_path = "deploy/k8s/scheduler.yaml";

    let worker_manifest = read_yaml_manifest_value(worker_manifest_path).and_then(|manifest| {
        (manifest.get("kind").and_then(Value::as_str) == Some("Deployment")).then_some(manifest)
    });
    let worker_manifest_present = worker_manifest.is_some();
    let service_account_name = worker_manifest
        .as_ref()
        .and_then(|manifest| manifest.pointer("/spec/template/spec/serviceAccountName"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let automount_service_account_token_disabled = worker_manifest
        .as_ref()
        .and_then(|manifest| manifest.pointer("/spec/template/spec/automountServiceAccountToken"))
        .and_then(Value::as_bool)
        == Some(false);
    let pod_run_as_non_root = worker_manifest
        .as_ref()
        .and_then(|manifest| manifest.pointer("/spec/template/spec/securityContext/runAsNonRoot"))
        .and_then(Value::as_bool)
        == Some(true);
    let seccomp_runtime_default = worker_manifest
        .as_ref()
        .and_then(|manifest| {
            manifest.pointer("/spec/template/spec/securityContext/seccompProfile/type")
        })
        .and_then(Value::as_str)
        == Some("RuntimeDefault");
    let worker_container = worker_manifest
        .as_ref()
        .and_then(|manifest| manifest.pointer("/spec/template/spec/containers"))
        .and_then(Value::as_array)
        .and_then(|containers| {
            containers
                .iter()
                .find(|container| container.get("name").and_then(Value::as_str) == Some("worker"))
        });
    let container_allow_privilege_escalation_disabled = worker_container
        .and_then(|container| container.pointer("/securityContext/allowPrivilegeEscalation"))
        .and_then(Value::as_bool)
        == Some(false);
    let container_read_only_root_filesystem = worker_container
        .and_then(|container| container.pointer("/securityContext/readOnlyRootFilesystem"))
        .and_then(Value::as_bool)
        == Some(true);
    let container_drops_all_capabilities = worker_container
        .and_then(|container| container.pointer("/securityContext/capabilities/drop"))
        .and_then(Value::as_array)
        .is_some_and(|drops| drops.iter().any(|drop| drop.as_str() == Some("ALL")));
    let resources_requests_configured = worker_container
        .and_then(|container| container.pointer("/resources/requests"))
        .and_then(Value::as_object)
        .is_some_and(|requests| requests.contains_key("cpu") && requests.contains_key("memory"));
    let resources_limits_configured = worker_container
        .and_then(|container| container.pointer("/resources/limits"))
        .and_then(Value::as_object)
        .is_some_and(|limits| limits.contains_key("cpu") && limits.contains_key("memory"));
    let service_account_manifest_present = service_account_name.as_deref().is_some_and(|name| {
        manifest_has_kind_name(service_account_manifest_path, "ServiceAccount", name)
    });
    let network_policy_present =
        network_policy_targets_app(network_policy_path, "mandoforge-worker");

    let hardening_complete = worker_manifest_present
        && service_account_manifest_present
        && automount_service_account_token_disabled
        && pod_run_as_non_root
        && seccomp_runtime_default
        && container_allow_privilege_escalation_disabled
        && container_read_only_root_filesystem
        && container_drops_all_capabilities
        && resources_requests_configured
        && resources_limits_configured
        && network_policy_present;
    let hardening_status = if hardening_complete {
        "hardened"
    } else if worker_manifest_present {
        "incomplete"
    } else {
        "missing"
    }
    .to_string();

    WorkerK8sReadiness {
        worker_manifest_present,
        worker_manifest_path: worker_manifest_path.to_string(),
        service_account_name,
        service_account_manifest_present,
        service_account_manifest_path: service_account_manifest_path.to_string(),
        automount_service_account_token_disabled,
        pod_run_as_non_root,
        seccomp_runtime_default,
        container_allow_privilege_escalation_disabled,
        container_read_only_root_filesystem,
        container_drops_all_capabilities,
        resources_requests_configured,
        resources_limits_configured,
        network_policy_present,
        network_policy_path: network_policy_path.to_string(),
        hardening_status,
        scheduler_manifest_present: project_file_path(scheduler_manifest_path).is_some(),
        scheduler_manifest_path: scheduler_manifest_path.to_string(),
    }
}

pub(crate) fn worker_autoscaling_readiness_from_manifests(
    paths: &[&str],
) -> WorkerAutoscalingReadiness {
    let mut autoscaling_manifest_paths = Vec::new();
    let mut configured_min_replicas = env_i64("MANDOFORGE_WORKER_MIN_REPLICAS");
    let mut configured_max_replicas = env_i64("MANDOFORGE_WORKER_MAX_REPLICAS");
    let mut scale_target_refs = Vec::new();
    let mut trigger_types = Vec::new();
    let mut queue_depth_scaling_present = false;

    for path in paths {
        let Some(resolved_path) = project_file_path(path) else {
            continue;
        };
        autoscaling_manifest_paths.push((*path).to_string());
        let Ok(content) = std::fs::read_to_string(resolved_path) else {
            continue;
        };
        let Ok(manifest) = serde_yaml::from_str::<K8sAutoscalingManifest>(&content) else {
            continue;
        };
        let Some(spec) = manifest.spec else {
            continue;
        };
        if let Some(min_replicas) = spec.min_replicas.or(spec.min_replica_count) {
            configured_min_replicas = Some(
                configured_min_replicas
                    .map(|current| current.min(min_replicas))
                    .unwrap_or(min_replicas),
            );
        }
        if let Some(max_replicas) = spec.max_replicas.or(spec.max_replica_count) {
            configured_max_replicas = Some(
                configured_max_replicas
                    .map(|current| current.max(max_replicas))
                    .unwrap_or(max_replicas),
            );
        }
        if let Some(target) = spec.scale_target_ref {
            let kind = target.kind.unwrap_or_else(|| "unknown".to_string());
            let name = target.name.unwrap_or_else(|| "unknown".to_string());
            scale_target_refs.push(format!("{kind}/{name}"));
        } else if let Some(kind) = manifest.kind {
            scale_target_refs.push(format!("{kind}/unknown"));
        }
        for trigger in spec.triggers.unwrap_or_default() {
            let trigger_type = trigger
                .trigger_type
                .unwrap_or_else(|| "unknown".to_string());
            let metadata = trigger.metadata.unwrap_or_else(|| json!({}));
            if trigger_type == "prometheus"
                && metadata
                    .get("query")
                    .and_then(Value::as_str)
                    .is_some_and(|query| {
                        query.contains("mandoforge_execution_jobs_queued")
                            || query.contains("queue_depth")
                    })
            {
                queue_depth_scaling_present = true;
            }
            trigger_types.push(trigger_type);
        }
    }

    let validation_status = if autoscaling_manifest_paths.is_empty() {
        "missing"
    } else if queue_depth_scaling_present
        && scale_target_refs
            .iter()
            .any(|target| target == "Deployment/mandoforge-worker")
    {
        "queue_depth_configured"
    } else {
        "skeleton"
    }
    .to_string();

    WorkerAutoscalingReadiness {
        autoscaling_manifest_present: !autoscaling_manifest_paths.is_empty(),
        autoscaling_manifest_paths,
        configured_min_replicas,
        configured_max_replicas,
        scale_target_refs,
        trigger_types,
        queue_depth_scaling_present,
        validation_status,
    }
}

pub(crate) fn worker_isolated_pool_configured_from_manifests() -> bool {
    let isolated_deployment_path = "deploy/k8s/worker-isolated-pool.yaml";
    let isolated_network_policy_path = "deploy/k8s/worker-isolated-pool-networkpolicy.yaml";
    let isolated_keda_path = "deploy/k8s/worker-isolated-pool-keda.yaml";
    let deployment_present = manifest_has_kind_name(
        isolated_deployment_path,
        "Deployment",
        "mandoforge-worker-isolated",
    );
    let network_policy_present =
        network_policy_targets_app(isolated_network_policy_path, "mandoforge-worker-isolated");
    let autoscaling = worker_autoscaling_readiness_from_manifests(&[isolated_keda_path]);
    deployment_present
        && network_policy_present
        && autoscaling.queue_depth_scaling_present
        && autoscaling
            .scale_target_refs
            .iter()
            .any(|target| target == "Deployment/mandoforge-worker-isolated")
}

pub(crate) fn build_worker_production_ops_readiness(
    queue_backend: &WorkerQueueBackendReadiness,
    worker_mode: &WorkerModeReadiness,
    k8s: &WorkerK8sReadiness,
    autoscaling: &WorkerAutoscalingReadiness,
    load_validation: &WorkerLoadValidationEvidence,
    failed_jobs: usize,
    stale_leases: usize,
) -> WorkerProductionOpsReadiness {
    let durable_queue = queue_backend.durable;
    let queue_worker_mode = worker_mode.mode == "queue";
    let hardened_worker_pod = k8s.hardening_status == "hardened";
    let queue_depth_autoscaling = autoscaling.validation_status == "queue_depth_configured";
    let no_failed_jobs = failed_jobs == 0;
    let no_stale_leases = stale_leases == 0;
    let mut blocking_reasons = Vec::new();

    if !durable_queue {
        blocking_reasons.push("execution queue is not durable".to_string());
    }
    if !queue_worker_mode {
        blocking_reasons.push("runtime is not using queue-backed worker mode".to_string());
    }
    if !hardened_worker_pod {
        blocking_reasons.push("worker Pod hardening is incomplete".to_string());
    }
    if !queue_depth_autoscaling {
        blocking_reasons.push("queue-depth autoscaling is not configured".to_string());
    }
    if !load_validation.load_validated {
        blocking_reasons.push("production-like worker load validation has not passed".to_string());
    }
    if !load_validation.isolated_worker_pool_configured {
        blocking_reasons.push("isolated worker pool is not configured".to_string());
    }
    if !no_failed_jobs {
        blocking_reasons.push("failed execution jobs require triage".to_string());
    }
    if !no_stale_leases {
        blocking_reasons.push("stale worker leases require reclaim".to_string());
    }

    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Worker production ops are blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Worker production ops have a durable queue, queue worker mode, hardened Pod, queue-depth autoscaling, isolated pool, and validated load evidence".to_string()
    };

    WorkerProductionOpsReadiness {
        status,
        production_blocked,
        durable_queue,
        queue_worker_mode,
        hardened_worker_pod,
        queue_depth_autoscaling,
        load_validated: load_validation.load_validated,
        isolated_worker_pool_configured: load_validation.isolated_worker_pool_configured,
        no_failed_jobs,
        no_stale_leases,
        blocking_reasons,
        message,
    }
}
