use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};

use crate::{
    AcknowledgeCostAlertRequest, AppError, AppState, AuthorizationRequest,
    CostAlertAcknowledgement, CostAlertDelivery, CostAlertRoute, CostAlertSummary,
    CreateCostAlertRoute, CreateUsageRollup, Permission, UsageFinanceDashboardSummary,
    UsageFinanceExportDelivery, UsageFinanceOperationsRun, UsageFinanceOperationsSummary,
    UsageRollup, UsageSummary, UsageTrendSummary, authorize_request, build_cost_alerts,
    build_usage_finance_csv, build_usage_finance_dashboard_summary,
    build_usage_finance_operations_summary, build_usage_summary, build_usage_trend_summary,
    enforce_resource_scope, execute_cost_alert_delivery, execute_usage_finance_export_delivery,
    execute_usage_finance_operations, execute_usage_finance_reconciliation_controller,
    new_audit_log, principal_from_request, validate_cost_alert_route_input,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/usage", get(get_usage_summary))
        .route("/api/usage/trends", get(get_usage_trends))
        .route("/api/usage/finance-summary", get(get_usage_finance_summary))
        .route(
            "/api/usage/finance-operations/summary",
            get(get_usage_finance_operations_summary),
        )
        .route(
            "/api/usage/finance-operations/run",
            post(run_usage_finance_operations),
        )
        .route(
            "/api/usage/finance-operations/reconcile",
            post(run_usage_finance_reconciliation),
        )
        .route("/api/usage/export.csv", get(export_usage_csv))
        .route("/api/usage/export/deliver", post(deliver_usage_export))
        .route("/api/usage/alerts", get(get_cost_alerts))
        .route("/api/usage/alerts/ack", post(acknowledge_cost_alert))
        .route("/api/usage/alerts/deliver", post(deliver_cost_alerts))
        .route(
            "/api/usage/alert-routes",
            get(list_cost_alert_routes).post(create_cost_alert_route),
        )
        .route(
            "/api/usage/rollups",
            get(list_usage_rollups).post(create_usage_rollup),
        )
}

async fn get_usage_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage", None).await?;
    Ok(Json(build_usage_summary(&state).await?))
}

async fn get_usage_trends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageTrendSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_trends", None).await?;
    Ok(Json(build_usage_trend_summary(&state).await?))
}

async fn get_usage_finance_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceDashboardSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_finance", None).await?;
    Ok(Json(build_usage_finance_dashboard_summary(&state).await?))
}

async fn get_usage_finance_operations_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceOperationsSummary>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "usage_finance_operations",
        None,
    )
    .await?;
    Ok(Json(build_usage_finance_operations_summary(&state).await?))
}

async fn run_usage_finance_operations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceOperationsRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_finance_operations".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_usage_finance_operations(&state, Some(principal.subject_id.as_str())).await?,
    ))
}

async fn run_usage_finance_reconciliation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_finance_operations".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let ran_at = Utc::now();
    let before = build_usage_finance_operations_summary(&state).await?;
    let execution = execute_usage_finance_reconciliation_controller(
        &|key| std::env::var(key).ok(),
        Some(principal.subject_id.as_str()),
        ran_at,
        &before,
    )
    .await?;
    let status = execution
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed")
        .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.finance_reconciliation_run",
            "usage_finance_operations",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "reconciliation_controller_configured": true,
                "reconciliation_controller_execution": execution,
                "before_status": before.status,
                "before_production_close_status": before.production_close.status,
                "ran_at": ran_at,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": status,
        "ran_at": ran_at,
        "before": before,
        "reconciliation_controller_configured": true,
        "reconciliation_controller_execution": execution,
    })))
}

async fn export_usage_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_export".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let summary = build_usage_summary(&state).await?;
    let trend = build_usage_trend_summary(&state).await?;
    let csv = build_usage_finance_csv(&summary, &trend);
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.finance_exported",
            "usage_export",
            None,
            json!({
                "subject": principal.subject_id,
                "provider_count": summary.by_provider.len(),
                "budget_pressure_count": trend.budget_pressure.pressure_count,
                "rollup_count": trend.rollup_count
            }),
        ))
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"mandoforge-usage-export.csv\"",
            ),
        ],
        csv,
    )
        .into_response())
}

async fn deliver_usage_export(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceExportDelivery>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_export".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_usage_finance_export_delivery(
            &state,
            false,
            "user",
            Some(principal.subject_id.as_str()),
        )
        .await?,
    ))
}

async fn get_cost_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CostAlertSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_alerts", None).await?;
    let summary = build_usage_summary(&state).await?;
    Ok(Json(CostAlertSummary {
        webhook_configured: state.cost_alert_webhook_url.is_some(),
        min_status: "warning".to_string(),
        alerts: build_cost_alerts(&summary.provider_budgets, Utc::now()),
    }))
}

async fn acknowledge_cost_alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AcknowledgeCostAlertRequest>,
) -> Result<Json<CostAlertAcknowledgement>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_alerts".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let provider_name = input.provider_name.trim();
    let severity = input.severity.trim();
    if provider_name.is_empty() {
        return Err(AppError::bad_request("provider_name is required"));
    }
    if !matches!(severity, "warning" | "critical") {
        return Err(AppError::bad_request(
            "severity must be warning or critical",
        ));
    }
    let acknowledged_at = Utc::now();
    let acknowledgement = CostAlertAcknowledgement {
        provider_name: provider_name.to_string(),
        severity: severity.to_string(),
        acknowledged_by: principal.subject_id.clone(),
        comment: input.comment.and_then(|comment| {
            let trimmed = comment.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }),
        acknowledged_at,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.alert_acknowledged",
            "usage_alert",
            None,
            serde_json::to_value(&acknowledgement)?,
        ))
        .await?;
    Ok(Json(acknowledgement))
}

async fn deliver_cost_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CostAlertDelivery>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_alerts", None).await?;
    Ok(Json(execute_cost_alert_delivery(&state, Utc::now()).await?))
}

async fn list_cost_alert_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CostAlertRoute>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "usage_alert_routes",
        None,
    )
    .await?;
    Ok(Json(state.list_cost_alert_routes().await?))
}

async fn create_cost_alert_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateCostAlertRoute>,
) -> Result<Json<CostAlertRoute>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "usage_alert_routes".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let route = state
        .create_cost_alert_route(validate_cost_alert_route_input(input)?)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "usage.alert_route_created",
            "usage_alert_route",
            Some(route.id),
            json!({
                "subject": principal.subject_id,
                "name": route.name,
                "channel": route.channel,
                "severity_filter": route.severity_filter
            }),
        ))
        .await?;
    Ok(Json(route))
}

async fn list_usage_rollups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UsageRollup>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_rollups", None).await?;
    Ok(Json(state.list_usage_rollups().await?))
}

async fn create_usage_rollup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateUsageRollup>,
) -> Result<Json<UsageRollup>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_rollups", None).await?;
    let period_end = input.period_end.unwrap_or_else(Utc::now);
    let period_start = input
        .period_start
        .unwrap_or_else(|| period_end - chrono::Duration::hours(24));
    if period_start >= period_end {
        return Err(AppError::bad_request(
            "usage rollup period_start must be before period_end",
        ));
    }
    let summary = serde_json::to_value(build_usage_summary(&state).await?)?;
    Ok(Json(
        state
            .create_usage_rollup(period_start, period_end, summary)
            .await?,
    ))
}
