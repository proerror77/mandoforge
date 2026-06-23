use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, ApprovalNotificationChannelPolicy, ApprovalNotificationDelivery,
    ApprovalNotificationDeliveryRun, ApprovalNotificationDeliveryRunSummary,
    ApprovalNotificationDeploymentValidationRun, ApprovalNotificationOpsValidationRun,
    ApprovalNotificationRoutingSummary, AuthorizationRequest,
    CreateApprovalNotificationChannelPolicy, Permission, approval_email_relay_url_from_env,
    approval_is_expired, approval_notification_deployment_controller_configured,
    approval_notification_deployment_controller_required,
    approval_notification_ops_controller_configured, approval_notification_ops_controller_required,
    approval_slack_webhook_url_from_env, authorize_request,
    build_approval_notification_delivery_run_summary, build_approval_notification_routing_summary,
    dedupe_strings, deliver_approval_notification, enforce_resource_scope,
    execute_approval_notification_delivery_run,
    execute_approval_notification_deployment_controller,
    execute_approval_notification_ops_controller, expire_approval_record, new_audit_log,
    principal_from_request, validate_approval_notification_channel_policy_input,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/approvals/notifications/run",
            post(run_approval_notifications),
        )
        .route(
            "/api/approvals/notifications/deployment/validate",
            post(validate_approval_notification_deployment),
        )
        .route(
            "/api/approvals/notifications/ops/validate",
            post(validate_approval_notification_ops),
        )
        .route(
            "/api/approvals/notifications/runs",
            get(get_approval_notification_delivery_runs),
        )
        .route(
            "/api/approvals/notification-channel-policies",
            get(list_approval_notification_channel_policies)
                .post(create_approval_notification_channel_policy),
        )
        .route(
            "/api/approvals/notification-channel-policies/{id}/archive",
            post(archive_approval_notification_channel_policy),
        )
        .route("/api/approvals/{id}/deliver", post(deliver_approval))
        .route(
            "/api/approvals/notification-routing/summary",
            get(get_approval_notification_routing_summary),
        )
}

async fn deliver_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDelivery>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "approval", Some(id)).await?;
    let approval = state.get_approval(id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be delivered",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
    let delivered_at = Utc::now();
    Ok(Json(
        deliver_approval_notification(&state, &approval, delivered_at).await?,
    ))
}

async fn run_approval_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeliveryRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_notifications".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_approval_notification_delivery_run(
            &state,
            Some(principal.subject_id),
            Utc::now(),
            50,
        )
        .await?,
    ))
}

async fn validate_approval_notification_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeploymentValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_notifications".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let routing = build_approval_notification_routing_summary(
        state.list_approvals().await?,
        state.list_approval_groups().await?,
        state.list_approval_escalation_rules().await?,
        state.list_approval_notification_channel_policies().await?,
        state.approval_webhook_url.is_some(),
        approval_slack_webhook_url_from_env().is_some(),
        approval_email_relay_url_from_env().is_some(),
    )
    .await;
    let checked_at = Utc::now();
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = approval_notification_deployment_controller_required(&lookup);
    let controller_configured = approval_notification_deployment_controller_configured(&lookup);
    let mut issues = Vec::new();
    let mut healthy = routing.channel_count > 0
        && routing.active_policy_count > 0
        && routing.unroutable_pending_count == 0;
    if routing.channel_count == 0 {
        issues.push("approval notification deployment validation found no delivery channels");
    }
    if routing.active_policy_count == 0 {
        issues.push("approval notification deployment validation found no active channel policies");
    }
    if routing.unroutable_pending_count > 0 {
        issues
            .push("approval notification deployment validation found unroutable pending approvals");
    }
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "routing_not_ready"
        } else {
            "controller_not_configured"
        }
    });
    if healthy && controller_configured {
        match execute_approval_notification_deployment_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &routing,
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
                    issues.push("approval notification deployment controller did not validate");
                }
            }
            Err(error) => {
                healthy = false;
                issues.push("approval notification deployment controller failed");
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
        issues.push("approval notification deployment controller is required but not configured");
    }
    let status = if healthy { "healthy" } else { "blocked" }.to_string();
    let run = ApprovalNotificationDeploymentValidationRun {
        status,
        pending_approval_count: routing.pending_approval_count,
        routable_pending_count: routing.routable_pending_count,
        unroutable_pending_count: routing.unroutable_pending_count,
        channel_count: routing.channel_count,
        persisted_policy_count: routing.persisted_policy_count,
        active_policy_count: routing.active_policy_count,
        routing_status: routing.status,
        controller_required,
        controller_configured,
        controller_execution,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.notification_deployment_validation_run",
            "approval_notifications",
            None,
            json!({
                "subject": principal.subject_id,
                "status": run.status,
                "pending_approval_count": run.pending_approval_count,
                "routable_pending_count": run.routable_pending_count,
                "unroutable_pending_count": run.unroutable_pending_count,
                "channel_count": run.channel_count,
                "persisted_policy_count": run.persisted_policy_count,
                "active_policy_count": run.active_policy_count,
                "routing_status": run.routing_status,
                "controller_required": run.controller_required,
                "controller_configured": run.controller_configured,
                "controller_execution": run.controller_execution,
                "issues": issues,
                "checked_at": run.checked_at,
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn validate_approval_notification_ops(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationOpsValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_notifications".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let audit_logs = state.list_audit_logs(None).await?;
    let routing = build_approval_notification_routing_summary(
        state.list_approvals().await?,
        state.list_approval_groups().await?,
        state.list_approval_escalation_rules().await?,
        state.list_approval_notification_channel_policies().await?,
        state.approval_webhook_url.is_some(),
        approval_slack_webhook_url_from_env().is_some(),
        approval_email_relay_url_from_env().is_some(),
    )
    .await;
    let delivery_summary =
        build_approval_notification_delivery_run_summary(&audit_logs, checked_at, &routing);
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = approval_notification_ops_controller_required(&lookup);
    let controller_configured = approval_notification_ops_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "controller_not_attempted"
        } else {
            "approval_notification_ops_controller_not_configured"
        }
    });
    let mut issues = Vec::new();
    if delivery_summary.production_ops.production_blocked {
        issues.push(delivery_summary.production_ops.message.clone());
    }
    if controller_configured {
        match execute_approval_notification_ops_controller(
            &lookup,
            Some(principal.subject_id.as_str()),
            checked_at,
            &routing,
            &delivery_summary,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues
                        .push("approval notification ops controller did not validate".to_string());
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("approval notification ops controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    } else if controller_required {
        issues.push(
            "approval notification ops controller is required but not configured".to_string(),
        );
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push(
            "approval notification ops controller evidence is missing or not validated".to_string(),
        );
    }
    dedupe_strings(&mut issues);
    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    let run = ApprovalNotificationOpsValidationRun {
        status,
        routing_status: routing.status.clone(),
        latest_run_status: delivery_summary
            .latest_run
            .as_ref()
            .map(|run| run.status.clone()),
        controller_required,
        controller_configured,
        controller_execution,
        checked_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.notification_ops_validation_run",
            "approval_notifications",
            None,
            json!({
                "subject": principal.subject_id,
                "status": run.status,
                "routing_status": run.routing_status,
                "latest_run_status": run.latest_run_status,
                "controller_required": run.controller_required,
                "controller_configured": run.controller_configured,
                "controller_execution": run.controller_execution,
                "issues": issues,
                "checked_at": run.checked_at,
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn get_approval_notification_delivery_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeliveryRunSummary>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "approval_notifications",
        None,
    )
    .await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let routing = build_approval_notification_routing_summary(
        state.list_approvals().await?,
        state.list_approval_groups().await?,
        state.list_approval_escalation_rules().await?,
        state.list_approval_notification_channel_policies().await?,
        state.approval_webhook_url.is_some(),
        approval_slack_webhook_url_from_env().is_some(),
        approval_email_relay_url_from_env().is_some(),
    )
    .await;
    Ok(Json(build_approval_notification_delivery_run_summary(
        &audit_logs,
        Utc::now(),
        &routing,
    )))
}

async fn list_approval_notification_channel_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalNotificationChannelPolicy>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "approval_notification_channel_policies",
        None,
    )
    .await?;
    Ok(Json(
        state.list_approval_notification_channel_policies().await?,
    ))
}

async fn create_approval_notification_channel_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalNotificationChannelPolicy>,
) -> Result<Json<ApprovalNotificationChannelPolicy>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_notification_channel_policies".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let policy = state
        .create_approval_notification_channel_policy(
            validate_approval_notification_channel_policy_input(input)?,
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.notification_channel_policy_created",
            "approval_notification_channel_policy",
            Some(policy.id),
            json!({
                "subject": principal.subject_id,
                "name": policy.name,
                "channel": policy.channel,
                "target_env": policy.target_env,
                "risk_filter": policy.risk_filter,
                "max_attempts": policy.max_attempts,
                "backoff_seconds": policy.backoff_seconds,
                "status": policy.status,
            }),
        ))
        .await?;
    Ok(Json(policy))
}

async fn archive_approval_notification_channel_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationChannelPolicy>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_notification_channel_policy".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let policy = state
        .archive_approval_notification_channel_policy(id)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.notification_channel_policy_archived",
            "approval_notification_channel_policy",
            Some(policy.id),
            json!({
                "subject": principal.subject_id,
                "name": policy.name,
                "channel": policy.channel,
                "target_env": policy.target_env,
                "risk_filter": policy.risk_filter,
                "max_attempts": policy.max_attempts,
                "backoff_seconds": policy.backoff_seconds,
                "status": policy.status,
            }),
        ))
        .await?;
    Ok(Json(policy))
}

async fn get_approval_notification_routing_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationRoutingSummary>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "approval_notifications",
        None,
    )
    .await?;
    Ok(Json(
        build_approval_notification_routing_summary(
            state.list_approvals().await?,
            state.list_approval_groups().await?,
            state.list_approval_escalation_rules().await?,
            state.list_approval_notification_channel_policies().await?,
            state.approval_webhook_url.is_some(),
            approval_slack_webhook_url_from_env().is_some(),
            approval_email_relay_url_from_env().is_some(),
        )
        .await,
    ))
}
