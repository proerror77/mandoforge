use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, BootstrapEvalSuite, CreateEvalCase, CreateEvalDataset,
    CreateEvalJudgeProfile, CreateEvalRun, EvalCase, EvalDataset, EvalDriftDecision,
    EvalGateDecision, EvalGateRequest, EvalRun, EvalSuiteBootstrap, Permission, ProviderRecord,
    authorize_request, bootstrap_stage2_eval_suite as bootstrap_stage2_eval_suite_impl,
    create_eval_case as create_eval_case_impl, create_eval_dataset as create_eval_dataset_impl,
    create_eval_judge_profile as create_eval_judge_profile_impl,
    create_eval_run as create_eval_run_impl, gate_eval_run as gate_eval_run_impl,
    get_eval_run_drift as get_eval_run_drift_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/eval/datasets",
            get(list_eval_datasets).post(create_eval_dataset),
        )
        .route(
            "/api/eval/datasets/{id}/cases",
            get(list_eval_cases).post(create_eval_case),
        )
        .route(
            "/api/eval/datasets/{id}/runs",
            get(list_dataset_eval_runs).post(create_eval_run),
        )
        .route(
            "/api/eval/judge-profiles",
            get(list_eval_judge_profiles).post(create_eval_judge_profile),
        )
        .route(
            "/api/eval/suites/stage2-regression",
            post(bootstrap_stage2_eval_suite),
        )
        .route("/api/eval/runs", get(list_eval_runs))
        .route("/api/eval/runs/{id}/gate", post(gate_eval_run))
        .route("/api/eval/runs/{id}/drift", get(get_eval_run_drift))
}

async fn list_eval_datasets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalDataset>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_datasets", None).await?;
    Ok(Json(state.list_eval_datasets().await?))
}

async fn create_eval_dataset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalDataset>,
) -> Result<Json<EvalDataset>, AppError> {
    create_eval_dataset_impl(state, headers, input).await
}

async fn list_eval_cases(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalCase>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_eval_cases(id).await?))
}

async fn create_eval_case(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalCase>,
) -> Result<Json<EvalCase>, AppError> {
    create_eval_case_impl(state, id, headers, input).await
}

async fn list_dataset_eval_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalRun>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_eval_runs(Some(id)).await?))
}

async fn create_eval_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalRun>,
) -> Result<Json<EvalRun>, AppError> {
    create_eval_run_impl(state, id, headers, input).await
}

async fn list_eval_judge_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderRecord>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_judge_profiles",
        None,
    )
    .await?;
    let mut profiles: Vec<_> = state
        .list_providers()
        .await?
        .into_iter()
        .filter(|provider| provider.provider_type == "eval_judge")
        .collect();
    profiles.sort_by_key(|profile| profile.created_at);
    profiles.reverse();
    Ok(Json(profiles))
}

async fn create_eval_judge_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalJudgeProfile>,
) -> Result<Json<ProviderRecord>, AppError> {
    create_eval_judge_profile_impl(state, headers, input).await
}

async fn bootstrap_stage2_eval_suite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BootstrapEvalSuite>,
) -> Result<Json<EvalSuiteBootstrap>, AppError> {
    bootstrap_stage2_eval_suite_impl(state, headers, input).await
}

async fn list_eval_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalRun>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_runs", None).await?;
    Ok(Json(state.list_eval_runs(None).await?))
}

async fn gate_eval_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EvalGateRequest>,
) -> Result<Json<EvalGateDecision>, AppError> {
    gate_eval_run_impl(state, id, headers, input).await
}

async fn get_eval_run_drift(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<EvalDriftDecision>, AppError> {
    get_eval_run_drift_impl(state, id, headers).await
}
