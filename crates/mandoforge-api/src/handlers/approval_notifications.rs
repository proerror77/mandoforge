use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, ApprovalNotificationChannelPolicy, ApprovalNotificationDelivery,
    ApprovalNotificationDeliveryRun, ApprovalNotificationDeliveryRunSummary,
    ApprovalNotificationDeploymentValidationRun, ApprovalNotificationOpsValidationRun,
    ApprovalNotificationRoutingSummary, CreateApprovalNotificationChannelPolicy,
    archive_approval_notification_channel_policy as archive_approval_notification_channel_policy_impl,
    create_approval_notification_channel_policy as create_approval_notification_channel_policy_impl,
    deliver_approval as deliver_approval_impl,
    get_approval_notification_delivery_runs as get_approval_notification_delivery_runs_impl,
    get_approval_notification_routing_summary as get_approval_notification_routing_summary_impl,
    list_approval_notification_channel_policies as list_approval_notification_channel_policies_impl,
    run_approval_notifications as run_approval_notifications_impl,
    validate_approval_notification_deployment as validate_approval_notification_deployment_impl,
    validate_approval_notification_ops as validate_approval_notification_ops_impl,
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
    deliver_approval_impl(state, id, headers).await
}

async fn run_approval_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeliveryRun>, AppError> {
    run_approval_notifications_impl(state, headers).await
}

async fn validate_approval_notification_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeploymentValidationRun>, AppError> {
    validate_approval_notification_deployment_impl(state, headers).await
}

async fn validate_approval_notification_ops(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationOpsValidationRun>, AppError> {
    validate_approval_notification_ops_impl(state, headers).await
}

async fn get_approval_notification_delivery_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationDeliveryRunSummary>, AppError> {
    get_approval_notification_delivery_runs_impl(state, headers).await
}

async fn list_approval_notification_channel_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalNotificationChannelPolicy>>, AppError> {
    list_approval_notification_channel_policies_impl(state, headers).await
}

async fn create_approval_notification_channel_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalNotificationChannelPolicy>,
) -> Result<Json<ApprovalNotificationChannelPolicy>, AppError> {
    create_approval_notification_channel_policy_impl(state, headers, input).await
}

async fn archive_approval_notification_channel_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationChannelPolicy>, AppError> {
    archive_approval_notification_channel_policy_impl(state, id, headers).await
}

async fn get_approval_notification_routing_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalNotificationRoutingSummary>, AppError> {
    get_approval_notification_routing_summary_impl(state, headers).await
}
