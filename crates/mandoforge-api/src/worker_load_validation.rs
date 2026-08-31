use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    AppError, AppState, AuditLog, WorkerAutoscalingReadiness, WorkerK8sReadiness,
    WorkerLoadValidationEvidence, WorkerLoadValidationRun, WorkerQueueBackendReadiness,
    controller_response_json, env_bool, new_audit_log, required_controller_status,
    worker_autoscaling_readiness_from_manifests, worker_isolated_pool_configured_from_manifests,
    worker_k8s_readiness_from_manifests, worker_queue_backend_readiness,
};

pub(crate) async fn execute_worker_load_validation(
    state: &AppState,
) -> Result<WorkerLoadValidationRun, AppError> {
    execute_worker_load_validation_with_lookup(state, |key| std::env::var(key).ok()).await
}

async fn execute_worker_load_validation_with_lookup<F>(
    state: &AppState,
    lookup: F,
) -> Result<WorkerLoadValidationRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let checked_at = Utc::now();
    let queue_backend = worker_queue_backend_readiness(state.execution_queue.backend_kind());
    let worker_mode = state.execution_worker.mode().to_string();
    let k8s = worker_k8s_readiness_from_manifests();
    let autoscaling = worker_autoscaling_readiness_from_manifests(&[
        "deploy/k8s/worker-hpa.yaml",
        "deploy/k8s/worker-keda.yaml",
        "deploy/k8s/keda.yaml",
        "deploy/k8s/worker-isolated-pool-keda.yaml",
    ]);
    let controller_configured = worker_load_validation_controller_configured(&lookup);
    let controller_required = worker_load_validation_controller_required(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "controller_not_attempted"
        } else {
            "load_validation_controller_not_configured"
        }
    });
    let mut load_validated = lookup_bool(&lookup, "MANDOFORGE_WORKER_LOAD_VALIDATED");
    let mut isolated_worker_pool_configured =
        lookup_bool(&lookup, "MANDOFORGE_WORKER_ISOLATED_POOL")
            || worker_isolated_pool_configured_from_manifests();
    if controller_configured {
        match execute_worker_load_validation_controller(
            &lookup,
            checked_at,
            &queue_backend,
            worker_mode.as_str(),
            &k8s,
            &autoscaling,
        )
        .await
        {
            Ok(execution) => {
                load_validated = execution
                    .get("load_validated")
                    .and_then(Value::as_bool)
                    .unwrap_or_else(|| {
                        execution.get("status").and_then(Value::as_str) == Some("validated")
                    });
                isolated_worker_pool_configured = execution
                    .get("isolated_worker_pool_configured")
                    .and_then(Value::as_bool)
                    .unwrap_or(isolated_worker_pool_configured);
                controller_execution = execution;
            }
            Err(error) => {
                load_validated = false;
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    } else if controller_required {
        load_validated = false;
    }
    let mut actions = Vec::new();
    if !queue_backend.durable {
        actions.push("configure_durable_worker_queue".to_string());
    }
    if k8s.hardening_status != "hardened" {
        actions.push("harden_worker_pod_manifest".to_string());
    }
    if autoscaling.validation_status != "queue_depth_configured" {
        actions.push("configure_queue_depth_autoscaling".to_string());
    }
    if !isolated_worker_pool_configured {
        actions.push("configure_isolated_worker_pool".to_string());
    }
    if !load_validated {
        actions.push("run_queue_pressure_load_validation".to_string());
    }
    if controller_required && !controller_configured {
        actions.push("configure_worker_load_validation_controller".to_string());
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        actions.push("obtain_validated_worker_load_controller_evidence".to_string());
    }
    let status = if actions.is_empty() {
        "validated"
    } else {
        "attention"
    }
    .to_string();
    let run = WorkerLoadValidationRun {
        status,
        checked_at,
        queue_backend: queue_backend.kind,
        worker_mode,
        autoscaling_status: autoscaling.validation_status.clone(),
        autoscaling,
        load_validated,
        isolated_worker_pool_configured,
        controller_configured,
        controller_execution,
        actions,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "worker.load_validation_run",
            "execution_worker",
            None,
            json!({
                "status": run.status,
                "queue_backend": run.queue_backend,
                "worker_mode": run.worker_mode,
                "autoscaling_status": run.autoscaling_status,
                "autoscaling": run.autoscaling.clone(),
                "load_validated": run.load_validated,
                "isolated_worker_pool_configured": run.isolated_worker_pool_configured,
                "controller_configured": run.controller_configured,
                "controller_execution": run.controller_execution,
                "actions": run.actions,
                "checked_at": run.checked_at,
            }),
        ))
        .await?;
    Ok(run)
}

fn lookup_bool<F>(lookup: &F, name: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup(name)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn worker_load_validation_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn worker_load_validation_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) async fn execute_worker_load_validation_controller<F>(
    lookup: &F,
    checked_at: DateTime<Utc>,
    queue_backend: &WorkerQueueBackendReadiness,
    worker_mode: &str,
    k8s: &WorkerK8sReadiness,
    autoscaling: &WorkerAutoscalingReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_WORKER_LOAD_VALIDATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.worker_load_validation",
        "checked_at": checked_at,
        "queue_backend": {
            "kind": queue_backend.kind,
            "durable": queue_backend.durable,
            "broker_handoff": queue_backend.broker_handoff,
            "jetstream_enabled": queue_backend.jetstream_enabled,
        },
        "worker_mode": worker_mode,
        "k8s": {
            "hardening_status": k8s.hardening_status,
            "service_account_name": k8s.service_account_name,
            "network_policy_present": k8s.network_policy_present,
            "resources_requests_configured": k8s.resources_requests_configured,
            "resources_limits_configured": k8s.resources_limits_configured,
        },
        "autoscaling": {
            "validation_status": autoscaling.validation_status,
            "configured_min_replicas": autoscaling.configured_min_replicas,
            "configured_max_replicas": autoscaling.configured_max_replicas,
            "queue_depth_scaling_present": autoscaling.queue_depth_scaling_present,
            "scale_target_refs": autoscaling.scale_target_refs,
            "trigger_types": autoscaling.trigger_types,
        },
        "isolated_worker_pool_manifest_configured": worker_isolated_pool_configured_from_manifests(),
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (http_status, body) =
        controller_response_json(response, "worker load validation controller").await?;
    let controller_status = required_controller_status(&body)?;
    let validated = matches!(controller_status, "validated" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "validation_id": body.get("validation_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "target_kind": body.get("target_kind").and_then(Value::as_str),
        "cluster_id": body.get("cluster_id").and_then(Value::as_str),
        "cluster_profile": body.get("cluster_profile").and_then(Value::as_str),
        "node_count": body.get("node_count").and_then(Value::as_u64),
        "worker_pool": body.get("worker_pool").and_then(Value::as_str),
        "load_validated": body.get("load_validated").and_then(Value::as_bool).unwrap_or(validated),
        "isolated_worker_pool_configured": body.get("isolated_worker_pool_configured").and_then(Value::as_bool).unwrap_or(false),
        "observed_replicas": body.get("observed_replicas").cloned().unwrap_or_else(|| json!({})),
        "checks": body.get("checks").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn worker_load_validation_evidence(
    state: &AppState,
) -> Result<WorkerLoadValidationEvidence, AppError> {
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(worker_load_validation_evidence_from_audit_logs(
        &audit_logs,
        Utc::now(),
        worker_load_validation_controller_required(&|key| std::env::var(key).ok()),
        worker_load_validation_controller_configured(&|key| std::env::var(key).ok()),
        worker_isolated_pool_configured_from_manifests(),
    ))
}

pub(crate) fn worker_load_validation_evidence_from_audit_logs(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
    manifest_isolated_worker_pool_configured: bool,
) -> WorkerLoadValidationEvidence {
    let latest_run = audit_logs
        .iter()
        .filter(|log| log.action == "worker.load_validation_run")
        .max_by_key(|log| log.created_at)
        .cloned();
    let latest_run_status = latest_run
        .as_ref()
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_status = latest_run
        .as_ref()
        .and_then(|log| log.details["controller_execution"]["status"].as_str())
        .map(str::to_string);
    let latest_controller_age_hours = latest_run
        .as_ref()
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let load_validated = latest_run_status.as_deref() == Some("validated")
        && (!controller_required || (latest_controller_validated && controller_evidence_fresh));
    let isolated_worker_pool_configured = latest_run
        .as_ref()
        .and_then(|log| log.details.get("isolated_worker_pool_configured"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            env_bool("MANDOFORGE_WORKER_ISOLATED_POOL") || manifest_isolated_worker_pool_configured
        });
    let status = if load_validated {
        "validated"
    } else if latest_run.is_some() {
        "attention"
    } else {
        "not_run"
    }
    .to_string();
    let message = if load_validated {
        "worker load validation has a latest validated audit run with required controller evidence"
            .to_string()
    } else if controller_required && !controller_configured {
        "worker load validation controller is required but not configured".to_string()
    } else if controller_required && !latest_controller_validated {
        "worker load validation controller has no recent validated evidence".to_string()
    } else if controller_required && latest_controller_validated && !controller_evidence_fresh {
        "worker load validation controller evidence is stale".to_string()
    } else if latest_run.is_some() {
        "latest worker load validation run did not prove production load and worker-pool isolation"
            .to_string()
    } else {
        "worker load validation has not been run; manifests alone do not prove autoscaling under production-like queue pressure".to_string()
    };
    WorkerLoadValidationEvidence {
        status,
        latest_run_at: latest_run.as_ref().map(|log| log.created_at),
        latest_run_status,
        load_validated,
        isolated_worker_pool_configured,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        required_profile:
            "durable queue + hardened worker Pod + queue-depth autoscaling + isolated worker pool + production-like load test"
                .to_string(),
        message,
    }
}
