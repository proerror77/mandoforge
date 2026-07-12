use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn execute_due_agent_release_promotions(
    state: &AppState,
) -> Result<AgentReleaseAutomationRun, AppError> {
    execute_due_agent_release_promotions_with_lookup(state, |key| std::env::var(key).ok()).await
}

pub(crate) async fn execute_due_agent_release_promotions_with_lookup<F>(
    state: &AppState,
    lookup: F,
) -> Result<AgentReleaseAutomationRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let checked_at = Utc::now();
    let releases = state.list_pending_agent_releases().await?;
    let pending_count = releases.len();
    let mut promoted_count = 0usize;
    let mut rejected_count = 0usize;
    let mut skipped_count = 0usize;
    let controller_required = agent_release_controller_required(&lookup);
    let controller_configured = agent_release_controller_configured(&lookup);
    let mut controller_execution_count = 0usize;
    let mut controller_failed_count = 0usize;
    let mut results = Vec::new();
    for release in releases {
        if release.status != "promotion_in_progress"
            && release_automation_is_expired(&release, checked_at)
        {
            let rejected = state
                .automate_agent_release_decision(
                    release.agent_id,
                    release.id,
                    "rejected",
                    "system".to_string(),
                    "release automation expired".to_string(),
                )
                .await?;
            rejected_count += 1;
            results.push(json!({
                "release_id": rejected.id,
                "agent_id": rejected.agent_id,
                "status": "rejected",
                "reason": "expired",
            }));
            state
                .append_audit_log(new_audit_log(
                    None,
                    "system",
                    None,
                    "agent.release_promotion_auto_rejected",
                    "agent_release",
                    Some(rejected.id),
                    json!({
                        "agent_id": rejected.agent_id,
                        "environment": rejected.environment,
                        "reason": "expired",
                    }),
                ))
                .await?;
            continue;
        }
        match release_automation_due_decision(&release, checked_at) {
            ReleaseAutomationDecision::Promote => {
                let mut controller_execution = json!({
                    "attempted": false,
                    "status": "skipped",
                    "reason": "controller_not_configured"
                });
                if controller_required && !controller_configured {
                    skipped_count += 1;
                    results.push(json!({
                        "release_id": release.id,
                        "agent_id": release.agent_id,
                        "status": "skipped",
                        "reason": "controller_required_not_configured",
                        "controller_execution": controller_execution,
                    }));
                    continue;
                }
                let in_progress = state
                    .begin_agent_release_promotion(
                        release.agent_id,
                        release.id,
                        "system".to_string(),
                        "release automation auto-approved".to_string(),
                    )
                    .await?;
                if controller_configured {
                    controller_execution_count += 1;
                    match execute_agent_release_controller(&lookup, &in_progress, checked_at).await
                    {
                        Ok(execution) => {
                            controller_execution = execution;
                            if controller_execution.get("status").and_then(Value::as_str)
                                != Some("promoted")
                            {
                                state
                                    .fail_agent_release_promotion(
                                        release.agent_id,
                                        release.id,
                                        "system".to_string(),
                                        "agent release controller did not confirm promotion"
                                            .to_string(),
                                    )
                                    .await?;
                                skipped_count += 1;
                                controller_failed_count += 1;
                                results.push(json!({
                                    "release_id": release.id,
                                    "agent_id": release.agent_id,
                                    "status": "skipped",
                                    "reason": "controller_not_promoted",
                                    "controller_execution": controller_execution,
                                }));
                                continue;
                            }
                        }
                        Err(error) => {
                            state
                                .fail_agent_release_promotion(
                                    release.agent_id,
                                    release.id,
                                    "system".to_string(),
                                    format!("release controller failed: {}", error.message),
                                )
                                .await?;
                            skipped_count += 1;
                            controller_failed_count += 1;
                            results.push(json!({
                                "release_id": release.id,
                                "agent_id": release.agent_id,
                                "status": "skipped",
                                "reason": "controller_failed",
                                "controller_execution": {
                                    "attempted": true,
                                    "status": "failed",
                                    "error": error.message
                                },
                            }));
                            continue;
                        }
                    }
                }
                let promoted = state
                    .complete_agent_release_promotion(
                        release.agent_id,
                        release.id,
                        "system".to_string(),
                    )
                    .await?;
                promoted_count += 1;
                results.push(json!({
                    "release_id": promoted.id,
                    "agent_id": promoted.agent_id,
                    "status": "promoted",
                    "reason": "auto_approved",
                    "controller_execution": controller_execution,
                }));
                state
                    .append_audit_log(new_audit_log(
                        None,
                        "system",
                        None,
                        "agent.release_promotion_auto_approved",
                        "agent_release",
                        Some(promoted.id),
                        json!({
                            "agent_id": promoted.agent_id,
                            "environment": promoted.environment,
                            "eval_score": promoted.eval_score,
                            "min_score": promoted.min_score,
                            "controller_configured": controller_configured,
                            "controller_execution": controller_execution,
                        }),
                    ))
                    .await?;
            }
            ReleaseAutomationDecision::Skip(reason) => {
                skipped_count += 1;
                results.push(json!({
                    "release_id": release.id,
                    "agent_id": release.agent_id,
                    "status": "skipped",
                    "reason": reason,
                }));
            }
        }
    }
    let run = AgentReleaseAutomationRun {
        checked_at,
        pending_count,
        promoted_count,
        rejected_count,
        skipped_count,
        controller_required,
        controller_configured,
        controller_execution_count,
        controller_failed_count,
        results,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "agent.release_promotion_due_run",
            "agent_release",
            None,
            json!({
                "status": agent_release_automation_run_status(&run),
                "pending_count": run.pending_count,
                "promoted_count": run.promoted_count,
                "rejected_count": run.rejected_count,
                "skipped_count": run.skipped_count,
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

pub(crate) fn agent_release_automation_run_status(run: &AgentReleaseAutomationRun) -> String {
    if run.promoted_count > 0 || run.rejected_count > 0 {
        "processed"
    } else if run.skipped_count > 0 {
        "skipped"
    } else {
        "no_pending"
    }
    .to_string()
}

pub(crate) fn agent_release_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn agent_release_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    agent_release_controller_required_for(lookup, "MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED")
}

fn agent_release_controller_required_for<F>(lookup: &F, key: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    let production = lookup("MANDOFORGE_PROVIDER_RUNTIME_ENV").is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "production" | "prod"
        )
    });
    production
        || lookup(key)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(false)
}

pub(crate) async fn execute_agent_release_controller<F>(
    lookup: &F,
    release: &AgentRelease,
    requested_at: DateTime<Utc>,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_AGENT_RELEASE_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_AGENT_RELEASE_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.agent_release_rollout",
        "idempotency_key": release.id,
        "release_id": release.id,
        "agent_id": release.agent_id,
        "agent_version_id": release.agent_version_id,
        "environment": release.environment,
        "eval_run_id": release.eval_run_id,
        "eval_score": release.eval_score,
        "min_score": release.min_score,
        "automation_policy": release.automation_policy,
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
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "agent release controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("promoted");
    let promoted = matches!(controller_status, "promoted" | "applied" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if promoted { "promoted" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn agent_release_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    agent_release_controller_required_for(
        lookup,
        "MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED",
    )
}

pub(crate) fn agent_release_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_agent_release_deployment_controller<F>(
    lookup: &F,
    subject: &str,
    requested_at: DateTime<Utc>,
    rollout_summary: &AgentReleaseRolloutSummary,
    automation_summary: &AgentReleaseAutomationRunSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let latest_promotions = rollout_summary
        .latest_promoted_by_environment
        .iter()
        .map(|promotion| {
            json!({
                "environment": promotion.environment,
                "release_id": promotion.release_id,
                "agent_id": promotion.agent_id,
                "agent_version_id": promotion.agent_version_id,
                "promoted_at": promotion.promoted_at,
                "eval_score": promotion.eval_score,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "type": "mandoforge.agent_release_deployment_validation",
        "subject": subject,
        "release_counts": {
            "release_count": rollout_summary.release_count,
            "pending_count": rollout_summary.pending_count,
            "promoted_count": rollout_summary.promoted_count,
            "rejected_count": rollout_summary.rejected_count,
            "rolled_back_count": rollout_summary.rolled_back_count,
            "auto_pending_count": rollout_summary.auto_pending_count,
            "manual_pending_count": rollout_summary.manual_pending_count,
            "expired_pending_count": rollout_summary.expired_pending_count,
            "stale_pending_count": rollout_summary.stale_pending_count,
        },
        "automation": {
            "run_count": automation_summary.run_count,
            "processed_run_count": automation_summary.processed_run_count,
            "skipped_run_count": automation_summary.skipped_run_count,
            "latest_run": automation_summary.latest_run,
            "production_ops": automation_summary.production_ops,
            "production_orchestration": automation_summary.production_orchestration,
        },
        "latest_promotions": latest_promotions,
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
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "agent release deployment controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(provider_status, "validated" | "success" | "ok" | "healthy");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn agent_release_orchestration_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    agent_release_controller_required_for(
        lookup,
        "MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED",
    )
}

pub(crate) fn agent_release_orchestration_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_agent_release_orchestration_controller<F>(
    lookup: &F,
    subject: &str,
    requested_at: DateTime<Utc>,
    rollout_summary: &AgentReleaseRolloutSummary,
    automation_summary: &AgentReleaseAutomationRunSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.agent_release_orchestration_validation",
        "subject": subject,
        "release_counts": {
            "release_count": rollout_summary.release_count,
            "pending_count": rollout_summary.pending_count,
            "promoted_count": rollout_summary.promoted_count,
            "rejected_count": rollout_summary.rejected_count,
            "rolled_back_count": rollout_summary.rolled_back_count,
            "auto_pending_count": rollout_summary.auto_pending_count,
            "manual_pending_count": rollout_summary.manual_pending_count,
            "expired_pending_count": rollout_summary.expired_pending_count,
            "stale_pending_count": rollout_summary.stale_pending_count,
        },
        "automation": {
            "run_count": automation_summary.run_count,
            "processed_run_count": automation_summary.processed_run_count,
            "skipped_run_count": automation_summary.skipped_run_count,
            "latest_run": automation_summary.latest_run,
            "production_ops": automation_summary.production_ops,
            "production_orchestration": automation_summary.production_orchestration,
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
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "agent release orchestration controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(provider_status, "validated" | "success" | "ok" | "healthy");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "orchestration_id": body.get("orchestration_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn agent_release_rollback_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    agent_release_controller_required_for(
        lookup,
        "MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED",
    )
}

pub(crate) fn agent_release_rollback_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_agent_release_rollback_controller<F>(
    lookup: &F,
    subject: &str,
    requested_at: DateTime<Utc>,
    release: &AgentRelease,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_AGENT_RELEASE_ROLLBACK_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.agent_release_rollback",
        "subject": subject,
        "release": {
            "release_id": release.id,
            "agent_id": release.agent_id,
            "agent_version_id": release.agent_version_id,
            "environment": release.environment,
            "status": release.status,
            "eval_run_id": release.eval_run_id,
            "eval_score": release.eval_score,
            "min_score": release.min_score,
            "promoted_by": release.promoted_by,
            "promoted_at": release.promoted_at,
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
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "agent release rollback controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("rolled_back");
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

pub(crate) fn build_agent_release_automation_run_summary(
    audit_logs: &[AuditLog],
    rollout_summary: &AgentReleaseRolloutSummary,
    generated_at: DateTime<Utc>,
) -> AgentReleaseAutomationRunSummary {
    let mut recent_runs: Vec<_> = audit_logs
        .iter()
        .filter_map(agent_release_automation_run_from_audit_log)
        .collect();
    recent_runs.sort_by_key(|run| std::cmp::Reverse(run.ran_at));
    let run_count = recent_runs.len();
    let processed_run_count = recent_runs
        .iter()
        .filter(|run| run.promoted_count > 0 || run.rejected_count > 0)
        .count();
    let skipped_run_count = recent_runs
        .iter()
        .filter(|run| run.status == "skipped")
        .count();
    let latest_run = recent_runs.first().cloned();
    let mut attention_items = Vec::new();
    match latest_run.as_ref() {
        Some(run) if run.status == "skipped" && run.pending_count > 0 => {
            attention_items.push(AgentReleaseAutomationRunAttentionItem {
                kind: "latest_run_skipped_pending".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "latest release automation run skipped {} pending release(s)",
                    run.skipped_count
                ),
            });
        }
        Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => {
            attention_items.push(AgentReleaseAutomationRunAttentionItem {
                kind: "stale_release_automation_run".to_string(),
                severity: "warning".to_string(),
                message: "release automation has not been run in the last 24 hours".to_string(),
            });
        }
        None => {
            attention_items.push(AgentReleaseAutomationRunAttentionItem {
                kind: "missing_release_automation_run".to_string(),
                severity: "warning".to_string(),
                message: "release automation has not been run yet".to_string(),
            });
        }
        _ => {}
    }
    let production_ops = build_agent_release_production_ops_readiness(
        latest_run.as_ref(),
        rollout_summary,
        generated_at,
    );
    let lookup = |key: &str| std::env::var(key).ok();
    let production_orchestration = build_agent_release_production_orchestration_readiness(
        latest_run.as_ref(),
        rollout_summary,
        audit_logs,
        generated_at,
        agent_release_orchestration_controller_required(&lookup),
        agent_release_orchestration_controller_configured(&lookup),
    );
    let deployment_readiness = build_agent_release_deployment_readiness(
        audit_logs,
        generated_at,
        agent_release_deployment_controller_required(&lookup),
        agent_release_deployment_controller_configured(&lookup),
    );
    if production_ops.production_blocked {
        attention_items.push(AgentReleaseAutomationRunAttentionItem {
            kind: "release_production_ops_blocked".to_string(),
            severity: if production_ops.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_ops.message.clone(),
        });
    }
    if production_orchestration.production_blocked {
        attention_items.push(AgentReleaseAutomationRunAttentionItem {
            kind: "release_production_orchestration_blocked".to_string(),
            severity: if production_orchestration.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_orchestration.message.clone(),
        });
    }
    if deployment_readiness.production_blocked {
        attention_items.push(AgentReleaseAutomationRunAttentionItem {
            kind: "release_deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
        });
    }
    recent_runs.truncate(10);
    AgentReleaseAutomationRunSummary {
        generated_at,
        run_count,
        processed_run_count,
        skipped_run_count,
        latest_run,
        recent_runs,
        production_ops,
        production_orchestration,
        deployment_readiness,
        attention_items,
    }
}

pub(crate) fn build_agent_release_deployment_readiness(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> AgentReleaseDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "agent.release_deployment_validation_run")
        .max_by_key(|log| log.created_at);
    let controller_validation_logs = audit_logs
        .iter()
        .filter(|log| {
            log.action == "agent.release_deployment_validation_run"
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
    let latest_validation_healthy = latest_validation_status.as_deref() == Some("healthy");
    let release_count = latest_validation
        .and_then(|log| log.details.get("release_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let pending_count = latest_validation
        .and_then(|log| log.details.get("pending_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let promoted_count = latest_validation
        .and_then(|log| log.details.get("promoted_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let rejected_count = latest_validation
        .and_then(|log| log.details.get("rejected_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let rolled_back_count = latest_validation
        .and_then(|log| log.details.get("rolled_back_count"))
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
        blocking_reasons.push("agent release deployment validation has not run".to_string());
    }
    if latest_validation.is_some() && !latest_validation_healthy {
        blocking_reasons
            .push("latest agent release deployment validation was not healthy".to_string());
    }
    if latest_validation.is_some() && release_count == 0 {
        blocking_reasons
            .push("agent release deployment validation covered no releases".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("agent release deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("agent release deployment controller is required but not configured".to_string());
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "agent release deployment controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("agent release deployment controller evidence is stale".to_string());
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
            "Agent release deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Agent release deployment has recent healthy validation evidence".to_string()
    };

    AgentReleaseDeploymentReadiness {
        status,
        production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        latest_validation_healthy,
        release_count,
        pending_count,
        promoted_count,
        rejected_count,
        rolled_back_count,
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

pub(crate) fn build_agent_release_production_ops_readiness(
    latest_run: Option<&AgentReleaseAutomationRunRecord>,
    rollout_summary: &AgentReleaseRolloutSummary,
    generated_at: DateTime<Utc>,
) -> AgentReleaseProductionOpsReadiness {
    let latest_run_age_hours = latest_run.map(|run| (generated_at - run.ran_at).num_hours());
    let latest_run_status = latest_run.map(|run| run.status.clone());
    let (status, production_blocked, message) = if rollout_summary.expired_pending_count > 0 {
        (
            "blocked",
            true,
            "Agent release production rollout is blocked by expired pending release request(s)"
                .to_string(),
        )
    } else if rollout_summary.stale_pending_count > 0 {
        (
            "blocked",
            true,
            "Agent release production rollout is blocked by stale pending release request(s)"
                .to_string(),
        )
    } else {
        match latest_run {
            None => (
                "blocked",
                true,
                "Agent release production rollout is blocked until release automation supervision has run"
                    .to_string(),
            ),
            Some(run) if (generated_at - run.ran_at).num_hours() >= 24 => (
                "stale",
                true,
                "Agent release production rollout is blocked until release automation supervision is refreshed"
                    .to_string(),
            ),
            Some(run) if run.status == "skipped" && rollout_summary.auto_pending_count > 0 => (
                "blocked",
                true,
                "Agent release production rollout is blocked because automation skipped auto-pending release request(s)"
                    .to_string(),
            ),
            Some(_) if rollout_summary.pending_count > 0 => (
                "attention",
                true,
                "Agent release production rollout is blocked while release request(s) remain pending"
                    .to_string(),
            ),
            Some(_) => (
                "ready",
                false,
                "Agent release automation supervision is fresh and no pending production release requests remain"
                    .to_string(),
            ),
        }
    };
    AgentReleaseProductionOpsReadiness {
        status: status.to_string(),
        production_blocked,
        pending_count: rollout_summary.pending_count,
        auto_pending_count: rollout_summary.auto_pending_count,
        manual_pending_count: rollout_summary.manual_pending_count,
        expired_pending_count: rollout_summary.expired_pending_count,
        stale_pending_count: rollout_summary.stale_pending_count,
        latest_run_status,
        latest_run_age_hours,
        message,
    }
}

pub(crate) fn build_agent_release_production_orchestration_readiness(
    latest_run: Option<&AgentReleaseAutomationRunRecord>,
    rollout_summary: &AgentReleaseRolloutSummary,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> AgentReleaseProductionOrchestrationReadiness {
    let automation_supervision_fresh =
        latest_run.is_some_and(|run| (generated_at - run.ran_at).num_hours() < 6);
    let pending_clear = rollout_summary.pending_count == 0
        && rollout_summary.auto_pending_count == 0
        && rollout_summary.manual_pending_count == 0;
    let expired_clear = rollout_summary.expired_pending_count == 0;
    let stale_clear = rollout_summary.stale_pending_count == 0;
    let skipped_automation_clear = latest_run.is_some_and(|run| run.skipped_count == 0);
    let manual_approval_clear = rollout_summary.manual_pending_count == 0;
    let latest_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "agent.release_orchestration_validation_run")
        .max_by_key(|log| log.created_at);
    let latest_controller_status = latest_controller_log
        .and_then(|log| {
            log.details
                .get("controller_execution")
                .and_then(|execution| execution.get("status"))
                .and_then(Value::as_str)
        })
        .map(str::to_string);
    let latest_controller_age_hours = latest_controller_log
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();

    if !automation_supervision_fresh {
        blocking_reasons.push("fresh release automation supervision is missing".to_string());
    }
    if !pending_clear {
        blocking_reasons.push("pending production release requests remain".to_string());
    }
    if !expired_clear {
        blocking_reasons.push("expired production release requests remain".to_string());
    }
    if !stale_clear {
        blocking_reasons.push("stale production release requests remain".to_string());
    }
    if !skipped_automation_clear {
        blocking_reasons.push("latest release automation skipped eligible work".to_string());
    }
    if !manual_approval_clear {
        blocking_reasons.push("manual release approval steps remain".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "agent release orchestration controller is required but not configured".to_string(),
        );
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "agent release orchestration controller evidence is missing or not validated"
                .to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons
            .push("agent release orchestration controller evidence is stale".to_string());
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
            "Agent release production orchestration is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Agent release production orchestration has fresh automation supervision, no pending releases, no stale or expired requests, and no skipped automation".to_string()
    };

    AgentReleaseProductionOrchestrationReadiness {
        status,
        production_blocked,
        automation_supervision_fresh,
        latest_run_status,
        pending_clear,
        expired_clear,
        stale_clear,
        skipped_automation_clear,
        manual_approval_clear,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        blocking_reasons,
        message,
    }
}

pub(crate) fn agent_release_automation_run_from_audit_log(
    log: &AuditLog,
) -> Option<AgentReleaseAutomationRunRecord> {
    if log.action != "agent.release_promotion_due_run" {
        return None;
    }
    let promoted_count = json_usize(&log.details, "promoted_count");
    let rejected_count = json_usize(&log.details, "rejected_count");
    let skipped_count = json_usize(&log.details, "skipped_count");
    let status = log
        .details
        .get("status")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if promoted_count > 0 || rejected_count > 0 {
                "processed".to_string()
            } else if skipped_count > 0 {
                "skipped".to_string()
            } else {
                "no_pending".to_string()
            }
        });
    Some(AgentReleaseAutomationRunRecord {
        id: log.id,
        status,
        pending_count: json_usize(&log.details, "pending_count"),
        promoted_count,
        rejected_count,
        skipped_count,
        ran_at: log.created_at,
    })
}

pub(crate) enum ReleaseAutomationDecision {
    Promote,
    Skip(String),
}

pub(crate) fn normalize_release_automation_policy(
    auto_approve: Option<bool>,
    activate_after: Option<&str>,
    expires_at: Option<&str>,
) -> Result<Value, AppError> {
    let activate_after = parse_optional_rfc3339("activate_after", activate_after)?;
    let expires_at = parse_optional_rfc3339("expires_at", expires_at)?;
    if let (Some(activate_after), Some(expires_at)) = (activate_after, expires_at)
        && activate_after >= expires_at
    {
        return Err(AppError::bad_request(
            "release automation activate_after must be before expires_at",
        ));
    }
    Ok(json!({
        "auto_approve": auto_approve.unwrap_or(false),
        "activate_after": activate_after,
        "expires_at": expires_at,
    }))
}

pub(crate) fn release_automation_due_decision(
    release: &AgentRelease,
    now: DateTime<Utc>,
) -> ReleaseAutomationDecision {
    if release
        .approver_subject
        .as_deref()
        .is_some_and(|subject| !subject.trim().is_empty() && subject.trim() != "system")
    {
        return ReleaseAutomationDecision::Skip("delegated_human_approver".to_string());
    }
    if release
        .automation_policy
        .get("auto_approve")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return ReleaseAutomationDecision::Skip("auto_approve_disabled".to_string());
    }
    if let Some(activate_after) = release_automation_time(release, "activate_after")
        && now < activate_after
    {
        return ReleaseAutomationDecision::Skip("activation_window_not_open".to_string());
    }
    let score = release.eval_score.unwrap_or(0.0);
    if score < release.min_score {
        return ReleaseAutomationDecision::Skip("eval_score_below_minimum".to_string());
    }
    ReleaseAutomationDecision::Promote
}

pub(crate) fn release_automation_is_expired(release: &AgentRelease, now: DateTime<Utc>) -> bool {
    release_automation_time(release, "expires_at").is_some_and(|expires_at| now > expires_at)
}

pub(crate) fn release_automation_time(
    release: &AgentRelease,
    field: &str,
) -> Option<DateTime<Utc>> {
    release
        .automation_policy
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

pub(crate) fn build_agent_release_rollout_summary(
    releases: Vec<AgentRelease>,
    now: DateTime<Utc>,
) -> AgentReleaseRolloutSummary {
    let mut by_status = BTreeMap::new();
    let mut by_environment = BTreeMap::new();
    let mut latest_promoted = BTreeMap::<String, AgentReleaseLatestPromotion>::new();
    let mut attention_items = Vec::new();
    let mut pending_count = 0usize;
    let mut promoted_count = 0usize;
    let mut rejected_count = 0usize;
    let mut rolled_back_count = 0usize;
    let mut auto_pending_count = 0usize;
    let mut manual_pending_count = 0usize;
    let mut expired_pending_count = 0usize;
    let mut expiring_soon_count = 0usize;
    let mut stale_pending_count = 0usize;
    let expiring_soon_cutoff = now + chrono::Duration::hours(24);
    let stale_cutoff = now - chrono::Duration::hours(24);

    for release in &releases {
        *by_status.entry(release.status.clone()).or_insert(0) += 1;
        *by_environment
            .entry(release.environment.clone())
            .or_insert(0) += 1;

        match release.status.as_str() {
            "pending_approval" | "promotion_in_progress" | "promotion_failed" => {
                pending_count += 1;
                let auto_approve = release
                    .automation_policy
                    .get("auto_approve")
                    .and_then(Value::as_bool)
                    == Some(true);
                if auto_approve {
                    auto_pending_count += 1;
                } else {
                    manual_pending_count += 1;
                }
                let activate_after = release_automation_time(release, "activate_after");
                let expires_at = release_automation_time(release, "expires_at");
                let mut reasons = Vec::new();
                if release.status == "promotion_in_progress" {
                    reasons.push("promotion_reconciliation_in_progress".to_string());
                } else {
                    if release.status == "promotion_failed" {
                        reasons.push("promotion_failed_retryable".to_string());
                    }
                    if expires_at.is_some_and(|expires_at| now > expires_at) {
                        expired_pending_count += 1;
                        reasons.push("expired_pending".to_string());
                    } else if expires_at
                        .is_some_and(|expires_at| expires_at <= expiring_soon_cutoff)
                    {
                        expiring_soon_count += 1;
                        reasons.push("expiring_soon".to_string());
                    }
                }
                if release
                    .requested_at
                    .is_some_and(|requested_at| requested_at < stale_cutoff)
                {
                    stale_pending_count += 1;
                    reasons.push("stale_pending".to_string());
                }
                if release.status != "promotion_in_progress" {
                    match release_automation_due_decision(release, now) {
                        ReleaseAutomationDecision::Promote => {
                            reasons.push("automation_ready".to_string());
                        }
                        ReleaseAutomationDecision::Skip(reason) => {
                            reasons.push(reason);
                        }
                    }
                }
                attention_items.push(AgentReleaseAttentionItem {
                    release_id: release.id,
                    agent_id: release.agent_id,
                    agent_version_id: release.agent_version_id,
                    environment: release.environment.clone(),
                    status: release.status.clone(),
                    reason: reasons.join(","),
                    requested_by: release.requested_by.clone(),
                    approver_subject: release.approver_subject.clone(),
                    requested_at: release.requested_at,
                    activate_after,
                    expires_at,
                    eval_score: release.eval_score,
                    min_score: release.min_score,
                });
            }
            "promoted" => {
                promoted_count += 1;
                if let Some(promoted_at) = release.promoted_at {
                    let candidate = AgentReleaseLatestPromotion {
                        environment: release.environment.clone(),
                        release_id: release.id,
                        agent_id: release.agent_id,
                        agent_version_id: release.agent_version_id,
                        promoted_at,
                        eval_score: release.eval_score,
                    };
                    let should_replace = latest_promoted
                        .get(&release.environment)
                        .is_none_or(|existing| existing.promoted_at < candidate.promoted_at);
                    if should_replace {
                        latest_promoted.insert(release.environment.clone(), candidate);
                    }
                }
            }
            "rejected" => rejected_count += 1,
            "rolled_back" => rolled_back_count += 1,
            _ => {}
        }
    }

    attention_items.sort_by(|left, right| {
        attention_priority(&left.reason)
            .cmp(&attention_priority(&right.reason))
            .then_with(|| left.environment.cmp(&right.environment))
            .then_with(|| left.release_id.cmp(&right.release_id))
    });

    AgentReleaseRolloutSummary {
        generated_at: now,
        release_count: releases.len(),
        by_status,
        by_environment,
        pending_count,
        promoted_count,
        rejected_count,
        rolled_back_count,
        auto_pending_count,
        manual_pending_count,
        expired_pending_count,
        expiring_soon_count,
        stale_pending_count,
        latest_promoted_by_environment: latest_promoted.into_values().collect(),
        attention_items,
    }
}

pub(crate) fn attention_priority(reason: &str) -> usize {
    if reason.contains("expired_pending") {
        0
    } else if reason.contains("eval_score_below_minimum") {
        1
    } else if reason.contains("stale_pending") {
        2
    } else if reason.contains("expiring_soon") {
        3
    } else if reason.contains("automation_ready") {
        4
    } else {
        5
    }
}
