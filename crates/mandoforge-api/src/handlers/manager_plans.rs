use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateManagerAgentPlan, ManagerAgentPlan, ReviewManagerAgentPlan,
    create_manager_agent_plan as create_manager_agent_plan_impl,
    get_manager_agent_plan as get_manager_agent_plan_impl,
    list_manager_agent_plans as list_manager_agent_plans_impl,
    list_session_manager_agent_plans as list_session_manager_agent_plans_impl,
    list_work_item_manager_agent_plans as list_work_item_manager_agent_plans_impl,
    review_manager_agent_plan as review_manager_agent_plan_impl,
};

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
    list_manager_agent_plans_impl(state, headers).await
}

async fn list_session_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    list_session_manager_agent_plans_impl(state, id, headers).await
}

async fn list_work_item_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    list_work_item_manager_agent_plans_impl(state, id, headers).await
}

async fn get_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    get_manager_agent_plan_impl(state, id, headers).await
}

async fn create_manager_agent_plan(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    create_manager_agent_plan_impl(state, session_id, headers, input).await
}

async fn review_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    review_manager_agent_plan_impl(state, id, headers, input).await
}
