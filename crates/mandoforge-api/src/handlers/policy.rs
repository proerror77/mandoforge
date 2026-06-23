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
    AppError, AppState, AuthorizationRequest, CreatePolicyRevision, Permission, PolicyRevision,
    PolicyRevisionDiff, PolicyRevisionGate, PolicyRevisionGateRequest, PolicyRollbackResult,
    PolicyRolloutOrchestrationReadiness, PolicyRolloutOrchestrationValidationRun,
    PolicyRuntimeStatus, PolicyScheduledRolloutRun, PolicyTestResult, SimulatePolicy,
    TestPolicyRequest, activate_policy_revision_for_runtime, authorize_request,
    build_policy_revision_diff, build_policy_revision_gate,
    build_policy_rollout_orchestration_readiness, dedupe_strings, enforce_resource_scope,
    execute_due_policy_rollouts, execute_policy_rollout_orchestration_controller, new_audit_log,
    policy, policy_revision_rollout_percent, policy_rollout_orchestration_controller_configured,
    policy_rollout_orchestration_controller_required,
    policy_rollout_orchestration_execution_is_production_target, principal_from_request,
    validate_policy_revision_input,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/policy", get(get_policy))
        .route("/api/policy/runtime", get(get_policy_runtime))
        .route("/api/policy/rollout/cancel", post(cancel_policy_rollout))
        .route(
            "/api/policy/rollout/orchestration/readiness",
            get(get_policy_rollout_orchestration_readiness),
        )
        .route(
            "/api/policy/rollout/orchestration/validate",
            post(validate_policy_rollout_orchestration),
        )
        .route(
            "/api/policy/rollout/rollback",
            post(rollback_policy_rollout),
        )
        .route("/api/policy/rollout/run-due", post(run_due_policy_rollouts))
        .route("/api/policy/simulate", post(simulate_policy))
        .route("/api/policy/test", post(test_policy))
        .route(
            "/api/policy/revisions",
            get(list_policy_revisions).post(create_policy_revision),
        )
        .route(
            "/api/policy/revisions/{id}/activate",
            post(activate_policy_revision),
        )
        .route("/api/policy/revisions/{id}/diff", get(diff_policy_revision))
        .route(
            "/api/policy/revisions/{id}/gate",
            post(gate_policy_revision),
        )
}

async fn get_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "policy", None).await?;
    Ok(Json(serde_json::to_value(state.active_policy().await)?))
}

async fn get_policy_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRuntimeStatus>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "policy", None).await?;
    Ok(Json(state.policy_runtime_status().await))
}

async fn cancel_policy_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRuntimeStatus>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let before = state.policy_runtime_status().await;
    let status = state.cancel_staged_policy_rollout().await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.rollout_cancelled",
            "policy",
            before.staged_revision_id,
            json!({
                "subject": principal.subject_id,
                "staged_revision_id": before.staged_revision_id,
                "staged_rollout_percent": before.staged_rollout_percent,
                "active_revision_id": status.active_revision_id
            }),
        ))
        .await?;
    Ok(Json(status))
}

async fn get_policy_rollout_orchestration_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRolloutOrchestrationReadiness>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "policy", None).await?;
    let lookup = |key: &str| std::env::var(key).ok();
    Ok(Json(build_policy_rollout_orchestration_readiness(
        &state.policy_runtime_status().await,
        &state.list_audit_logs(None).await?,
        Utc::now(),
        policy_rollout_orchestration_controller_required(&lookup),
        policy_rollout_orchestration_controller_configured(&lookup),
    )))
}

async fn validate_policy_rollout_orchestration(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRolloutOrchestrationValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;

    let checked_at = Utc::now();
    let runtime = state.policy_runtime_status().await;
    let audit_logs = state.list_audit_logs(None).await?;
    let lookup = |key: &str| std::env::var(key).ok();
    let controller_required = policy_rollout_orchestration_controller_required(&lookup);
    let controller_configured = policy_rollout_orchestration_controller_configured(&lookup);
    let readiness = build_policy_rollout_orchestration_readiness(
        &runtime,
        &audit_logs,
        checked_at,
        controller_required,
        controller_configured,
    );
    let mut issues = readiness.blocking_reasons.clone();
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "policy_rollout_orchestration_not_ready"
        } else {
            "controller_not_configured"
        }
    });

    if controller_configured {
        match execute_policy_rollout_orchestration_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &runtime,
            &readiness,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues.push(
                        "policy rollout orchestration controller did not validate".to_string(),
                    );
                }
                if execution.get("status").and_then(Value::as_str) == Some("validated")
                    && !policy_rollout_orchestration_execution_is_production_target(&execution)
                {
                    issues.push(
                        "policy rollout orchestration controller did not identify a real production policy controller target"
                            .to_string(),
                    );
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("policy rollout orchestration controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues.push(
            "policy rollout orchestration controller evidence is missing or not validated"
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
            "policy.rollout_orchestration_validation_run",
            "policy",
            runtime.active_revision_id,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "rollout_active": runtime.rollout_active,
                "active_revision_id": runtime.active_revision_id,
                "staged_revision_id": runtime.staged_revision_id,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "issues": issues,
                "checked_at": checked_at,
            }),
        ))
        .await?;

    Ok(Json(PolicyRolloutOrchestrationValidationRun {
        status,
        rollout_active: runtime.rollout_active,
        active_revision_id: runtime.active_revision_id,
        staged_revision_id: runtime.staged_revision_id,
        controller_required,
        controller_configured,
        controller_execution,
        issues,
        checked_at,
    }))
}

async fn rollback_policy_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRollbackResult>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let runtime = state.policy_runtime_status().await;
    if runtime.rollout_active {
        return Err(AppError::bad_request(
            "cancel staged policy rollout before rollback",
        ));
    }
    let current_id = runtime
        .active_revision_id
        .ok_or_else(|| AppError::bad_request("no active policy revision to roll back"))?;
    let target = state
        .previous_activated_policy_revision(current_id)
        .await?
        .ok_or_else(|| AppError::bad_request("no previous policy revision to roll back to"))?;
    let active_revision = state
        .rollback_policy_revision(current_id, target.id)
        .await?;
    state.rollback_runtime_policy(&active_revision).await?;
    let result = PolicyRollbackResult {
        rolled_back_from_revision_id: current_id,
        active_revision_id: active_revision.id,
        active_revision,
        rolled_back_at: Utc::now(),
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.rollback_completed",
            "policy_revision",
            Some(result.active_revision_id),
            json!({
                "subject": principal.subject_id,
                "rolled_back_from_revision_id": result.rolled_back_from_revision_id,
                "active_revision_id": result.active_revision_id,
                "rolled_back_at": result.rolled_back_at
            }),
        ))
        .await?;
    Ok(Json(result))
}

async fn run_due_policy_rollouts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyScheduledRolloutRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    Ok(Json(
        execute_due_policy_rollouts(&state, &principal.subject_id, "user").await?,
    ))
}

async fn simulate_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SimulatePolicy>,
) -> Result<Json<policy::ToolPolicyDecision>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let tool_name = input.tool_name.trim();
    if tool_name.is_empty() {
        return Err(AppError::bad_request("tool_name is required"));
    }
    let policy = state.active_policy().await;
    let decision = policy.evaluate_tool(tool_name);
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.simulated",
            "policy",
            None,
            json!({
                "subject": principal.subject_id,
                "tool_name": tool_name,
                "decision": decision.decision,
                "risk_level": decision.risk_level,
                "reason": decision.reason
            }),
        ))
        .await?;
    Ok(Json(decision))
}

async fn test_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestPolicyRequest>,
) -> Result<Json<PolicyTestResult>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let tool_names: Vec<_> = input
        .tool_names
        .iter()
        .map(|tool| tool.trim())
        .filter(|tool| !tool.is_empty())
        .collect();
    if tool_names.is_empty() {
        return Err(AppError::bad_request(
            "tool_names must include at least one tool",
        ));
    }
    if tool_names.len() > 50 {
        return Err(AppError::bad_request(
            "policy test supports at most 50 tool names",
        ));
    }
    let policy = state.active_policy().await;
    let decisions: Vec<_> = tool_names
        .iter()
        .map(|tool_name| policy.evaluate_tool(tool_name))
        .collect();
    let result = PolicyTestResult {
        decisions,
        tested_at: Utc::now(),
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.tested",
            "policy",
            None,
            json!({
                "subject": principal.subject_id,
                "tool_names": tool_names,
                "decision_count": result.decisions.len()
            }),
        ))
        .await?;
    Ok(Json(result))
}

async fn list_policy_revisions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PolicyRevision>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "policy", None).await?;
    Ok(Json(state.list_policy_revisions().await?))
}

async fn create_policy_revision(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreatePolicyRevision>,
) -> Result<Json<PolicyRevision>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let input = validate_policy_revision_input(input)?;
    let revision = state
        .create_policy_revision(input, principal.subject_id.clone())
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.revision_created",
            "policy_revision",
            Some(revision.id),
            json!({
                "subject": principal.subject_id,
                "name": revision.name,
                "status": revision.status
            }),
        ))
        .await?;
    Ok(Json(revision))
}

async fn activate_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PolicyRevision>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy_revision".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let revision = activate_policy_revision_for_runtime(&state, id).await?;
    let rollout_percent = policy_revision_rollout_percent(&revision);
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.revision_activated",
            "policy_revision",
            Some(revision.id),
            json!({
                "subject": principal.subject_id,
                "name": revision.name,
                "status": revision.status,
                "rollout_percent": rollout_percent,
                "activated_at": revision.activated_at
            }),
        ))
        .await?;
    Ok(Json(revision))
}

async fn diff_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PolicyRevisionDiff>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "policy_revision",
        Some(id),
    )
    .await?;
    let revision = state.get_policy_revision(id).await?;
    let policy = state.active_policy().await;
    Ok(Json(build_policy_revision_diff(&policy, &revision)?))
}

async fn gate_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    input: Option<Json<PolicyRevisionGateRequest>>,
) -> Result<Json<PolicyRevisionGate>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "policy_revision".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let revision = state.get_policy_revision(id).await?;
    let policy = state.active_policy().await;
    let gate = build_policy_revision_gate(
        &policy,
        &revision,
        input.map(|Json(input)| input).unwrap_or_default(),
    )?;
    state.update_policy_revision_gate(&gate).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "policy.revision_gated",
            "policy_revision",
            Some(revision.id),
            json!({
                "subject": principal.subject_id,
                "name": revision.name,
                "status": gate.status,
                "suite_source": gate.suite_source,
                "rollout_percent": gate.rollout_percent,
                "case_count": gate.cases.len(),
                "change_count": gate.diff.changes.len()
            }),
        ))
        .await?;
    Ok(Json(gate))
}
