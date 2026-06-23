use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    AppError, AuditLog, RemoteComputerProductionStateSyncReadiness,
    RemoteComputerStateFilesystemReadiness,
};

pub(crate) fn remote_computer_state_sync_base_issues(
    state_filesystem: &RemoteComputerStateFilesystemReadiness,
) -> Vec<String> {
    let mut blocking_reasons = Vec::new();
    if !state_filesystem.distributed_filesystem_configured {
        blocking_reasons.push(
            "no real distributed filesystem provider is configured for multi-Pod state sync"
                .to_string(),
        );
    }
    if !state_filesystem.production_profile_present {
        blocking_reasons.push(
            "production state filesystem profile manifest is missing or not packaged".to_string(),
        );
    }
    if !state_filesystem.state_contract_present {
        blocking_reasons.push("Memory/Notes/Skills state contract is missing".to_string());
    }
    if !state_filesystem.lock_manager_configured {
        blocking_reasons
            .push("lock-aware state sync manager is not configured for shared writes".to_string());
    }
    if state_filesystem.conflict_policy != "one-active-writer-per-session" {
        blocking_reasons.push(
            "state conflict policy is not the expected one-active-writer contract".to_string(),
        );
    }
    blocking_reasons
}

pub(crate) fn remote_computer_state_sync_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn remote_computer_state_sync_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_remote_computer_state_sync_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    state_filesystem: &RemoteComputerStateFilesystemReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.remote_computer_state_sync_validation",
        "subject": subject,
        "checked_at": checked_at,
        "provider": state_filesystem.provider,
        "distributed_filesystem_configured": state_filesystem.distributed_filesystem_configured,
        "production_profile_present": state_filesystem.production_profile_present,
        "state_contract_present": state_filesystem.state_contract_present,
        "lock_manager_configured": state_filesystem.lock_manager_configured,
        "conflict_policy": state_filesystem.conflict_policy,
        "mount_path": state_filesystem.mount_path,
        "state_layout_paths": state_filesystem.state_layout_paths,
        "production_claim_name": state_filesystem.production_claim_name,
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
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "remote computer state sync controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(provider_status, "validated" | "healthy" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "state_sync_id": body.get("state_sync_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "target_kind": body.get("target_kind").and_then(Value::as_str),
        "cluster_id": body.get("cluster_id").and_then(Value::as_str),
        "cluster_profile": body.get("cluster_profile").and_then(Value::as_str),
        "node_count": body.get("node_count").and_then(Value::as_u64),
        "distributed_state_backend": body
            .get("distributed_state_backend")
            .or_else(|| body.get("storage_backend"))
            .or_else(|| body.get("state_backend"))
            .and_then(Value::as_str),
        "state_claim": body.get("state_claim").and_then(Value::as_str),
        "checked_path_count": body.get("checked_path_count").and_then(Value::as_u64),
        "checked_paths": body
            .get("checked_paths")
            .or_else(|| body.get("checked_state_paths"))
            .or_else(|| body.get("path_checks"))
            .cloned()
            .unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn build_remote_computer_production_state_sync_readiness(
    state_filesystem: &RemoteComputerStateFilesystemReadiness,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> RemoteComputerProductionStateSyncReadiness {
    let mut blocking_reasons = remote_computer_state_sync_base_issues(state_filesystem);
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "remote_computer.production_state_sync_validation")
        .max_by_key(|log| log.created_at);
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_status = latest_validation
        .and_then(|log| log.details.get("controller_execution"))
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_age_hours = latest_validation
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    if controller_required && !controller_configured {
        blocking_reasons.push("state sync controller is required but not configured".to_string());
    }
    if controller_required && latest_validation.is_none() {
        blocking_reasons.push("state sync validation has not run".to_string());
    }
    if latest_validation_at.is_some_and(|created_at| (generated_at - created_at).num_hours() >= 24)
    {
        blocking_reasons.push("state sync validation evidence is stale".to_string());
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons
            .push("state sync controller evidence is missing or not validated".to_string());
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("state sync controller evidence is stale".to_string());
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
            "Remote Computer production state sync is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Remote Computer production state sync has a distributed filesystem provider, production profile, state contract, and lock-aware write coordination".to_string()
    };

    RemoteComputerProductionStateSyncReadiness {
        status,
        production_blocked,
        distributed_filesystem_configured: state_filesystem.distributed_filesystem_configured,
        production_profile_present: state_filesystem.production_profile_present,
        state_contract_present: state_filesystem.state_contract_present,
        lock_manager_configured: state_filesystem.lock_manager_configured,
        controller_required,
        controller_configured,
        latest_validation_at,
        latest_validation_status,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        conflict_policy: state_filesystem.conflict_policy.clone(),
        provider: state_filesystem.provider.clone(),
        blocking_reasons,
        message,
    }
}
