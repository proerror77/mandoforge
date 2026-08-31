use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn execute_vault_kms_rotation(
    state: &AppState,
) -> Result<VaultKmsRotationRun, AppError> {
    execute_vault_kms_rotation_with_lookup(state, |key| std::env::var(key).ok()).await
}

pub(crate) async fn execute_vault_kms_rotation_with_lookup<F>(
    state: &AppState,
    lookup: F,
) -> Result<VaultKmsRotationRun, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let checked_at = Utc::now();
    let secret_provider = secret_provider_health_from_lookup(&lookup).await;
    let kms = kms_readiness_from_lookup(&lookup);
    let secret_records = state.list_secret_records().await?;
    let stale_cutoff = checked_at - ChronoDuration::days(90);
    let stale_rotation_count = secret_records
        .iter()
        .filter(|record| record.status == "active" && record.updated_at < stale_cutoff)
        .count();
    let mut actions = Vec::new();
    if !secret_provider.healthy {
        actions.push("configure_healthy_vault_secret_provider".to_string());
    }
    if !kms.configured {
        actions.push("configure_external_kms_or_hsm".to_string());
    }
    if stale_rotation_count > 0 {
        actions.push("rotate_stale_secret_values_with_new_vault_versions".to_string());
    }
    let blocked_count = usize::from(!secret_provider.healthy) + usize::from(!kms.configured);
    let mut rotated_count = 0;
    let mut catalog_updated_count = 0;
    let audit_id = Uuid::new_v4();
    let mut rotation_details = Vec::new();
    let mut external_execution = json!({
        "attempted": false,
        "status": "skipped",
        "reason": "blocked_before_external_execution"
    });
    let status = if blocked_count > 0 {
        "blocked".to_string()
    } else {
        match execute_external_kms_rotation(
            &lookup,
            &kms,
            &secret_records,
            stale_rotation_count,
            checked_at,
        )
        .await
        {
            Ok(outcome) => {
                external_execution = outcome.summary.clone();
                rotated_count = outcome.rotated_count;
                let rotation_detail_key_id = external_execution
                    .get("key_id")
                    .and_then(Value::as_str)
                    .unwrap_or(kms.provider.as_str())
                    .to_string();
                let rotation_detail_rotation_id = external_execution
                    .get("rotation_id")
                    .and_then(Value::as_str)
                    .unwrap_or("external-kms-rotation")
                    .to_string();
                for record_id in outcome.rotated_secret_record_ids {
                    if let Some(record) =
                        secret_records.iter().find(|record| record.id == record_id)
                    {
                        let input = RotateSecretRecord {
                            path: record.path.clone(),
                            key: record.key.clone(),
                            value: None,
                        };
                        state.rotate_secret_record(record.id, input).await?;
                        catalog_updated_count += 1;
                        rotation_details.push(VaultKmsRotationDetail {
                            key_id: rotation_detail_key_id.clone(),
                            rotation_id: rotation_detail_rotation_id.clone(),
                            secret_record_id: record.id,
                            status: "validated".to_string(),
                            catalog_updated: true,
                            audit_id,
                            rotated_at: checked_at,
                        });
                    }
                }
                actions.extend(outcome.actions);
                if outcome.status == "validated" && stale_rotation_count == 0 {
                    "validated".to_string()
                } else if outcome.status == "validated" {
                    "attention".to_string()
                } else {
                    "blocked".to_string()
                }
            }
            Err(error) => {
                actions.push("fix_external_kms_rotation_endpoint".to_string());
                external_execution = json!({
                    "attempted": true,
                    "status": "failed",
                    "error": error.message
                });
                "blocked".to_string()
            }
        }
    };
    if catalog_updated_count > 0 {
        actions.push("verify_downstream_consumers_after_external_rotation".to_string());
    }
    let run = VaultKmsRotationRun {
        status,
        checked_at,
        kms_provider: kms.provider,
        kms_status: kms.status,
        kms_endpoint_configured: kms.endpoint_configured,
        secret_provider_status: secret_provider.status,
        secret_record_count: secret_records.len(),
        stale_rotation_count,
        rotated_count,
        catalog_updated_count,
        rotation_details,
        blocked_count,
        actions,
        external_execution,
    };
    state
        .append_audit_log(AuditLog {
            id: audit_id,
            session_id: None,
            actor_type: "system".to_string(),
            actor_id: None,
            action: "vault.kms_rotation_run".to_string(),
            resource_type: "vault".to_string(),
            resource_id: None,
            details: json!({
                "status": run.status,
                "kms_provider": run.kms_provider,
                "kms_status": run.kms_status,
                "kms_endpoint_configured": run.kms_endpoint_configured,
                "secret_provider_status": run.secret_provider_status,
                "secret_record_count": run.secret_record_count,
                "stale_rotation_count": run.stale_rotation_count,
                "rotated_count": run.rotated_count,
                "catalog_updated_count": run.catalog_updated_count,
                "rotation_details": run.rotation_details,
                "blocked_count": run.blocked_count,
                "actions": run.actions,
                "external_execution": run.external_execution,
                "checked_at": run.checked_at,
            }),
            created_at: Utc::now(),
        })
        .await?;
    Ok(run)
}

struct ExternalKmsRotationOutcome {
    status: String,
    rotated_count: usize,
    rotated_secret_record_ids: Vec<Uuid>,
    actions: Vec<String>,
    summary: Value,
}

async fn execute_external_kms_rotation<F>(
    lookup: &F,
    kms: &VaultKmsReadiness,
    secret_records: &[SecretRecord],
    stale_rotation_count: usize,
    checked_at: DateTime<Utc>,
) -> Result<ExternalKmsRotationOutcome, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_KMS_ENDPOINT")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_KMS_ENDPOINT is required"))?;
    let key_id = lookup("MANDOFORGE_KMS_KEY_ID")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_KMS_KEY_ID is required"))?;
    let rotation_policy = lookup("MANDOFORGE_KMS_ROTATION_POLICY")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_KMS_ROTATION_POLICY is required"))?;
    let timeout_seconds = lookup("MANDOFORGE_KMS_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=60).contains(seconds))
        .unwrap_or(10);
    let token = lookup("MANDOFORGE_KMS_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.kms_rotation_validation",
        "provider": kms.provider,
        "key_id": key_id,
        "rotation_policy": rotation_policy,
        "validation_mode": kms.validation_mode,
        "secret_record_count": secret_records.len(),
        "stale_rotation_count": stale_rotation_count,
        "secret_records": secret_records.iter().map(|record| {
            json!({
                "id": record.id,
                "name": record.name,
                "ref": secret_record_ref(record),
                "scope_type": record.scope_type,
                "scope_id": record.scope_id,
                "version": record.version,
                "status": record.status,
                "updated_at": record.updated_at,
            })
        }).collect::<Vec<_>>(),
        "checked_at": checked_at,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client
        .post(endpoint.clone())
        .header("x-mandoforge-kms-provider", kms.provider.as_str())
        .json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (http_status, response_body) =
        controller_response_json(response, "external KMS rotation endpoint").await?;
    let response_status = required_controller_status(&response_body)?;
    let validated = matches!(response_status, "validated" | "rotated" | "ok" | "success");
    let rotated_secret_record_ids = response_body
        .get("rotated_secret_record_ids")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let rotated_count = response_body
        .get("rotated_count")
        .and_then(Value::as_u64)
        .map(|count| count as usize)
        .unwrap_or(rotated_secret_record_ids.len());
    let actions = response_body
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut summary = json!({
        "attempted": true,
        "status": if validated { "validated" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": response_status,
        "backend_kind": response_body.get("backend_kind").and_then(Value::as_str),
        "backend_id": response_body.get("backend_id").and_then(Value::as_str),
        "key_id": response_body
            .get("key_id")
            .and_then(Value::as_str)
            .or(Some(key_id.as_str())),
        "environment": response_body.get("environment").and_then(Value::as_str),
        "hsm_provider": response_body.get("hsm_provider").and_then(Value::as_str),
        "rotation_id": response_body.get("rotation_id").and_then(Value::as_str),
        "rotated_count": rotated_count,
        "returned_rotated_secret_record_ids": response_body
            .get("rotated_secret_record_ids")
            .and_then(Value::as_array)
            .map(|ids| ids.len())
            .unwrap_or(0),
        "message": response_body.get("message").and_then(Value::as_str),
        "endpoint_configured": true,
    });
    let production_backend = kms_controller_execution_is_production_backend(&summary);
    summary["status"] = json!(if validated && production_backend {
        "validated"
    } else {
        "blocked"
    });
    summary["production_backend"] = json!(production_backend);

    Ok(ExternalKmsRotationOutcome {
        status: if validated && production_backend {
            "validated"
        } else {
            "blocked"
        }
        .to_string(),
        rotated_count,
        rotated_secret_record_ids,
        actions,
        summary,
    })
}

pub(crate) fn validate_secret_record_input(
    mut input: CreateSecretRecord,
) -> Result<CreateSecretRecord, AppError> {
    input.name = input.name.trim().to_string();
    input.path = input.path.trim().to_string();
    input.key = input.key.trim().to_string();
    input.scope_type = input.scope_type.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::bad_request("secret name is required"));
    }
    if !matches!(input.scope_type.as_str(), "tenant" | "team" | "project") {
        return Err(AppError::bad_request(
            "secret scope_type must be tenant, team, or project",
        ));
    }
    SecretRef::new(input.path.as_str(), input.key.as_str())?;
    Ok(input)
}

pub(crate) async fn write_secret_value_if_provided(
    secret_ref: &SecretRef,
    value: Option<&String>,
) -> Result<Option<()>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let provider = secret_provider_from_env()?;
    let config = SecretProviderConfig::from_env()?;
    provider
        .write_secret(&config, secret_ref, &SecretValue::from_plaintext(value))
        .await?;
    Ok(Some(()))
}
