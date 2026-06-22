use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, Approval, ApprovalEscalationDueRun, ApprovalEscalationRule,
    ApprovalGroup, CreateApprovalEscalationRule, CreateApprovalGroup, EscalateApproval,
    ModifyApproval, approve as approve_impl, create_approval_escalation_rule as create_approval_escalation_rule_impl,
    create_approval_group as create_approval_group_impl, escalate_approval as escalate_approval_impl,
    expire as expire_impl, list_approval_escalation_rules as list_approval_escalation_rules_impl,
    list_approval_groups as list_approval_groups_impl, list_approvals as list_approvals_impl,
    modify_approval as modify_approval_impl, reject as reject_impl,
    run_due_approval_escalations as run_due_approval_escalations_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve))
        .route("/api/approvals/{id}/reject", post(reject))
        .route("/api/approvals/{id}/expire", post(expire))
        .route("/api/approvals/{id}/modify", post(modify_approval))
        .route("/api/approvals/{id}/escalate", post(escalate_approval))
        .route(
            "/api/approvals/escalations/run-due",
            post(run_due_approval_escalations),
        )
        .route(
            "/api/approval-groups",
            get(list_approval_groups).post(create_approval_group),
        )
        .route(
            "/api/approval-escalation-rules",
            get(list_approval_escalation_rules).post(create_approval_escalation_rule),
        )
}

async fn list_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Approval>>, AppError> {
    list_approvals_impl(state, headers).await
}

async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    approve_impl(state, id, headers).await
}

async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    reject_impl(state, id, headers).await
}

async fn expire(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    expire_impl(state, id, headers).await
}

async fn modify_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ModifyApproval>,
) -> Result<Json<Approval>, AppError> {
    modify_approval_impl(state, id, headers, input).await
}

async fn escalate_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EscalateApproval>,
) -> Result<Json<Approval>, AppError> {
    escalate_approval_impl(state, id, headers, input).await
}

async fn run_due_approval_escalations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalEscalationDueRun>, AppError> {
    run_due_approval_escalations_impl(state, headers).await
}

async fn list_approval_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalGroup>>, AppError> {
    list_approval_groups_impl(state, headers).await
}

async fn create_approval_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalGroup>,
) -> Result<Json<ApprovalGroup>, AppError> {
    create_approval_group_impl(state, headers, input).await
}

async fn list_approval_escalation_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalEscalationRule>>, AppError> {
    list_approval_escalation_rules_impl(state, headers).await
}

async fn create_approval_escalation_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalEscalationRule>,
) -> Result<Json<ApprovalEscalationRule>, AppError> {
    create_approval_escalation_rule_impl(state, headers, input).await
}
