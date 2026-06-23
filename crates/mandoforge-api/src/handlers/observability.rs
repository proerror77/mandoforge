use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};

use crate::{
    AppError, AppState, AuthorizationRequest, ObservabilityCollectorClusterRolloutValidationRun,
    ObservabilityCollectorReadiness, ObservabilityRemediationPlan, ObservabilityRemediationRun,
    ObservabilitySummary, Permission, authorize_request,
    build_observability_collector_deployment_readiness, build_observability_collector_readiness,
    build_observability_remediation_plan, build_observability_summary, dedupe_strings,
    enforce_resource_scope, execute_observability_collector_cluster_controller,
    execute_observability_collector_deployment_controller,
    execute_observability_remediation_with_lookup, new_audit_log,
    observability_collector_cluster_controller_configured,
    observability_collector_cluster_controller_required,
    observability_collector_deployment_controller_configured,
    observability_collector_deployment_controller_required, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/observability", get(get_observability_summary))
        .route(
            "/api/observability/collector-readiness",
            get(get_observability_collector_readiness),
        )
        .route(
            "/api/observability/collector/deployment/validate",
            post(validate_observability_collector_deployment),
        )
        .route(
            "/api/observability/collector/cluster/validate",
            post(validate_observability_collector_cluster_rollout),
        )
        .route(
            "/api/observability/remediation/plan",
            get(get_observability_remediation_plan),
        )
        .route(
            "/api/observability/remediation/run",
            post(run_observability_remediation),
        )
}

async fn get_observability_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilitySummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    Ok(Json(build_observability_summary(&state).await?))
}

async fn get_observability_collector_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityCollectorReadiness>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    Ok(Json(build_observability_collector_readiness(&state).await))
}

async fn validate_observability_collector_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "observability".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let config = &state.observability_config;
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = observability_collector_deployment_controller_required(&lookup);
    let controller_configured = observability_collector_deployment_controller_configured(&lookup);
    let endpoint_configured = config
        .otlp_endpoint
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let mut issues = Vec::new();
    let mut healthy = false;
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "collector_health_not_ready"
        } else {
            "controller_not_configured"
        }
    });

    if !config.is_enabled() {
        issues.push("OTLP export is disabled".to_string());
    } else if !endpoint_configured {
        issues.push("OTLP collector endpoint is not configured".to_string());
    } else {
        match state.telemetry_exporter.health_check(config).await {
            Ok(()) => {
                healthy = true;
            }
            Err(error) => {
                issues.push(error.message);
            }
        }
    }
    if healthy && controller_configured {
        match execute_observability_collector_deployment_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            config,
        )
        .await
        {
            Ok(execution) => {
                let controller_status = execution
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed")
                    .to_string();
                controller_execution = execution;
                if controller_status != "validated" {
                    healthy = false;
                    issues.push("collector deployment controller did not validate".to_string());
                }
            }
            Err(error) => {
                healthy = false;
                issues.push(error.message.clone());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if healthy && controller_required && !controller_configured {
        healthy = false;
        issues.push("collector deployment controller is required but not configured".to_string());
    }

    let status = if healthy { "healthy" } else { "blocked" };
    let result = json!({
        "status": status,
        "healthy": healthy,
        "otlp_enabled": config.is_enabled(),
        "endpoint_configured": endpoint_configured,
        "service_name": config.service_name.clone(),
        "sample_ratio": config.sample_ratio,
        "controller_required": controller_required,
        "controller_configured": controller_configured,
        "controller_execution": controller_execution,
        "issues": issues,
        "subject": principal.subject_id,
        "checked_at": checked_at,
    });
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "observability.collector_deployment_validation",
            "observability",
            None,
            result.clone(),
        ))
        .await?;
    Ok(Json(result))
}

async fn validate_observability_collector_cluster_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityCollectorClusterRolloutValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "observability".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let lookup = |key: &str| std::env::var(key).ok();
    let audit_logs = state.list_audit_logs(None).await?;
    let deployment_readiness = build_observability_collector_deployment_readiness(
        state.observability_config.is_enabled(),
        state.observability_config.otlp_endpoint.is_some(),
        observability_collector_deployment_controller_required(&lookup),
        observability_collector_deployment_controller_configured(&lookup),
        &audit_logs,
        checked_at,
    );
    let controller_required = observability_collector_cluster_controller_required(&lookup);
    let controller_configured = observability_collector_cluster_controller_configured(&lookup);
    let mut issues = Vec::new();
    if !deployment_readiness.deployment_validated {
        issues.push("collector deployment validation is not ready".to_string());
    }
    if controller_required && !controller_configured {
        issues.push(
            "collector cluster rollout controller is required but not configured".to_string(),
        );
    }
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            if deployment_readiness.deployment_validated {
                "controller_not_required"
            } else {
                "collector_deployment_not_ready"
            }
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured && deployment_readiness.deployment_validated {
        match execute_observability_collector_cluster_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &state.observability_config,
            &deployment_readiness,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues
                        .push("collector cluster rollout controller did not validate".to_string());
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("collector cluster rollout controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push(
            "collector cluster rollout controller evidence is missing or not validated".to_string(),
        );
    }
    dedupe_strings(&mut issues);
    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "observability.collector_cluster_rollout_validation",
            "observability",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "deployment_validated": deployment_readiness.deployment_validated,
                "issues": issues,
                "checked_at": checked_at,
            }),
        ))
        .await?;
    Ok(Json(ObservabilityCollectorClusterRolloutValidationRun {
        status,
        checked_at,
        controller_required,
        controller_configured,
        controller_execution,
        issues,
    }))
}

async fn get_observability_remediation_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityRemediationPlan>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    let summary = build_observability_summary(&state).await?;
    Ok(Json(build_observability_remediation_plan(summary)))
}

async fn run_observability_remediation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ObservabilityRemediationRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "observability", None).await?;
    Ok(Json(
        execute_observability_remediation_with_lookup(&state, |key| std::env::var(key).ok())
            .await?,
    ))
}
