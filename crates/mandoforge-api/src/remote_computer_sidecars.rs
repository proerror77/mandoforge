use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerReadiness, RemoteComputerSidecarHeartbeat,
    RemoteComputerSidecarRecoveryReadiness, RemoteComputerSidecarRecoveryRun,
    RemoteComputerSidecarRecoveryTarget, RemoteComputerSidecarSupervisionReadiness,
    env_bool_lookup, env_i64, new_audit_log, remote_computer_runner_for_config,
    required_controller_status,
};

pub(crate) async fn execute_remote_computer_sidecar_recovery(
    state: &AppState,
) -> Result<RemoteComputerSidecarRecoveryRun, AppError> {
    execute_remote_computer_sidecar_recovery_with_lookup(state, |key| std::env::var(key).ok()).await
}

async fn execute_remote_computer_sidecar_recovery_with_lookup<F>(
    state: &AppState,
    lookup: F,
) -> Result<RemoteComputerSidecarRecoveryRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let targets = remote_computer_sidecar_recovery_targets(state).await?;
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    let runner_readiness = runner.readiness(&config);
    let replacement_enabled = env_bool_lookup(
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED",
        &lookup,
    );
    let validation_controller_required =
        remote_computer_sidecar_validation_controller_required(&lookup);
    let validation_controller_configured =
        remote_computer_sidecar_validation_controller_configured(&lookup);
    let live_replacement_enabled = replacement_enabled
        && runner_readiness.configured
        && runner_readiness.live_mutation_enabled
        && config.mutation_enabled
        && config.live_mutation_enabled
        && (!validation_controller_required || validation_controller_configured);
    let mut runner_responses = Vec::new();
    let mut attempted_replacement_count = 0;
    let mut blocked_replacement_count = 0;
    for target in &targets {
        let Some(pod_name) = target.pod_name.clone() else {
            blocked_replacement_count += 1;
            continue;
        };
        if !live_replacement_enabled {
            blocked_replacement_count += 1;
            continue;
        }
        let delete = runner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_delete".to_string()),
                    remote_computer_id: Some(target.remote_computer_id),
                    session_id: None,
                    pod_name: Some(pod_name.clone()),
                    metadata: Some(json!({
                        "reason": target.reason,
                        "sidecar_recovery": true,
                        "replacement_step": "delete_unhealthy_pod"
                    })),
                },
            )
            .await;
        attempted_replacement_count += 1;
        runner_responses.push(delete);
        let create = runner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: Some(target.remote_computer_id),
                    session_id: None,
                    pod_name: Some(pod_name),
                    metadata: Some(json!({
                        "reason": target.reason,
                        "sidecar_recovery": true,
                        "artifact_discovery_enabled": true,
                        "replacement_step": "create_replacement_pod"
                    })),
                },
            )
            .await;
        runner_responses.push(create);
    }
    let failed_attempts = runner_responses
        .iter()
        .filter(|response| !matches!(response.status.as_str(), "mutation_ok"))
        .count();
    let replacement_blocked = !replacement_enabled
        || (validation_controller_required && !validation_controller_configured)
        || attempted_replacement_count == 0;
    let status = if targets.is_empty() {
        "noop"
    } else if replacement_blocked {
        "blocked"
    } else if failed_attempts > 0 {
        "attention"
    } else {
        "completed"
    }
    .to_string();
    let message = if targets.is_empty() {
        "No unhealthy Remote Computer sidecars require recovery".to_string()
    } else if !replacement_enabled {
        "Sidecar recovery planned replacements but live replacement is blocked until MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED is enabled".to_string()
    } else if validation_controller_required && !validation_controller_configured {
        "Sidecar recovery planned replacements but live replacement is blocked until MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL is configured".to_string()
    } else if !live_replacement_enabled {
        "Sidecar recovery is enabled, but the Kubernetes runner live mutation gates are not fully open".to_string()
    } else if failed_attempts > 0 {
        "One or more Remote Computer sidecar replacement mutations failed".to_string()
    } else {
        "Remote Computer sidecar replacement run completed for unhealthy Pods".to_string()
    };
    let mut run = RemoteComputerSidecarRecoveryRun {
        generated_at: Utc::now(),
        status,
        replacement_enabled,
        validation_controller_required,
        validation_controller_configured,
        runner_status: runner_readiness.status,
        unhealthy_count: targets.len(),
        planned_replacement_count: targets
            .iter()
            .filter(|target| target.pod_name.is_some())
            .count(),
        attempted_replacement_count,
        blocked_replacement_count,
        targets,
        runner_responses,
        validation_result: json!({
            "attempted": false,
            "status": "skipped",
            "reason": if validation_controller_configured {
                "no_replacement_attempted"
            } else {
                "validation_controller_not_configured"
            }
        }),
        execution_enabled: false,
        message,
    };
    if validation_controller_configured && run.attempted_replacement_count > 0 {
        match execute_remote_computer_sidecar_validation_controller(&lookup, &run).await {
            Ok(validation) => {
                let validation_status = required_controller_status(&validation)?.to_string();
                run.validation_result = validation;
                if validation_status != "validated" && run.status == "completed" {
                    run.status = "attention".to_string();
                    run.message = "Remote Computer sidecar replacement completed but validation controller did not confirm healthy replacement".to_string();
                }
            }
            Err(error) => {
                run.validation_result = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
                if run.status == "completed" {
                    run.status = "attention".to_string();
                    run.message = "Remote Computer sidecar replacement completed but validation controller failed".to_string();
                }
            }
        }
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "remote_computer.sidecar_recovery_run",
            "remote_computer_sidecar",
            None,
            json!(&run),
        ))
        .await?;
    Ok(run)
}

fn remote_computer_sidecar_validation_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn remote_computer_sidecar_validation_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(crate) async fn execute_remote_computer_sidecar_validation_controller<F>(
    lookup: &F,
    run: &RemoteComputerSidecarRecoveryRun,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.remote_computer_sidecar_replacement_validation",
        "generated_at": run.generated_at,
        "status": run.status,
        "validation_controller_required": run.validation_controller_required,
        "unhealthy_count": run.unhealthy_count,
        "planned_replacement_count": run.planned_replacement_count,
        "attempted_replacement_count": run.attempted_replacement_count,
        "blocked_replacement_count": run.blocked_replacement_count,
        "targets": run.targets,
        "runner_response_statuses": run.runner_responses.iter().map(|response| {
            json!({
                "status": response.status,
                "operation": response.operation,
                "pod_name": response.pod_name,
            })
        }).collect::<Vec<_>>(),
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let http_status = response.status();
    let body = response.json::<Value>().await?;
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "remote computer sidecar validation controller failed with status {http_status}"
        )));
    }
    let controller_status = required_controller_status(&body)?;
    let validated = matches!(
        controller_status,
        "validated" | "healthy" | "success" | "ok"
    );
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "message": body.get("message").and_then(Value::as_str),
        "target_kind": body.get("target_kind").and_then(Value::as_str),
        "cluster_id": body.get("cluster_id").and_then(Value::as_str),
        "cluster_profile": body.get("cluster_profile").and_then(Value::as_str),
        "node_count": body.get("node_count").and_then(Value::as_u64),
        "replacement_scope": body.get("replacement_scope").and_then(Value::as_str),
        "replacement_pods_healthy": body.get("replacement_pods_healthy").and_then(Value::as_bool),
        "checked_pod_count": body.get("checked_pod_count").and_then(Value::as_u64),
    }))
}

fn sidecar_stale_after_seconds() -> i64 {
    env_i64("MANDOFORGE_REMOTE_COMPUTER_SIDECAR_STALE_AFTER_SECONDS")
        .filter(|seconds| *seconds > 0)
        .unwrap_or(180)
}

pub(crate) async fn build_remote_computer_sidecar_supervision(
    state: &AppState,
    sidecar_manifest_present: bool,
) -> Result<RemoteComputerSidecarSupervisionReadiness, AppError> {
    let active_remote_computers: Vec<_> = state
        .list_remote_computers()
        .await?
        .into_iter()
        .filter(|computer| matches!(computer.status.as_str(), "available" | "leased" | "running"))
        .collect();
    let heartbeats = state.list_remote_computer_sidecar_heartbeats().await?;
    let stale_after_seconds = sidecar_stale_after_seconds();
    let now = Utc::now();
    let mut latest_by_remote: HashMap<Uuid, RemoteComputerSidecarHeartbeat> = HashMap::new();
    for heartbeat in heartbeats.iter().filter(|heartbeat| {
        heartbeat.sidecar_name == "artifact-discovery" && heartbeat.status != "disabled"
    }) {
        latest_by_remote
            .entry(heartbeat.remote_computer_id)
            .and_modify(|existing| {
                if heartbeat.observed_at > existing.observed_at {
                    *existing = heartbeat.clone();
                }
            })
            .or_insert_with(|| heartbeat.clone());
    }
    let missing_heartbeat_count = active_remote_computers
        .iter()
        .filter(|computer| !latest_by_remote.contains_key(&computer.id))
        .count();
    let stale_heartbeat_count = active_remote_computers
        .iter()
        .filter_map(|computer| latest_by_remote.get(&computer.id))
        .filter(|heartbeat| {
            now.signed_duration_since(heartbeat.observed_at)
                .num_seconds()
                > stale_after_seconds
        })
        .count();
    let latest_observed_at = latest_by_remote
        .values()
        .map(|heartbeat| heartbeat.observed_at)
        .max();
    let status = if !sidecar_manifest_present {
        "manifest_missing"
    } else if missing_heartbeat_count > 0 || stale_heartbeat_count > 0 {
        "attention"
    } else if !latest_by_remote.is_empty() {
        "observed"
    } else {
        "not_observed"
    }
    .to_string();
    Ok(RemoteComputerSidecarSupervisionReadiness {
        status,
        heartbeat_count: heartbeats.len(),
        active_remote_computer_count: active_remote_computers.len(),
        missing_heartbeat_count,
        stale_heartbeat_count,
        stale_after_seconds,
        latest_observed_at,
    })
}

pub(crate) async fn remote_computer_sidecar_recovery_targets(
    state: &AppState,
) -> Result<Vec<RemoteComputerSidecarRecoveryTarget>, AppError> {
    let active_remote_computers: Vec<_> = state
        .list_remote_computers()
        .await?
        .into_iter()
        .filter(|computer| matches!(computer.status.as_str(), "available" | "leased" | "running"))
        .collect();
    let heartbeats = state.list_remote_computer_sidecar_heartbeats().await?;
    let stale_after_seconds = sidecar_stale_after_seconds();
    let now = Utc::now();
    let mut latest_by_remote: HashMap<Uuid, RemoteComputerSidecarHeartbeat> = HashMap::new();
    for heartbeat in heartbeats.iter().filter(|heartbeat| {
        heartbeat.sidecar_name == "artifact-discovery" && heartbeat.status != "disabled"
    }) {
        latest_by_remote
            .entry(heartbeat.remote_computer_id)
            .and_modify(|existing| {
                if heartbeat.observed_at > existing.observed_at {
                    *existing = heartbeat.clone();
                }
            })
            .or_insert_with(|| heartbeat.clone());
    }
    let mut targets = Vec::new();
    for computer in active_remote_computers {
        match latest_by_remote.get(&computer.id) {
            None => targets.push(RemoteComputerSidecarRecoveryTarget {
                remote_computer_id: computer.id,
                name: computer.name,
                pod_name: computer.pod_name,
                reason: "missing_artifact_discovery_heartbeat".to_string(),
                latest_observed_at: None,
            }),
            Some(heartbeat)
                if now
                    .signed_duration_since(heartbeat.observed_at)
                    .num_seconds()
                    > stale_after_seconds =>
            {
                targets.push(RemoteComputerSidecarRecoveryTarget {
                    remote_computer_id: computer.id,
                    name: computer.name,
                    pod_name: computer.pod_name,
                    reason: "stale_artifact_discovery_heartbeat".to_string(),
                    latest_observed_at: Some(heartbeat.observed_at),
                });
            }
            Some(_) => {}
        }
    }
    Ok(targets)
}

pub(crate) fn build_remote_computer_sidecar_recovery_readiness(
    targets: &[RemoteComputerSidecarRecoveryTarget],
    runner: &RemoteComputerRunnerReadiness,
) -> RemoteComputerSidecarRecoveryReadiness {
    build_remote_computer_sidecar_recovery_readiness_with_lookup(targets, runner, &|key| {
        std::env::var(key).ok()
    })
}

pub(crate) fn build_remote_computer_sidecar_recovery_readiness_with_lookup<F>(
    targets: &[RemoteComputerSidecarRecoveryTarget],
    runner: &RemoteComputerRunnerReadiness,
    lookup: &F,
) -> RemoteComputerSidecarRecoveryReadiness
where
    F: Fn(&str) -> Option<String>,
{
    let replacement_enabled = env_bool_lookup(
        "MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED",
        lookup,
    );
    let validation_controller_required =
        remote_computer_sidecar_validation_controller_required(lookup);
    let validation_controller_configured =
        remote_computer_sidecar_validation_controller_configured(lookup);
    let replaceable_pod_count = targets
        .iter()
        .filter(|target| target.pod_name.is_some())
        .count();
    let blocked_reason = if targets.is_empty() {
        None
    } else if !replacement_enabled {
        Some("replacement_gate_disabled".to_string())
    } else if !runner.configured {
        Some("runner_not_configured".to_string())
    } else if !runner.live_mutation_enabled {
        Some("runner_live_mutation_disabled".to_string())
    } else if replaceable_pod_count < targets.len() {
        Some("unhealthy_remote_computer_missing_pod_name".to_string())
    } else if validation_controller_required && !validation_controller_configured {
        Some("validation_controller_required".to_string())
    } else {
        None
    };
    let status = if targets.is_empty() {
        "ready"
    } else if blocked_reason.is_some() {
        "blocked"
    } else {
        "ready_to_replace"
    };
    let message = if targets.is_empty() {
        "No unhealthy Remote Computer sidecars require Pod replacement".to_string()
    } else if let Some(reason) = &blocked_reason {
        format!("Remote Computer sidecar replacement is blocked by {reason}")
    } else {
        "Remote Computer sidecar replacement gate is open for unhealthy Pods".to_string()
    };
    RemoteComputerSidecarRecoveryReadiness {
        status: status.to_string(),
        replacement_enabled,
        validation_controller_required,
        validation_controller_configured,
        runner_configured: runner.configured,
        runner_live_mutation_enabled: runner.live_mutation_enabled,
        unhealthy_count: targets.len(),
        replaceable_pod_count,
        blocked_reason,
        message,
    }
}
