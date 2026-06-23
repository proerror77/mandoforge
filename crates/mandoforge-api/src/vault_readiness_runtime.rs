use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn secret_provider_health_from_lookup<F>(lookup: F) -> SecretProviderHealth
where
    F: Fn(&str) -> Option<String>,
{
    let checked_at = Utc::now();
    let kind = match SecretProviderKind::from_lookup(&lookup) {
        Ok(kind) => kind,
        Err(error) => {
            return SecretProviderHealth {
                provider_kind: "invalid".to_string(),
                healthy: false,
                status: "misconfigured".to_string(),
                issues: vec![error.message],
                checks: json!({}),
                checked_at,
            };
        }
    };
    match kind {
        SecretProviderKind::Reserved => SecretProviderHealth {
            provider_kind: "reserved".to_string(),
            healthy: false,
            status: "reserved".to_string(),
            issues: vec![
                "secret reads are disabled until MANDOFORGE_SECRET_PROVIDER=vault is configured"
                    .to_string(),
            ],
            checks: json!({"provider": "reserved"}),
            checked_at,
        },
        SecretProviderKind::Vault => match SecretProviderConfig::from_lookup(&lookup) {
            Ok(config) => match VaultSecretProvider::new() {
                Ok(provider) => match provider.health_check(&config).await {
                    Ok(()) => SecretProviderHealth {
                        provider_kind: "vault".to_string(),
                        healthy: true,
                        status: "healthy".to_string(),
                        issues: vec![],
                        checks: json!({
                            "vault_addr_configured": !config.vault_addr.trim().is_empty(),
                            "mount": config.mount,
                            "namespace_configured": config.namespace.is_some(),
                            "token_configured": config.token.is_some(),
                        }),
                        checked_at,
                    },
                    Err(error) => SecretProviderHealth {
                        provider_kind: "vault".to_string(),
                        healthy: false,
                        status: "unhealthy".to_string(),
                        issues: vec![error.message],
                        checks: json!({
                            "vault_addr_configured": !config.vault_addr.trim().is_empty(),
                            "mount": config.mount,
                            "namespace_configured": config.namespace.is_some(),
                            "token_configured": config.token.is_some(),
                        }),
                        checked_at,
                    },
                },
                Err(error) => SecretProviderHealth {
                    provider_kind: "vault".to_string(),
                    healthy: false,
                    status: "client_error".to_string(),
                    issues: vec![error.message],
                    checks: json!({}),
                    checked_at,
                },
            },
            Err(error) => SecretProviderHealth {
                provider_kind: "vault".to_string(),
                healthy: false,
                status: "misconfigured".to_string(),
                issues: vec![error.message],
                checks: json!({"provider": "vault"}),
                checked_at,
            },
        },
    }
}

pub(crate) fn build_vault_readiness_report<F>(
    secret_provider: SecretProviderHealth,
    secret_records: &[SecretRecord],
    providers: &[ProviderRecord],
    mcp_servers: &[McpServerRecord],
    audit_logs: &[AuditLog],
    lookup: F,
) -> VaultReadinessReport
where
    F: Fn(&str) -> Option<String>,
{
    let generated_at = Utc::now();
    let registered_refs: BTreeSet<String> = secret_records.iter().map(secret_record_ref).collect();
    let mut checks = Vec::new();
    let mut attention_items = Vec::new();
    let mut unresolved_refs = BTreeSet::new();
    let mut provider_ref_count = 0;
    let mut eval_judge_secret_ref_count = 0;
    let mut mcp_secret_ref_count = 0;

    let mut provider_blockers = Vec::new();
    let mut provider_warnings = Vec::new();
    let mut provider_recommendations = Vec::new();
    if !secret_provider.healthy {
        provider_blockers.push(format!(
            "secret provider is {}: {}",
            secret_provider.status,
            secret_provider.issues.join("; ")
        ));
        provider_recommendations
            .push("configure MANDOFORGE_SECRET_PROVIDER=vault and verify Vault health".to_string());
    }
    if secret_provider.provider_kind == "reserved" {
        provider_blockers
            .push("reserved secret provider cannot read production secrets".to_string());
    }
    if secret_provider.provider_kind == "vault"
        && secret_provider
            .checks
            .get("token_configured")
            .and_then(Value::as_bool)
            == Some(false)
    {
        provider_blockers.push("Vault token is not configured".to_string());
    }
    if secret_records.is_empty() {
        provider_warnings.push("no secret refs are registered in the catalog".to_string());
        provider_recommendations
            .push("register provider, MCP, and judge secret refs before pilot rollout".to_string());
    }
    checks.push(VaultReadinessCheck {
        resource_type: "secret_provider".to_string(),
        resource_id: None,
        resource_name: secret_provider.provider_kind.clone(),
        status: vault_check_status(&provider_blockers, &provider_warnings),
        secret_refs: vec![],
        blockers: provider_blockers,
        warnings: provider_warnings,
        recommendations: provider_recommendations,
    });

    let kms = kms_readiness_from_lookup(&lookup);
    checks.push(VaultReadinessCheck {
        resource_type: "kms".to_string(),
        resource_id: None,
        resource_name: kms.provider.clone(),
        status: if kms.status == "ready" {
            "passed".to_string()
        } else {
            "failed".to_string()
        },
        secret_refs: vec![],
        blockers: kms.issues.clone(),
        warnings: vec![],
        recommendations: if kms.status == "ready" {
            vec![]
        } else {
            vec![
                "configure MANDOFORGE_KMS_PROVIDER, MANDOFORGE_KMS_KEY_ID, and MANDOFORGE_KMS_ROTATION_POLICY".to_string(),
            ]
        },
    });

    let stale_cutoff = generated_at - ChronoDuration::days(90);
    for record in secret_records {
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut recommendations = Vec::new();
        if record.status != "active" {
            blockers.push(format!("secret record status is {}", record.status));
            recommendations.push("reactivate or rotate this secret ref before rollout".to_string());
        }
        if record.updated_at < stale_cutoff {
            warnings.push("secret ref has not rotated in more than 90 days".to_string());
            recommendations
                .push("rotate this secret ref and verify downstream consumers".to_string());
        }
        checks.push(VaultReadinessCheck {
            resource_type: "secret_record".to_string(),
            resource_id: Some(record.id),
            resource_name: record.name.clone(),
            status: vault_check_status(&blockers, &warnings),
            secret_refs: vec![secret_record_ref(record)],
            blockers,
            warnings,
            recommendations,
        });
    }

    for provider in providers {
        let Some(secret_ref) = provider_api_key_ref(provider) else {
            if provider_requires_api_key(&provider.provider_type) {
                let has_env = provider
                    .config
                    .get("api_key_env")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty());
                let mut blockers = Vec::new();
                let mut warnings = Vec::new();
                let mut recommendations = Vec::new();
                if has_env {
                    warnings.push("provider uses env credential instead of Vault ref".to_string());
                    recommendations
                        .push("rotate provider credential to vault:path#key".to_string());
                } else {
                    blockers.push("provider requires api_key_ref or api_key_env".to_string());
                    recommendations.push(
                        "bind provider credential through a registered Vault ref".to_string(),
                    );
                }
                checks.push(VaultReadinessCheck {
                    resource_type: "provider".to_string(),
                    resource_id: Some(provider.id),
                    resource_name: provider.name.clone(),
                    status: vault_check_status(&blockers, &warnings),
                    secret_refs: vec![],
                    blockers,
                    warnings,
                    recommendations,
                });
            }
            continue;
        };
        if provider.provider_type == "eval_judge" {
            eval_judge_secret_ref_count += 1;
        } else {
            provider_ref_count += 1;
        }
        let mut blockers = Vec::new();
        let mut recommendations = Vec::new();
        if !registered_refs.contains(&secret_ref) {
            unresolved_refs.insert(secret_ref.clone());
            blockers.push("api_key_ref is not registered in the secret catalog".to_string());
            recommendations.push("create a matching /api/vault/secrets catalog entry".to_string());
        }
        checks.push(VaultReadinessCheck {
            resource_type: if provider.provider_type == "eval_judge" {
                "eval_judge_profile".to_string()
            } else {
                "provider".to_string()
            },
            resource_id: Some(provider.id),
            resource_name: provider.name.clone(),
            status: vault_check_status(&blockers, &[]),
            secret_refs: vec![secret_ref],
            blockers,
            warnings: vec![],
            recommendations,
        });
    }

    for server in mcp_servers {
        let secret_refs = mcp_config_secret_refs(&server.config);
        if secret_refs.is_empty() {
            continue;
        }
        mcp_secret_ref_count += secret_refs.len();
        let mut blockers = Vec::new();
        let mut recommendations = Vec::new();
        for secret_ref in &secret_refs {
            if !registered_refs.contains(secret_ref) {
                unresolved_refs.insert(secret_ref.clone());
                blockers.push(format!(
                    "{secret_ref} is not registered in the secret catalog"
                ));
            }
        }
        if !blockers.is_empty() {
            recommendations.push(
                "register every MCP connector secret ref before enabling the connector".to_string(),
            );
        }
        checks.push(VaultReadinessCheck {
            resource_type: "mcp_server".to_string(),
            resource_id: Some(server.id),
            resource_name: server.name.clone(),
            status: vault_check_status(&blockers, &[]),
            secret_refs,
            blockers,
            warnings: vec![],
            recommendations,
        });
    }

    for check in &checks {
        for blocker in &check.blockers {
            attention_items.push(VaultReadinessAttentionItem {
                resource_type: check.resource_type.clone(),
                resource_id: check.resource_id,
                resource_name: check.resource_name.clone(),
                kind: "blocker".to_string(),
                severity: "critical".to_string(),
                message: blocker.clone(),
            });
        }
        for warning in &check.warnings {
            attention_items.push(VaultReadinessAttentionItem {
                resource_type: check.resource_type.clone(),
                resource_id: check.resource_id,
                resource_name: check.resource_name.clone(),
                kind: "warning".to_string(),
                severity: "warning".to_string(),
                message: warning.clone(),
            });
        }
    }

    let stale_rotation_count = secret_records
        .iter()
        .filter(|record| record.updated_at < stale_cutoff)
        .count();
    let production_rotation = build_vault_production_rotation_readiness(
        &secret_provider,
        &kms,
        unresolved_refs.len(),
        stale_rotation_count,
        audit_logs,
        generated_at,
    );
    let production_recovery = build_vault_kms_recovery_readiness(
        &kms,
        audit_logs,
        generated_at,
        vault_kms_recovery_controller_required(&lookup),
        vault_kms_recovery_controller_configured(&lookup),
    );
    if production_rotation.production_blocked {
        attention_items.push(VaultReadinessAttentionItem {
            resource_type: "vault".to_string(),
            resource_id: None,
            resource_name: "production_rotation".to_string(),
            kind: "production_rotation_blocked".to_string(),
            severity: "critical".to_string(),
            message: production_rotation.message.clone(),
        });
    }
    if production_recovery.production_blocked {
        attention_items.push(VaultReadinessAttentionItem {
            resource_type: "vault".to_string(),
            resource_id: None,
            resource_name: "kms_recovery".to_string(),
            kind: "production_recovery_blocked".to_string(),
            severity: "critical".to_string(),
            message: production_recovery.message.clone(),
        });
    }

    let failed_count = checks
        .iter()
        .filter(|check| check.status == "failed")
        .count();
    let warning_count = checks
        .iter()
        .filter(|check| check.status == "warning")
        .count();
    let status = if failed_count > 0 {
        "failed"
    } else if warning_count > 0 {
        "warning"
    } else {
        "passed"
    }
    .to_string();

    VaultReadinessReport {
        generated_at,
        status,
        secret_provider,
        kms,
        production_rotation,
        production_recovery,
        secret_record_count: secret_records.len(),
        active_secret_record_count: secret_records
            .iter()
            .filter(|record| record.status == "active")
            .count(),
        provider_ref_count,
        mcp_secret_ref_count,
        eval_judge_secret_ref_count,
        unresolved_ref_count: unresolved_refs.len(),
        stale_rotation_count,
        checks,
        attention_items,
    }
}

pub(crate) fn build_vault_production_rotation_readiness(
    secret_provider: &SecretProviderHealth,
    kms: &VaultKmsReadiness,
    unresolved_ref_count: usize,
    stale_rotation_count: usize,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> VaultProductionRotationReadiness {
    let latest_rotation_run = audit_logs
        .iter()
        .filter(|log| log.action == "vault.kms_rotation_run")
        .max_by_key(|log| log.created_at);
    let latest_rotation_run_status = latest_rotation_run
        .and_then(|log| log.details.get("status").and_then(Value::as_str))
        .map(ToString::to_string);
    let latest_rotation_validated = latest_rotation_run.is_some_and(|log| {
        latest_rotation_run_status.as_deref() == Some("validated")
            && (generated_at - log.created_at).num_hours() < 30
    });
    let vault_healthy = secret_provider.healthy && secret_provider.provider_kind == "vault";
    let kms_ready = kms.status == "ready" && kms.configured;
    let unresolved_refs_clear = unresolved_ref_count == 0;
    let stale_rotations_clear = stale_rotation_count == 0;
    let mut blocking_reasons = Vec::new();

    if !vault_healthy {
        blocking_reasons.push("Vault secret provider is not healthy and selected".to_string());
    }
    if !kms_ready {
        blocking_reasons
            .push("external KMS/HSM readiness is not configured and validated".to_string());
    }
    if !unresolved_refs_clear {
        blocking_reasons
            .push("registered consumers reference secrets missing from the catalog".to_string());
    }
    if !stale_rotations_clear {
        blocking_reasons
            .push("one or more active secret refs are past the rotation window".to_string());
    }
    if !latest_rotation_validated {
        blocking_reasons.push("no recent validated KMS rotation gate run exists".to_string());
    }

    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Vault production rotation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Vault production rotation has healthy Vault, ready KMS/HSM, resolved refs, fresh rotations, and recent validated gate evidence".to_string()
    };

    VaultProductionRotationReadiness {
        status,
        production_blocked,
        vault_healthy,
        kms_ready,
        unresolved_refs_clear,
        stale_rotations_clear,
        latest_rotation_validated,
        latest_rotation_run_at: latest_rotation_run.map(|log| log.created_at),
        latest_rotation_run_status,
        blocking_reasons,
        message,
    }
}

pub(crate) fn build_vault_kms_recovery_readiness(
    kms: &VaultKmsReadiness,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> VaultKmsRecoveryReadiness {
    let latest_rotation_run = audit_logs
        .iter()
        .filter(|log| log.action == "vault.kms_rotation_run")
        .max_by_key(|log| log.created_at);
    let latest_rotation_validated = latest_rotation_run.is_some_and(|log| {
        log.details.get("status").and_then(Value::as_str) == Some("validated")
            && (generated_at - log.created_at).num_hours() < 30
    });
    let latest_recovery = audit_logs
        .iter()
        .filter(|log| log.action == "vault.kms_recovery_validation")
        .max_by_key(|log| log.created_at);
    let latest_recovery_status = latest_recovery
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_status = latest_recovery
        .and_then(|log| log.details.get("controller_execution"))
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_execution =
        latest_recovery.and_then(|log| log.details.get("controller_execution"));
    let latest_controller_backend_kind = latest_controller_execution
        .and_then(|execution| execution.get("backend_kind"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_environment = latest_controller_execution
        .and_then(|execution| execution.get("environment"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_backend_id = latest_controller_execution
        .and_then(|execution| execution.get("backend_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_key_id = latest_controller_execution
        .and_then(|execution| execution.get("key_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_hsm_provider = latest_controller_execution
        .and_then(|execution| execution.get("hsm_provider"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_age_hours = latest_recovery
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_production_backend =
        latest_controller_execution.is_some_and(kms_controller_execution_is_production_backend);
    let latest_controller_validated = latest_recovery_status.as_deref() == Some("validated")
        && latest_controller_status.as_deref() == Some("validated")
        && latest_controller_production_backend;
    let mut blocking_reasons = Vec::new();
    if kms.status != "ready" || !kms.configured {
        blocking_reasons
            .push("external KMS/HSM readiness is not configured and validated".to_string());
    }
    if !latest_rotation_validated {
        blocking_reasons.push(
            "validated KMS rotation evidence is required before recovery validation".to_string(),
        );
    }
    if latest_recovery_status.as_deref() != Some("validated") {
        blocking_reasons.push("no validated KMS recovery drill evidence exists".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("vault KMS recovery controller is required but not configured".to_string());
    }
    if controller_required
        && controller_configured
        && latest_controller_status.as_deref() != Some("validated")
    {
        blocking_reasons
            .push("vault KMS recovery controller evidence is missing or not validated".to_string());
    }
    if controller_required
        && latest_controller_status.as_deref() == Some("validated")
        && !latest_controller_production_backend
    {
        blocking_reasons.push(
            "vault KMS recovery controller did not identify a real production KMS/HSM backend"
                .to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("vault KMS recovery controller evidence is stale".to_string());
    }
    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let message = if production_blocked {
        format!(
            "Vault KMS recovery readiness is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Vault KMS recovery has recent rotation evidence and validated recovery-controller drill evidence".to_string()
    };
    VaultKmsRecoveryReadiness {
        status,
        production_blocked,
        controller_required,
        controller_configured,
        latest_recovery_at: latest_recovery.map(|log| log.created_at),
        latest_recovery_status,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        latest_controller_production_backend,
        latest_controller_backend_kind,
        latest_controller_environment,
        latest_controller_backend_id,
        latest_controller_key_id,
        latest_controller_hsm_provider,
        latest_rotation_validated,
        blocking_reasons,
        message,
    }
}

pub(crate) fn vault_kms_recovery_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn vault_kms_recovery_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_vault_kms_recovery_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    kms: &VaultKmsReadiness,
    secret_records: &[SecretRecord],
    readiness: &VaultKmsRecoveryReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_KMS_RECOVERY_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_KMS_RECOVERY_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let secret_refs = secret_records
        .iter()
        .map(|record| {
            json!({
                "id": record.id,
                "name": record.name,
                "scope_type": record.scope_type,
                "scope_id": record.scope_id,
                "path": record.path,
                "key": record.key,
                "status": record.status,
                "version": record.version,
                "updated_at": record.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "type": "mandoforge.kms_recovery_validation",
        "subject": subject,
        "checked_at": checked_at,
        "kms": {
            "provider": kms.provider,
            "status": kms.status,
            "key_id_configured": kms.key_id_configured,
            "rotation_policy_configured": kms.rotation_policy_configured,
            "endpoint_configured": kms.endpoint_configured,
            "validation_mode": kms.validation_mode,
        },
        "readiness": {
            "status": readiness.status,
            "production_blocked": readiness.production_blocked,
            "latest_rotation_validated": readiness.latest_rotation_validated,
            "blocking_reasons": readiness.blocking_reasons,
        },
        "secret_refs": secret_refs,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "vault KMS recovery controller failed with status {http_status}"
        )));
    }
    let provider_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(provider_status, "validated" | "success" | "ok" | "healthy");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "blocked" },
        "http_status": http_status.as_u16(),
        "provider_status": provider_status,
        "recovery_id": body.get("recovery_id").and_then(Value::as_str),
        "backend_kind": body.get("backend_kind").and_then(Value::as_str),
        "backend_id": body.get("backend_id").and_then(Value::as_str),
        "key_id": body.get("key_id").and_then(Value::as_str),
        "environment": body.get("environment").and_then(Value::as_str),
        "hsm_provider": body.get("hsm_provider").and_then(Value::as_str),
        "recovery_target_kind": body.get("recovery_target_kind").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn normalized_kms_kind(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

pub(crate) fn is_production_kms_provider(value: &str) -> bool {
    matches!(
        normalized_kms_kind(value).as_str(),
        "external"
            | "external_kms"
            | "aws_kms"
            | "gcp_kms"
            | "azure_key_vault"
            | "hashicorp_vault_transit"
            | "vault_transit"
            | "hsm"
            | "cloudhsm"
            | "pkcs11_hsm"
    )
}

pub(crate) fn is_production_kms_backend_kind(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            normalized_kms_kind(value).as_str(),
            "external_kms"
                | "aws_kms"
                | "gcp_kms"
                | "azure_key_vault"
                | "hashicorp_vault_transit"
                | "vault_transit"
                | "hsm"
                | "cloudhsm"
                | "pkcs11_hsm"
        )
    })
}

pub(crate) fn is_production_kms_environment(value: Option<&str>) -> bool {
    matches!(value, Some("production" | "prod"))
}

pub(crate) fn kms_controller_execution_is_production_backend(execution: &Value) -> bool {
    let backend_id_present = execution
        .get("backend_id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    let key_id_present = execution
        .get("key_id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    is_production_kms_backend_kind(execution.get("backend_kind").and_then(Value::as_str))
        && is_production_kms_environment(execution.get("environment").and_then(Value::as_str))
        && backend_id_present
        && key_id_present
}

pub(crate) fn kms_readiness_from_lookup<F>(lookup: &F) -> VaultKmsReadiness
where
    F: Fn(&str) -> Option<String>,
{
    let provider = lookup("MANDOFORGE_KMS_PROVIDER")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "reserved".to_string());
    let key_id_configured = lookup("MANDOFORGE_KMS_KEY_ID")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let rotation_policy_configured = lookup("MANDOFORGE_KMS_ROTATION_POLICY")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let endpoint_configured = lookup("MANDOFORGE_KMS_ENDPOINT")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let validation_mode = lookup("MANDOFORGE_KMS_VALIDATION_MODE")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "health-check".to_string());
    let production_provider = is_production_kms_provider(&provider);
    let external_validation = validation_mode.trim().eq_ignore_ascii_case("external");
    let configured = provider != "reserved"
        && production_provider
        && key_id_configured
        && rotation_policy_configured
        && endpoint_configured
        && external_validation;
    let mut issues = Vec::new();
    if provider == "reserved" {
        issues.push("external KMS/HSM provider is not configured".to_string());
    } else if !production_provider {
        issues.push(format!(
            "KMS/HSM provider is not a production backend: {provider}"
        ));
    }
    if !key_id_configured {
        issues.push("MANDOFORGE_KMS_KEY_ID is not configured".to_string());
    }
    if !rotation_policy_configured {
        issues.push("MANDOFORGE_KMS_ROTATION_POLICY is not configured".to_string());
    }
    if !endpoint_configured {
        issues.push(
            "MANDOFORGE_KMS_ENDPOINT is not configured for external KMS validation".to_string(),
        );
    }
    if !external_validation {
        issues.push(
            "MANDOFORGE_KMS_VALIDATION_MODE must be external for production evidence".to_string(),
        );
    }
    VaultKmsReadiness {
        provider,
        status: if configured { "ready" } else { "reserved" }.to_string(),
        configured,
        key_id_configured,
        rotation_policy_configured,
        endpoint_configured,
        validation_mode,
        issues,
    }
}

pub(crate) fn vault_check_status(blockers: &[String], warnings: &[String]) -> String {
    if !blockers.is_empty() {
        "failed"
    } else if !warnings.is_empty() {
        "warning"
    } else {
        "passed"
    }
    .to_string()
}

pub(crate) fn secret_record_ref(record: &SecretRecord) -> String {
    format!("vault:{}#{}", record.path, record.key)
}

pub(crate) fn provider_api_key_ref(provider: &ProviderRecord) -> Option<String> {
    provider
        .config
        .get("api_key_ref")
        .and_then(Value::as_str)
        .and_then(|value| normalize_provider_api_key_ref(value).ok())
}
