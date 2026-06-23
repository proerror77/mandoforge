use serde_json::{Value, json};

use crate::{
    K8sAutoscalingManifest, WorkerAutoscalingReadiness, WorkerK8sReadiness,
    WorkerQueueBackendReadiness, env_i64, manifest_has_kind_name, network_policy_targets_app,
    project_file_path, read_yaml_manifest_value,
};

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
