use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateSecretRecord, Permission, RotateSecretRecord, SecretProviderHealth,
    SecretRecord, VaultKmsRecoveryValidationRun, VaultKmsRotationRun, VaultReadinessReport,
    authorize_request, build_vault_readiness_report,
    create_secret_record as create_secret_record_impl,
    rotate_secret_record as rotate_secret_record_impl,
    run_vault_kms_rotation as run_vault_kms_rotation_impl,
    secret_provider_health_from_lookup,
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
    authorize_request(&state, &headers, Permission::Admin, "vault", None).await?;
    Ok(Json(
        secret_provider_health_from_lookup(|key| std::env::var(key).ok()).await,
    ))
}

async fn get_vault_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VaultReadinessReport>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "vault", None).await?;
    let secret_provider = secret_provider_health_from_lookup(|key| std::env::var(key).ok()).await;
    let secret_records = state.list_secret_records().await?;
    let providers = state.list_providers().await?;
    let mut mcp_servers = Vec::new();
    for organization in state.list_organizations().await? {
        for team in state.list_teams(organization.id).await? {
            mcp_servers.extend(state.list_mcp_servers(team.id).await?);
        }
    }
    let audit_logs = state.list_audit_logs(None).await?;
    Ok(Json(build_vault_readiness_report(
        secret_provider,
        &secret_records,
        &providers,
        &mcp_servers,
        &audit_logs,
        |key| std::env::var(key).ok(),
    )))
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
    authorize_request(&state, &headers, Permission::Admin, "vault", None).await?;
    Ok(Json(state.list_secret_records().await?))
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
