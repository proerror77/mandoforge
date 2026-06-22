use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, AuthorizationRequest, CreateSecretRecord, Permission, RotateSecretRecord,
    SecretProviderHealth, SecretRecord, SecretRef, VaultKmsRecoveryValidationRun,
    VaultKmsRotationRun, VaultReadinessReport, authorize_request,
    build_vault_kms_recovery_readiness, build_vault_readiness_report, dedupe_strings,
    enforce_resource_scope, execute_vault_kms_recovery_controller, execute_vault_kms_rotation,
    kms_controller_execution_is_production_backend, kms_readiness_from_lookup, new_audit_log,
    principal_from_request, secret_provider_health_from_lookup, validate_secret_record_input,
    vault_kms_recovery_controller_configured, vault_kms_recovery_controller_required,
    write_secret_value_if_provided,
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "vault_kms_rotation",
        None,
    )
    .await?;
    Ok(Json(execute_vault_kms_rotation(&state).await?))
}

async fn validate_vault_kms_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VaultKmsRecoveryValidationRun>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "vault".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let checked_at = Utc::now();
    let lookup = |key: &str| std::env::var(key).ok();
    let kms = kms_readiness_from_lookup(&lookup);
    let secret_records = state.list_secret_records().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let controller_required = vault_kms_recovery_controller_required(&lookup);
    let controller_configured = vault_kms_recovery_controller_configured(&lookup);
    let readiness = build_vault_kms_recovery_readiness(
        &kms,
        &audit_logs,
        checked_at,
        controller_required,
        controller_configured,
    );
    let mut issues = readiness.blocking_reasons.clone();
    let mut controller_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": if controller_configured {
            "vault_kms_recovery_not_ready"
        } else {
            "controller_not_configured"
        }
    });
    if controller_configured {
        match execute_vault_kms_recovery_controller(
            &lookup,
            &principal.subject_id,
            checked_at,
            &kms,
            &secret_records,
            &readiness,
        )
        .await
        {
            Ok(execution) => {
                if execution.get("status").and_then(Value::as_str) != Some("validated") {
                    issues.push("vault KMS recovery controller did not validate".to_string());
                } else if kms_controller_execution_is_production_backend(&execution) {
                    issues.retain(|issue| {
                        issue != "no validated KMS recovery drill evidence exists"
                            && issue
                                != "vault KMS recovery controller evidence is missing or not validated"
                    });
                } else {
                    issues.push(
                        "vault KMS recovery controller did not identify a real production KMS/HSM backend"
                            .to_string(),
                    );
                }
                controller_execution = execution;
            }
            Err(error) => {
                issues.push("vault KMS recovery controller failed".to_string());
                controller_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
            }
        }
    }
    if controller_required
        && controller_execution.get("status").and_then(Value::as_str) != Some("validated")
    {
        issues
            .push("vault KMS recovery controller evidence is missing or not validated".to_string());
    }
    dedupe_strings(&mut issues);
    let status = if issues.is_empty() {
        "validated"
    } else {
        "blocked"
    }
    .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "vault.kms_recovery_validation",
            "vault",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "kms_provider": kms.provider,
                "secret_record_count": secret_records.len(),
                "latest_rotation_validated": readiness.latest_rotation_validated,
                "controller_required": controller_required,
                "controller_configured": controller_configured,
                "controller_execution": controller_execution,
                "issues": issues,
                "checked_at": checked_at,
            }),
        ))
        .await?;
    Ok(Json(VaultKmsRecoveryValidationRun {
        status,
        checked_at,
        kms_provider: kms.provider,
        secret_record_count: secret_records.len(),
        latest_rotation_validated: readiness.latest_rotation_validated,
        controller_required,
        controller_configured,
        controller_execution,
        issues,
    }))
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
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "vault".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let input = validate_secret_record_input(input)?;
    let secret_ref = SecretRef::new(input.path.as_str(), input.key.as_str())?;
    let secret_value_written = write_secret_value_if_provided(&secret_ref, input.value.as_ref())
        .await?
        .is_some();
    let record = state.create_secret_record(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "secret.created",
            "secret_record",
            Some(record.id),
            json!({
                "subject": principal.subject_id,
                "name": record.name,
                "scope_type": record.scope_type,
                "scope_id": record.scope_id,
                "version": record.version,
                "secret_value_written": secret_value_written
            }),
        ))
        .await?;
    Ok(Json(record))
}

async fn rotate_secret_record(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RotateSecretRecord>,
) -> Result<Json<SecretRecord>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "secret_record".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let secret_ref = SecretRef::new(input.path.as_str(), input.key.as_str())?;
    let secret_value_written = write_secret_value_if_provided(&secret_ref, input.value.as_ref())
        .await?
        .is_some();
    let record = state.rotate_secret_record(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "secret.rotated",
            "secret_record",
            Some(record.id),
            json!({
                "subject": principal.subject_id,
                "name": record.name,
                "scope_type": record.scope_type,
                "scope_id": record.scope_id,
                "version": record.version,
                "secret_value_written": secret_value_written
            }),
        ))
        .await?;
    Ok(Json(record))
}
