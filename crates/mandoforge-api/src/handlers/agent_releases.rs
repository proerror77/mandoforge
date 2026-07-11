use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_releases::validate_agent_release_decision;
use crate::{
    AgentRelease, AgentReleaseAutomationRun, AgentReleaseAutomationRunSummary,
    AgentReleaseDeploymentValidationRun, AgentReleaseOrchestrationValidationRun,
    AgentReleaseRolloutSummary, AppError, AppState, AuthorizationRequest, CreateAgentRelease,
    Permission, RejectAgentReleasePromotion, RequestAgentReleasePromotion,
    agent_release_controller_configured, agent_release_controller_required,
    agent_release_deployment_controller_configured, agent_release_deployment_controller_required,
    agent_release_orchestration_controller_configured,
    agent_release_orchestration_controller_required, agent_release_rollback_controller_configured,
    agent_release_rollback_controller_required, authorize_request,
    build_agent_release_automation_run_summary, build_agent_release_rollout_summary,
    dedupe_strings, enforce_resource_scope, execute_agent_release_controller,
    execute_agent_release_deployment_controller, execute_agent_release_orchestration_controller,
    execute_agent_release_rollback_controller, execute_due_agent_release_promotions, new_audit_log,
    normalize_release_automation_policy, optional_trimmed, principal_from_request,
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
            "/api/agents/releases/deployment/validate",
            post(validate_agent_release_deployment),
        )
        .route(
            "/api/agents/releases/orchestration/validate",
            post(validate_agent_release_orchestration),
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

async fn validate_agent_release_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseDeploymentValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent_release".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let audit_logs = state.list_audit_logs(None).await?;
    let rollout_summary =
        build_agent_release_rollout_summary(state.list_all_agent_releases().await?, checked_at);
    let automation_summary =
        build_agent_release_automation_run_summary(&audit_logs, &rollout_summary, checked_at);
    let production_ops = automation_summary.production_ops.clone();
    let production_orchestration = automation_summary.production_orchestration.clone();
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = agent_release_deployment_controller_required(&lookup);
    let controller_configured = agent_release_deployment_controller_configured(&lookup);
    let mut issues = Vec::new();

    if rollout_summary.release_count == 0 {
        issues.push("agent release deployment validation covered no releases".to_string());
    }
    if production_ops.production_blocked {
        issues.push(production_ops.message.clone());
    }
    if production_orchestration.production_blocked {
        issues.push(production_orchestration.message.clone());
    }

    let mut healthy = issues.is_empty();
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "release_deployment_not_ready"
        } else {
            "controller_not_configured"
        }
    });

    if healthy && controller_configured {
        match execute_agent_release_deployment_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &rollout_summary,
            &automation_summary,
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
                    issues.push("agent release deployment controller did not validate".to_string());
                }
            }
            Err(error) => {
                healthy = false;
                issues.push("agent release deployment controller failed".to_string());
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
        issues
            .push("agent release deployment controller is required but not configured".to_string());
    }

    let status = if healthy { "healthy" } else { "blocked" }.to_string();
    let audit_log = state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent.release_deployment_validation_run",
            "agent_release",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "release_count": rollout_summary.release_count,
                "pending_count": rollout_summary.pending_count,
                "promoted_count": rollout_summary.promoted_count,
                "rejected_count": rollout_summary.rejected_count,
                "rolled_back_count": rollout_summary.rolled_back_count,
                "latest_automation_status": automation_summary.latest_run.as_ref().map(|run| run.status.clone()),
                "latest_automation_run_id": automation_summary.latest_run.as_ref().map(|run| run.id),
                "production_ops": production_ops,
                "production_orchestration": production_orchestration,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "issues": issues,
            }),
        ))
        .await?;
    Ok(Json(AgentReleaseDeploymentValidationRun {
        status,
        release_count: rollout_summary.release_count,
        pending_count: rollout_summary.pending_count,
        promoted_count: rollout_summary.promoted_count,
        rejected_count: rollout_summary.rejected_count,
        rolled_back_count: rollout_summary.rolled_back_count,
        latest_automation_status: automation_summary
            .latest_run
            .as_ref()
            .map(|run| run.status.clone()),
        controller_required,
        controller_configured,
        controller_execution,
        issues,
        checked_at: audit_log.created_at,
    }))
}

async fn validate_agent_release_orchestration(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentReleaseOrchestrationValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "agent_release".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let audit_logs = state.list_audit_logs(None).await?;
    let rollout_summary =
        build_agent_release_rollout_summary(state.list_all_agent_releases().await?, checked_at);
    let automation_summary =
        build_agent_release_automation_run_summary(&audit_logs, &rollout_summary, checked_at);
    let production_orchestration = automation_summary.production_orchestration.clone();
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = agent_release_orchestration_controller_required(&lookup);
    let controller_configured = agent_release_orchestration_controller_configured(&lookup);
    let mut issues = Vec::new();

    if rollout_summary.release_count == 0 {
        issues.push("agent release orchestration validation covered no releases".to_string());
    }
    if production_orchestration.production_blocked {
        issues.push(production_orchestration.message.clone());
    }

    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "release_orchestration_not_ready"
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured {
        match execute_agent_release_orchestration_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &rollout_summary,
            &automation_summary,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues.push(
                        "agent release orchestration controller did not validate".to_string(),
                    );
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("agent release orchestration controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    } else if controller_required {
        issues.push(
            "agent release orchestration controller is required but not configured".to_string(),
        );
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push(
            "agent release orchestration controller evidence is missing or not validated"
                .to_string(),
        );
    }
    dedupe_strings(&mut issues);

    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent.release_orchestration_validation_run",
            "agent_release",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "release_count": rollout_summary.release_count,
                "latest_automation_status": automation_summary.latest_run.as_ref().map(|run| run.status.clone()),
                "latest_automation_run_id": automation_summary.latest_run.as_ref().map(|run| run.id),
                "production_orchestration": production_orchestration,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "issues": issues,
            }),
        ))
        .await?;

    Ok(Json(AgentReleaseOrchestrationValidationRun {
        status,
        release_count: rollout_summary.release_count,
        latest_automation_status: automation_summary
            .latest_run
            .as_ref()
            .map(|run| run.status.clone()),
        controller_required,
        controller_configured,
        controller_execution,
        issues,
        checked_at,
    }))
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
    if crate::store_entities::agent_release_enforcement_required() {
        return Err(AppError::forbidden(
            "production agent releases require a release request and independent approval",
        ));
    }
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
    let mut controller_execution = Value::Null;
    if decision == "approve" {
        let release = state
            .list_agent_releases(agent_id)
            .await?
            .into_iter()
            .find(|release| release.id == release_id)
            .ok_or_else(|| AppError::not_found("agent release not found"))?;
        validate_agent_release_decision(&release, &principal.subject_id)?;
        let lookup = |key: &str| std::env::var(key).ok();
        let controller_required = agent_release_controller_required(&lookup);
        let controller_configured = agent_release_controller_configured(&lookup);
        if controller_required && !controller_configured {
            return Err(AppError::bad_request(
                "agent release controller is required but not configured",
            ));
        }
        if controller_configured {
            controller_execution =
                execute_agent_release_controller(&lookup, &release, Utc::now()).await?;
            if controller_execution.get("status").and_then(Value::as_str) != Some("promoted") {
                return Err(AppError::bad_request(
                    "agent release controller did not confirm promotion",
                ));
            }
        }
    }
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
                "controller_execution": controller_execution,
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
