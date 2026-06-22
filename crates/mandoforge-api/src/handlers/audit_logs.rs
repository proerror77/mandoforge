use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::{
    AppError, AppState, AuditLog, list_audit_logs as list_audit_logs_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/audit-logs", get(list_audit_logs))
}

async fn list_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    list_audit_logs_impl(state, headers).await
}
