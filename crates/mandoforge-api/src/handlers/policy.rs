use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;

use crate::{
    AppError, AppState, PolicyRollbackResult, PolicyRolloutOrchestrationReadiness,
    PolicyRolloutOrchestrationValidationRun, PolicyRuntimeStatus, PolicyScheduledRolloutRun,
    PolicyTestResult, SimulatePolicy, TestPolicyRequest,
    cancel_policy_rollout as cancel_policy_rollout_impl, get_policy as get_policy_impl,
    get_policy_rollout_orchestration_readiness as get_policy_rollout_orchestration_readiness_impl,
    get_policy_runtime as get_policy_runtime_impl, policy,
    rollback_policy_rollout as rollback_policy_rollout_impl,
    run_due_policy_rollouts as run_due_policy_rollouts_impl,
    simulate_policy as simulate_policy_impl, test_policy as test_policy_impl,
    validate_policy_rollout_orchestration as validate_policy_rollout_orchestration_impl,
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
}

async fn get_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    get_policy_impl(state, headers).await
}

async fn get_policy_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRuntimeStatus>, AppError> {
    get_policy_runtime_impl(state, headers).await
}

async fn cancel_policy_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRuntimeStatus>, AppError> {
    cancel_policy_rollout_impl(state, headers).await
}

async fn get_policy_rollout_orchestration_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRolloutOrchestrationReadiness>, AppError> {
    get_policy_rollout_orchestration_readiness_impl(state, headers).await
}

async fn validate_policy_rollout_orchestration(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRolloutOrchestrationValidationRun>, AppError> {
    validate_policy_rollout_orchestration_impl(state, headers).await
}

async fn rollback_policy_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyRollbackResult>, AppError> {
    rollback_policy_rollout_impl(state, headers).await
}

async fn run_due_policy_rollouts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PolicyScheduledRolloutRun>, AppError> {
    run_due_policy_rollouts_impl(state, headers).await
}

async fn simulate_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SimulatePolicy>,
) -> Result<Json<policy::ToolPolicyDecision>, AppError> {
    simulate_policy_impl(state, headers, input).await
}

async fn test_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestPolicyRequest>,
) -> Result<Json<PolicyTestResult>, AppError> {
    test_policy_impl(state, headers, input).await
}
