use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn execute_provider_policy_gate(
    state: &AppState,
    subject: Option<String>,
    actor_type: &str,
) -> Result<ProviderPolicyGateRunResponse, AppError> {
    let providers = state.list_providers().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let report = build_provider_policy_gate_report(&providers, &audit_logs);
    let failed_provider_names = report
        .checks
        .iter()
        .filter(|check| check.gate_status == "failed")
        .map(|check| check.provider_name.clone())
        .collect::<Vec<_>>();
    let warning_provider_names = report
        .checks
        .iter()
        .filter(|check| check.gate_status == "warning")
        .map(|check| check.provider_name.clone())
        .collect::<Vec<_>>();
    let audit_log = state
        .append_audit_log(new_audit_log(
            None,
            actor_type,
            None,
            "provider.policy_gate_run",
            "providers",
            None,
            json!({
                "subject": subject,
                "status": report.status,
                "provider_count": report.provider_count,
                "passed_count": report.passed_count,
                "failed_count": report.failed_count,
                "warning_count": report.warning_count,
                "failed_provider_names": failed_provider_names,
                "warning_provider_names": warning_provider_names,
                "report": report,
            }),
        ))
        .await?;
    let run = provider_policy_gate_run_from_audit_log(&audit_log)
        .ok_or_else(|| anyhow::anyhow!("failed to build provider policy gate run"))?;
    Ok(ProviderPolicyGateRunResponse { run, report })
}

pub(crate) async fn execute_provider_production_rollout_with_lookup<F>(
    state: &AppState,
    subject_id: String,
    input: RunProviderProductionRollout,
    lookup: F,
) -> Result<ProviderProductionRolloutRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let environment =
        optional_trimmed(input.environment.as_deref()).unwrap_or_else(|| "production".to_string());
    let reason = optional_trimmed(input.reason.as_deref());
    let providers = state.list_providers().await?;
    let provider_ids = if input.provider_ids.is_empty() {
        providers
            .iter()
            .filter(|provider| provider.status == "active")
            .map(|provider| provider.id)
            .collect::<Vec<_>>()
    } else {
        let known_provider_ids = providers
            .iter()
            .map(|provider| provider.id)
            .collect::<HashSet<_>>();
        let mut missing_provider_ids = Vec::new();
        for provider_id in &input.provider_ids {
            if !known_provider_ids.contains(provider_id) {
                missing_provider_ids.push(*provider_id);
            }
        }
        if !missing_provider_ids.is_empty() {
            return Err(AppError::bad_request(format!(
                "provider production rollout references unknown provider id(s): {}",
                missing_provider_ids
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        input.provider_ids
    };
    let generated_at = Utc::now();
    let audit_logs = state.list_audit_logs(None).await?;
    let gate_summary = build_provider_policy_gate_run_summary(&audit_logs, generated_at);
    let latest_run = gate_summary.latest_run.as_ref();
    let mut status = "applied".to_string();
    let message: String;
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": "blocked_before_controller_execution"
    });
    let controller_configured = provider_rollout_controller_configured(&lookup);
    if provider_ids.is_empty() {
        status = "blocked".to_string();
        message = "provider production rollout is blocked because no active providers are selected"
            .to_string();
    } else if gate_summary.production_enforcement.production_blocked {
        status = "blocked".to_string();
        message = gate_summary.production_enforcement.message.clone();
    } else if latest_run
        .map(|run| run.provider_count != providers.len())
        .unwrap_or(true)
    {
        status = "blocked".to_string();
        message =
            "provider production rollout is blocked because the latest provider gate does not cover the current provider set"
                .to_string();
    } else if !controller_configured {
        status = "blocked".to_string();
        message =
            "provider production rollout is blocked because MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL is not configured"
                .to_string();
        controller_execution = json!({
            "attempted": false,
            "status": "skipped",
            "reason": "controller_not_configured"
        });
    } else {
        match execute_provider_rollout_controller(
            &lookup,
            &environment,
            reason.as_deref(),
            &provider_ids,
            &providers,
            &gate_summary.production_enforcement,
            latest_run,
            generated_at,
        )
        .await
        {
            Ok(execution) => {
                controller_execution = execution.clone();
                let controller_status = required_controller_status(&execution)?;
                if controller_status == "applied" {
                    message =
                        "provider production rollout applied through external rollout controller"
                            .to_string();
                } else {
                    status = "blocked".to_string();
                    message =
                        "provider production rollout controller did not confirm apply".to_string();
                }
            }
            Err(error) => {
                status = "blocked".to_string();
                message = "provider production rollout controller failed".to_string();
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    let action = if status == "blocked" {
        "provider.production_rollout_blocked"
    } else {
        "provider.production_rollout_applied"
    };
    let audit_log = state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "providers",
            None,
            json!({
                "subject": subject_id,
                "status": status,
                "environment": environment,
                "reason": reason,
                "provider_ids": provider_ids,
                "provider_count": provider_ids.len(),
                "production_enforcement": gate_summary.production_enforcement.clone(),
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "latest_gate_run_id": latest_run.map(|run| run.id),
                "latest_gate_provider_count": latest_run.map(|run| run.provider_count),
                "current_provider_count": providers.len(),
                "message": message,
            }),
        ))
        .await?;
    Ok(ProviderProductionRolloutRun {
        id: audit_log.id,
        status,
        environment,
        reason,
        provider_count: provider_ids.len(),
        provider_ids,
        enforcement: gate_summary.production_enforcement,
        controller_configured,
        controller_execution,
        message,
        ran_at: audit_log.created_at,
    })
}

pub(crate) async fn execute_provider_production_rollback_with_lookup<F>(
    state: &AppState,
    subject_id: String,
    input: RunProviderProductionRollback,
    lookup: F,
) -> Result<ProviderProductionRollbackRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let reason = optional_trimmed(input.reason.as_deref());
    let generated_at = Utc::now();
    let audit_logs = state.list_audit_logs(None).await?;
    let latest_rollout = audit_logs
        .iter()
        .filter(|log| {
            log.action == "provider.production_rollout_applied"
                && log.details.get("status").and_then(Value::as_str) == Some("applied")
        })
        .max_by_key(|log| log.created_at);
    let mut status = "rolled_back".to_string();
    let mut message = "provider production rollout rollback completed".to_string();
    let mut environment = "production".to_string();
    let mut provider_ids = Vec::new();
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": "blocked_before_controller_execution"
    });
    let controller_configured = provider_rollout_rollback_controller_configured(&lookup);

    if let Some(rollout) = latest_rollout {
        environment = rollout
            .details
            .get("environment")
            .and_then(Value::as_str)
            .unwrap_or("production")
            .to_string();
        provider_ids = rollout
            .details
            .get("provider_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|value| Uuid::parse_str(value).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
    }

    if latest_rollout.is_none() {
        status = "blocked".to_string();
        message =
            "provider production rollback is blocked because no applied rollout evidence exists"
                .to_string();
        controller_execution = json!({
            "attempted": false,
            "status": "skipped",
            "reason": "missing_applied_rollout"
        });
    } else if provider_ids.is_empty() {
        status = "blocked".to_string();
        message =
            "provider production rollback is blocked because latest rollout has no provider ids"
                .to_string();
        controller_execution = json!({
            "attempted": false,
            "status": "skipped",
            "reason": "missing_provider_ids"
        });
    } else if !controller_configured {
        status = "blocked".to_string();
        message =
            "provider production rollback is blocked because MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL is not configured"
                .to_string();
        controller_execution = json!({
            "attempted": false,
            "status": "skipped",
            "reason": "controller_not_configured"
        });
    } else if let Some(rollout) = latest_rollout {
        match execute_provider_rollout_rollback_controller(
            &lookup,
            &environment,
            reason.as_deref(),
            &provider_ids,
            rollout,
            generated_at,
        )
        .await
        {
            Ok(execution) => {
                controller_execution = execution.clone();
                let controller_status = required_controller_status(&execution)?;
                if controller_status == "rolled_back" {
                    message =
                        "provider production rollout rollback confirmed by external controller"
                            .to_string();
                } else {
                    status = "blocked".to_string();
                    message = "provider production rollback controller did not confirm rollback"
                        .to_string();
                }
            }
            Err(error) => {
                status = "blocked".to_string();
                message = "provider production rollback controller failed".to_string();
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }

    let action = if status == "blocked" {
        "provider.production_rollout_rollback_blocked"
    } else {
        "provider.production_rollout_rolled_back"
    };
    let source_rollout_id = latest_rollout.map(|log| log.id);
    let audit_log = state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "providers",
            None,
            json!({
                "subject": subject_id,
                "status": status,
                "environment": environment,
                "reason": reason,
                "provider_ids": provider_ids,
                "provider_count": provider_ids.len(),
                "source_rollout_id": source_rollout_id,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "message": message,
            }),
        ))
        .await?;
    Ok(ProviderProductionRollbackRun {
        id: audit_log.id,
        status,
        environment,
        reason,
        provider_count: provider_ids.len(),
        provider_ids,
        source_rollout_id,
        controller_configured,
        controller_execution,
        message,
        ran_at: audit_log.created_at,
    })
}

pub(crate) fn provider_rollout_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn provider_rollout_rollback_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_provider_rollout_controller<F>(
    lookup: &F,
    environment: &str,
    reason: Option<&str>,
    provider_ids: &[Uuid],
    providers: &[ProviderRecord],
    enforcement: &ProviderPolicyGateEnforcement,
    latest_run: Option<&ProviderPolicyGateRun>,
    requested_at: DateTime<Utc>,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_PROVIDER_ROLLOUT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_PROVIDER_ROLLOUT_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let selected_ids = provider_ids.iter().copied().collect::<HashSet<_>>();
    let selected_providers = providers
        .iter()
        .filter(|provider| selected_ids.contains(&provider.id))
        .map(|provider| {
            json!({
                "id": provider.id,
                "name": provider.name,
                "provider_type": provider.provider_type,
                "status": provider.status,
                "default_model": provider.default_model,
                "base_url_configured": provider.base_url.as_deref().is_some_and(|value| !value.trim().is_empty()),
                "has_api_key_ref": provider.config.get("api_key_ref").and_then(Value::as_str).is_some(),
                "has_api_key_env": provider.config.get("api_key_env").and_then(Value::as_str).is_some(),
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "type": "mandoforge.provider_production_rollout",
        "environment": environment,
        "reason": reason,
        "provider_ids": provider_ids,
        "providers": selected_providers,
        "production_enforcement": enforcement,
        "latest_gate_run_id": latest_run.map(|run| run.id),
        "latest_gate_run_status": latest_run.map(|run| run.status.clone()),
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
        controller_response_json(response, "provider rollout controller").await?;
    let provider_status = required_controller_status(&body)?;
    let applied = matches!(provider_status, "applied" | "success" | "ok" | "validated");
    Ok(json!({
        "attempted": true,
        "status": if applied { "applied" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn execute_provider_rollout_rollback_controller<F>(
    lookup: &F,
    environment: &str,
    reason: Option<&str>,
    provider_ids: &[Uuid],
    source_rollout: &AuditLog,
    requested_at: DateTime<Utc>,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.provider_production_rollout_rollback",
        "environment": environment,
        "reason": reason,
        "provider_ids": provider_ids,
        "source_rollout": {
            "id": source_rollout.id,
            "ran_at": source_rollout.created_at,
            "status": source_rollout.details.get("status").and_then(Value::as_str),
            "provider_count": source_rollout.details.get("provider_count").and_then(Value::as_u64),
            "latest_gate_run_id": source_rollout.details.get("latest_gate_run_id"),
            "latest_gate_provider_count": source_rollout.details.get("latest_gate_provider_count"),
            "controller_execution": source_rollout.details.get("controller_execution"),
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
        controller_response_json(response, "provider rollout rollback controller").await?;
    let provider_status = required_controller_status(&body)?;
    let rolled_back = matches!(
        provider_status,
        "rolled_back" | "recovered" | "success" | "ok" | "applied"
    );
    Ok(json!({
        "attempted": true,
        "status": if rolled_back { "rolled_back" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "rollback_id": body.get("rollback_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn build_provider_governance_summary(
    providers: &[ProviderRecord],
    audit_logs: &[AuditLog],
) -> ProviderGovernanceSummary {
    let mut by_status = BTreeMap::new();
    let mut by_type = BTreeMap::new();
    let mut pending_status_approval_count = 0;
    let mut last_status_approval_count = 0;
    let mut credential_ref_count = 0;
    let mut env_key_count = 0;
    let mut missing_credential_count = 0;
    let mut budgeted_provider_count = 0;
    let mut active_provider_count = 0;
    let mut inactive_provider_count = 0;
    let mut attention_items = Vec::new();

    for provider in providers {
        *by_status.entry(provider.status.clone()).or_insert(0) += 1;
        *by_type.entry(provider.provider_type.clone()).or_insert(0) += 1;

        if provider.status == "active" {
            active_provider_count += 1;
        } else {
            inactive_provider_count += 1;
            attention_items.push(ProviderGovernanceAttentionItem {
                provider_id: provider.id,
                provider_name: provider.name.clone(),
                kind: "inactive_provider".to_string(),
                severity: if provider.status == "archived" {
                    "critical".to_string()
                } else {
                    "warning".to_string()
                },
                message: format!("provider status is {}", provider.status),
            });
        }

        if provider
            .config
            .get("pending_status_approval")
            .and_then(|approval| approval.get("status"))
            .and_then(Value::as_str)
            == Some("pending")
        {
            pending_status_approval_count += 1;
            attention_items.push(ProviderGovernanceAttentionItem {
                provider_id: provider.id,
                provider_name: provider.name.clone(),
                kind: "pending_status_approval".to_string(),
                severity: "warning".to_string(),
                message: "provider lifecycle change is waiting for approval".to_string(),
            });
        }
        if provider.config.get("last_status_approval").is_some() {
            last_status_approval_count += 1;
        }

        let has_api_key_ref = provider
            .config
            .get("api_key_ref")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let has_api_key_env = provider
            .config
            .get("api_key_env")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if has_api_key_ref {
            credential_ref_count += 1;
        }
        if has_api_key_env {
            env_key_count += 1;
        }
        if provider_requires_api_key(&provider.provider_type)
            && !has_api_key_ref
            && !has_api_key_env
        {
            missing_credential_count += 1;
            attention_items.push(ProviderGovernanceAttentionItem {
                provider_id: provider.id,
                provider_name: provider.name.clone(),
                kind: "missing_credential".to_string(),
                severity: "critical".to_string(),
                message: "provider requires config.api_key_ref or config.api_key_env".to_string(),
            });
        }

        if provider_requires_base_url(&provider.provider_type)
            && provider
                .base_url
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            attention_items.push(ProviderGovernanceAttentionItem {
                provider_id: provider.id,
                provider_name: provider.name.clone(),
                kind: "missing_base_url".to_string(),
                severity: "critical".to_string(),
                message: "provider requires a base URL before external health checks can run"
                    .to_string(),
            });
        }

        if provider_daily_request_limit(provider).is_some()
            || provider_daily_cost_limit_cents(provider).is_some()
        {
            budgeted_provider_count += 1;
        }
    }

    let emergency_lifecycle_count = audit_logs
        .iter()
        .filter(|log| {
            log.action == "provider.status_updated"
                && log.details["policy_decision"]["gate"] == "provider_lifecycle_emergency"
        })
        .count();
    let lookup = |key: &str| std::env::var(key).ok();
    let deployment_readiness = build_provider_deployment_readiness(
        audit_logs,
        Utc::now(),
        provider_deployment_controller_required(&lookup),
        provider_deployment_controller_configured(&lookup),
    );
    if deployment_readiness.production_blocked {
        attention_items.push(ProviderGovernanceAttentionItem {
            provider_id: Uuid::nil(),
            provider_name: "providers".to_string(),
            kind: "provider_deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
        });
    }

    ProviderGovernanceSummary {
        provider_count: providers.len(),
        by_status,
        by_type,
        pending_status_approval_count,
        last_status_approval_count,
        emergency_lifecycle_count,
        credential_ref_count,
        env_key_count,
        missing_credential_count,
        budgeted_provider_count,
        active_provider_count,
        inactive_provider_count,
        deployment_readiness,
        attention_items,
        generated_at: Utc::now(),
    }
}

pub(crate) fn build_provider_deployment_readiness(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> ProviderDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "provider.deployment_validation_run")
        .max_by_key(|log| log.created_at);
    let controller_validation_logs = audit_logs
        .iter()
        .filter(|log| {
            log.action == "provider.deployment_validation_run"
                && log
                    .details
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
    let provider_count = latest_validation
        .and_then(|log| log.details.get("provider_count"))
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
        blocking_reasons.push("provider deployment validation has not run".to_string());
    }
    if latest_validation.is_some() && provider_count == 0 {
        blocking_reasons
            .push("provider deployment validation covered no active providers".to_string());
    }
    if unhealthy_count > 0 {
        blocking_reasons
            .push("provider deployment validation found unhealthy providers".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("provider deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("provider deployment controller is required but not configured".to_string());
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "provider deployment controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("provider deployment controller evidence is stale".to_string());
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
            "Provider deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Provider deployment has a recent healthy validation run".to_string()
    };

    ProviderDeploymentReadiness {
        status,
        production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        provider_count,
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

pub(crate) fn provider_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn provider_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_provider_deployment_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    results: &[ProviderHealth],
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_PROVIDER_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let providers = results
        .iter()
        .map(|health| {
            json!({
                "provider_id": health.provider_id,
                "name": health.name,
                "status": health.status,
                "healthy": health.healthy,
                "issues": health.issues,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "type": "mandoforge.provider_deployment",
        "subject": subject,
        "checked_at": checked_at,
        "provider_count": results.len(),
        "healthy_count": results.iter().filter(|health| health.healthy).count(),
        "unhealthy_count": results.iter().filter(|health| !health.healthy).count(),
        "providers": providers,
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
        controller_response_json(response, "provider deployment controller").await?;
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

pub(crate) fn build_provider_policy_gate_report(
    providers: &[ProviderRecord],
    audit_logs: &[AuditLog],
) -> ProviderPolicyGateReport {
    let emergency_provider_ids: HashSet<Uuid> = audit_logs
        .iter()
        .filter(|log| {
            log.action == "provider.status_updated"
                && log.details["policy_decision"]["gate"] == "provider_lifecycle_emergency"
        })
        .filter_map(|log| log.resource_id)
        .collect();
    let mut checks = Vec::with_capacity(providers.len());
    for provider in providers {
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut recommendations = Vec::new();
        let has_api_key_ref = provider
            .config
            .get("api_key_ref")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let has_api_key_env = provider
            .config
            .get("api_key_env")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if provider.status != "active" {
            blockers.push(format!("provider status is {}", provider.status));
            recommendations.push("activate the provider through lifecycle approval".to_string());
        }
        if provider
            .config
            .get("pending_status_approval")
            .and_then(|approval| approval.get("status"))
            .and_then(Value::as_str)
            == Some("pending")
        {
            blockers.push("provider lifecycle approval is still pending".to_string());
            recommendations.push("approve or reject the pending lifecycle request".to_string());
        }
        if provider_requires_api_key(&provider.provider_type)
            && !has_api_key_ref
            && !has_api_key_env
        {
            blockers.push("provider requires api_key_ref or api_key_env".to_string());
            recommendations
                .push("bind provider credentials through Vault ref or env key".to_string());
        }
        if provider_requires_base_url(&provider.provider_type)
            && provider
                .base_url
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            blockers.push("provider requires base_url".to_string());
            recommendations.push("configure provider base_url before production use".to_string());
        }
        if provider_daily_request_limit(provider).is_none()
            && provider_daily_cost_limit_cents(provider).is_none()
        {
            warnings.push("provider has no request or cost budget".to_string());
            recommendations.push("configure daily request or cost budget".to_string());
        }
        if has_api_key_env && !has_api_key_ref {
            warnings.push("provider uses env credential instead of Vault ref".to_string());
            recommendations.push("rotate provider credential to api_key_ref".to_string());
        }
        if emergency_provider_ids.contains(&provider.id) {
            warnings.push("provider has emergency lifecycle changes in audit history".to_string());
            recommendations
                .push("review emergency lifecycle change before production rollout".to_string());
        }
        let gate_status = if !blockers.is_empty() {
            "failed"
        } else if !warnings.is_empty() {
            "warning"
        } else {
            "passed"
        }
        .to_string();
        checks.push(ProviderPolicyGateCheck {
            provider_id: provider.id,
            provider_name: provider.name.clone(),
            provider_type: provider.provider_type.clone(),
            status: provider.status.clone(),
            gate_status,
            blockers,
            warnings,
            recommendations,
        });
    }
    let passed_count = checks
        .iter()
        .filter(|check| check.gate_status == "passed")
        .count();
    let failed_count = checks
        .iter()
        .filter(|check| check.gate_status == "failed")
        .count();
    let warning_count = checks
        .iter()
        .filter(|check| check.gate_status == "warning")
        .count();
    let status = if failed_count > 0 {
        "failed"
    } else if warning_count > 0 {
        "warning"
    } else {
        "passed"
    }
    .to_string();
    ProviderPolicyGateReport {
        generated_at: Utc::now(),
        status,
        provider_count: providers.len(),
        passed_count,
        failed_count,
        warning_count,
        checks,
    }
}

pub(crate) fn build_provider_policy_gate_run_summary(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> ProviderPolicyGateRunSummary {
    let mut recent_runs: Vec<_> = audit_logs
        .iter()
        .filter_map(provider_policy_gate_run_from_audit_log)
        .collect();
    recent_runs.sort_by_key(|run| std::cmp::Reverse(run.ran_at));
    let run_count = recent_runs.len();
    let passed_run_count = recent_runs
        .iter()
        .filter(|run| run.status == "passed")
        .count();
    let failed_run_count = recent_runs
        .iter()
        .filter(|run| run.status == "failed")
        .count();
    let warning_run_count = recent_runs
        .iter()
        .filter(|run| run.status == "warning")
        .count();
    let latest_run = recent_runs.first().cloned();
    let mut attention_items = Vec::new();
    match latest_run.as_ref() {
        Some(run) if run.status == "failed" => {
            attention_items.push(ProviderPolicyGateRunAttentionItem {
                kind: "latest_gate_failed".to_string(),
                severity: "critical".to_string(),
                message: format!(
                    "latest provider policy gate failed for {} provider(s)",
                    run.failed_count
                ),
            });
        }
        Some(run) if run.status == "warning" => {
            attention_items.push(ProviderPolicyGateRunAttentionItem {
                kind: "latest_gate_warning".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "latest provider policy gate produced {} warning provider(s)",
                    run.warning_count
                ),
            });
        }
        Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => {
            attention_items.push(ProviderPolicyGateRunAttentionItem {
                kind: "stale_gate_run".to_string(),
                severity: "warning".to_string(),
                message: "provider policy gate has not been run in the last 24 hours".to_string(),
            });
        }
        None => {
            attention_items.push(ProviderPolicyGateRunAttentionItem {
                kind: "missing_gate_run".to_string(),
                severity: "warning".to_string(),
                message: "provider policy gate has not been run yet".to_string(),
            });
        }
        _ => {}
    }
    let production_enforcement =
        provider_policy_gate_production_enforcement(latest_run.as_ref(), generated_at);
    if production_enforcement.production_blocked {
        attention_items.push(ProviderPolicyGateRunAttentionItem {
            kind: "production_provider_gate_blocked".to_string(),
            severity: if production_enforcement.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_enforcement.message.clone(),
        });
    }
    recent_runs.truncate(10);
    ProviderPolicyGateRunSummary {
        generated_at,
        run_count,
        passed_run_count,
        failed_run_count,
        warning_run_count,
        latest_run,
        recent_runs,
        production_enforcement,
        attention_items,
    }
}

pub(crate) fn provider_policy_gate_production_enforcement(
    latest_run: Option<&ProviderPolicyGateRun>,
    generated_at: DateTime<Utc>,
) -> ProviderPolicyGateEnforcement {
    let required_fresh_hours = 24;
    let latest_run_age_hours = latest_run.map(|run| (generated_at - run.ran_at).num_hours());
    let latest_run_status = latest_run.map(|run| run.status.clone());
    let (status, production_blocked, message) = match latest_run {
        None => (
            "blocked",
            true,
            "production provider rollout is blocked until provider policy gate runs".to_string(),
        ),
        Some(run) if run.status == "failed" => (
            "blocked",
            true,
            "production provider rollout is blocked by a failed provider policy gate".to_string(),
        ),
        Some(run) if run.status == "warning" => (
            "attention",
            true,
            "production provider rollout requires manual approval because the latest provider policy gate has warnings".to_string(),
        ),
        Some(run) if (generated_at - run.ran_at).num_hours() >= required_fresh_hours => (
            "stale",
            true,
            "production provider rollout is blocked until provider policy gate is refreshed"
                .to_string(),
        ),
        Some(_) => (
            "ready",
            false,
            "latest provider policy gate is fresh and passed".to_string(),
        ),
    };
    ProviderPolicyGateEnforcement {
        status: status.to_string(),
        production_blocked,
        required_fresh_hours,
        latest_run_status,
        latest_run_age_hours,
        message,
    }
}

pub(crate) fn provider_policy_gate_run_from_audit_log(
    log: &AuditLog,
) -> Option<ProviderPolicyGateRun> {
    if log.action != "provider.policy_gate_run" {
        return None;
    }
    let failed_provider_names = log
        .details
        .get("failed_provider_names")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let warning_provider_names = log
        .details
        .get("warning_provider_names")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(ProviderPolicyGateRun {
        id: log.id,
        status: log
            .details
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        subject: log
            .details
            .get("subject")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        provider_count: json_usize(&log.details, "provider_count"),
        passed_count: json_usize(&log.details, "passed_count"),
        failed_count: json_usize(&log.details, "failed_count"),
        warning_count: json_usize(&log.details, "warning_count"),
        failed_provider_names,
        warning_provider_names,
        ran_at: log.created_at,
    })
}

pub(crate) fn provider_policy_gate_is_due(
    providers: &[ProviderRecord],
    audit_logs: &[AuditLog],
    now: DateTime<Utc>,
) -> bool {
    if providers.is_empty() {
        return false;
    }
    let latest_run = audit_logs
        .iter()
        .filter_map(provider_policy_gate_run_from_audit_log)
        .max_by_key(|run| run.ran_at);
    latest_run
        .map(|run| (now - run.ran_at).num_hours() >= 24)
        .unwrap_or(true)
}

pub(crate) fn json_usize(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().map(|value| value as u64))
        })
        .unwrap_or_default() as usize
}

pub(crate) fn provider_requires_api_key(provider_type: &str) -> bool {
    matches!(
        provider_type.trim().to_ascii_lowercase().as_str(),
        "openai-compatible" | "openai_compatible" | "anthropic"
    )
}

pub(crate) fn provider_requires_base_url(provider_type: &str) -> bool {
    matches!(
        provider_type.trim().to_ascii_lowercase().as_str(),
        "openai-compatible" | "openai_compatible" | "eval_judge"
    )
}
