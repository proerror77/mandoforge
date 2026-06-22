use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AcknowledgeCostAlertRequest, AppError, AppState, CostAlertAcknowledgement, CostAlertDelivery,
    CostAlertRoute, CostAlertSummary, CreateCostAlertRoute, CreateUsageRollup,
    UsageFinanceDashboardSummary, UsageFinanceExportDelivery, UsageFinanceOperationsRun,
    UsageFinanceOperationsSummary, UsageRollup, UsageSummary, UsageTrendSummary,
    acknowledge_cost_alert as acknowledge_cost_alert_impl,
    create_cost_alert_route as create_cost_alert_route_impl,
    create_usage_rollup as create_usage_rollup_impl,
    deliver_cost_alerts as deliver_cost_alerts_impl,
    deliver_usage_export as deliver_usage_export_impl, export_usage_csv as export_usage_csv_impl,
    get_cost_alerts as get_cost_alerts_impl,
    get_usage_finance_operations_summary as get_usage_finance_operations_summary_impl,
    get_usage_finance_summary as get_usage_finance_summary_impl,
    get_usage_summary as get_usage_summary_impl, get_usage_trends as get_usage_trends_impl,
    list_cost_alert_routes as list_cost_alert_routes_impl,
    list_usage_rollups as list_usage_rollups_impl,
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
    get_usage_summary_impl(state, headers).await
}

async fn get_usage_trends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageTrendSummary>, AppError> {
    get_usage_trends_impl(state, headers).await
}

async fn get_usage_finance_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageFinanceDashboardSummary>, AppError> {
    get_usage_finance_summary_impl(state, headers).await
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
    get_cost_alerts_impl(state, headers).await
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
    list_cost_alert_routes_impl(state, headers).await
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
    list_usage_rollups_impl(state, headers).await
}

async fn create_usage_rollup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateUsageRollup>,
) -> Result<Json<UsageRollup>, AppError> {
    create_usage_rollup_impl(state, headers, input).await
}
