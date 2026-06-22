use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::Value;

use crate::{
    AcknowledgeCostAlertRequest, AppError, AppState, CostAlertAcknowledgement, CostAlertDelivery,
    CostAlertRoute, CostAlertSummary, CreateCostAlertRoute, CreateUsageRollup,
    UsageFinanceDashboardSummary, UsageFinanceExportDelivery, UsageFinanceOperationsRun,
    UsageFinanceOperationsSummary, UsageRollup, UsageSummary, UsageTrendSummary,
    acknowledge_cost_alert as acknowledge_cost_alert_impl,
    authorize_request, build_cost_alerts, build_usage_finance_dashboard_summary,
    build_usage_summary, build_usage_trend_summary,
    create_cost_alert_route as create_cost_alert_route_impl,
    create_usage_rollup as create_usage_rollup_impl,
    deliver_cost_alerts as deliver_cost_alerts_impl,
    deliver_usage_export as deliver_usage_export_impl, export_usage_csv as export_usage_csv_impl,
    get_usage_finance_operations_summary as get_usage_finance_operations_summary_impl,
    Permission,
    run_usage_finance_operations as run_usage_finance_operations_impl,
    run_usage_finance_reconciliation as run_usage_finance_reconciliation_impl,
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
    get_usage_finance_operations_summary_impl(state, headers).await
}

async fn run_usage_finance_operations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceOperationsRun>, AppError> {
    run_usage_finance_operations_impl(state, headers).await
}

async fn run_usage_finance_reconciliation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    run_usage_finance_reconciliation_impl(state, headers).await
}

async fn export_usage_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    export_usage_csv_impl(state, headers).await
}

async fn deliver_usage_export(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceExportDelivery>, AppError> {
    deliver_usage_export_impl(state, headers).await
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
    acknowledge_cost_alert_impl(state, headers, input).await
}

async fn deliver_cost_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CostAlertDelivery>, AppError> {
    deliver_cost_alerts_impl(state, headers).await
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
    create_cost_alert_route_impl(state, headers, input).await
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
    create_usage_rollup_impl(state, headers, input).await
}
