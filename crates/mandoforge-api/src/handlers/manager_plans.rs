use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateManagerAgentPlan, ManagerAgentPlan, Permission,
    ReviewManagerAgentPlan, authorize_collection_request, authorize_request,
    record_manager_agent_plan_audit_and_event, record_manager_agent_plan_work_item_activity,
    visible_session_ids_for_principal,
};

fn normalize_manager_plan_risk(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "risk_classification must be one of low, medium, or high",
        )),
    }
}

fn normalize_manager_plan_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "planned" | "reviewed" | "approved" | "needs_changes" | "blocked" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "manager agent plan status must be planned, reviewed, approved, needs_changes, or blocked",
        )),
    }
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/sessions/{id}/manager-plans",
            get(list_session_manager_agent_plans).post(create_manager_agent_plan),
        )
        .route(
            "/api/work-items/{id}/manager-plans",
            get(list_work_item_manager_agent_plans),
        )
        .route("/api/manager-plans", get(list_manager_agent_plans))
        .route("/api/manager-plans/{id}", get(get_manager_agent_plan))
        .route(
            "/api/manager-plans/{id}/review",
            post(review_manager_agent_plan),
        )
}

async fn list_manager_agent_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "manager_agent_plans",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_manager_agent_plans(None)
            .await?
            .into_iter()
            .filter(|plan| visible_session_ids.contains(&plan.session_id))
            .collect(),
    ))
}

async fn list_session_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_manager_agent_plans(Some(id)).await?))
}

async fn list_work_item_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_manager_agent_plans(id).await?))
}

async fn get_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    let plan = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(plan.session_id),
    )
    .await?;
    Ok(Json(plan))
}

async fn create_manager_agent_plan(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    let session = state.get_session(session_id).await?;
    let manager_agent = state.get_agent(session.agent_id).await?;
    if manager_agent.agent_role != "manager" {
        return Err(AppError::bad_request(
            "manager agent plans require a session bound to a manager agent",
        ));
    }
    if !input.task_intake.is_object()
        || !input.decomposition.is_object()
        || !input.specialist_selection.is_object()
        || !input.review.is_object()
    {
        return Err(AppError::bad_request(
            "manager agent plan sections must be JSON objects",
        ));
    }
    let risk_classification = normalize_manager_plan_risk(&input.risk_classification)?;
    if let Some(specialist_agent_id) = input.specialist_agent_id {
        let specialist = state.get_agent(specialist_agent_id).await?;
        if specialist.agent_role != "specialist" {
            return Err(AppError::bad_request(
                "specialist_agent_id must reference a specialist agent",
            ));
        }
    }
    if let Some(work_item_id) = input.work_item_id {
        state.ensure_work_item_exists(work_item_id).await?;
    }
    let now = Utc::now();
    let plan = state
        .create_manager_agent_plan(ManagerAgentPlan {
            id: Uuid::new_v4(),
            session_id,
            manager_agent_id: manager_agent.id,
            work_item_id: input.work_item_id,
            specialist_agent_id: input.specialist_agent_id,
            task_intake: input.task_intake,
            decomposition: input.decomposition,
            specialist_selection: input.specialist_selection,
            risk_classification,
            review: input.review,
            status: "planned".to_string(),
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let audit =
        record_manager_agent_plan_audit_and_event(&state, &plan, "manager_plan.created").await?;
    record_manager_agent_plan_work_item_activity(&state, &plan, "manager_plan.created").await?;
    let plan = state
        .update_manager_agent_plan_review(
            plan.id,
            plan.review.clone(),
            plan.status.clone(),
            Some(audit.id),
        )
        .await?;
    Ok(Json(plan))
}

async fn review_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    let current = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(current.session_id),
    )
    .await?;
    if !input.review.is_object() {
        return Err(AppError::bad_request(
            "manager agent plan review must be a JSON object",
        ));
    }
    let status = match input.status {
        Some(status) => normalize_manager_plan_status(&status)?,
        None => "reviewed".to_string(),
    };
    let reviewed = state
        .update_manager_agent_plan_review(current.id, input.review, status, current.audit_trace_id)
        .await?;
    let audit =
        record_manager_agent_plan_audit_and_event(&state, &reviewed, "manager_plan.reviewed")
            .await?;
    record_manager_agent_plan_work_item_activity(&state, &reviewed, "manager_plan.reviewed")
        .await?;
    let reviewed = state
        .update_manager_agent_plan_review(
            reviewed.id,
            reviewed.review.clone(),
            reviewed.status.clone(),
            Some(audit.id),
        )
        .await?;
    Ok(Json(reviewed))
}
