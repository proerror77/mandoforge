use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, AuthorizationRequest, BootstrapEvalSuite, CreateEvalCase,
    CreateEvalDataset, CreateEvalJudgeProfile, CreateEvalRun, CreateProviderRecord, EvalCase,
    EvalDataset, EvalDriftDecision, EvalGateDecision, EvalGateRequest, EvalRun, EvalSuiteBootstrap,
    Permission, ProviderRecord, authorize_request, build_eval_drift_decision,
    build_eval_gate_decision, enforce_resource_scope, new_audit_log,
    normalize_provider_api_key_ref, optional_trimmed, principal_from_request, required_trimmed,
    stage2_regression_suite_cases,
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
    authorize_request(&state, &headers, Permission::Admin, "eval_datasets", None).await?;
    Ok(Json(state.create_eval_dataset(input).await?))
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_eval_case(id, input).await?))
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_eval_run(id, input).await?))
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
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "eval_judge_profile".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let name = required_trimmed(&input.name, "name")?;
    let endpoint = required_trimmed(&input.endpoint, "endpoint")?;
    let model = required_trimmed(&input.model, "model")?;
    let mut config = serde_json::Map::new();
    config.insert(
        "timeout_seconds".to_string(),
        json!(input.timeout_seconds.unwrap_or(30).clamp(1, 600)),
    );
    if let Some(api_key_ref) = optional_trimmed(input.api_key_ref.as_deref()) {
        config.insert(
            "api_key_ref".to_string(),
            json!(normalize_provider_api_key_ref(&api_key_ref)?),
        );
    }
    let profile = state
        .create_provider(CreateProviderRecord {
            provider_type: "eval_judge".to_string(),
            name,
            base_url: Some(endpoint),
            default_model: Some(model),
            config: Value::Object(config),
        })
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "eval.judge_profile_saved",
            "eval_judge_profile",
            Some(profile.id),
            json!({
                "subject": principal.subject_id,
                "name": profile.name,
                "model": profile.default_model,
                "endpoint_configured": profile.base_url.is_some(),
                "api_key_ref_configured": profile.config.get("api_key_ref").is_some()
            }),
        ))
        .await?;
    Ok(Json(profile))
}

async fn bootstrap_stage2_eval_suite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BootstrapEvalSuite>,
) -> Result<Json<EvalSuiteBootstrap>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "eval_suite".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let judge_profile = optional_trimmed(input.judge_profile.as_deref());
    if let Some(profile_name) = judge_profile.as_deref() {
        let profile = state
            .provider_by_name(profile_name)
            .await?
            .ok_or_else(|| AppError::bad_request("eval judge profile not found"))?;
        if profile.provider_type != "eval_judge" || profile.status != "active" {
            return Err(AppError::bad_request(
                "eval judge profile must be an active eval_judge provider",
            ));
        }
    }
    let dataset = state
        .create_eval_dataset(CreateEvalDataset {
            name: optional_trimmed(input.name.as_deref())
                .unwrap_or_else(|| "Stage 2 regression suite".to_string()),
            description: optional_trimmed(input.description.as_deref()).or_else(|| {
                Some(
                    "Default Stage 2 policy, tool, SQL, sandbox, answer, and optional judge checks"
                        .to_string(),
                )
            }),
        })
        .await?;
    let mut cases = Vec::new();
    for case in stage2_regression_suite_cases(judge_profile.as_deref()) {
        cases.push(state.create_eval_case(dataset.id, case).await?);
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "eval.suite_bootstrapped",
            "eval_dataset",
            Some(dataset.id),
            json!({
                "subject": principal.subject_id,
                "dataset": dataset.name,
                "case_count": cases.len(),
                "judge_profile": judge_profile,
            }),
        ))
        .await?;
    Ok(Json(EvalSuiteBootstrap { dataset, cases }))
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
    authorize_request(&state, &headers, Permission::Admin, "eval_run", Some(id)).await?;
    let min_score = input.min_score.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&min_score) {
        return Err(AppError::bad_request(
            "eval gate min_score must be between 0.0 and 1.0",
        ));
    }
    let require_completed = input.require_completed.unwrap_or(true);
    let run = state
        .list_eval_runs(None)
        .await?
        .into_iter()
        .find(|run| run.id == id)
        .ok_or_else(|| AppError::not_found("eval run not found"))?;
    Ok(Json(build_eval_gate_decision(
        &run,
        min_score,
        require_completed,
    )))
}

async fn get_eval_run_drift(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<EvalDriftDecision>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_run", Some(id)).await?;
    let run = state
        .list_eval_runs(None)
        .await?
        .into_iter()
        .find(|run| run.id == id)
        .ok_or_else(|| AppError::not_found("eval run not found"))?;
    let baseline = state
        .list_eval_runs(Some(run.dataset_id))
        .await?
        .into_iter()
        .filter(|candidate| candidate.id != run.id && candidate.agent_id == run.agent_id)
        .filter(|candidate| candidate.created_at <= run.created_at)
        .max_by_key(|candidate| candidate.created_at);
    Ok(Json(build_eval_drift_decision(&run, baseline.as_ref())))
}
