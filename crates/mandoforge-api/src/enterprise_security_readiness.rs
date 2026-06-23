use chrono::Utc;
use serde_json::{Value, json};

use crate::{
    AppError, AppState, EnterpriseSecurityAdminCheck, EnterpriseSecurityAdminReadiness,
    approval_email_relay_url_from_env, approval_slack_webhook_url_from_env,
    build_approval_notification_delivery_run_summary, build_approval_notification_routing_summary,
    build_tenant_isolation_readiness, build_vault_readiness_report, env_bool,
    secret_provider_health_from_lookup,
};

pub(crate) async fn build_enterprise_security_admin_readiness(
    state: &AppState,
) -> Result<EnterpriseSecurityAdminReadiness, AppError> {
    let generated_at = Utc::now();
    let tenant = build_tenant_isolation_readiness(state).await?;
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
    let vault = build_vault_readiness_report(
        secret_provider,
        &secret_records,
        &providers,
        &mcp_servers,
        &audit_logs,
        |key| std::env::var(key).ok(),
    );
    let approval_routing = build_approval_notification_routing_summary(
        state.list_approvals().await?,
        state.list_approval_groups().await?,
        state.list_approval_escalation_rules().await?,
        state.list_approval_notification_channel_policies().await?,
        state.approval_webhook_url.is_some(),
        approval_slack_webhook_url_from_env().is_some(),
        approval_email_relay_url_from_env().is_some(),
    )
    .await;
    let approval_delivery = build_approval_notification_delivery_run_summary(
        &audit_logs,
        generated_at,
        &approval_routing,
    );

    let mut checks = Vec::new();
    checks.push(enterprise_security_admin_check(
        "identity-provisioning",
        "SSO and directory provisioning",
        enterprise_identity_provider_ready(),
        "repo_controlled",
        json!({
            "identity_provider": enterprise_env("MANDOFORGE_ENTERPRISE_IDENTITY_PROVIDER"),
            "oidc_issuer_configured": enterprise_env_present("MANDOFORGE_OIDC_ISSUER_URL"),
            "saml_metadata_configured": enterprise_env_present("MANDOFORGE_SAML_METADATA_URL"),
            "scim_enabled": env_bool("MANDOFORGE_SCIM_PROVISIONING_ENABLED"),
            "scim_directory_id": enterprise_env("MANDOFORGE_SCIM_DIRECTORY_ID"),
        }),
        vec!["SSO and SCIM are not configured with customer production identity evidence"],
        vec!["configure OIDC or SAML plus SCIM directory provisioning for the customer tenant"],
    ));
    checks.push(enterprise_security_admin_check(
        "tenant-rls-abac",
        "Tenant isolation and scoped authorization",
        tenant.status == "ready" && !tenant.production_routing.production_blocked,
        if tenant.production_routing.latest_controller_validated {
            "production_like_pilot"
        } else {
            "repo_controlled"
        },
        json!({
            "tenant_status": tenant.status,
            "runtime_tenant_mode": tenant.runtime_tenant_mode,
            "header_fail_closed": tenant.header_fail_closed,
            "membership_scope_enforced": tenant.membership_scope_enforced,
            "rls_status": tenant.rls.status,
            "rls_enabled": tenant.rls.enabled,
            "rls_forced": tenant.rls.forced,
            "tenant_context_configured": tenant.rls.tenant_context_configured,
            "production_routing_status": tenant.production_routing.status,
            "production_routing_blocking_reasons": tenant.production_routing.blocking_reasons,
        }),
        vec!["tenant routing, RLS, or scoped authorization is not customer-grade ready"],
        vec![
            "run tenant-isolation production routing evidence with tenant-routed runtime and forced RLS",
        ],
    ));
    checks.push(enterprise_security_admin_check(
        "vault-kms-rotation-recovery",
        "Production Vault/KMS rotation and recovery",
        vault.status == "ready"
            && !vault.production_rotation.production_blocked
            && !vault.production_recovery.production_blocked
            && vault.production_recovery.latest_controller_production_backend,
        if vault.production_recovery.latest_controller_validated {
            "production_like_pilot"
        } else {
            "repo_controlled"
        },
        json!({
            "vault_status": vault.status,
            "secret_provider_status": vault.secret_provider.status,
            "secret_provider_kind": vault.secret_provider.provider_kind,
            "kms_provider": vault.kms.provider,
            "kms_status": vault.kms.status,
            "production_rotation_status": vault.production_rotation.status,
            "production_rotation_blocking_reasons": vault.production_rotation.blocking_reasons,
            "production_recovery_status": vault.production_recovery.status,
            "production_recovery_blocking_reasons": vault.production_recovery.blocking_reasons,
            "latest_controller_backend_kind": vault.production_recovery.latest_controller_backend_kind,
            "latest_controller_environment": vault.production_recovery.latest_controller_environment,
            "latest_controller_production_backend": vault.production_recovery.latest_controller_production_backend,
        }),
        vec!["Vault/KMS production rotation or recovery evidence is incomplete"],
        vec![
            "run Vault/KMS recovery validation against production-capable KMS/HSM backend and rotate stale secrets",
        ],
    ));
    checks.push(enterprise_security_admin_check(
        "approval-break-glass",
        "Delegated approval, escalation, and break-glass audit",
        approval_routing.status == "ready"
            && approval_delivery.production_ops.status == "ready"
            && approval_delivery.deployment_readiness.status == "ready"
            && env_bool("MANDOFORGE_BREAK_GLASS_POLICY_ENABLED")
            && enterprise_env_present("MANDOFORGE_BREAK_GLASS_APPROVAL_GROUP"),
        if approval_delivery.production_ops.latest_controller_validated {
            "production_like_pilot"
        } else {
            "repo_controlled"
        },
        json!({
            "routing_status": approval_routing.status,
            "channel_count": approval_routing.channel_count,
            "active_policy_count": approval_routing.active_policy_count,
            "approval_group_count": approval_routing.approval_group_count,
            "escalation_rule_count": approval_routing.escalation_rule_count,
            "unroutable_pending_count": approval_routing.unroutable_pending_count,
            "production_ops_status": approval_delivery.production_ops.status,
            "deployment_readiness_status": approval_delivery.deployment_readiness.status,
            "break_glass_policy_enabled": env_bool("MANDOFORGE_BREAK_GLASS_POLICY_ENABLED"),
            "break_glass_approval_group": enterprise_env("MANDOFORGE_BREAK_GLASS_APPROVAL_GROUP"),
        }),
        vec!["approval notification, escalation, or break-glass audit evidence is incomplete"],
        vec![
            "configure break-glass approval group and run approval notification deployment, ops, and delivery evidence",
        ],
    ));
    checks.push(enterprise_security_admin_check(
        "audit-export-siem",
        "Audit export and SIEM ingestion",
        enterprise_env_present("MANDOFORGE_AUDIT_EXPORT_SIEM_URL")
            && enterprise_env_present("MANDOFORGE_AUDIT_EXPORT_CONTROLLER_URL")
            && enterprise_env_present("MANDOFORGE_AUDIT_EXPORT_FORMAT"),
        "not_started",
        json!({
            "siem_url_configured": enterprise_env_present("MANDOFORGE_AUDIT_EXPORT_SIEM_URL"),
            "controller_url_configured": enterprise_env_present("MANDOFORGE_AUDIT_EXPORT_CONTROLLER_URL"),
            "format": enterprise_env("MANDOFORGE_AUDIT_EXPORT_FORMAT"),
            "audit_log_count": audit_logs.len(),
        }),
        vec!["SIEM export controller and production ingestion evidence are missing"],
        vec![
            "add an audit export controller that proves SIEM delivery with tenant/session/tool-call correlation",
        ],
    ));
    checks.push(enterprise_security_admin_check(
        "data-governance",
        "Retention, legal hold, export, deletion, PII redaction, and DLP",
        enterprise_env_present("MANDOFORGE_DATA_RETENTION_POLICY_ID")
            && env_bool("MANDOFORGE_LEGAL_HOLD_ENABLED")
            && env_bool("MANDOFORGE_DATA_EXPORT_ENABLED")
            && env_bool("MANDOFORGE_DATA_DELETION_ENABLED")
            && env_bool("MANDOFORGE_PII_REDACTION_ENABLED")
            && env_bool("MANDOFORGE_DLP_POLICY_ENABLED"),
        "not_started",
        json!({
            "retention_policy_id": enterprise_env("MANDOFORGE_DATA_RETENTION_POLICY_ID"),
            "legal_hold_enabled": env_bool("MANDOFORGE_LEGAL_HOLD_ENABLED"),
            "data_export_enabled": env_bool("MANDOFORGE_DATA_EXPORT_ENABLED"),
            "data_deletion_enabled": env_bool("MANDOFORGE_DATA_DELETION_ENABLED"),
            "pii_redaction_enabled": env_bool("MANDOFORGE_PII_REDACTION_ENABLED"),
            "dlp_policy_enabled": env_bool("MANDOFORGE_DLP_POLICY_ENABLED"),
        }),
        vec!["data retention, legal hold, export/delete, PII redaction, or DLP evidence is missing"],
        vec![
            "implement data-governance policy APIs and run export/delete/redaction/DLP evidence drills",
        ],
    ));

    let check_count = checks.len();
    let ready_check_count = checks
        .iter()
        .filter(|check| check.status == "ready")
        .count();
    let blocked_check_count = check_count - ready_check_count;
    let completion_blocked = blocked_check_count > 0;
    let status = if completion_blocked {
        "blocked"
    } else {
        "ready"
    }
    .to_string();
    let next_actions = checks
        .iter()
        .filter(|check| check.status != "ready")
        .flat_map(|check| check.next_actions.clone())
        .collect::<Vec<_>>();
    let message = if completion_blocked {
        format!(
            "Enterprise security/admin readiness is blocked: {ready_check_count}/{check_count} checks are customer-grade ready"
        )
    } else {
        "Enterprise security/admin readiness has customer-grade evidence for every required check"
            .to_string()
    };

    Ok(EnterpriseSecurityAdminReadiness {
        generated_at,
        status,
        required_evidence_class: "customer_grade".to_string(),
        check_count,
        ready_check_count,
        blocked_check_count,
        completion_blocked,
        checks,
        next_actions,
        message,
    })
}

fn enterprise_security_admin_check(
    id: &str,
    title: &str,
    ready: bool,
    current_evidence_class: &str,
    evidence: Value,
    blockers: Vec<&str>,
    next_actions: Vec<&str>,
) -> EnterpriseSecurityAdminCheck {
    EnterpriseSecurityAdminCheck {
        id: id.to_string(),
        title: title.to_string(),
        status: if ready { "ready" } else { "blocked" }.to_string(),
        current_evidence_class: current_evidence_class.to_string(),
        required_evidence_class: "customer_grade".to_string(),
        evidence,
        blockers: if ready {
            Vec::new()
        } else {
            blockers.into_iter().map(str::to_string).collect()
        },
        next_actions: if ready {
            Vec::new()
        } else {
            next_actions.into_iter().map(str::to_string).collect()
        },
    }
}

fn enterprise_env_present(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn enterprise_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn enterprise_identity_provider_ready() -> bool {
    let provider = enterprise_env("MANDOFORGE_ENTERPRISE_IDENTITY_PROVIDER")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let sso_ready = matches!(provider.as_str(), "oidc" | "saml")
        && (enterprise_env_present("MANDOFORGE_OIDC_ISSUER_URL")
            || enterprise_env_present("MANDOFORGE_SAML_METADATA_URL"));
    sso_ready
        && env_bool("MANDOFORGE_SCIM_PROVISIONING_ENABLED")
        && enterprise_env_present("MANDOFORGE_SCIM_DIRECTORY_ID")
}
