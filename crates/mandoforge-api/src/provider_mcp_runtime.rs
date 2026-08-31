use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn decide_provider_status_approval(
    state: AppState,
    id: Uuid,
    headers: HeaderMap,
    decision: &str,
    comment: Option<String>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "provider".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let provider = provider_by_id(&state, id).await?;
    let mut approval = provider
        .config
        .get("pending_status_approval")
        .cloned()
        .ok_or_else(|| AppError::bad_request("provider has no pending status approval"))?;
    if approval.get("status").and_then(Value::as_str) != Some("pending") {
        return Err(AppError::bad_request(
            "provider status approval is not pending",
        ));
    }
    let requested_by = approval
        .get("requested_by")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if requested_by == principal.subject_id {
        return Err(AppError::forbidden(
            "provider status approval requires a different approver",
        ));
    }
    if let Some(approver_subject) = approval
        .get("approver_subject")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && approver_subject != principal.subject_id
    {
        return Err(AppError::forbidden(format!(
            "provider status approval is delegated to {approver_subject}"
        )));
    }
    let decided_at = Utc::now();
    approval["status"] = json!(decision);
    approval["decided_by"] = json!(principal.subject_id.clone());
    approval["decided_at"] = json!(decided_at);
    approval["comment"] = json!(optional_trimmed(comment.as_deref()));
    let mut config = provider.config.as_object().cloned().unwrap_or_default();
    config.remove("pending_status_approval");
    config.insert("last_status_approval".to_string(), approval.clone());
    let updated = state
        .update_provider_config(id, Value::Object(config))
        .await?;
    let updated = if decision == "approved" {
        let requested_status = approval
            .get("requested_status")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::bad_request("pending approval missing requested_status"))?;
        state.update_provider_status(id, requested_status).await?
    } else {
        updated
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            &format!("provider.status_approval_{decision}"),
            "provider",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "provider_name": provider.name,
                "decision": decision,
                "approval": approval
            }),
        ))
        .await?;
    Ok(Json(ProviderStatusApprovalResponse {
        provider: updated,
        approval,
    }))
}

pub(crate) async fn provider_by_id(state: &AppState, id: Uuid) -> Result<ProviderRecord, AppError> {
    state
        .list_providers()
        .await?
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::not_found("provider not found"))
}

pub(crate) fn normalize_provider_status(status: &str) -> Result<String, AppError> {
    match status.trim() {
        "active" => Ok("active".to_string()),
        "disabled" => Ok("disabled".to_string()),
        "archived" => Ok("archived".to_string()),
        other => Err(AppError::bad_request(format!(
            "unsupported provider status: {other}"
        ))),
    }
}

pub(crate) fn normalize_provider_api_key_ref(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    let Some(reference) = trimmed.strip_prefix("vault:") else {
        return Err(AppError::bad_request(
            "provider api key ref must use vault:path#key",
        ));
    };
    let Some((path, key)) = reference.split_once('#') else {
        return Err(AppError::bad_request(
            "provider api key ref must use vault:path#key",
        ));
    };
    let secret_ref = SecretRef::new(path, key)?;
    Ok(format!("vault:{}#{}", secret_ref.path, secret_ref.key))
}

pub(crate) fn normalize_mcp_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("MCP server name is required"));
    }
    Ok(name.to_string())
}

pub(crate) fn normalize_mcp_transport(transport: &str) -> Result<String, AppError> {
    let transport = transport.trim();
    if transport.is_empty() {
        return Err(AppError::bad_request("MCP server transport is required"));
    }
    Ok(transport.to_string())
}

pub(crate) fn normalize_mcp_status(status: &str) -> Result<String, AppError> {
    match status.trim() {
        "active" => Ok("active".to_string()),
        "disabled" => Ok("disabled".to_string()),
        "archived" => Ok("archived".to_string()),
        other => Err(AppError::bad_request(format!(
            "unsupported MCP server status: {other}"
        ))),
    }
}

pub(crate) fn normalize_mcp_tool_allowlist(
    tool_allowlist: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let mut tools: Vec<_> = tool_allowlist
        .into_iter()
        .map(|tool| tool.trim().to_string())
        .filter(|tool| !tool.is_empty())
        .collect();
    tools.sort();
    tools.dedup();
    if tools.len() > 100 {
        return Err(AppError::bad_request(
            "MCP server tool allowlist cannot exceed 100 tools",
        ));
    }
    Ok(tools)
}

pub(crate) fn normalize_mcp_config(config: Value) -> Result<Value, AppError> {
    let mut map = config
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::bad_request("MCP server config must be a JSON object"))?;
    let mut secret_refs = Vec::new();
    if let Some(secret_ref) = map.get("secret_ref").and_then(Value::as_str) {
        secret_refs.push(normalize_mcp_secret_ref(secret_ref)?);
    }
    if let Some(value) = map.get("secret_refs") {
        let refs = value.as_array().ok_or_else(|| {
            AppError::bad_request("MCP server config secret_refs must be an array")
        })?;
        for (index, item) in refs.iter().enumerate() {
            let Some(secret_ref) = item.as_str() else {
                return Err(AppError::bad_request(format!(
                    "MCP server config secret_refs[{index}] must be a string"
                )));
            };
            secret_refs.push(normalize_mcp_secret_ref(secret_ref)?);
        }
    }
    secret_refs.sort();
    secret_refs.dedup();
    if !secret_refs.is_empty() {
        map.insert("secret_refs".to_string(), json!(secret_refs));
        map.remove("secret_ref");
    }
    Ok(Value::Object(map))
}

pub(crate) fn normalize_mcp_secret_ref(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    let Some(reference) = trimmed.strip_prefix("vault:") else {
        return Err(AppError::bad_request(
            "MCP server secret refs must use vault:path#key",
        ));
    };
    let Some((path, key)) = reference.split_once('#') else {
        return Err(AppError::bad_request(
            "MCP server secret refs must use vault:path#key",
        ));
    };
    let secret_ref = SecretRef::new(path, key)?;
    Ok(format!("vault:{}#{}", secret_ref.path, secret_ref.key))
}

pub(crate) fn mcp_config_secret_refs(config: &Value) -> Vec<String> {
    config
        .get("secret_refs")
        .and_then(Value::as_array)
        .map(|refs| {
            refs.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn mcp_secret_ref_from_stored_value(value: &str) -> Result<SecretRef, AppError> {
    let Some(reference) = value.trim().strip_prefix("vault:") else {
        return Err(AppError::bad_request(
            "MCP server secret refs must use vault:path#key",
        ));
    };
    let Some((path, key)) = reference.split_once('#') else {
        return Err(AppError::bad_request(
            "MCP server secret refs must use vault:path#key",
        ));
    };
    SecretRef::new(path, key)
}

pub(crate) async fn resolve_mcp_runtime_secret_refs(
    server: &McpServerRecord,
) -> Result<usize, AppError> {
    let refs = mcp_config_secret_refs(&server.config);
    if refs.is_empty() {
        return Ok(0);
    }
    let secret_provider = secret_provider_from_env().map_err(|error| {
        AppError::forbidden(format!(
            "MCP server {} secret ref could not be resolved: {}",
            server.name, error.message
        ))
    })?;
    let secret_config = SecretProviderConfig::from_env().map_err(|error| {
        AppError::forbidden(format!(
            "MCP server {} secret ref could not be resolved: {}",
            server.name, error.message
        ))
    })?;
    for value in &refs {
        let secret_ref = mcp_secret_ref_from_stored_value(value)?;
        secret_provider
            .read_secret(&secret_config, &secret_ref)
            .await
            .map_err(|error| {
                AppError::forbidden(format!(
                    "MCP server {} secret ref could not be resolved: {}",
                    server.name, error.message
                ))
            })?;
    }
    Ok(refs.len())
}

pub(crate) async fn provider_health(provider: &ProviderRecord) -> ProviderHealth {
    let secret_provider = secret_provider_from_env();
    let (secret_provider, secret_provider_error) = match secret_provider {
        Ok(secret_provider) => (Some(secret_provider), None),
        Err(error) => (None, Some(error.message)),
    };
    provider_health_from_lookup(
        provider,
        &|key| std::env::var(key).ok(),
        secret_provider.as_deref(),
        secret_provider_error,
    )
    .await
}

pub(crate) async fn provider_health_from_lookup<F>(
    provider: &ProviderRecord,
    lookup: &F,
    secret_provider: Option<&dyn SecretProvider>,
    secret_provider_error: Option<String>,
) -> ProviderHealth
where
    F: Fn(&str) -> Option<String>,
{
    let mut issues = Vec::new();
    let provider_type = provider.provider_type.trim().to_ascii_lowercase();
    if provider.status != "active" {
        issues.push(format!("provider status is {}", provider.status));
    }
    let has_base_url = provider
        .base_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_default_model = provider
        .default_model
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let api_key_env = provider
        .config
        .get("api_key_env")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let api_key_ref = provider
        .config
        .get("api_key_ref")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let api_key_env_present = api_key_env.is_some_and(|env_key| lookup(env_key).is_some());
    let mut api_key_ref_resolved = false;
    let mut external_probe = "not_applicable".to_string();
    let mut external_probe_status = Value::Null;

    match provider_type.as_str() {
        "mock" | "mock_openai_compatible" => {}
        "openai_compatible" | "openai-compatible" => {
            if !has_base_url {
                issues.push("openai-compatible provider requires base_url".to_string());
            }
            if api_key_env.is_none() && api_key_ref.is_none() {
                issues.push(
                    "openai-compatible provider requires config.api_key_env or config.api_key_ref"
                        .to_string(),
                );
            }
            if api_key_env.is_some() && !api_key_env_present {
                issues.push("configured api_key_env is not present in the environment".to_string());
            }
            let mut api_key_for_probe = None;
            if let Some(env_key) = api_key_env.filter(|_| api_key_env_present) {
                api_key_for_probe = lookup(env_key);
            } else if let Some(api_key_ref) = api_key_ref {
                match secret_provider {
                    Some(secret_provider) => {
                        match provider::provider_api_key_from_stored_value_with_lookup(
                            api_key_ref,
                            lookup,
                            secret_provider,
                        )
                        .await
                        {
                            Ok(api_key) => {
                                api_key_ref_resolved = true;
                                api_key_for_probe = Some(api_key);
                            }
                            Err(error) => {
                                external_probe = "failed_api_key_ref".to_string();
                                issues.push(format!(
                                    "configured api_key_ref could not be read: {}",
                                    error.message
                                ));
                            }
                        }
                    }
                    None => {
                        external_probe = "failed_api_key_ref".to_string();
                        issues.push(format!(
                            "configured api_key_ref could not be read: {}",
                            secret_provider_error
                                .as_deref()
                                .unwrap_or("secret provider is not configured")
                        ));
                    }
                }
            }
            if provider.status == "active" && has_base_url {
                if let Some(api_key) = api_key_for_probe {
                    let probe = probe_openai_compatible_provider(
                        provider.base_url.as_deref().unwrap_or_default(),
                        &api_key,
                    )
                    .await;
                    external_probe = probe.0;
                    external_probe_status = probe.1;
                    if let Some(issue) = probe.2 {
                        issues.push(issue);
                    }
                } else if external_probe == "not_applicable" {
                    external_probe = "skipped_configuration".to_string();
                }
            } else if provider.status != "active" {
                external_probe = "skipped_inactive".to_string();
            } else if !has_base_url || (!api_key_env_present && api_key_ref.is_none()) {
                external_probe = "skipped_configuration".to_string();
            }
        }
        other => issues.push(format!("provider type {other} is not supported")),
    }

    ProviderHealth {
        provider_id: provider.id,
        name: provider.name.clone(),
        status: provider.status.clone(),
        healthy: issues.is_empty(),
        issues,
        checks: json!({
            "provider_type": provider.provider_type,
            "has_base_url": has_base_url,
            "has_default_model": has_default_model,
            "has_api_key_env": api_key_env.is_some(),
            "api_key_env_present": api_key_env_present,
            "has_api_key_ref": api_key_ref.is_some(),
            "api_key_ref_resolved": api_key_ref_resolved,
            "external_probe": external_probe,
            "external_probe_status": external_probe_status,
        }),
        checked_at: Utc::now(),
    }
}

pub(crate) async fn probe_openai_compatible_provider(
    base_url: &str,
    api_key: &str,
) -> (String, Value, Option<String>) {
    let endpoint = format!("{}/v1/models", base_url.trim().trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return (
                "failed".to_string(),
                json!({"error": error.to_string()}),
                Some("provider health probe client could not be created".to_string()),
            );
        }
    };
    match client.get(&endpoint).bearer_auth(api_key).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                (
                    "healthy".to_string(),
                    json!({"url": endpoint, "status": status.as_u16()}),
                    None,
                )
            } else {
                (
                    "failed".to_string(),
                    json!({"url": endpoint, "status": status.as_u16()}),
                    Some(format!(
                        "provider external health probe failed with status {status}"
                    )),
                )
            }
        }
        Err(error) => (
            "failed".to_string(),
            json!({"url": endpoint, "error": error.to_string()}),
            Some("provider external health probe failed".to_string()),
        ),
    }
}

pub(crate) async fn execute_due_mcp_server_health_checks(
    state: &AppState,
    team_id: Uuid,
) -> Result<McpServerScheduledHealthRun, AppError> {
    let checked_at = Utc::now();
    let servers = state.list_mcp_servers(team_id).await?;
    let mut skipped_count = 0usize;
    let mut results = Vec::new();
    for server in servers {
        if !mcp_server_health_check_is_due(&server, checked_at) {
            skipped_count += 1;
            continue;
        }
        let health = mcp_server_health(state, &server).await;
        let config = mcp_server_config_with_health_result(&server.config, &health, checked_at);
        state
            .update_mcp_server(
                team_id,
                server.id,
                UpdateMcpServerRecord {
                    transport: None,
                    config: Some(config),
                    tool_allowlist: None,
                },
            )
            .await?;
        results.push(health);
    }
    let healthy_count = results.iter().filter(|health| health.healthy).count();
    let run = McpServerScheduledHealthRun {
        team_id,
        due_count: results.len(),
        skipped_count,
        healthy_count,
        unhealthy_count: results.len().saturating_sub(healthy_count),
        results,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_scheduled_health_run",
            "team",
            Some(team_id),
            json!({
                "team_id": team_id,
                "due_count": run.due_count,
                "skipped_count": run.skipped_count,
                "healthy_count": run.healthy_count,
                "unhealthy_count": run.unhealthy_count,
            }),
        ))
        .await?;
    Ok(run)
}

pub(crate) async fn execute_due_mcp_server_rollouts(
    state: &AppState,
    team_id: Uuid,
) -> Result<McpServerRolloutDueRun, AppError> {
    execute_due_mcp_server_rollouts_with_lookup(state, team_id, |key| std::env::var(key).ok()).await
}

pub(crate) async fn execute_due_mcp_server_rollouts_with_lookup<F>(
    state: &AppState,
    team_id: Uuid,
    lookup: F,
) -> Result<McpServerRolloutDueRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let checked_at = Utc::now();
    let servers = state.list_mcp_servers(team_id).await?;
    let mut applied_count = 0usize;
    let mut skipped_count = 0usize;
    let mut expired_count = 0usize;
    let mut failed_count = 0usize;
    let controller_required = mcp_server_rollout_controller_required(&lookup);
    let controller_configured = mcp_server_rollout_controller_configured(&lookup);
    let mut controller_execution_count = 0usize;
    let mut controller_failed_count = 0usize;
    let mut results = Vec::new();
    for server in servers {
        let Some(rollout) = mcp_pending_rollout(&server).cloned() else {
            skipped_count += 1;
            continue;
        };
        let rollout_id = rollout
            .get("id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let Some(rollout_id) = rollout_id else {
            failed_count += 1;
            results.push(json!({
                "server_id": server.id,
                "status": "failed",
                "error": "pending rollout has invalid id",
            }));
            continue;
        };
        if mcp_rollout_is_expired(&rollout, checked_at) {
            expired_count += 1;
            let expired =
                mark_mcp_server_rollout_expired(state, team_id, &server, &rollout).await?;
            results.push(json!({
                "server_id": server.id,
                "rollout_id": rollout_id,
                "status": "expired",
                "rollout": expired,
            }));
            continue;
        }
        if !mcp_rollout_is_due(&rollout, checked_at) {
            skipped_count += 1;
            results.push(json!({
                "server_id": server.id,
                "rollout_id": rollout_id,
                "status": "skipped",
            }));
            continue;
        }
        let mut controller_execution = json!({
            "attempted": false,
            "status": "skipped",
            "reason": "controller_not_configured"
        });
        if controller_required && !controller_configured {
            skipped_count += 1;
            results.push(json!({
                "server_id": server.id,
                "rollout_id": rollout_id,
                "status": "skipped",
                "reason": "controller_required_not_configured",
                "controller_execution": controller_execution,
            }));
            continue;
        }
        if controller_configured {
            controller_execution_count += 1;
            match execute_mcp_server_rollout_controller(
                &lookup, team_id, &server, &rollout, checked_at,
            )
            .await
            {
                Ok(execution) => {
                    controller_execution = execution;
                    if controller_execution.get("status").and_then(Value::as_str)
                        != Some("approved")
                    {
                        skipped_count += 1;
                        controller_failed_count += 1;
                        results.push(json!({
                            "server_id": server.id,
                            "rollout_id": rollout_id,
                            "status": "skipped",
                            "reason": "controller_not_approved",
                            "controller_execution": controller_execution,
                        }));
                        continue;
                    }
                }
                Err(error) => {
                    failed_count += 1;
                    controller_failed_count += 1;
                    results.push(json!({
                        "server_id": server.id,
                        "rollout_id": rollout_id,
                        "status": "failed",
                        "reason": "controller_failed",
                        "error": error.message,
                    }));
                    continue;
                }
            }
        }
        match apply_mcp_server_rollout_inner(state, team_id, server.id, rollout_id, "system").await
        {
            Ok(response) => {
                applied_count += 1;
                results.push(json!({
                    "server_id": server.id,
                    "rollout_id": rollout_id,
                    "status": "applied",
                    "rollout": response.rollout,
                    "controller_execution": controller_execution,
                }));
            }
            Err(error) => {
                failed_count += 1;
                results.push(json!({
                    "server_id": server.id,
                    "rollout_id": rollout_id,
                    "status": "failed",
                    "error": error.message,
                }));
            }
        }
    }
    let run = McpServerRolloutDueRun {
        team_id,
        applied_count,
        skipped_count,
        expired_count,
        failed_count,
        controller_required,
        controller_configured,
        controller_execution_count,
        controller_failed_count,
        results,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_rollout_due_run",
            "team",
            Some(team_id),
            json!({
                "team_id": team_id,
                "status": mcp_server_rollout_run_status(&run),
                "applied_count": run.applied_count,
                "skipped_count": run.skipped_count,
                "expired_count": run.expired_count,
                "failed_count": run.failed_count,
                "controller_required": run.controller_required,
                "controller_configured": run.controller_configured,
                "controller_execution_count": run.controller_execution_count,
                "controller_failed_count": run.controller_failed_count,
                "results": run.results.clone(),
            }),
        ))
        .await?;
    Ok(run)
}

pub(crate) fn mcp_server_rollout_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn mcp_server_rollout_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn mcp_server_rollout_rollback_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn mcp_server_rollout_rollback_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_mcp_server_rollout_controller<F>(
    lookup: &F,
    team_id: Uuid,
    server: &McpServerRecord,
    rollout: &Value,
    checked_at: DateTime<Utc>,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_MCP_ROLLOUT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_MCP_ROLLOUT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.mcp_connector_rollout",
        "team_id": team_id,
        "server_id": server.id,
        "server_name": server.name.clone(),
        "transport": server.transport.clone(),
        "tool_allowlist": server.tool_allowlist.clone(),
        "rollout": rollout,
        "checked_at": checked_at,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (http_status, body) = controller_response_json(response, "MCP rollout controller").await?;
    let controller_status = required_controller_status(&body)?;
    let approved = matches!(
        controller_status,
        "approved" | "applied" | "validated" | "success" | "ok"
    );
    Ok(json!({
        "attempted": true,
        "status": if approved { "approved" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn execute_mcp_server_rollout_rollback_controller<F>(
    lookup: &F,
    subject: &str,
    requested_at: DateTime<Utc>,
    server: &McpServerRecord,
    rollout: &Value,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.mcp_connector_rollout_rollback",
        "subject": subject,
        "team_id": server.team_id,
        "server_id": server.id,
        "server_name": server.name.clone(),
        "transport": server.transport.clone(),
        "status": server.status.clone(),
        "tool_allowlist": server.tool_allowlist.clone(),
        "rollout": {
            "id": rollout.get("id"),
            "status": rollout.get("status"),
            "requested_by": rollout.get("requested_by"),
            "applied_by": rollout.get("applied_by"),
            "applied_at": rollout.get("applied_at"),
            "candidate": rollout.get("candidate"),
            "previous_snapshot": rollout.get("previous_snapshot"),
        },
        "requested_at": requested_at,
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
        controller_response_json(response, "MCP rollout rollback controller").await?;
    let controller_status = required_controller_status(&body)?;
    let rolled_back = matches!(
        controller_status,
        "rolled_back" | "recovered" | "success" | "ok" | "applied"
    );
    Ok(json!({
        "attempted": true,
        "status": if rolled_back { "rolled_back" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "rollback_id": body.get("rollback_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn mcp_server_rollout_run_status(run: &McpServerRolloutDueRun) -> String {
    if run.failed_count > 0 && (run.applied_count > 0 || run.expired_count > 0) {
        "partial_failure"
    } else if run.failed_count > 0 {
        "failed"
    } else if run.applied_count > 0 || run.expired_count > 0 {
        "processed"
    } else if run.skipped_count > 0 {
        "skipped"
    } else {
        "no_pending"
    }
    .to_string()
}

pub(crate) fn build_mcp_server_rollout_run_summary(
    team_id: Uuid,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    rollout_summary: &McpServerRolloutSummary,
) -> McpServerRolloutRunSummary {
    let mut recent_runs: Vec<_> = audit_logs
        .iter()
        .filter_map(mcp_server_rollout_run_from_audit_log)
        .filter(|run| run.team_id == team_id)
        .collect();
    recent_runs.sort_by_key(|run| std::cmp::Reverse(run.ran_at));
    let run_count = recent_runs.len();
    let processed_run_count = recent_runs
        .iter()
        .filter(|run| run.applied_count > 0 || run.expired_count > 0)
        .count();
    let failed_run_count = recent_runs
        .iter()
        .filter(|run| run.failed_count > 0 || run.status == "failed")
        .count();
    let latest_run = recent_runs.first().cloned();
    let mut attention_items = Vec::new();
    match latest_run.as_ref() {
        Some(run) if run.failed_count > 0 => {
            attention_items.push(McpServerRolloutRunAttentionItem {
                kind: "latest_rollout_run_failed".to_string(),
                severity: "critical".to_string(),
                message: format!(
                    "latest MCP rollout due-run failed for {} connector(s)",
                    run.failed_count
                ),
            });
        }
        Some(run) if run.status == "skipped" && run.skipped_count > 0 => {
            attention_items.push(McpServerRolloutRunAttentionItem {
                kind: "latest_rollout_run_skipped".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "latest MCP rollout due-run skipped {} connector(s)",
                    run.skipped_count
                ),
            });
        }
        Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => {
            attention_items.push(McpServerRolloutRunAttentionItem {
                kind: "stale_rollout_run".to_string(),
                severity: "warning".to_string(),
                message: "MCP connector rollouts have not been checked in the last 24 hours"
                    .to_string(),
            });
        }
        None => {
            attention_items.push(McpServerRolloutRunAttentionItem {
                kind: "missing_rollout_run".to_string(),
                severity: "warning".to_string(),
                message: "MCP connector rollout due-run has not been run for this team".to_string(),
            });
        }
        _ => {}
    }
    let production_ops =
        build_mcp_server_rollout_production_ops(latest_run.as_ref(), rollout_summary, generated_at);
    let production_orchestration = build_mcp_server_rollout_production_orchestration(
        latest_run.as_ref(),
        rollout_summary,
        failed_run_count,
        generated_at,
    );
    let lookup = |key: &str| std::env::var(key).ok();
    let deployment_readiness = build_mcp_server_deployment_readiness(
        team_id,
        audit_logs,
        generated_at,
        mcp_server_deployment_controller_required(&lookup),
        mcp_server_deployment_controller_configured(&lookup),
    );
    if production_ops.production_blocked {
        attention_items.push(McpServerRolloutRunAttentionItem {
            kind: "mcp_rollout_production_blocked".to_string(),
            severity: if production_ops.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_ops.message.clone(),
        });
    }
    if production_orchestration.production_blocked {
        attention_items.push(McpServerRolloutRunAttentionItem {
            kind: "mcp_rollout_orchestration_blocked".to_string(),
            severity: if production_orchestration.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_orchestration.message.clone(),
        });
    }
    if deployment_readiness.production_blocked {
        attention_items.push(McpServerRolloutRunAttentionItem {
            kind: "mcp_deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
        });
    }
    recent_runs.truncate(10);
    McpServerRolloutRunSummary {
        team_id,
        generated_at,
        run_count,
        processed_run_count,
        failed_run_count,
        latest_run,
        recent_runs,
        production_ops,
        production_orchestration,
        deployment_readiness,
        attention_items,
    }
}

pub(crate) fn build_mcp_server_rollout_production_ops(
    latest_run: Option<&McpServerRolloutRunRecord>,
    rollout_summary: &McpServerRolloutSummary,
    generated_at: DateTime<Utc>,
) -> McpServerRolloutProductionOpsReadiness {
    let latest_run_age_hours = latest_run.map(|run| (generated_at - run.ran_at).num_hours());
    let latest_run_status = latest_run.map(|run| run.status.clone());
    let (status, production_blocked, message) = if rollout_summary.failed_preflight_count > 0 {
        (
            "blocked",
            true,
            "MCP connector production rollout is blocked by failed preflight checks".to_string(),
        )
    } else if rollout_summary.expired_pending_count > 0 {
        (
            "blocked",
            true,
            "MCP connector production rollout is blocked by expired pending rollout(s)".to_string(),
        )
    } else {
        match latest_run {
            None => (
                "blocked",
                true,
                "MCP connector production rollout is blocked until a rollout due-run is recorded"
                    .to_string(),
            ),
            Some(run) if run.failed_count > 0 || run.status == "failed" => (
                "blocked",
                true,
                "MCP connector production rollout is blocked by the latest failed due-run"
                    .to_string(),
            ),
            Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => (
                "stale",
                true,
                "MCP connector production rollout is blocked until due-run supervision is refreshed"
                    .to_string(),
            ),
            Some(_) if rollout_summary.due_pending_count > 0 => (
                "blocked",
                true,
                "MCP connector production rollout is blocked while rollout(s) are due for activation"
                    .to_string(),
            ),
            Some(_) if rollout_summary.pending_rollout_count > 0 => (
                "attention",
                true,
                "MCP connector production rollout requires attention because rollout(s) are still pending"
                    .to_string(),
            ),
            Some(_) => (
                "ready",
                false,
                "MCP connector rollout due-run supervision is fresh and no rollout is pending"
                    .to_string(),
            ),
        }
    };
    McpServerRolloutProductionOpsReadiness {
        status: status.to_string(),
        production_blocked,
        latest_run_status,
        latest_run_age_hours,
        pending_rollout_count: rollout_summary.pending_rollout_count,
        due_pending_count: rollout_summary.due_pending_count,
        expired_pending_count: rollout_summary.expired_pending_count,
        failed_preflight_count: rollout_summary.failed_preflight_count,
        message,
    }
}

pub(crate) fn build_mcp_server_rollout_production_orchestration(
    latest_run: Option<&McpServerRolloutRunRecord>,
    rollout_summary: &McpServerRolloutSummary,
    failed_run_count: usize,
    generated_at: DateTime<Utc>,
) -> McpServerRolloutProductionOrchestrationReadiness {
    let scheduler_supervision_fresh =
        latest_run.is_some_and(|run| (generated_at - run.ran_at).num_hours() < 6);
    let pending_clear = rollout_summary.pending_rollout_count == 0
        && rollout_summary.due_pending_count == 0
        && rollout_summary.expired_pending_count == 0;
    let failed_preflight_clear = rollout_summary.failed_preflight_count == 0;
    let failed_runs_clear = failed_run_count == 0;
    let manual_apply_required_count = rollout_summary.manual_pending_count;
    let mut blocking_reasons = Vec::new();

    if !scheduler_supervision_fresh {
        blocking_reasons.push("fresh scheduler due-run supervision is missing".to_string());
    }
    if !pending_clear {
        blocking_reasons.push("pending, due, or expired connector rollouts remain".to_string());
    }
    if manual_apply_required_count > 0 {
        blocking_reasons.push("manual connector rollout apply steps remain".to_string());
    }
    if !failed_preflight_clear {
        blocking_reasons.push("connector rollout preflight failures remain".to_string());
    }
    if !failed_runs_clear {
        blocking_reasons
            .push("failed connector rollout due-run history requires review".to_string());
    }

    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let latest_run_status = latest_run.map(|run| run.status.clone());
    let message = if production_blocked {
        format!(
            "MCP connector production orchestration is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "MCP connector production orchestration has fresh scheduler supervision, no pending rollout work, no failed preflight, and no failed due-run history".to_string()
    };

    McpServerRolloutProductionOrchestrationReadiness {
        status,
        production_blocked,
        scheduler_supervision_fresh,
        latest_run_status,
        pending_clear,
        failed_preflight_clear,
        failed_runs_clear,
        manual_apply_required_count,
        blocking_reasons,
        message,
    }
}

pub(crate) fn build_mcp_server_deployment_readiness(
    team_id: Uuid,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> McpServerDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "mcp.server_deployment_validation_run")
        .filter(|log| {
            log.details
                .get("team_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                == Some(team_id)
        })
        .max_by_key(|log| log.created_at);
    let controller_validation_logs = audit_logs
        .iter()
        .filter(|log| log.action == "mcp.server_deployment_validation_run")
        .filter(|log| {
            log.details
                .get("team_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                == Some(team_id)
        })
        .filter(|log| {
            log.details
                .get("controller_execution")
                .and_then(|execution| execution.get("attempted"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let controller_execution_count = controller_validation_logs.len();
    let controller_failed_count = controller_validation_logs
        .iter()
        .filter(|log| {
            log.details
                .get("controller_execution")
                .and_then(|execution| execution.get("status"))
                .and_then(Value::as_str)
                .is_some_and(|status| status != "validated")
        })
        .count();
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_age_hours =
        latest_validation_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let server_count = latest_validation
        .and_then(|log| log.details.get("server_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let healthy_count = latest_validation
        .and_then(|log| log.details.get("healthy_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let unhealthy_count = latest_validation
        .and_then(|log| log.details.get("unhealthy_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
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
    let mut blocking_reasons = Vec::new();

    if latest_validation.is_none() {
        blocking_reasons.push("MCP connector deployment validation has not run".to_string());
    }
    if latest_validation.is_some() && server_count == 0 {
        blocking_reasons
            .push("MCP connector deployment validation covered no connectors".to_string());
    }
    if unhealthy_count > 0 {
        blocking_reasons
            .push("MCP connector deployment validation found unhealthy connectors".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("MCP connector deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("MCP connector deployment controller is required but not configured".to_string());
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "MCP connector deployment controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("MCP connector deployment controller evidence is stale".to_string());
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
            "MCP connector deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "MCP connector deployment has a recent healthy validation run".to_string()
    };

    McpServerDeploymentReadiness {
        status,
        production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        server_count,
        healthy_count,
        unhealthy_count,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        controller_execution_count,
        controller_failed_count,
        deployment_validated: !production_blocked,
        blocking_reasons,
        message,
    }
}

pub(crate) fn mcp_server_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn mcp_server_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_mcp_server_deployment_controller<F>(
    lookup: &F,
    team_id: Uuid,
    subject: &str,
    checked_at: DateTime<Utc>,
    results: &[McpServerHealth],
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_MCP_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let connectors = results
        .iter()
        .map(|health| {
            json!({
                "server_id": health.server_id,
                "name": health.name,
                "status": health.status,
                "healthy": health.healthy,
                "issues": health.issues,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "type": "mandoforge.mcp_connector_deployment",
        "team_id": team_id,
        "subject": subject,
        "checked_at": checked_at,
        "server_count": results.len(),
        "healthy_count": results.iter().filter(|health| health.healthy).count(),
        "unhealthy_count": results.iter().filter(|health| !health.healthy).count(),
        "connectors": connectors,
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
        controller_response_json(response, "MCP connector deployment controller").await?;
    let controller_status = required_controller_status(&body)?;
    let validated = matches!(
        controller_status,
        "validated" | "deployed" | "healthy" | "success" | "ok"
    );
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn mcp_server_rollout_run_from_audit_log(
    log: &AuditLog,
) -> Option<McpServerRolloutRunRecord> {
    if log.action != "mcp.server_rollout_due_run" {
        return None;
    }
    let team_id = log
        .details
        .get("team_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .or(log.resource_id)?;
    let applied_count = json_usize(&log.details, "applied_count");
    let expired_count = json_usize(&log.details, "expired_count");
    let failed_count = json_usize(&log.details, "failed_count");
    let skipped_count = json_usize(&log.details, "skipped_count");
    let controller_required = log
        .details
        .get("controller_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let controller_configured = log
        .details
        .get("controller_configured")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let controller_execution_count = json_usize(&log.details, "controller_execution_count");
    let controller_failed_count = json_usize(&log.details, "controller_failed_count");
    let status = log
        .details
        .get("status")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if failed_count > 0 && (applied_count > 0 || expired_count > 0) {
                "partial_failure".to_string()
            } else if failed_count > 0 {
                "failed".to_string()
            } else if applied_count > 0 || expired_count > 0 {
                "processed".to_string()
            } else if skipped_count > 0 {
                "skipped".to_string()
            } else {
                "no_pending".to_string()
            }
        });
    Some(McpServerRolloutRunRecord {
        id: log.id,
        team_id,
        status,
        applied_count,
        skipped_count,
        expired_count,
        failed_count,
        controller_required,
        controller_configured,
        controller_execution_count,
        controller_failed_count,
        ran_at: log.created_at,
    })
}

pub(crate) fn build_mcp_server_rollout_summary(
    team_id: Uuid,
    servers: Vec<McpServerRecord>,
    now: DateTime<Utc>,
) -> McpServerRolloutSummary {
    let mut by_server_status = BTreeMap::new();
    let mut by_transport = BTreeMap::new();
    let mut pending_rollout_count = 0usize;
    let mut manual_pending_count = 0usize;
    let mut scheduled_pending_count = 0usize;
    let mut due_pending_count = 0usize;
    let mut not_due_pending_count = 0usize;
    let mut expired_pending_count = 0usize;
    let mut applied_rollout_count = 0usize;
    let mut rolled_back_rollout_count = 0usize;
    let mut expired_rollout_count = 0usize;
    let mut failed_preflight_count = 0usize;
    let mut attention_items = Vec::new();
    let mut latest_rollouts = Vec::new();

    for server in &servers {
        *by_server_status.entry(server.status.clone()).or_insert(0) += 1;
        *by_transport.entry(server.transport.clone()).or_insert(0) += 1;

        if let Some(rollout) = mcp_pending_rollout(server) {
            pending_rollout_count += 1;
            let activation_window = mcp_rollout_activation_window(rollout);
            if activation_window
                .as_ref()
                .and_then(|window| window.activate_after)
                .is_some()
            {
                scheduled_pending_count += 1;
            } else {
                manual_pending_count += 1;
            }
            let mut reasons = Vec::new();
            if mcp_rollout_is_expired(rollout, now) {
                expired_pending_count += 1;
                reasons.push("expired_pending".to_string());
            } else if mcp_rollout_is_due(rollout, now) {
                due_pending_count += 1;
                reasons.push("due_for_activation".to_string());
            } else {
                not_due_pending_count += 1;
                if activation_window.is_some() {
                    reasons.push("activation_window_not_open".to_string());
                } else {
                    reasons.push("manual_apply_required".to_string());
                }
            }
            let preflight_healthy = rollout
                .get("preflight")
                .and_then(|preflight| preflight.get("healthy"))
                .and_then(Value::as_bool);
            let preflight_issues = rollout
                .get("preflight")
                .and_then(|preflight| preflight.get("issues"))
                .and_then(Value::as_array)
                .map(|issues| {
                    issues
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if preflight_healthy == Some(false) {
                failed_preflight_count += 1;
                reasons.push("preflight_failed".to_string());
            }
            attention_items.push(McpServerRolloutAttentionItem {
                server_id: server.id,
                name: server.name.clone(),
                server_status: server.status.clone(),
                rollout_id: rollout
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                rollout_status: rollout
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("pending")
                    .to_string(),
                reason: reasons.join(","),
                requested_by: rollout
                    .get("requested_by")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                requested_at: mcp_rollout_time(rollout, "requested_at"),
                activate_after: activation_window
                    .as_ref()
                    .and_then(|window| window.activate_after),
                activate_before: activation_window
                    .as_ref()
                    .and_then(|window| window.activate_before),
                target_keys: mcp_rollout_target_keys(rollout),
                preflight_healthy,
                preflight_issues,
            });
        }

        if let Some(last_rollout) = server.config.get("last_rollout") {
            let status = last_rollout
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            match status.as_str() {
                "applied" => applied_rollout_count += 1,
                "rolled_back" => rolled_back_rollout_count += 1,
                "expired" => expired_rollout_count += 1,
                _ => {}
            }
            latest_rollouts.push(McpServerLatestRollout {
                server_id: server.id,
                name: server.name.clone(),
                rollout_id: last_rollout
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                status,
                updated_at: mcp_rollout_time(last_rollout, "rolled_back_at")
                    .or_else(|| mcp_rollout_time(last_rollout, "applied_at"))
                    .or_else(|| mcp_rollout_time(last_rollout, "expired_at"))
                    .or_else(|| mcp_rollout_time(last_rollout, "requested_at")),
                requested_by: last_rollout
                    .get("requested_by")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                applied_by: last_rollout
                    .get("applied_by")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                rolled_back_by: last_rollout
                    .get("rolled_back_by")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            });
        }
    }

    attention_items.sort_by(|left, right| {
        mcp_rollout_attention_priority(&left.reason)
            .cmp(&mcp_rollout_attention_priority(&right.reason))
            .then_with(|| left.name.cmp(&right.name))
    });
    latest_rollouts.sort_by_key(|rollout| std::cmp::Reverse(rollout.updated_at));

    McpServerRolloutSummary {
        team_id,
        generated_at: now,
        server_count: servers.len(),
        by_server_status,
        by_transport,
        pending_rollout_count,
        manual_pending_count,
        scheduled_pending_count,
        due_pending_count,
        not_due_pending_count,
        expired_pending_count,
        applied_rollout_count,
        rolled_back_rollout_count,
        expired_rollout_count,
        failed_preflight_count,
        attention_items,
        latest_rollouts,
    }
}

pub(crate) fn mcp_rollout_target_keys(rollout: &Value) -> Vec<String> {
    let mut keys = rollout
        .get("target")
        .and_then(Value::as_object)
        .map(|target| target.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

pub(crate) fn mcp_rollout_time(rollout: &Value, field: &str) -> Option<DateTime<Utc>> {
    rollout
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

pub(crate) fn mcp_rollout_attention_priority(reason: &str) -> usize {
    if reason.contains("expired_pending") {
        0
    } else if reason.contains("preflight_failed") {
        1
    } else if reason.contains("due_for_activation") {
        2
    } else if reason.contains("activation_window_not_open") {
        3
    } else {
        4
    }
}

pub(crate) struct BuiltMcpServerRollout {
    pub(crate) rollout: Value,
    pub(crate) preflight_health: McpServerHealth,
}

pub(crate) async fn build_mcp_server_rollout(
    state: &AppState,
    server: &McpServerRecord,
    requested_by: &str,
    input: &mut RequestMcpServerRollout,
) -> Result<BuiltMcpServerRollout, AppError> {
    if let Some(transport) = input.transport.as_deref() {
        input.transport = Some(normalize_mcp_transport(transport)?);
    }
    if let Some(config) = input.config.take() {
        input.config = Some(mcp_config_without_rollout_metadata(&normalize_mcp_config(
            config,
        )?));
    }
    if let Some(tool_allowlist) = input.tool_allowlist.take() {
        input.tool_allowlist = Some(normalize_mcp_tool_allowlist(tool_allowlist)?);
    }
    if let Some(status) = input.status.as_deref() {
        input.status = Some(normalize_mcp_status(status)?);
    }
    let has_target = input.transport.is_some()
        || input.config.is_some()
        || input.tool_allowlist.is_some()
        || input.status.is_some();
    if !has_target {
        return Err(AppError::bad_request(
            "MCP server rollout requires at least one target field",
        ));
    }
    let activation_window = normalize_policy_activation_window(
        input.activate_after.as_deref(),
        input.activate_before.as_deref(),
    )?;
    let mut candidate = server.clone();
    if let Some(transport) = input.transport.clone() {
        candidate.transport = transport;
    }
    if let Some(config) = input.config.clone() {
        candidate.config = config;
    }
    if let Some(tool_allowlist) = input.tool_allowlist.clone() {
        candidate.tool_allowlist = tool_allowlist;
    }
    if let Some(status) = input.status.clone() {
        candidate.status = status;
    }
    let preflight_health = mcp_server_health(state, &candidate).await;
    if candidate.status == "active" && !preflight_health.healthy {
        return Err(AppError::bad_request(format!(
            "MCP server rollout preflight failed: {}",
            preflight_health.issues.join("; ")
        )));
    }
    let mut target = serde_json::Map::new();
    if let Some(transport) = input.transport.clone() {
        target.insert("transport".to_string(), json!(transport));
    }
    if let Some(config) = input.config.clone() {
        target.insert("config".to_string(), config);
    }
    if let Some(tool_allowlist) = input.tool_allowlist.clone() {
        target.insert("tool_allowlist".to_string(), json!(tool_allowlist));
    }
    if let Some(status) = input.status.clone() {
        target.insert("status".to_string(), json!(status));
    }
    let rollout = json!({
        "id": Uuid::new_v4(),
        "status": "pending",
        "requested_by": requested_by,
        "requested_at": Utc::now(),
        "reason": optional_trimmed(input.reason.as_deref()),
        "activation_window": activation_window,
        "target": Value::Object(target),
        "previous_snapshot": {
            "transport": server.transport,
            "config": mcp_config_without_rollout_metadata(&server.config),
            "tool_allowlist": server.tool_allowlist,
            "status": server.status,
        },
        "preflight": {
            "healthy": preflight_health.healthy,
            "issues": preflight_health.issues,
            "checked_at": preflight_health.checked_at,
        }
    });
    Ok(BuiltMcpServerRollout {
        rollout,
        preflight_health,
    })
}

pub(crate) async fn apply_mcp_server_rollout_inner(
    state: &AppState,
    team_id: Uuid,
    server_id: Uuid,
    rollout_id: Uuid,
    applied_by: &str,
) -> Result<McpServerRolloutResponse, AppError> {
    let server = state.get_mcp_server(team_id, server_id).await?;
    let rollout = mcp_pending_rollout(&server)
        .cloned()
        .ok_or_else(|| AppError::bad_request("MCP server has no pending rollout"))?;
    if rollout.get("id").and_then(Value::as_str) != Some(&rollout_id.to_string()) {
        return Err(AppError::bad_request(
            "MCP server pending rollout does not match requested rollout id",
        ));
    }
    if rollout.get("status").and_then(Value::as_str) != Some("pending") {
        return Err(AppError::bad_request("MCP server rollout is not pending"));
    }
    enforce_mcp_rollout_activation_window(&rollout, Utc::now())?;
    let target = rollout
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::bad_request("MCP server rollout missing target"))?;
    let mut candidate = server.clone();
    if let Some(transport) = target.get("transport").and_then(Value::as_str) {
        candidate.transport = transport.to_string();
    }
    if let Some(config) = target.get("config") {
        candidate.config = config.clone();
    }
    if let Some(tool_allowlist) = target.get("tool_allowlist").cloned() {
        candidate.tool_allowlist = serde_json::from_value(tool_allowlist).map_err(|error| {
            AppError::bad_request(format!("invalid rollout tool_allowlist: {error}"))
        })?;
    }
    if let Some(status) = target.get("status").and_then(Value::as_str) {
        candidate.status = status.to_string();
    }
    let preflight_health = mcp_server_health(state, &candidate).await;
    if candidate.status == "active" && !preflight_health.healthy {
        return Err(AppError::bad_request(format!(
            "MCP server rollout preflight failed: {}",
            preflight_health.issues.join("; ")
        )));
    }
    let mut applied_rollout = rollout;
    applied_rollout["status"] = json!("applied");
    applied_rollout["applied_by"] = json!(applied_by);
    applied_rollout["applied_at"] = json!(Utc::now());
    let mut config = candidate.config.as_object().cloned().unwrap_or_default();
    config.remove("pending_rollout");
    config.insert("last_rollout".to_string(), applied_rollout.clone());
    let updated = state
        .update_mcp_server(
            team_id,
            server_id,
            UpdateMcpServerRecord {
                transport: Some(candidate.transport),
                config: Some(Value::Object(config)),
                tool_allowlist: Some(candidate.tool_allowlist),
            },
        )
        .await?;
    let updated = if updated.status != candidate.status {
        state
            .update_mcp_server_status(team_id, server_id, &candidate.status)
            .await?
    } else {
        updated
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "mcp.server_rollout_applied",
            "mcp_server",
            Some(server_id),
            json!({
                "subject": applied_by,
                "team_id": team_id,
                "name": updated.name,
                "rollout": applied_rollout,
            }),
        ))
        .await?;
    Ok(McpServerRolloutResponse {
        server: updated,
        rollout: applied_rollout,
        preflight_health: Some(preflight_health),
    })
}

pub(crate) async fn mark_mcp_server_rollout_expired(
    state: &AppState,
    team_id: Uuid,
    server: &McpServerRecord,
    rollout: &Value,
) -> Result<Value, AppError> {
    let mut expired = rollout.clone();
    expired["status"] = json!("expired");
    expired["expired_at"] = json!(Utc::now());
    let mut config = server.config.as_object().cloned().unwrap_or_default();
    config.remove("pending_rollout");
    config.insert("last_rollout".to_string(), expired.clone());
    state
        .update_mcp_server(
            team_id,
            server.id,
            UpdateMcpServerRecord {
                transport: None,
                config: Some(Value::Object(config)),
                tool_allowlist: None,
            },
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "mcp.server_rollout_expired",
            "mcp_server",
            Some(server.id),
            json!({
                "team_id": team_id,
                "name": server.name,
                "rollout": expired,
            }),
        ))
        .await?;
    Ok(expired)
}

pub(crate) fn mcp_pending_rollout(server: &McpServerRecord) -> Option<&Value> {
    server
        .config
        .get("pending_rollout")
        .filter(|rollout| rollout.get("status").and_then(Value::as_str) == Some("pending"))
}

pub(crate) fn mcp_config_without_rollout_metadata(config: &Value) -> Value {
    let mut object = config.as_object().cloned().unwrap_or_default();
    object.remove("pending_rollout");
    object.remove("last_rollout");
    Value::Object(object)
}

pub(crate) fn mcp_rollout_activation_window(rollout: &Value) -> Option<PolicyActivationWindow> {
    let window = rollout.get("activation_window")?;
    if window.is_null() {
        return None;
    }
    Some(PolicyActivationWindow {
        activate_after: window
            .get("activate_after")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
        activate_before: window
            .get("activate_before")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
    })
}

pub(crate) fn enforce_mcp_rollout_activation_window(
    rollout: &Value,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let Some(window) = mcp_rollout_activation_window(rollout) else {
        return Ok(());
    };
    if let Some(activate_after) = window.activate_after
        && now < activate_after
    {
        return Err(AppError::bad_request(format!(
            "MCP server rollout activation window is not open until {}",
            activate_after.to_rfc3339()
        )));
    }
    if let Some(activate_before) = window.activate_before
        && now > activate_before
    {
        return Err(AppError::bad_request(format!(
            "MCP server rollout activation window closed at {}",
            activate_before.to_rfc3339()
        )));
    }
    Ok(())
}

pub(crate) fn mcp_rollout_is_due(rollout: &Value, now: DateTime<Utc>) -> bool {
    let Some(window) = mcp_rollout_activation_window(rollout) else {
        return false;
    };
    let Some(activate_after) = window.activate_after else {
        return false;
    };
    now >= activate_after
}

pub(crate) fn mcp_rollout_is_expired(rollout: &Value, now: DateTime<Utc>) -> bool {
    mcp_rollout_activation_window(rollout)
        .and_then(|window| window.activate_before)
        .is_some_and(|activate_before| now > activate_before)
}

pub(crate) fn mcp_server_health_check_is_due(server: &McpServerRecord, now: DateTime<Utc>) -> bool {
    let Some(interval_seconds) = mcp_server_health_interval_seconds(server) else {
        return false;
    };
    let Some(last_checked_at) = server
        .config
        .pointer("/health_check/last_checked_at")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return true;
    };
    now.signed_duration_since(last_checked_at) >= chrono::Duration::seconds(interval_seconds)
}

pub(crate) fn mcp_server_health_interval_seconds(server: &McpServerRecord) -> Option<i64> {
    if server
        .config
        .pointer("/health_check/enabled")
        .and_then(Value::as_bool)
        == Some(false)
    {
        return None;
    }
    server
        .config
        .pointer("/health_check/interval_seconds")
        .or_else(|| server.config.get("health_check_interval_seconds"))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

pub(crate) fn mcp_server_config_with_health_result(
    config: &Value,
    health: &McpServerHealth,
    checked_at: DateTime<Utc>,
) -> Value {
    let mut object = config.as_object().cloned().unwrap_or_default();
    let mut health_config = object
        .get("health_check")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    health_config.insert(
        "last_checked_at".to_string(),
        Value::String(checked_at.to_rfc3339()),
    );
    health_config.insert("last_healthy".to_string(), Value::Bool(health.healthy));
    health_config.insert(
        "last_status".to_string(),
        Value::String(
            if health.healthy {
                "healthy"
            } else {
                "unhealthy"
            }
            .to_string(),
        ),
    );
    health_config.insert("last_issues".to_string(), json!(health.issues));
    object.insert("health_check".to_string(), Value::Object(health_config));
    Value::Object(object)
}

pub(crate) async fn mcp_server_health(
    state: &AppState,
    server: &McpServerRecord,
) -> McpServerHealth {
    let mut issues = Vec::new();
    let gateway_configured = state.mcp_gateway_config.is_some();
    let mut gateway_allows_server = false;
    let mut gateway_reachable = false;
    let secret_refs = mcp_config_secret_refs(&server.config);

    if server.status != "active" {
        issues.push(format!("MCP server status is {}", server.status));
    }
    if server.transport.trim().is_empty() {
        issues.push("MCP server transport is empty".to_string());
    }
    if server.tool_allowlist.is_empty() {
        issues.push("MCP server tool allowlist is empty".to_string());
    }

    if let Some(config) = state.mcp_gateway_config.as_ref() {
        gateway_allows_server = config.allows_server(&server.name);
        if !gateway_allows_server {
            issues.push(format!(
                "MCP gateway config does not allow server {}",
                server.name
            ));
        }
        match state.mcp_gateway_client.health_check(config).await {
            Ok(()) => {
                gateway_reachable = true;
            }
            Err(error) => issues.push(format!(
                "MCP gateway health check failed: {}",
                error.message
            )),
        }
    } else {
        issues.push("MCP gateway is not configured".to_string());
    }

    McpServerHealth {
        server_id: server.id,
        team_id: server.team_id,
        name: server.name.clone(),
        status: server.status.clone(),
        healthy: issues.is_empty(),
        issues,
        checks: json!({
            "transport": server.transport,
            "tool_allowlist_count": server.tool_allowlist.len(),
            "secret_refs_count": secret_refs.len(),
            "secret_refs": secret_refs,
            "secret_values_loaded": false,
            "gateway_configured": gateway_configured,
            "gateway_allows_server": gateway_allows_server,
            "gateway_reachable": gateway_reachable,
        }),
        checked_at: Utc::now(),
    }
}
