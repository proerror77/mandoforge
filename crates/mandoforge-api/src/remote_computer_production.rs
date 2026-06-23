use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    AuditLog, RemoteComputerExecutionTransportReadiness, RemoteComputerReadinessReport,
    WorkerReadinessReport,
};

pub(crate) fn build_remote_computer_production_path_payload(
    generated_at: DateTime<Utc>,
    readiness: RemoteComputerReadinessReport,
    execution_transport: RemoteComputerExecutionTransportReadiness,
    worker_readiness: WorkerReadinessReport,
    audit_logs: &[AuditLog],
) -> Value {
    let state_sync_evidence = remote_computer_latest_audit_details(
        audit_logs,
        "remote_computer.production_state_sync_validation",
    );
    let sidecar_recovery_evidence =
        remote_computer_latest_audit_details(audit_logs, "remote_computer.sidecar_recovery_run");
    let state_sync_controller = state_sync_evidence
        .and_then(|details| details.get("controller_execution"))
        .unwrap_or(&Value::Null);
    let sidecar_validation = sidecar_recovery_evidence
        .and_then(|details| details.get("validation_result"))
        .unwrap_or(&Value::Null);

    let state_sync_target_kind = state_sync_controller
        .get("target_kind")
        .and_then(Value::as_str);
    let state_sync_node_count = state_sync_controller
        .get("node_count")
        .and_then(Value::as_u64);
    let state_sync_cluster_id = state_sync_controller
        .get("cluster_id")
        .and_then(Value::as_str);
    let state_sync_backend = state_sync_controller
        .get("distributed_state_backend")
        .or_else(|| state_sync_controller.get("storage_backend"))
        .or_else(|| state_sync_controller.get("state_backend"))
        .and_then(Value::as_str);
    let state_sync_claim = state_sync_controller
        .get("state_claim")
        .and_then(Value::as_str);
    let state_sync_checked_path_count = state_sync_controller
        .get("checked_path_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let state_sync_checked_path_detail_count =
        remote_computer_checked_state_path_detail_count(state_sync_controller);

    let sidecar_target_kind = sidecar_validation
        .get("target_kind")
        .and_then(Value::as_str);
    let sidecar_node_count = sidecar_validation.get("node_count").and_then(Value::as_u64);
    let sidecar_cluster_id = sidecar_validation.get("cluster_id").and_then(Value::as_str);
    let sidecar_checked_pod_count = sidecar_validation
        .get("checked_pod_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let sidecar_checked_pod_detail_count =
        remote_computer_checked_sidecar_pod_detail_count(sidecar_validation);

    let production_cluster_id = std::env::var("MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let state_sync_cluster_matches_expected = production_cluster_id
        .as_deref()
        .is_none_or(|expected| state_sync_cluster_id == Some(expected));
    let sidecar_cluster_matches_expected = production_cluster_id
        .as_deref()
        .is_none_or(|expected| sidecar_cluster_id == Some(expected));

    let mut checks = Vec::new();
    checks.push(remote_computer_production_check(
        "remote_computer_readiness",
        "Remote Computer base readiness is clean",
        readiness.status == "ready",
        json!({
            "status": readiness.status,
            "readiness_score": readiness.readiness_score,
            "critical_attention_items": readiness.attention_items.iter().filter(|item| item.severity == "critical").count(),
            "warning_attention_items": readiness.attention_items.iter().filter(|item| item.severity == "warning").count(),
        }),
        vec!["base Remote Computer readiness is not ready"],
        vec!["clear critical and warning items from /api/remote-computers/readiness"],
    ));
    checks.push(remote_computer_production_check(
        "distributed_state_contract",
        "Distributed state provider, profile, contract, and lock manager are configured",
        readiness.state_filesystem.distributed_filesystem_configured
            && readiness.state_filesystem.production_profile_present
            && readiness.state_filesystem.state_contract_present
            && readiness.state_filesystem.lock_manager_configured
            && !readiness.production_state_sync.production_blocked,
        json!({
            "provider": readiness.state_filesystem.provider,
            "distributed_filesystem_configured": readiness.state_filesystem.distributed_filesystem_configured,
            "production_profile_present": readiness.state_filesystem.production_profile_present,
            "state_contract_present": readiness.state_filesystem.state_contract_present,
            "lock_manager_configured": readiness.state_filesystem.lock_manager_configured,
            "conflict_policy": readiness.state_filesystem.conflict_policy,
            "production_state_sync_status": readiness.production_state_sync.status,
            "production_state_sync_blocking_reasons": readiness.production_state_sync.blocking_reasons,
        }),
        vec![
            "distributed RWX state, production profile, state contract, lock manager, or state-sync readiness is missing",
        ],
        vec![
            "configure JuiceFS/CephFS/Longhorn RWX state, enable the lock manager, then rerun state sync validation",
        ],
    ));
    checks.push(remote_computer_production_check(
        "multi_node_state_sync_evidence",
        "Fresh state-sync evidence proves a production multi-node cluster",
        state_sync_controller.get("status").and_then(Value::as_str) == Some("validated")
            && readiness.production_state_sync.controller_evidence_fresh
            && remote_computer_real_cluster_kind(state_sync_target_kind)
            && state_sync_node_count.is_some_and(|count| count >= 2)
            && remote_computer_production_identity(state_sync_cluster_id)
            && state_sync_cluster_matches_expected
            && remote_computer_distributed_state_backend(state_sync_backend)
            && state_sync_claim.is_some_and(|claim| !claim.trim().is_empty())
            && state_sync_checked_path_count > 0
            && state_sync_checked_path_detail_count >= state_sync_checked_path_count,
        json!({
            "controller_status": state_sync_controller.get("status").and_then(Value::as_str),
            "controller_evidence_fresh": readiness.production_state_sync.controller_evidence_fresh,
            "target_kind": state_sync_target_kind,
            "node_count": state_sync_node_count,
            "cluster_id": state_sync_cluster_id,
            "expected_production_cluster_id": production_cluster_id,
            "cluster_matches_expected": state_sync_cluster_matches_expected,
            "distributed_state_backend": state_sync_backend,
            "state_claim": state_sync_claim,
            "checked_path_count": state_sync_checked_path_count,
            "checked_path_detail_count": state_sync_checked_path_detail_count,
        }),
        vec![
            "state-sync controller evidence is missing, stale, single-node, non-production, or not bound to checked state paths",
        ],
        vec![
            "run /api/remote-computers/state-sync/validate against a two-node production cluster and return checked path details for every state layout path",
        ],
    ));
    checks.push(remote_computer_production_check(
        "worker_pool_production_ops",
        "Worker queue and isolated pool are production-ready",
        worker_readiness.production_ops.status == "ready"
            && !worker_readiness.production_ops.production_blocked
            && worker_readiness.load_validation.latest_controller_validated
            && worker_readiness.load_validation.controller_evidence_fresh,
        json!({
            "worker_status": worker_readiness.status,
            "production_ops_status": worker_readiness.production_ops.status,
            "production_blocked": worker_readiness.production_ops.production_blocked,
            "queue_backend": worker_readiness.queue_backend.kind,
            "load_validated": worker_readiness.load_validation.load_validated,
            "latest_controller_validated": worker_readiness.load_validation.latest_controller_validated,
            "controller_evidence_fresh": worker_readiness.load_validation.controller_evidence_fresh,
            "blocking_reasons": worker_readiness.production_ops.blocking_reasons,
        }),
        vec!["worker production ops or isolated load validation evidence is not ready"],
        vec!["run worker load validation with a durable queue and isolated worker pool before scaling Remote Computer execution"],
    ));
    checks.push(remote_computer_production_check(
        "execution_transport_and_runner",
        "Kubernetes execution transport and live runner mutation are enabled behind policy",
        execution_transport.execution_enabled
            && execution_transport.status == "enabled"
            && readiness.runner.configured
            && readiness.runner.live_mutation_enabled
            && !readiness.runner.dry_run_only,
        json!({
            "execution_transport_status": execution_transport.status,
            "execution_enabled": execution_transport.execution_enabled,
            "runner_status": readiness.runner.status,
            "runner_configured": readiness.runner.configured,
            "runner_live_mutation_enabled": readiness.runner.live_mutation_enabled,
            "runner_dry_run_only": readiness.runner.dry_run_only,
            "supported_operations": execution_transport.supported_operations,
        }),
        vec!["Remote Computer execution transport or Kubernetes runner live mutation remains fail-closed"],
        vec!["enable Kubernetes execution transport only after state sync, worker, and sidecar evidence are ready"],
    ));
    checks.push(remote_computer_production_check(
        "sidecar_recovery_evidence",
        "Sidecar recovery evidence is validated across the production cluster",
        sidecar_validation.get("status").and_then(Value::as_str) == Some("validated")
            && remote_computer_real_cluster_kind(sidecar_target_kind)
            && sidecar_node_count.is_some_and(|count| count >= 2)
            && remote_computer_production_identity(sidecar_cluster_id)
            && sidecar_cluster_matches_expected
            && sidecar_validation.get("replacement_scope").and_then(Value::as_str)
                == Some("cluster")
            && sidecar_validation
                .get("replacement_pods_healthy")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && sidecar_checked_pod_count > 0
            && sidecar_checked_pod_detail_count >= sidecar_checked_pod_count,
        json!({
            "validation_status": sidecar_validation.get("status").and_then(Value::as_str),
            "target_kind": sidecar_target_kind,
            "node_count": sidecar_node_count,
            "cluster_id": sidecar_cluster_id,
            "expected_production_cluster_id": production_cluster_id,
            "cluster_matches_expected": sidecar_cluster_matches_expected,
            "replacement_scope": sidecar_validation.get("replacement_scope").and_then(Value::as_str),
            "replacement_pods_healthy": sidecar_validation.get("replacement_pods_healthy").and_then(Value::as_bool),
            "checked_pod_count": sidecar_checked_pod_count,
            "checked_pod_detail_count": sidecar_checked_pod_detail_count,
        }),
        vec![
            "sidecar recovery validation is missing, single-node, non-production, or not cluster-wide",
        ],
        vec![
            "run /api/remote-computers/sidecars/recovery/run with a validation controller that proves cluster-wide replacement health",
        ],
    ));
    checks.push(remote_computer_production_check(
        "pod_security_and_artifact_sync",
        "Network policy, autoscaling, and artifact sidecar wiring are production-ready",
        readiness.network_policy.present
            && readiness.autoscaling.queue_depth_scaling_present
            && readiness.warm_pool.manifest_present
            && readiness.artifact_discovery_sidecar.present
            && readiness.artifact_discovery_sidecar_config.status == "configured",
        json!({
            "network_policy_present": readiness.network_policy.present,
            "queue_depth_scaling_present": readiness.autoscaling.queue_depth_scaling_present,
            "warm_pool_manifest_present": readiness.warm_pool.manifest_present,
            "artifact_sidecar_present": readiness.artifact_discovery_sidecar.present,
            "artifact_sidecar_config_status": readiness.artifact_discovery_sidecar_config.status,
        }),
        vec!["pod network policy, queue-depth scaling, warm pool, or artifact sidecar wiring is incomplete"],
        vec!["keep Remote Computer execution behind the existing worker path until pod security and artifact sync are fully wired"],
    ));

    let blocked_check_count = checks
        .iter()
        .filter(|check| check.get("status").and_then(Value::as_str) != Some("ready"))
        .count();
    let ready_check_count = checks.len() - blocked_check_count;
    let check_count = checks.len();
    let completion_blocked = blocked_check_count > 0;
    let status = if completion_blocked {
        "blocked"
    } else {
        "ready"
    };
    let blockers: Vec<String> = checks
        .iter()
        .filter(|check| check.get("status").and_then(Value::as_str) != Some("ready"))
        .filter_map(|check| {
            let id = check.get("id").and_then(Value::as_str)?;
            let blockers = check.get("blockers").and_then(Value::as_array)?;
            Some(format!(
                "{id}: {}",
                blockers
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })
        .collect();

    json!({
        "status": status,
        "completion_blocked": completion_blocked,
        "generated_at": generated_at,
        "required_evidence_class": "customer_grade",
        "objective": "Remote Computer multi-node production state",
        "ready_check_count": ready_check_count,
        "blocked_check_count": blocked_check_count,
        "check_count": check_count,
        "production_cluster_id": production_cluster_id,
        "checks": checks,
        "blockers": blockers,
        "raw_readiness": {
            "remote_computer": readiness,
            "execution_transport": execution_transport,
            "worker": worker_readiness,
        },
        "production_path": [
            {"key": "lease_remote_computer", "status": "available"},
            {"key": "assign_execution_job", "status": "available"},
            {"key": "worker_executes_assigned_job", "status": if completion_blocked { "fail_closed" } else { "enabled" }},
            {"key": "sync_artifacts", "status": if completion_blocked { "blocked" } else { "available" }},
            {"key": "audit_and_reclaim", "status": "available"}
        ],
        "next_actions": [
            "configure a real distributed RWX state provider and lock-aware state sync manager",
            "run state-sync validation against a non-pilot two-node production cluster",
            "run sidecar recovery validation against the same production cluster",
            "enable Kubernetes execution transport only after worker, state, and sidecar evidence are fresh"
        ],
        "message": if completion_blocked {
            format!("Remote Computer customer-grade multi-node production path is blocked: {ready_check_count}/{check_count} checks are ready")
        } else {
            "Remote Computer customer-grade multi-node production path is ready".to_string()
        }
    })
}

fn remote_computer_production_check(
    id: &str,
    title: &str,
    ready: bool,
    evidence: Value,
    blockers: Vec<&str>,
    next_actions: Vec<&str>,
) -> Value {
    json!({
        "id": id,
        "title": title,
        "status": if ready { "ready" } else { "blocked" },
        "evidence": evidence,
        "blockers": if ready { Vec::<String>::new() } else { blockers.into_iter().map(str::to_string).collect::<Vec<_>>() },
        "next_actions": if ready { Vec::<String>::new() } else { next_actions.into_iter().map(str::to_string).collect::<Vec<_>>() },
    })
}

fn remote_computer_latest_audit_details<'a>(
    audit_logs: &'a [AuditLog],
    action: &str,
) -> Option<&'a Value> {
    audit_logs
        .iter()
        .filter(|log| log.action == action)
        .max_by_key(|log| log.created_at)
        .map(|log| &log.details)
}

fn remote_computer_real_cluster_kind(value: Option<&str>) -> bool {
    matches!(
        value,
        Some("k8s_cluster" | "kubernetes_cluster" | "production_cluster" | "real_cluster")
    )
}

fn remote_computer_distributed_state_backend(value: Option<&str>) -> bool {
    matches!(value, Some("juicefs" | "cephfs" | "longhorn-rwx"))
}

fn remote_computer_production_identity(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let normalized = value.to_ascii_lowercase();
    ![
        "whiskey",
        "pilot",
        "mock",
        "example",
        "sample",
        "demo",
        "local",
        "localhost",
        "127.0.0.1",
        "[::1]",
    ]
    .iter()
    .any(|token| normalized.contains(token))
}

fn remote_computer_checked_state_path_detail_count(controller_execution: &Value) -> u64 {
    let state_claim = controller_execution
        .get("state_claim")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cluster_id = controller_execution
        .get("cluster_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if state_claim.is_empty() || cluster_id.is_empty() {
        return 0;
    }
    let mut checked_paths = BTreeSet::new();
    for key in ["checked_paths", "checked_state_paths", "path_checks"] {
        if let Some(items) = controller_execution.get(key).and_then(Value::as_array) {
            for item in items {
                let item_cluster = item
                    .get("cluster_id")
                    .or_else(|| item.get("state_sync_cluster_id"))
                    .or_else(|| item.get("target_cluster_id"))
                    .and_then(Value::as_str);
                let item_claim = item
                    .get("state_claim")
                    .or_else(|| item.get("claim"))
                    .or_else(|| item.get("pvc"))
                    .or_else(|| item.get("persistent_volume_claim"))
                    .and_then(Value::as_str);
                let item_path = item
                    .get("path")
                    .or_else(|| item.get("state_path"))
                    .or_else(|| item.get("name"))
                    .and_then(Value::as_str);
                let status = item
                    .get("status")
                    .or_else(|| item.get("result"))
                    .or_else(|| item.get("health"))
                    .and_then(Value::as_str)
                    .map(|value| value.to_ascii_lowercase());
                let has_audit_ref = item
                    .get("audit_id")
                    .or_else(|| item.get("audit_log_id"))
                    .or_else(|| item.get("trace_id"))
                    .or_else(|| item.get("run_id"))
                    .or_else(|| item.get("checked_at"))
                    .or_else(|| item.get("validated_at"))
                    .or_else(|| item.get("timestamp"))
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
                if item_cluster == Some(cluster_id)
                    && item_claim == Some(state_claim)
                    && item_path.is_some_and(|path| !path.trim().is_empty())
                    && status.as_deref().is_some_and(|status| {
                        matches!(
                            status,
                            "passed"
                                | "validated"
                                | "completed"
                                | "ready"
                                | "exists"
                                | "mounted"
                                | "available"
                                | "ok"
                                | "healthy"
                                | "accessible"
                                | "readable"
                                | "writable"
                        )
                    })
                    && has_audit_ref
                {
                    checked_paths.insert(item_path.unwrap().to_string());
                }
            }
        }
    }
    checked_paths.len() as u64
}

fn remote_computer_checked_sidecar_pod_detail_count(validation_result: &Value) -> u64 {
    let cluster_id = validation_result
        .get("cluster_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if cluster_id.is_empty() {
        return 0;
    }
    let mut checked_pods = BTreeSet::new();
    for key in ["checked_pods", "replacement_pods", "pod_checks"] {
        if let Some(items) = validation_result.get(key).and_then(Value::as_array) {
            for item in items {
                let item_cluster = item
                    .get("cluster_id")
                    .or_else(|| item.get("sidecar_cluster_id"))
                    .or_else(|| item.get("target_cluster_id"))
                    .and_then(Value::as_str);
                let pod = item
                    .get("pod")
                    .or_else(|| item.get("pod_name"))
                    .or_else(|| item.get("name"))
                    .and_then(Value::as_str);
                let status = item
                    .get("status")
                    .or_else(|| item.get("phase"))
                    .or_else(|| item.get("health"))
                    .and_then(Value::as_str)
                    .map(|value| value.to_ascii_lowercase());
                let has_audit_ref = item
                    .get("audit_id")
                    .or_else(|| item.get("audit_log_id"))
                    .or_else(|| item.get("trace_id"))
                    .or_else(|| item.get("run_id"))
                    .or_else(|| item.get("checked_at"))
                    .or_else(|| item.get("validated_at"))
                    .or_else(|| item.get("timestamp"))
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
                if item_cluster == Some(cluster_id)
                    && pod.is_some_and(|pod| !pod.trim().is_empty())
                    && status.as_deref().is_some_and(|status| {
                        matches!(
                            status,
                            "running" | "ready" | "healthy" | "succeeded" | "validated"
                        )
                    })
                    && has_audit_ref
                {
                    checked_pods.insert(pod.unwrap().to_string());
                }
            }
        }
    }
    checked_pods.len() as u64
}
