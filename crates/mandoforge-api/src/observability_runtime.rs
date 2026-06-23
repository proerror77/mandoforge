use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

use crate::*;

pub(crate) fn observability_collector_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn observability_collector_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_observability_collector_deployment_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    config: &ObservabilityConfig,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let collector_endpoint = config
        .otlp_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string());
    let signal_paths = collector_endpoint
        .as_ref()
        .map(|endpoint| {
            json!({
                "logs": format!("{endpoint}/v1/logs"),
                "traces": format!("{endpoint}/v1/traces"),
                "metrics": format!("{endpoint}/v1/metrics")
            })
        })
        .unwrap_or_else(|| json!({}));
    let payload = json!({
        "type": "mandoforge.observability_collector_deployment",
        "subject": subject,
        "checked_at": checked_at,
        "service_name": config.service_name.clone(),
        "otlp_endpoint": collector_endpoint,
        "collector_health_endpoint": config.collector_health_endpoint.clone(),
        "sample_ratio": config.sample_ratio,
        "signal_paths": signal_paths,
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
            "observability collector deployment controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
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

pub(crate) fn observability_collector_cluster_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn observability_collector_cluster_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_observability_collector_cluster_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    config: &ObservabilityConfig,
    deployment_readiness: &ObservabilityCollectorDeploymentReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.observability_collector_cluster_rollout",
        "subject": subject,
        "checked_at": checked_at,
        "service_name": config.service_name,
        "otlp_enabled": config.is_enabled(),
        "otlp_endpoint_configured": config.otlp_endpoint.is_some(),
        "sample_ratio": config.sample_ratio,
        "deployment_readiness": {
            "status": deployment_readiness.status,
            "deployment_validated": deployment_readiness.deployment_validated,
            "latest_validation_status": deployment_readiness.latest_validation_status,
            "latest_controller_status": deployment_readiness.latest_controller_status,
        },
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
            "collector cluster rollout controller failed with status {http_status}"
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
        "cluster_rollout_id": body.get("cluster_rollout_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn build_observability_remediation_plan(
    summary: ObservabilitySummary,
) -> ObservabilityRemediationPlan {
    let mut actions = Vec::new();
    let backpressure = summary.backpressure;

    if !summary.telemetry.otlp_enabled {
        actions.push(ObservabilityRemediationPlanAction {
            action: "configure_otlp_exporter".to_string(),
            mode: "configuration".to_string(),
            severity: "warning".to_string(),
            reason: "OTLP export is disabled; traces, metrics, and logs stay local".to_string(),
        });
    } else if !summary.telemetry.endpoint_configured {
        actions.push(ObservabilityRemediationPlanAction {
            action: "configure_otlp_endpoint".to_string(),
            mode: "configuration".to_string(),
            severity: "warning".to_string(),
            reason: "OTLP export is enabled without a configured collector endpoint".to_string(),
        });
    }
    if backpressure.pending_approvals > 0 {
        actions.push(ObservabilityRemediationPlanAction {
            action: "approval_escalation_due_run".to_string(),
            mode: "auto".to_string(),
            severity: "warning".to_string(),
            reason: format!(
                "{} pending approvals can be processed through due escalation automation",
                backpressure.pending_approvals
            ),
        });
    }
    if backpressure.queued_jobs > 0 || backpressure.retryable_jobs > 0 {
        actions.push(ObservabilityRemediationPlanAction {
            action: "worker_drain_required".to_string(),
            mode: "manual".to_string(),
            severity: "warning".to_string(),
            reason: format!(
                "{} queued and {} retryable jobs require worker drain supervision",
                backpressure.queued_jobs, backpressure.retryable_jobs
            ),
        });
    }
    if backpressure
        .oldest_queued_job_age_seconds
        .is_some_and(|age| age >= 300)
    {
        actions.push(ObservabilityRemediationPlanAction {
            action: "queue_age_triage_required".to_string(),
            mode: "manual".to_string(),
            severity: "critical".to_string(),
            reason: "oldest queued job is at least five minutes old".to_string(),
        });
    }
    if backpressure.failed_jobs > 0
        || backpressure.failed_sessions > 0
        || backpressure.failed_tool_calls > 0
    {
        actions.push(ObservabilityRemediationPlanAction {
            action: "manual_failure_triage_required".to_string(),
            mode: "manual".to_string(),
            severity: "critical".to_string(),
            reason: format!(
                "{} failed jobs, {} failed sessions, and {} failed tool calls need human triage",
                backpressure.failed_jobs,
                backpressure.failed_sessions,
                backpressure.failed_tool_calls
            ),
        });
    }
    if !summary.recent_error_events.is_empty() {
        actions.push(ObservabilityRemediationPlanAction {
            action: "recent_error_review_required".to_string(),
            mode: "manual".to_string(),
            severity: "warning".to_string(),
            reason: format!(
                "{} recent error events should be reviewed before declaring the runtime healthy",
                summary.recent_error_events.len()
            ),
        });
    }

    let auto_action_count = actions
        .iter()
        .filter(|action| action.mode == "auto")
        .count();
    let manual_action_count = actions
        .iter()
        .filter(|action| action.mode == "manual")
        .count();
    let configuration_action_count = actions
        .iter()
        .filter(|action| action.mode == "configuration")
        .count();
    let status = if actions.iter().any(|action| action.severity == "critical") {
        "critical"
    } else if actions.is_empty() {
        "clean"
    } else {
        "attention"
    }
    .to_string();

    ObservabilityRemediationPlan {
        status,
        generated_at: summary.generated_at,
        auto_action_count,
        manual_action_count,
        configuration_action_count,
        backpressure,
        actions,
    }
}

pub(crate) async fn execute_observability_remediation_with_lookup<F>(
    state: &AppState,
    lookup: F,
) -> Result<ObservabilityRemediationRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let before_summary = build_observability_summary(state).await?;
    let before = before_summary.backpressure;
    let mut actions = Vec::new();
    let controller_configured = observability_remediation_controller_configured(&lookup);
    let approval_escalation_run = if before.pending_approvals > 0 {
        actions.push("approval_escalation_due_run".to_string());
        Some(execute_due_approval_escalations(state).await?)
    } else {
        None
    };
    let codex_app_server_stale_polls = execute_stale_codex_app_server_polls(
        state,
        CodexAppServerStalePollRequest::default(),
        "system",
        "observability",
    )
    .await?;
    if codex_app_server_stale_polls.polled_count > 0
        || codex_app_server_stale_polls.failed_count > 0
    {
        actions.push("codex_app_server_stale_poll_due_run".to_string());
    }
    if before.queued_jobs > 0 || before.retryable_jobs > 0 {
        actions.push("worker_drain_required".to_string());
    }
    if before.failed_jobs > 0 || before.failed_sessions > 0 || before.failed_tool_calls > 0 {
        actions.push("manual_failure_triage_required".to_string());
    }
    let after_summary = build_observability_summary(state).await?;
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "no_remediation_actions"
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured && !actions.is_empty() {
        match execute_observability_remediation_controller(
            &lookup,
            &before,
            &after_summary.backpressure,
            &actions,
        )
        .await
        {
            Ok(execution) => {
                let execution_status = execution
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed")
                    .to_string();
                controller_execution = execution;
                actions.push("observability_remediation_controller_executed".to_string());
                if execution_status != "remediated" {
                    actions.push("observability_remediation_controller_attention".to_string());
                }
            }
            Err(error) => {
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
                actions.push("observability_remediation_controller_failed".to_string());
            }
        }
    }
    let run = ObservabilityRemediationRun {
        status: if actions.is_empty() {
            "no_action".to_string()
        } else if controller_execution.get("status").and_then(Value::as_str) == Some("failed") {
            "attention".to_string()
        } else {
            "completed".to_string()
        },
        ran_at: Utc::now(),
        actions,
        before,
        after: after_summary.backpressure,
        approval_escalation_run,
        codex_app_server_stale_polls: Some(codex_app_server_stale_polls),
        controller_configured,
        controller_execution,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "observability.remediation_run",
            "observability",
            None,
            serde_json::to_value(&run)?,
        ))
        .await?;
    Ok(run)
}

pub(crate) fn observability_remediation_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_observability_remediation_controller<F>(
    lookup: &F,
    before: &ObservabilityBackpressure,
    after: &ObservabilityBackpressure,
    actions: &[String],
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_OBSERVABILITY_REMEDIATION_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.observability_remediation",
        "actions": actions,
        "before": before,
        "after": after,
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
            "observability remediation controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("remediated");
    let remediated = matches!(
        controller_status,
        "remediated" | "success" | "ok" | "validated"
    );
    Ok(json!({
        "attempted": true,
        "status": if remediated { "remediated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "remediation_id": body.get("remediation_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn build_observability_collector_readiness(
    state: &AppState,
) -> ObservabilityCollectorReadiness {
    let generated_at = Utc::now();
    let config = &state.observability_config;
    let endpoint = config
        .otlp_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string());
    let endpoint_configured = endpoint.is_some();
    let mut attention_items = Vec::new();
    if !config.is_enabled() {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "otlp_disabled".to_string(),
            severity: "warning".to_string(),
            message: "OTLP export is disabled; collector readiness cannot be proven".to_string(),
        });
    }
    if config.is_enabled() && !endpoint_configured {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "otlp_endpoint_missing".to_string(),
            severity: "critical".to_string(),
            message: "OTLP export is enabled without a collector endpoint".to_string(),
        });
    }
    if config.sample_ratio <= 0.0 {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "telemetry_sampling_zero".to_string(),
            severity: "warning".to_string(),
            message: "telemetry sample ratio is zero; runtime events will not be exported"
                .to_string(),
        });
    }
    let health_check = if config.is_enabled() && endpoint_configured {
        match state.telemetry_exporter.health_check(config).await {
            Ok(()) => ObservabilityCollectorHealthCheck {
                status: "healthy".to_string(),
                checked: true,
                message: "collector health check succeeded".to_string(),
            },
            Err(error) => {
                attention_items.push(ObservabilityCollectorAttentionItem {
                    kind: "collector_health_failed".to_string(),
                    severity: "critical".to_string(),
                    message: error.message.clone(),
                });
                ObservabilityCollectorHealthCheck {
                    status: "failed".to_string(),
                    checked: true,
                    message: error.message,
                }
            }
        }
    } else {
        ObservabilityCollectorHealthCheck {
            status: "skipped".to_string(),
            checked: false,
            message: "collector health check skipped because OTLP endpoint is not configured"
                .to_string(),
        }
    };
    let signal_paths = ["logs", "traces", "metrics"]
        .into_iter()
        .map(|signal| ObservabilityCollectorSignalPath {
            signal: signal.to_string(),
            url: endpoint.as_ref().map(|endpoint| match signal {
                "logs" => format!("{endpoint}/v1/logs"),
                "traces" => format!("{endpoint}/v1/traces"),
                "metrics" => format!("{endpoint}/v1/metrics"),
                _ => endpoint.clone(),
            }),
            status: if endpoint_configured {
                "configured".to_string()
            } else {
                "missing".to_string()
            },
        })
        .collect::<Vec<_>>();
    let production_ops = build_observability_collector_production_ops(
        config.is_enabled(),
        endpoint_configured,
        config.sample_ratio,
        &health_check,
        &signal_paths,
    );
    let audit_logs = state.list_audit_logs(None).await.unwrap_or_default();
    let deployment_readiness = build_observability_collector_deployment_readiness(
        config.is_enabled(),
        endpoint_configured,
        observability_collector_deployment_controller_required(&|key| std::env::var(key).ok()),
        observability_collector_deployment_controller_configured(&|key| std::env::var(key).ok()),
        &audit_logs,
        generated_at,
    );
    let cluster_rollout = build_observability_collector_cluster_rollout_readiness(
        &deployment_readiness,
        &audit_logs,
        generated_at,
        observability_collector_cluster_controller_required(&|key| std::env::var(key).ok()),
        observability_collector_cluster_controller_configured(&|key| std::env::var(key).ok()),
    );
    let remediation_supervision = build_observability_remediation_supervision_readiness(
        observability_remediation_controller_required(&|key| std::env::var(key).ok()),
        observability_remediation_controller_configured(&|key| std::env::var(key).ok()),
        &audit_logs,
        generated_at,
    );
    if production_ops.production_blocked {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "collector_production_ops_blocked".to_string(),
            severity: if production_ops.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_ops.message.clone(),
        });
    }
    if deployment_readiness.production_blocked {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "collector_deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
        });
    }
    if remediation_supervision.production_blocked {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "observability_remediation_supervision_blocked".to_string(),
            severity: "critical".to_string(),
            message: remediation_supervision.message.clone(),
        });
    }
    if cluster_rollout.production_blocked {
        attention_items.push(ObservabilityCollectorAttentionItem {
            kind: "collector_cluster_rollout_blocked".to_string(),
            severity: "critical".to_string(),
            message: cluster_rollout.message.clone(),
        });
    }
    let status = if attention_items
        .iter()
        .any(|item| item.severity == "critical")
    {
        "failed"
    } else if attention_items.is_empty() {
        "ready"
    } else {
        "warning"
    }
    .to_string();
    ObservabilityCollectorReadiness {
        generated_at,
        status,
        service_name: config.service_name.clone(),
        otlp_enabled: config.is_enabled(),
        endpoint_configured,
        endpoint,
        sample_ratio: config.sample_ratio,
        health_check,
        signal_paths,
        production_ops,
        deployment_readiness,
        cluster_rollout,
        remediation_supervision,
        attention_items,
    }
}

pub(crate) fn observability_remediation_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn build_observability_collector_production_ops(
    otlp_enabled: bool,
    endpoint_configured: bool,
    sample_ratio: f64,
    health_check: &ObservabilityCollectorHealthCheck,
    signal_paths: &[ObservabilityCollectorSignalPath],
) -> ObservabilityCollectorProductionOpsReadiness {
    let signal_path_count = signal_paths.len();
    let configured_signal_path_count = signal_paths
        .iter()
        .filter(|path| path.status == "configured" && path.url.is_some())
        .count();
    let (status, production_blocked, message) = if !otlp_enabled {
        (
            "blocked",
            true,
            "Observability production ops are blocked until OTLP export is enabled".to_string(),
        )
    } else if !endpoint_configured {
        (
            "blocked",
            true,
            "Observability production ops are blocked until an OTLP collector endpoint is configured"
                .to_string(),
        )
    } else if configured_signal_path_count < 3 {
        (
            "blocked",
            true,
            "Observability production ops are blocked until logs, traces, and metrics paths are configured"
                .to_string(),
        )
    } else if sample_ratio <= 0.0 {
        (
            "blocked",
            true,
            "Observability production ops are blocked because telemetry sampling is disabled"
                .to_string(),
        )
    } else if !health_check.checked {
        (
            "blocked",
            true,
            "Observability production ops are blocked until collector health is checked"
                .to_string(),
        )
    } else if health_check.status != "healthy" {
        (
            "blocked",
            true,
            "Observability production ops are blocked by a failed collector health check"
                .to_string(),
        )
    } else {
        (
            "ready",
            false,
            "Observability collector is healthy and logs, traces, and metrics paths are configured"
                .to_string(),
        )
    };
    ObservabilityCollectorProductionOpsReadiness {
        status: status.to_string(),
        production_blocked,
        signal_path_count,
        configured_signal_path_count,
        health_checked: health_check.checked,
        health_status: health_check.status.clone(),
        message,
    }
}

pub(crate) fn build_observability_collector_deployment_readiness(
    otlp_enabled: bool,
    endpoint_configured: bool,
    controller_required: bool,
    controller_configured: bool,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> ObservabilityCollectorDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "observability.collector_deployment_validation")
        .max_by_key(|log| log.created_at);
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_age_hours =
        latest_validation_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_validation_healthy = latest_validation
        .and_then(|log| log.details.get("healthy"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let latest_controller_status = latest_validation
        .and_then(|log| log.details["controller_execution"]["status"].as_str())
        .map(str::to_string);
    let latest_controller_age_hours = latest_validation
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let latest_controller_validated = latest_controller_status
        .as_deref()
        .map(|status| status == "validated")
        .unwrap_or(false);
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let mut blocking_reasons = Vec::new();

    if !otlp_enabled {
        blocking_reasons.push("OTLP export is disabled".to_string());
    }
    if !endpoint_configured {
        blocking_reasons.push("collector endpoint is not configured".to_string());
    }
    if latest_validation.is_none() {
        blocking_reasons.push("collector deployment validation has not run".to_string());
    }
    if latest_validation.is_some() && !latest_validation_healthy {
        blocking_reasons.push("latest collector deployment validation was not healthy".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("collector deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("collector deployment controller is required but not configured".to_string());
    }
    if controller_required && latest_validation.is_some() && !latest_controller_validated {
        blocking_reasons
            .push("latest collector deployment controller execution was not validated".to_string());
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("collector deployment controller evidence is stale".to_string());
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
            "Observability collector deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Observability collector deployment has a recent healthy validation run".to_string()
    };

    ObservabilityCollectorDeploymentReadiness {
        status,
        production_blocked,
        otlp_enabled,
        endpoint_configured,
        controller_required,
        controller_configured,
        deployment_validated: latest_validation_healthy && !production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        latest_validation_healthy,
        latest_controller_status,
        latest_controller_age_hours,
        latest_controller_validated,
        controller_evidence_fresh,
        blocking_reasons,
        message,
    }
}

pub(crate) fn build_observability_collector_cluster_rollout_readiness(
    deployment_readiness: &ObservabilityCollectorDeploymentReadiness,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> ObservabilityCollectorClusterRolloutReadiness {
    let latest_rollout = audit_logs
        .iter()
        .filter(|log| log.action == "observability.collector_cluster_rollout_validation")
        .max_by_key(|log| log.created_at);
    let latest_rollout_at = latest_rollout.map(|log| log.created_at);
    let latest_rollout_status = latest_rollout
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_status = latest_rollout
        .and_then(|log| log.details.get("controller_execution"))
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_age_hours = latest_rollout
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let mut blocking_reasons = Vec::new();
    if !deployment_readiness.deployment_validated {
        blocking_reasons.push("collector deployment validation is not ready".to_string());
    }
    if latest_rollout.is_none() {
        blocking_reasons.push("collector cluster rollout validation has not run".to_string());
    }
    if latest_rollout_at.is_some_and(|created_at| (generated_at - created_at).num_hours() >= 24) {
        blocking_reasons.push("collector cluster rollout validation evidence is stale".to_string());
    }
    if latest_rollout_status.as_deref() != Some("validated") {
        blocking_reasons
            .push("latest collector cluster rollout validation was not validated".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "collector cluster rollout controller is required but not configured".to_string(),
        );
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "collector cluster rollout controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("collector cluster rollout controller evidence is stale".to_string());
    }
    let production_blocked = controller_required && !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else if blocking_reasons.is_empty() {
        "ready"
    } else {
        "optional"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Observability collector cluster rollout is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else if blocking_reasons.is_empty() && controller_required {
        "Observability collector cluster rollout has validated deployment and recent cluster-controller evidence".to_string()
    } else if blocking_reasons.is_empty() {
        "Observability collector cluster rollout has recent validation evidence".to_string()
    } else {
        format!(
            "Observability collector cluster rollout is optional until required: {}",
            blocking_reasons.join("; ")
        )
    };
    ObservabilityCollectorClusterRolloutReadiness {
        status,
        production_blocked,
        controller_required,
        controller_configured,
        latest_rollout_at,
        latest_rollout_status,
        latest_controller_status,
        latest_controller_age_hours,
        latest_controller_validated,
        controller_evidence_fresh,
        deployment_validated: deployment_readiness.deployment_validated,
        blocking_reasons,
        message,
    }
}

pub(crate) fn build_observability_remediation_supervision_readiness(
    required: bool,
    controller_configured: bool,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> ObservabilityRemediationSupervisionReadiness {
    let latest_controller_run = audit_logs
        .iter()
        .filter(|log| {
            log.action == "observability.remediation_run"
                && log.details["controller_execution"]["attempted"] == true
        })
        .max_by_key(|log| log.created_at);
    let latest_controller_run_at = latest_controller_run.map(|log| log.created_at);
    let latest_controller_run_age_hours =
        latest_controller_run_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_controller_status = latest_controller_run
        .and_then(|log| log.details["controller_execution"]["status"].as_str())
        .map(str::to_string);
    let latest_controller_remediated = latest_controller_status
        .as_deref()
        .map(|status| status == "remediated")
        .unwrap_or(false);
    let mut blocking_reasons = Vec::new();

    if required && !controller_configured {
        blocking_reasons.push("remediation controller is required but not configured".to_string());
    }
    if required && latest_controller_run.is_none() {
        blocking_reasons
            .push("remediation controller has no audited execution evidence".to_string());
    }
    if required && latest_controller_run.is_some() && !latest_controller_remediated {
        blocking_reasons
            .push("latest remediation controller execution did not remediate".to_string());
    }
    if required && latest_controller_run_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("remediation controller execution evidence is stale".to_string());
    }

    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else if required {
        "ready"
    } else if controller_configured {
        "configured"
    } else {
        "optional"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Observability remediation supervision is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else if required {
        "Observability remediation controller has recent audited remediation evidence".to_string()
    } else if controller_configured {
        "Observability remediation controller is configured but not required for this runtime"
            .to_string()
    } else {
        "Observability remediation controller is optional for this runtime".to_string()
    };

    ObservabilityRemediationSupervisionReadiness {
        status,
        production_blocked,
        required,
        controller_configured,
        latest_controller_run_at,
        latest_controller_run_age_hours,
        latest_controller_status,
        latest_controller_remediated,
        blocking_reasons,
        message,
    }
}

pub(crate) async fn execute_cost_alert_delivery(
    state: &AppState,
    delivered_at: DateTime<Utc>,
) -> Result<CostAlertDelivery, AppError> {
    let summary = build_usage_summary(state).await?;
    let alerts = build_cost_alerts(&summary.provider_budgets, delivered_at);
    if alerts.is_empty() {
        let delivery = CostAlertDelivery {
            status: "no_alerts".to_string(),
            delivered: false,
            channel: "webhook".to_string(),
            webhook_configured: state.cost_alert_webhook_url.is_some(),
            alerts,
            route_deliveries: vec![],
            delivered_at,
        };
        audit_cost_alert_delivery(state, &delivery).await?;
        return Ok(delivery);
    }
    let routes: Vec<_> = state
        .list_cost_alert_routes()
        .await?
        .into_iter()
        .filter(|route| route.status == "active")
        .collect();
    if !routes.is_empty() {
        let mut route_deliveries = Vec::new();
        for route in routes {
            route_deliveries
                .push(deliver_cost_alert_route(state, &route, &alerts, delivered_at).await?);
        }
        let delivered = route_deliveries.iter().any(|delivery| delivery.delivered);
        let delivery = CostAlertDelivery {
            status: if delivered { "delivered" } else { "reserved" }.to_string(),
            delivered,
            channel: "routes".to_string(),
            webhook_configured: state.cost_alert_webhook_url.is_some(),
            alerts,
            route_deliveries,
            delivered_at,
        };
        audit_cost_alert_delivery(state, &delivery).await?;
        return Ok(delivery);
    }
    let Some(webhook_url) = state.cost_alert_webhook_url.as_ref() else {
        let delivery = CostAlertDelivery {
            status: "reserved".to_string(),
            delivered: false,
            channel: "webhook".to_string(),
            webhook_configured: false,
            alerts,
            route_deliveries: vec![],
            delivered_at,
        };
        audit_cost_alert_delivery(state, &delivery).await?;
        return Ok(delivery);
    };
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        reqwest::Client::new()
            .post(webhook_url)
            .json(&json!({
                "type": "mandoforge.cost_alerts",
                "alerts": alerts,
                "delivered_at": delivered_at,
            }))
            .send(),
    )
    .await??;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "cost alert webhook returned status {}",
            response.status()
        )));
    }
    let alert_count = alerts.len();
    let delivery = CostAlertDelivery {
        status: "delivered".to_string(),
        delivered: true,
        channel: "webhook".to_string(),
        webhook_configured: true,
        alerts,
        route_deliveries: vec![CostAlertRouteDelivery {
            route_id: None,
            route_name: "default-webhook".to_string(),
            channel: "webhook".to_string(),
            status: "delivered".to_string(),
            delivered: true,
            matched_alert_count: alert_count,
            target: Some(webhook_url.clone()),
        }],
        delivered_at,
    };
    audit_cost_alert_delivery(state, &delivery).await?;
    Ok(delivery)
}

pub(crate) async fn audit_cost_alert_delivery(
    state: &AppState,
    delivery: &CostAlertDelivery,
) -> Result<(), AppError> {
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.cost_alerts_delivered",
            "usage_alert",
            None,
            json!({
                "status": delivery.status,
                "delivered": delivery.delivered,
                "channel": delivery.channel,
                "webhook_configured": delivery.webhook_configured,
                "alert_count": delivery.alerts.len(),
                "route_delivery_count": delivery.route_deliveries.len(),
                "delivered_at": delivery.delivered_at,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn deliver_cost_alert_route(
    state: &AppState,
    route: &CostAlertRoute,
    alerts: &[CostAlert],
    delivered_at: DateTime<Utc>,
) -> Result<CostAlertRouteDelivery, AppError> {
    let matched_alerts: Vec<_> = alerts
        .iter()
        .filter(|alert| severity_rank(&alert.severity) >= severity_rank(&route.severity_filter))
        .collect();
    if matched_alerts.is_empty() {
        return Ok(CostAlertRouteDelivery {
            route_id: Some(route.id),
            route_name: route.name.clone(),
            channel: route.channel.clone(),
            status: "no_matching_alerts".to_string(),
            delivered: false,
            matched_alert_count: 0,
            target: route.target.clone(),
        });
    }
    if route.channel == "email" && state.cost_alert_email_relay_url.is_none() {
        let Some(smtp_config) = state.cost_alert_smtp_config.as_ref() else {
            return Ok(CostAlertRouteDelivery {
                route_id: Some(route.id),
                route_name: route.name.clone(),
                channel: route.channel.clone(),
                status: "reserved".to_string(),
                delivered: false,
                matched_alert_count: matched_alerts.len(),
                target: route.target.clone(),
            });
        };
        deliver_cost_alert_email_smtp(smtp_config, route, &matched_alerts, delivered_at).await?;
        return Ok(CostAlertRouteDelivery {
            route_id: Some(route.id),
            route_name: route.name.clone(),
            channel: route.channel.clone(),
            status: "delivered".to_string(),
            delivered: true,
            matched_alert_count: matched_alerts.len(),
            target: route.target.clone(),
        });
    }
    let webhook_url = match route.channel.as_str() {
        "webhook" => route
            .target
            .as_ref()
            .or(state.cost_alert_webhook_url.as_ref())
            .ok_or_else(|| AppError::bad_request("webhook cost alert route requires a target"))?,
        "slack" => route
            .target
            .as_ref()
            .ok_or_else(|| AppError::bad_request("slack cost alert route requires a target"))?,
        "email" => {
            let Some(relay_url) = state.cost_alert_email_relay_url.as_ref() else {
                return Ok(CostAlertRouteDelivery {
                    route_id: Some(route.id),
                    route_name: route.name.clone(),
                    channel: route.channel.clone(),
                    status: "reserved".to_string(),
                    delivered: false,
                    matched_alert_count: matched_alerts.len(),
                    target: route.target.clone(),
                });
            };
            relay_url
        }
        other => {
            return Ok(CostAlertRouteDelivery {
                route_id: Some(route.id),
                route_name: route.name.clone(),
                channel: other.to_string(),
                status: "reserved".to_string(),
                delivered: false,
                matched_alert_count: matched_alerts.len(),
                target: route.target.clone(),
            });
        }
    };
    let payload = match route.channel.as_str() {
        "slack" => slack_cost_alert_payload(route, &matched_alerts, delivered_at),
        "email" => email_cost_alert_payload(route, &matched_alerts, delivered_at)?,
        _ => json!({
            "type": "mandoforge.cost_alerts",
            "route_id": route.id,
            "route_name": route.name,
            "alerts": matched_alerts,
            "delivered_at": delivered_at,
        }),
    };
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        reqwest::Client::new()
            .post(webhook_url)
            .json(&payload)
            .send(),
    )
    .await??;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "cost alert route {} returned status {}",
            route.name,
            response.status()
        )));
    }
    Ok(CostAlertRouteDelivery {
        route_id: Some(route.id),
        route_name: route.name.clone(),
        channel: route.channel.clone(),
        status: "delivered".to_string(),
        delivered: true,
        matched_alert_count: matched_alerts.len(),
        target: Some(webhook_url.clone()),
    })
}

pub(crate) fn slack_cost_alert_payload(
    route: &CostAlertRoute,
    alerts: &[&CostAlert],
    delivered_at: DateTime<Utc>,
) -> Value {
    let title = format!(
        "MandoForge cost alert: {} matching {} route",
        alerts.len(),
        route.severity_filter
    );
    let lines: Vec<_> = alerts
        .iter()
        .map(|alert| {
            format!(
                "*{}* `{}`: {}",
                alert.severity, alert.provider_name, alert.message
            )
        })
        .collect();
    json!({
        "text": format!("{title}\n{}", lines.join("\n")),
        "blocks": [
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*{}*\n{}", title, lines.join("\n"))
                }
            },
            {
                "type": "context",
                "elements": [
                    {
                        "type": "mrkdwn",
                        "text": format!("route `{}` · delivered {}", route.name, delivered_at)
                    }
                ]
            }
        ]
    })
}

pub(crate) async fn deliver_cost_alert_email_smtp(
    config: &CostAlertSmtpConfig,
    route: &CostAlertRoute,
    alerts: &[&CostAlert],
    delivered_at: DateTime<Utc>,
) -> Result<(), AppError> {
    let email = cost_alert_email_message(config, route, alerts, delivered_at)?;
    let mut stream =
        tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&config.addr)).await??;
    smtp_expect(&mut stream, 220).await?;
    smtp_command(
        &mut stream,
        format!("EHLO {}\r\n", smtp_sanitize_header(&config.helo_domain)),
        250,
    )
    .await?;
    smtp_command(
        &mut stream,
        format!("MAIL FROM:<{}>\r\n", smtp_sanitize_addr(&config.from)?),
        250,
    )
    .await?;
    smtp_command(
        &mut stream,
        format!("RCPT TO:<{}>\r\n", smtp_sanitize_addr(&email.to)?),
        250,
    )
    .await?;
    smtp_command(&mut stream, "DATA\r\n".to_string(), 354).await?;
    tokio::time::timeout(
        Duration::from_secs(10),
        stream.write_all(email.raw.as_bytes()),
    )
    .await??;
    smtp_expect(&mut stream, 250).await?;
    let _ = smtp_command(&mut stream, "QUIT\r\n".to_string(), 221).await;
    Ok(())
}

struct CostAlertEmailMessage {
    to: String,
    raw: String,
}

fn cost_alert_email_message(
    config: &CostAlertSmtpConfig,
    route: &CostAlertRoute,
    alerts: &[&CostAlert],
    delivered_at: DateTime<Utc>,
) -> Result<CostAlertEmailMessage, AppError> {
    let (to, subject, body) = cost_alert_email_parts(route, alerts)?;
    let from = smtp_sanitize_addr(&config.from)?;
    let to_addr = smtp_sanitize_addr(&to)?;
    let subject = smtp_sanitize_header(&subject);
    let body = smtp_escape_body(&body);
    Ok(CostAlertEmailMessage {
        to,
        raw: format!(
            "From: <{from}>\r\nTo: <{to_addr}>\r\nSubject: {subject}\r\nDate: {delivered_at}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n.\r\n"
        ),
    })
}

pub(crate) async fn smtp_command(
    stream: &mut TcpStream,
    command: String,
    expected_code: u16,
) -> Result<(), AppError> {
    tokio::time::timeout(
        Duration::from_secs(10),
        stream.write_all(command.as_bytes()),
    )
    .await??;
    smtp_expect(stream, expected_code).await
}

pub(crate) async fn smtp_expect(
    stream: &mut TcpStream,
    expected_code: u16,
) -> Result<(), AppError> {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        let read =
            tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut line)).await??;
        if read == 0 {
            return Err(AppError::bad_request("SMTP server closed connection"));
        }
        if line.len() < 4 {
            return Err(AppError::bad_request(format!(
                "SMTP server returned malformed response: {}",
                line.trim_end()
            )));
        }
        let code: u16 = line[0..3].parse().map_err(|_| {
            AppError::bad_request(format!(
                "SMTP server returned non-numeric response: {}",
                line.trim_end()
            ))
        })?;
        if code != expected_code {
            return Err(AppError::bad_request(format!(
                "SMTP server returned {}, expected {}: {}",
                code,
                expected_code,
                line.trim_end()
            )));
        }
        if line.as_bytes().get(3) != Some(&b'-') {
            return Ok(());
        }
    }
}

pub(crate) fn smtp_sanitize_addr(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty()
        || value.contains(['\r', '\n', '<', '>'])
        || value.contains(' ')
        || !value.contains('@')
    {
        return Err(AppError::bad_request("SMTP email address is invalid"));
    }
    Ok(value.to_string())
}

pub(crate) fn smtp_sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

pub(crate) fn smtp_escape_body(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| {
            if line.starts_with('.') {
                format!(".{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

pub(crate) fn email_cost_alert_payload(
    route: &CostAlertRoute,
    alerts: &[&CostAlert],
    delivered_at: DateTime<Utc>,
) -> Result<Value, AppError> {
    let (to, subject, body) = cost_alert_email_parts(route, alerts)?;
    Ok(json!({
        "type": "mandoforge.cost_alert_email",
        "to": to,
        "subject": subject,
        "text": body,
        "route_id": route.id,
        "route_name": route.name,
        "severity_filter": route.severity_filter,
        "delivered_at": delivered_at,
    }))
}

pub(crate) fn cost_alert_email_parts(
    route: &CostAlertRoute,
    alerts: &[&CostAlert],
) -> Result<(String, String, String), AppError> {
    let Some(to) = route.target.as_ref() else {
        return Err(AppError::bad_request(
            "email cost alert route requires a recipient target",
        ));
    };
    let subject = format!(
        "MandoForge cost alert: {} {} alerts",
        alerts.len(),
        route.severity_filter
    );
    let body = alerts
        .iter()
        .map(|alert| {
            format!(
                "{} [{}]: {}\n{}",
                alert.provider_name,
                alert.severity,
                alert.message,
                alert.messages.join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok((to.clone(), subject, body))
}

pub(crate) fn severity_rank(severity: &str) -> i32 {
    match severity {
        "critical" => 2,
        "warning" => 1,
        _ => 0,
    }
}

pub(crate) async fn build_observability_summary(
    state: &AppState,
) -> Result<ObservabilitySummary, AppError> {
    let sessions = state.list_sessions().await?;
    let tool_calls = state.list_tool_calls(None).await?;
    let approvals = state.list_approvals().await?;
    let execution_jobs = state.execution_queue.list().await?;
    let now = Utc::now();

    let mut sessions_by_status = HashMap::new();
    let mut event_categories = HashMap::new();
    let mut recent_error_events = Vec::new();
    for session in &sessions {
        increment_count(&mut sessions_by_status, session.status.as_str());
        for event in state.list_events(session.id).await? {
            let category = event.event_type.split('.').next().unwrap_or("event");
            increment_count(&mut event_categories, category);
            if telemetry_status_for_event(&event) == "error" {
                recent_error_events.push(ObservabilityErrorEvent {
                    session_id: event.session_id,
                    event_type: event.event_type,
                    seq: event.seq,
                    status: "error".to_string(),
                    created_at: event.created_at,
                });
            }
        }
    }
    recent_error_events.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.seq.cmp(&left.seq))
    });
    recent_error_events.truncate(10);

    let mut tool_calls_by_status = HashMap::new();
    let mut failed_tool_calls = 0;
    for call in &tool_calls {
        increment_count(&mut tool_calls_by_status, &call.status);
        if matches!(call.status.as_str(), "failed" | "denied") {
            failed_tool_calls += 1;
        }
    }

    let mut approvals_by_status = HashMap::new();
    let mut pending_approvals = 0;
    for approval in &approvals {
        increment_count(&mut approvals_by_status, &approval.status);
        if approval.status == "pending" {
            pending_approvals += 1;
        }
    }

    let mut execution_jobs_by_status = HashMap::new();
    let mut queued_jobs = 0;
    let mut running_jobs = 0;
    let mut failed_jobs = 0;
    let mut retryable_jobs = 0;
    let mut oldest_queued_at = None;
    for job in &execution_jobs {
        let status = execution_job_status_label(&job.status);
        increment_count(&mut execution_jobs_by_status, status);
        match job.status {
            ExecutionJobStatus::Queued => {
                queued_jobs += 1;
                oldest_queued_at = Some(match oldest_queued_at {
                    Some(oldest) if oldest <= job.enqueued_at => oldest,
                    _ => job.enqueued_at,
                });
            }
            ExecutionJobStatus::Running => running_jobs += 1,
            ExecutionJobStatus::Failed => failed_jobs += 1,
            ExecutionJobStatus::Completed | ExecutionJobStatus::Canceled => {}
        }
        if job.attempt_count > 0
            && job.attempt_count < job.max_attempts
            && job.status != ExecutionJobStatus::Completed
            && job.status != ExecutionJobStatus::Canceled
        {
            retryable_jobs += 1;
        }
    }

    let waiting_approval_sessions = sessions
        .iter()
        .filter(|session| session.status == SessionStatus::RequiresAction)
        .count();
    let failed_sessions = sessions
        .iter()
        .filter(|session| session.status == SessionStatus::Failed)
        .count();
    let oldest_queued_job_age_seconds =
        oldest_queued_at.map(|queued_at| now.signed_duration_since(queued_at).num_seconds().max(0));
    let backpressure_status = if failed_jobs > 0 || failed_sessions > 0 || failed_tool_calls > 0 {
        "error"
    } else if queued_jobs > 0 || running_jobs > 0 || pending_approvals > 0 {
        "attention"
    } else {
        "healthy"
    }
    .to_string();

    Ok(ObservabilitySummary {
        generated_at: now,
        telemetry: ObservabilityTelemetryStatus {
            service_name: state.observability_config.service_name.clone(),
            otlp_enabled: state.observability_config.is_enabled(),
            sample_ratio: state.observability_config.sample_ratio,
            endpoint_configured: state.observability_config.otlp_endpoint.is_some(),
        },
        sessions_by_status,
        tool_calls_by_status,
        approvals_by_status,
        execution_jobs_by_status,
        event_categories,
        recent_error_events,
        backpressure: ObservabilityBackpressure {
            status: backpressure_status,
            queued_jobs,
            running_jobs,
            failed_jobs,
            retryable_jobs,
            pending_approvals,
            waiting_approval_sessions,
            failed_sessions,
            failed_tool_calls,
            oldest_queued_job_age_seconds,
        },
    })
}

pub(crate) fn increment_count(counts: &mut HashMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

pub(crate) fn execution_job_status_label(status: &ExecutionJobStatus) -> &'static str {
    match status {
        ExecutionJobStatus::Queued => "queued",
        ExecutionJobStatus::Running => "running",
        ExecutionJobStatus::Completed => "completed",
        ExecutionJobStatus::Failed => "failed",
        ExecutionJobStatus::Canceled => "canceled",
    }
}
