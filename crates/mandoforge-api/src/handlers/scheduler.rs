use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::json;

use crate::{
    AppError, AppState, AuthorizationRequest, Permission, SchedulerDeploymentValidationRun,
    SchedulerDuePlan, SchedulerDueRun, SchedulerOrchestrationSummary, SchedulerRunDueRequest,
    authorize_request, build_scheduler_due_plan, build_scheduler_orchestration_summary,
    enforce_resource_scope, execute_scheduler_deployment_controller, execute_scheduler_due_tasks,
    new_audit_log, principal_from_request, scheduler_deployment_controller_configured,
    scheduler_deployment_controller_required, scheduler_deployment_readiness_from_manifests,
    validate_scheduler_shared_token,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/scheduler/summary", get(get_scheduler_summary))
        .route("/api/scheduler/due-plan", get(get_scheduler_due_plan))
        .route(
            "/api/scheduler/deployment/validate",
            post(validate_scheduler_deployment),
        )
        .route("/api/scheduler/run-due", post(run_scheduler_due_tasks))
}

async fn get_scheduler_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerOrchestrationSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "scheduler", None).await?;
    Ok(Json(build_scheduler_orchestration_summary(&state).await?))
}

async fn get_scheduler_due_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerDuePlan>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "scheduler", None).await?;
    Ok(Json(build_scheduler_due_plan(&state).await?))
}

async fn validate_scheduler_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SchedulerDeploymentValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "scheduler".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let lookup = |key: &str| std::env::var(key).ok();
    let audit_logs = state.list_audit_logs(None).await?;
    let readiness = scheduler_deployment_readiness_from_manifests(&audit_logs, checked_at, &lookup);
    let controller_required = scheduler_deployment_controller_required(&lookup);
    let controller_configured = scheduler_deployment_controller_configured(&lookup);
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "not_attempted"
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured {
        match execute_scheduler_deployment_controller(
            &lookup,
            Some(principal.subject_id.as_str()),
            checked_at,
            &readiness,
        )
        .await
        {
            Ok(execution) => {
                controller_execution = execution;
            }
            Err(error) => {
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message,
                });
            }
        }
    }
    let controller_status = controller_execution
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("skipped");
    let status = if readiness.blocking_reasons.iter().any(|reason| {
        reason != "scheduler deployment controller has no recent validated evidence"
            && reason != "scheduler deployment controller evidence is stale"
    }) {
        "blocked"
    } else if controller_required && !controller_configured {
        "blocked"
    } else if controller_required && controller_status != "validated" {
        "blocked"
    } else if controller_configured && controller_status != "validated" {
        "failed"
    } else {
        "validated"
    }
    .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "scheduler.deployment_validation_run",
            "scheduler",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "readiness_status": readiness.status.clone(),
                "blocking_reasons": readiness.blocking_reasons.clone(),
                "checked_at": checked_at,
            }),
        ))
        .await?;
    Ok(Json(SchedulerDeploymentValidationRun {
        status,
        checked_at,
        controller_required,
        controller_configured,
        controller_execution,
        readiness_status: readiness.status,
        blocking_reasons: readiness.blocking_reasons,
    }))
}

async fn run_scheduler_due_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    input: Option<Json<SchedulerRunDueRequest>>,
) -> Result<Json<SchedulerDueRun>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "scheduler", None).await?;
    validate_scheduler_shared_token(&headers)?;
    Ok(Json(
        execute_scheduler_due_tasks(&state, input.map(|Json(input)| input)).await?,
    ))
}
