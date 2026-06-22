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
    AgentRelease, AgentReleaseAutomationRun, AgentReleaseAutomationRunSummary,
    AgentReleaseRolloutSummary, AppError, AppState, AuthorizationRequest, CreateAgentRelease,
    Permission, RejectAgentReleasePromotion, RequestAgentReleasePromotion,
    agent_release_rollback_controller_configured, agent_release_rollback_controller_required,
    authorize_request, build_agent_release_automation_run_summary,
    build_agent_release_rollout_summary, enforce_resource_scope,
    execute_agent_release_rollback_controller, execute_due_agent_release_promotions,
    new_audit_log, normalize_release_automation_policy, optional_trimmed, principal_from_request,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/agents/releases/summary",
            get(get_agent_release_rollout_summary),
        )
        .route(
            "/api/agents/releases/automation-runs",
            get(get_agent_release_automation_runs),
        )
        .route(
            "/api/agents/releases/run-due",
            post(run_due_agent_release_promotions),
        )
        .route(
            "/api/agents/{id}/releases",
            get(list_agent_releases).post(create_agent_release),
        )
        .route(
            "/api/agents/{id}/release-requests",
            post(request_agent_release_promotion),
        )
        .route(
            "/api/agents/{id}/releases/{release_id}/approve",
            post(approve_agent_release_promotion),
        )
        .route(
            "/api/agents/{id}/releases/{release_id}/reject",
            post(reject_agent_release_promotion),
        )
        .route(
            "/api/agents/{id}/releases/{release_id}/rollback",
            post(rollback_agent_release),
        )
}

async fn list_agent_releases(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRelease>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.list_agent_releases(id).await?))
}

async fn get_agent_release_rollout_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseRolloutSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "agent_release", None).await?;
    Ok(Json(build_agent_release_rollout_summary(
        state.list_all_agent_releases().await?,
        Utc::now(),
    )))
}

async fn get_agent_release_automation_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseAutomationRunSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "agent_release", None).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let rollout_summary =
        build_agent_release_rollout_summary(state.list_all_agent_releases().await?, Utc::now());
    Ok(Json(build_agent_release_automation_run_summary(
        &audit_logs,
        &rollout_summary,
        Utc::now(),
    )))
}

async fn run_due_agent_release_promotions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseAutomationRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "agent_release", None).await?;
    Ok(Json(execute_due_agent_release_promotions(&state).await?))
}

async fn create_agent_release(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentRelease>,
) -> Result<Json<AgentRelease>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        state
            .create_agent_release(id, input, principal.subject_id)
            .await?,
    ))
}

async fn request_agent_release_promotion(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RequestAgentReleasePromotion>,
) -> Result<Json<AgentRelease>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let release = state
        .request_agent_release_promotion(
            id,
            input.release,
            principal.subject_id.clone(),
            optional_trimmed(input.approver_subject.as_deref()),
            optional_trimmed(input.reason.as_deref()),
            normalize_release_automation_policy(
                input.auto_approve,
                input.activate_after.as_deref(),
                input.expires_at.as_deref(),
            )?,
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent.release_promotion_requested",
            "agent_release",
            Some(release.id),
            json!({
                "subject": principal.subject_id,
                "agent_id": id,
                "environment": release.environment,
                "eval_run_id": release.eval_run_id,
                "min_score": release.min_score,
                "approver_subject": release.approver_subject,
            }),
        ))
        .await?;
    Ok(Json(release))
}

async fn approve_agent_release_promotion(
    State(state): State<AppState>,
    Path((id, release_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<AgentRelease>, AppError> {
    decide_agent_release_promotion(state, id, release_id, headers, "approve", None).await
}

async fn reject_agent_release_promotion(
    State(state): State<AppState>,
    Path((id, release_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<RejectAgentReleasePromotion>,
) -> Result<Json<AgentRelease>, AppError> {
    decide_agent_release_promotion(
        state,
        id,
        release_id,
        headers,
        "reject",
        optional_trimmed(input.reason.as_deref()),
    )
    .await
}

async fn decide_agent_release_promotion(
    state: AppState,
    agent_id: Uuid,
    release_id: Uuid,
    headers: HeaderMap,
    decision: &str,
    reason: Option<String>,
) -> Result<Json<AgentRelease>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent".to_string(),
        resource_id: Some(agent_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let release = match decision {
        "approve" => {
            state
                .approve_agent_release_promotion(agent_id, release_id, principal.subject_id.clone())
                .await?
        }
        "reject" => {
            state
                .reject_agent_release_promotion(
                    agent_id,
                    release_id,
                    principal.subject_id.clone(),
                    reason,
                )
                .await?
        }
        _ => return Err(AppError::bad_request("unsupported release decision")),
    };
    let action = if decision == "approve" {
        "agent.release_promotion_approved"
    } else {
        "agent.release_promotion_rejected"
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "agent_release",
            Some(release.id),
            json!({
                "subject": principal.subject_id,
                "agent_id": agent_id,
                "environment": release.environment,
                "status": release.status,
                "requested_by": release.requested_by,
                "decision_by": release.decision_by,
            }),
        ))
        .await?;
    Ok(Json(release))
}

async fn rollback_agent_release(
    State(state): State<AppState>,
    Path((id, release_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<AgentRelease>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let release = state
        .list_agent_releases(id)
        .await?
        .into_iter()
        .find(|release| release.id == release_id)
        .ok_or_else(|| AppError::not_found("agent release not found"))?;
    if release.status != "promoted" {
        return Err(AppError::bad_request("agent release is not promoted"));
    }

    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = agent_release_rollback_controller_required(&lookup);
    let controller_configured = agent_release_rollback_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": "controller_not_configured"
    });
    if controller_required && !controller_configured {
        return Err(AppError::bad_request(
            "agent release rollback controller is required but not configured",
        ));
    }
    if controller_configured {
        controller_execution = execute_agent_release_rollback_controller(
            &lookup,
            &principal.subject_id,
            Utc::now(),
            &release,
        )
        .await?;
        if controller_execution.get("status").and_then(Value::as_str) != Some("rolled_back") {
            return Err(AppError::bad_request(
                "agent release rollback controller did not confirm rollback",
            ));
        }
    }

    let rolled_back = state.rollback_agent_release(id, release_id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent.release_rolled_back",
            "agent_release",
            Some(rolled_back.id),
            json!({
                "subject": principal.subject_id,
                "agent_id": id,
                "agent_version_id": rolled_back.agent_version_id,
                "environment": rolled_back.environment,
                "eval_run_id": rolled_back.eval_run_id,
                "eval_score": rolled_back.eval_score,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
            }),
        ))
        .await?;
    Ok(Json(rolled_back))
}
