use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateSecretRecord, RotateSecretRecord, SecretProviderHealth, SecretRecord,
    VaultKmsRecoveryValidationRun, VaultKmsRotationRun, VaultReadinessReport,
    create_secret_record as create_secret_record_impl, get_vault_health as get_vault_health_impl,
    get_vault_readiness as get_vault_readiness_impl,
    list_secret_records as list_secret_records_impl,
    rotate_secret_record as rotate_secret_record_impl,
    run_vault_kms_rotation as run_vault_kms_rotation_impl,
    validate_vault_kms_recovery as validate_vault_kms_recovery_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vault/health", get(get_vault_health))
        .route("/api/vault/readiness", get(get_vault_readiness))
        .route("/api/vault/kms/rotation/run", post(run_vault_kms_rotation))
        .route(
            "/api/vault/kms/recovery/validate",
            post(validate_vault_kms_recovery),
        )
        .route(
            "/api/vault/secrets",
            get(list_secret_records).post(create_secret_record),
        )
        .route("/api/vault/secrets/{id}/rotate", post(rotate_secret_record))
}

async fn get_vault_health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SecretProviderHealth>, AppError> {
    get_vault_health_impl(state, headers).await
}

async fn get_vault_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VaultReadinessReport>, AppError> {
    get_vault_readiness_impl(state, headers).await
}

async fn run_vault_kms_rotation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VaultKmsRotationRun>, AppError> {
    run_vault_kms_rotation_impl(state, headers).await
}

async fn validate_vault_kms_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VaultKmsRecoveryValidationRun>, AppError> {
    validate_vault_kms_recovery_impl(state, headers).await
}

async fn list_secret_records(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SecretRecord>>, AppError> {
    list_secret_records_impl(state, headers).await
}

async fn create_secret_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSecretRecord>,
) -> Result<Json<SecretRecord>, AppError> {
    create_secret_record_impl(state, headers, input).await
}

async fn rotate_secret_record(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RotateSecretRecord>,
) -> Result<Json<SecretRecord>, AppError> {
    rotate_secret_record_impl(state, id, headers, input).await
}
