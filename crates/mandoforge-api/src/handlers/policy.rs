use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppError, AppState, CreatePolicyRevision, PolicyRevision, PolicyRevisionDiff,
    PolicyRevisionGate, PolicyRevisionGateRequest, PolicyRollbackResult,
    PolicyRolloutOrchestrationReadiness, PolicyRolloutOrchestrationValidationRun,
    PolicyRuntimeStatus, PolicyScheduledRolloutRun, PolicyTestResult, SimulatePolicy,
    TestPolicyRequest, activate_policy_revision as activate_policy_revision_impl,
    cancel_policy_rollout as cancel_policy_rollout_impl,
    create_policy_revision as create_policy_revision_impl,
    diff_policy_revision as diff_policy_revision_impl, gate_policy_revision as gate_policy_revision_impl,
    get_policy as get_policy_impl,
    get_policy_rollout_orchestration_readiness as get_policy_rollout_orchestration_readiness_impl,
    get_policy_runtime as get_policy_runtime_impl, list_policy_revisions as list_policy_revisions_impl, policy,
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

async fn list_policy_revisions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PolicyRevision>>, AppError> {
    list_policy_revisions_impl(state, headers).await
}

async fn create_policy_revision(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreatePolicyRevision>,
) -> Result<Json<PolicyRevision>, AppError> {
    create_policy_revision_impl(state, headers, input).await
}

async fn activate_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PolicyRevision>, AppError> {
    activate_policy_revision_impl(state, id, headers).await
}

async fn diff_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PolicyRevisionDiff>, AppError> {
    diff_policy_revision_impl(state, id, headers).await
}

async fn gate_policy_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    input: Option<Json<PolicyRevisionGateRequest>>,
) -> Result<Json<PolicyRevisionGate>, AppError> {
    gate_policy_revision_impl(state, id, headers, input.map(|Json(input)| input)).await
}
