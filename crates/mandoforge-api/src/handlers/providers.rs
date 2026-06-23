use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, AuthorizationRequest, CreateProviderAccess, CreateProviderRecord,
    DecideProviderStatusApproval, Permission, ProviderAccess, ProviderDeploymentValidationRun,
    ProviderGovernanceSummary, ProviderHealth, ProviderPolicyGateReport,
    ProviderPolicyGateRunResponse, ProviderPolicyGateRunSummary, ProviderProductionRollbackRun,
    ProviderProductionRolloutRun, ProviderRecord, ProviderStatusApprovalResponse,
    RequestProviderStatusApproval, RotateProviderApiKeyRef, RunProviderProductionRollback,
    RunProviderProductionRollout, UpdateProviderAccess, UpdateProviderStatus, authorize_request,
    build_provider_governance_summary, build_provider_policy_gate_report,
    build_provider_policy_gate_run_summary, decide_provider_status_approval,
    enforce_resource_scope, execute_provider_deployment_controller, execute_provider_policy_gate,
    execute_provider_production_rollback_with_lookup,
    execute_provider_production_rollout_with_lookup, new_audit_log, normalize_provider_api_key_ref,
    normalize_provider_status, optional_trimmed, principal_from_request, provider_by_id,
    provider_deployment_controller_configured, provider_deployment_controller_required,
    provider_health,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list_providers).post(create_provider))
        .route("/api/providers/{id}", patch(update_provider))
        .route("/api/providers/summary", get(get_provider_summary))
        .route(
            "/api/providers/deployment/validate",
            post(validate_provider_deployment),
        )
        .route("/api/providers/policy-gate", get(get_provider_policy_gate))
        .route(
            "/api/providers/policy-gate/run",
            post(run_provider_policy_gate),
        )
        .route(
            "/api/providers/policy-gate/runs",
            get(get_provider_policy_gate_runs),
        )
        .route(
            "/api/providers/production-rollout/run",
            post(run_provider_production_rollout),
        )
        .route(
            "/api/providers/production-rollout/rollback",
            post(run_provider_production_rollback),
        )
        .route("/api/providers/{id}/status", patch(update_provider_status))
        .route(
            "/api/providers/{id}/status-approval",
            post(request_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/status-approval/approve",
            post(approve_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/status-approval/reject",
            post(reject_provider_status_approval),
        )
        .route(
            "/api/providers/{id}/api-key-ref/rotate",
            post(rotate_provider_api_key_ref),
        )
        .route("/api/providers/{id}/health", get(get_provider_health))
        .route(
            "/api/teams/{id}/provider-access",
            get(list_provider_access).post(create_provider_access),
        )
        .route("/api/provider-access/{id}", patch(update_provider_access))
        .route(
            "/api/provider-access/{id}/archive",
            post(archive_provider_access),
        )
}

async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderRecord>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    Ok(Json(state.list_providers().await?))
}

async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    Ok(Json(state.create_provider(input).await?))
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", Some(id)).await?;
    let provider = state.update_provider(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.updated",
            "provider",
            Some(id),
            json!({
                "subject": principal_from_request(&state, &headers).await?.subject_id,
                "provider_name": provider.name,
                "provider_type": provider.provider_type,
                "default_model": provider.default_model
            }),
        ))
        .await?;
    Ok(Json(provider))
}

async fn get_provider_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderGovernanceSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    let providers = state.list_providers().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_provider_governance_summary(
        &providers,
        &audit_logs,
    )))
}

async fn validate_provider_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderDeploymentValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "providers".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let providers = state.list_providers().await?;
    let active_providers = providers
        .into_iter()
        .filter(|provider| provider.status == "active")
        .collect::<Vec<_>>();
    let mut results = Vec::with_capacity(active_providers.len());
    for provider in &active_providers {
        results.push(provider_health(provider).await);
    }
    let healthy_count = results.iter().filter(|health| health.healthy).count();
    let unhealthy_count = results.len().saturating_sub(healthy_count);
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = provider_deployment_controller_required(&lookup);
    let controller_configured = provider_deployment_controller_configured(&lookup);
    let mut healthy = !results.is_empty() && unhealthy_count == 0;
    let mut issues = Vec::new();
    if results.is_empty() {
        issues.push("provider deployment validation covered no active providers");
    }
    if unhealthy_count > 0 {
        issues.push("provider deployment validation found unhealthy providers");
    }
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "provider_health_not_ready"
        } else {
            "controller_not_configured"
        }
    });
    if healthy && controller_configured {
        match execute_provider_deployment_controller(
            &lookup,
            &principal.subject_id,
            Utc::now(),
            &results,
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
                    issues.push("provider deployment controller did not validate");
                }
            }
            Err(error) => {
                healthy = false;
                issues.push("provider deployment controller failed");
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
        issues.push("provider deployment controller is required but not configured");
    }
    let status = if healthy { "healthy" } else { "blocked" }.to_string();
    let audit_log = state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.deployment_validation_run",
            "providers",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "provider_count": results.len(),
                "healthy_count": healthy_count,
                "unhealthy_count": unhealthy_count,
                "provider_names": results.iter().map(|health| health.name.clone()).collect::<Vec<_>>(),
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "issues": issues,
            }),
        ))
        .await?;
    Ok(Json(ProviderDeploymentValidationRun {
        status,
        provider_count: results.len(),
        healthy_count,
        unhealthy_count,
        results,
        controller_required,
        controller_configured,
        controller_execution,
        ran_at: audit_log.created_at,
    }))
}

async fn get_provider_policy_gate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateReport>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    let providers = state.list_providers().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_provider_policy_gate_report(
        &providers,
        &audit_logs,
    )))
}

async fn run_provider_policy_gate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateRunResponse>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "providers".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_provider_policy_gate(&state, Some(principal.subject_id), "user").await?,
    ))
}

async fn get_provider_policy_gate_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProviderPolicyGateRunSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_provider_policy_gate_run_summary(
        &audit_logs,
        Utc::now(),
    )))
}

async fn run_provider_production_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunProviderProductionRollout>,
) -> Result<Json<ProviderProductionRolloutRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "providers".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_provider_production_rollout_with_lookup(
            &state,
            principal.subject_id,
            input,
            |key| std::env::var(key).ok(),
        )
        .await?,
    ))
}

async fn run_provider_production_rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunProviderProductionRollback>,
) -> Result<Json<ProviderProductionRollbackRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "providers".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_provider_production_rollback_with_lookup(
            &state,
            principal.subject_id,
            input,
            |key| std::env::var(key).ok(),
        )
        .await?,
    ))
}

async fn update_provider_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderStatus>,
) -> Result<Json<ProviderRecord>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "provider".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let status = normalize_provider_status(&input.status)?;
    if !input.emergency {
        return Err(AppError::bad_request(
            "direct provider status changes require emergency=true; use status approval for normal changes",
        ));
    }
    let reason = optional_trimmed(input.reason.as_deref()).ok_or_else(|| {
        AppError::bad_request("direct provider status changes require an emergency reason")
    })?;
    let previous = provider_by_id(&state, id).await?;
    let policy_decision = json!({
        "decision": "allowed",
        "gate": "provider_lifecycle_emergency",
        "emergency": true,
        "reason": reason,
        "previous_status": previous.status,
        "requested_status": status,
    });
    let provider = state.update_provider_status(id, &status).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.status_updated",
            "provider",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "provider_name": provider.name,
                "status": provider.status,
                "policy_decision": policy_decision
            }),
        ))
        .await?;
    Ok(Json(provider))
}

async fn request_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RequestProviderStatusApproval>,
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
    let requested_status = normalize_provider_status(&input.status)?;
    let provider = provider_by_id(&state, id).await?;
    if provider.status == requested_status {
        return Err(AppError::bad_request(
            "provider already has requested status",
        ));
    }
    if provider
        .config
        .get("pending_status_approval")
        .and_then(|approval| approval.get("status"))
        .and_then(Value::as_str)
        == Some("pending")
    {
        return Err(AppError::bad_request(
            "provider already has a pending status approval",
        ));
    }
    let requested_at = Utc::now();
    let approval = json!({
        "id": Uuid::new_v4(),
        "status": "pending",
        "provider_id": provider.id,
        "provider_name": provider.name,
        "previous_status": provider.status,
        "requested_status": requested_status,
        "requested_by": principal.subject_id,
        "reason": optional_trimmed(input.reason.as_deref()),
        "approver_subject": optional_trimmed(input.approver_subject.as_deref()),
        "requested_at": requested_at,
    });
    let mut config = provider.config.as_object().cloned().unwrap_or_default();
    config.insert("pending_status_approval".to_string(), approval.clone());
    let updated = state
        .update_provider_config(id, Value::Object(config))
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.status_approval_requested",
            "provider",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "provider_name": provider.name,
                "previous_status": provider.status,
                "requested_status": requested_status,
                "approval": approval
            }),
        ))
        .await?;
    Ok(Json(ProviderStatusApprovalResponse {
        provider: updated,
        approval,
    }))
}

async fn approve_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DecideProviderStatusApproval>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    decide_provider_status_approval(state, id, headers, "approved", input.comment).await
}

async fn reject_provider_status_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DecideProviderStatusApproval>,
) -> Result<Json<ProviderStatusApprovalResponse>, AppError> {
    decide_provider_status_approval(state, id, headers, "rejected", input.comment).await
}

async fn rotate_provider_api_key_ref(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RotateProviderApiKeyRef>,
) -> Result<Json<ProviderRecord>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "provider".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let new_ref = normalize_provider_api_key_ref(&input.api_key_ref)?;
    let provider = state
        .list_providers()
        .await?
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::not_found("provider not found"))?;
    let previous_ref = provider
        .config
        .get("api_key_ref")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let mut config = provider.config.as_object().cloned().unwrap_or_default();
    config.insert("api_key_ref".to_string(), Value::String(new_ref.clone()));
    config.remove("api_key_env");
    let updated = state
        .update_provider_config(id, Value::Object(config))
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.api_key_ref_rotated",
            "provider",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "provider_name": provider.name,
                "previous_api_key_ref": previous_ref,
                "new_api_key_ref": new_ref,
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn get_provider_health(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderHealth>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "provider".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let provider = state
        .list_providers()
        .await?
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::not_found("provider not found"))?;
    let health = provider_health(&provider).await;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider.health_checked",
            "provider",
            Some(provider.id),
            json!({
                "subject": principal.subject_id,
                "provider_name": provider.name,
                "healthy": health.healthy,
                "issues": health.issues,
                "checks": health.checks
            }),
        ))
        .await?;
    Ok(Json(health))
}

async fn list_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderAccess>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.list_provider_access(id).await?))
}

async fn create_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.create_provider_access(id, input).await?))
}

async fn update_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "provider_access",
        Some(id),
    )
    .await?;
    let access = state.update_provider_access(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider_access.updated",
            "provider_access",
            Some(id),
            json!({
                "subject": principal_from_request(&state, &headers).await?.subject_id,
                "team_id": access.team_id,
                "provider_name": access.provider_name,
                "model_allowlist": access.model_allowlist,
                "status": access.status
            }),
        ))
        .await?;
    Ok(Json(access))
}

async fn archive_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderAccess>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "provider_access",
        Some(id),
    )
    .await?;
    let access = state.archive_provider_access(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "provider_access.archived",
            "provider_access",
            Some(id),
            json!({
                "subject": principal_from_request(&state, &headers).await?.subject_id,
                "team_id": access.team_id,
                "provider_name": access.provider_name,
                "model_allowlist": access.model_allowlist,
                "status": access.status
            }),
        ))
        .await?;
    Ok(Json(access))
}
