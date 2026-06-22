use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CompileDynamicWorkflowPlan, CreateDynamicWorkflowPlan,
    DynamicWorkflowAdjudicationRequest, DynamicWorkflowAdjudicationResponse, DynamicWorkflowPlan,
    DynamicWorkflowPlanCompilationResponse, DynamicWorkflowPlanMaterializationResponse,
    DynamicWorkflowPressureTestRequest, DynamicWorkflowPressureTestResponse,
    MaterializeDynamicWorkflowPlan, ReviewDynamicWorkflowPlan,
    adjudicate_dynamic_workflow_plan as adjudicate_dynamic_workflow_plan_impl,
    compile_dynamic_workflow_plan as compile_dynamic_workflow_plan_impl,
    create_dynamic_workflow_plan as create_dynamic_workflow_plan_impl,
    get_dynamic_workflow_plan as get_dynamic_workflow_plan_impl,
    list_dynamic_workflow_plans as list_dynamic_workflow_plans_impl,
    materialize_dynamic_workflow_plan as materialize_dynamic_workflow_plan_impl,
    pressure_test_dynamic_workflow_plan as pressure_test_dynamic_workflow_plan_impl,
    review_dynamic_workflow_plan as review_dynamic_workflow_plan_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/dynamic-workflow-plans",
            get(list_dynamic_workflow_plans).post(create_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/compile",
            post(compile_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}",
            get(get_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/review",
            post(review_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/adjudicate",
            post(adjudicate_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/pressure-test",
            post(pressure_test_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/materialize",
            post(materialize_dynamic_workflow_plan),
        )
}

async fn list_dynamic_workflow_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DynamicWorkflowPlan>>, AppError> {
    list_dynamic_workflow_plans_impl(state, headers).await
}

async fn get_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    get_dynamic_workflow_plan_impl(state, id, headers).await
}

async fn create_dynamic_workflow_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    create_dynamic_workflow_plan_impl(state, headers, input).await
}

async fn compile_dynamic_workflow_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CompileDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlanCompilationResponse>, AppError> {
    compile_dynamic_workflow_plan_impl(state, headers, input).await
}

async fn review_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    review_dynamic_workflow_plan_impl(state, id, headers, input).await
}

async fn adjudicate_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DynamicWorkflowAdjudicationRequest>,
) -> Result<Json<DynamicWorkflowAdjudicationResponse>, AppError> {
    adjudicate_dynamic_workflow_plan_impl(state, id, headers, input).await
}

async fn pressure_test_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DynamicWorkflowPressureTestRequest>,
) -> Result<Json<DynamicWorkflowPressureTestResponse>, AppError> {
    pressure_test_dynamic_workflow_plan_impl(state, id, headers, input).await
}

async fn materialize_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<MaterializeDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlanMaterializationResponse>, AppError> {
    materialize_dynamic_workflow_plan_impl(state, id, headers, input).await
}
