use serde_json::Value;

use crate::{
    WorkerK8sReadiness, WorkerQueueBackendReadiness, manifest_has_kind_name,
    network_policy_targets_app, project_file_path, read_yaml_manifest_value,
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
