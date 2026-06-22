use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::{
    AppError, AppState, AuditLog, Permission, Role, authorize_collection_request,
    visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/audit-logs", get(list_audit_logs))
}

async fn list_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    let principal =
        authorize_collection_request(&state, &headers, Permission::AuditRead, "audit_logs").await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_audit_logs(None)
            .await?
            .into_iter()
            .filter(|log| {
                log.session_id
                    .map(|session_id| visible_session_ids.contains(&session_id))
                    .unwrap_or_else(|| principal.roles.contains(&Role::Admin))
            })
            .collect(),
    ))
}
