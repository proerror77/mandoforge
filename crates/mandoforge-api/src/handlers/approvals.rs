use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AuthorizationRequest,
    AppError, AppState, Approval, ApprovalEscalationDueRun, ApprovalEscalationRule,
    ApprovalGroup, CreateApprovalEscalationRule, CreateApprovalGroup, EscalateApproval,
    ModifyApproval, Permission, approval_is_expired, authorize_approval_decision,
    authorize_collection_request, authorize_request, decide_approval, enforce_resource_scope,
    escalate_approval_record, execute_due_approval_escalations, expire_approval_record,
    new_audit_log, principal_from_request, refresh_tool_call_commit_binding_if_required,
    validate_approval_escalation_rule_input, validate_approval_group_input,
    visible_session_ids_for_principal,
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
    let principal =
        authorize_collection_request(&state, &headers, Permission::SessionsRead, "approvals")
            .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_approvals()
            .await?
            .into_iter()
            .filter(|approval| visible_session_ids.contains(&approval.session_id))
            .collect(),
    ))
}

async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    let principal = authorize_approval_decision(&state, &headers, id).await?;
    decide_approval(state, id, "approved", Some(principal.subject_id)).await
}

async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    decide_approval(state, id, "rejected", None).await
}

async fn expire(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    Ok(Json(expire_approval_record(&state, id).await?))
}

async fn modify_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ModifyApproval>,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    let approval = state.get_approval(id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be modified",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
    if let Some(tool_call_id) = approval.tool_call_id {
        let tool_call = state
            .update_tool_call_args(tool_call_id, input.args.clone())
            .await?;
        refresh_tool_call_commit_binding_if_required(&state, tool_call).await?;
    }
    let updated = state
        .modify_approval(id, input.args.clone(), input.comment.clone())
        .await?;
    state
        .append_event(
            "user",
            Some(id),
            updated.session_id,
            "approval.modified",
            json!({
                "approval_id": id,
                "tool_call_id": updated.tool_call_id,
                "modified_args": input.args,
                "comment": input.comment,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "user",
            Some(id),
            "approval.modified",
            "approval",
            Some(id),
            json!({
                "tool_call_id": updated.tool_call_id,
                "action": updated.action,
                "comment": updated.decision_payload.get("comment").cloned().unwrap_or(Value::Null),
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn escalate_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EscalateApproval>,
) -> Result<Json<Approval>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let approval = state.get_approval(id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be escalated",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
    let (group, rule_id) = if let Some(group_id) = input.group_id {
        (state.get_approval_group(group_id).await?, None)
    } else {
        let rule = state
            .first_active_escalation_rule_for_risk(&approval.risk_level)
            .await?
            .ok_or_else(|| AppError::bad_request("no active escalation rule for approval risk"))?;
        (
            state.get_approval_group(rule.group_id).await?,
            Some(rule.id),
        )
    };
    if group.status != "active" {
        return Err(AppError::bad_request("approval group is not active"));
    }
    let updated = escalate_approval_record(
        &state,
        &approval,
        &group,
        rule_id,
        input
            .reason
            .unwrap_or_else(|| "Manual escalation".to_string()),
        principal.subject_id,
        "user",
        Some(id),
    )
    .await?;
    Ok(Json(updated))
}

async fn run_due_approval_escalations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApprovalEscalationDueRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "approval_escalation_rules",
        None,
    )
    .await?;
    Ok(Json(execute_due_approval_escalations(&state).await?))
}

async fn list_approval_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalGroup>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "approval_groups", None).await?;
    Ok(Json(state.list_approval_groups().await?))
}

async fn create_approval_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalGroup>,
) -> Result<Json<ApprovalGroup>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_groups".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let group = state
        .create_approval_group(validate_approval_group_input(input)?)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.group_created",
            "approval_group",
            Some(group.id),
            json!({
                "subject": principal.subject_id,
                "name": group.name,
                "subject_count": group.subjects.len()
            }),
        ))
        .await?;
    Ok(Json(group))
}

async fn list_approval_escalation_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalEscalationRule>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "approval_escalation_rules",
        None,
    )
    .await?;
    Ok(Json(state.list_approval_escalation_rules().await?))
}

async fn create_approval_escalation_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateApprovalEscalationRule>,
) -> Result<Json<ApprovalEscalationRule>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "approval_escalation_rules".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let rule = state
        .create_approval_escalation_rule(validate_approval_escalation_rule_input(input)?)
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "approval.escalation_rule_created",
            "approval_escalation_rule",
            Some(rule.id),
            json!({
                "subject": principal.subject_id,
                "name": rule.name,
                "risk_level": rule.risk_level,
                "group_id": rule.group_id
            }),
        ))
        .await?;
    Ok(Json(rule))
}
