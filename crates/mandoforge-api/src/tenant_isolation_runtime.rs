use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn build_tenant_isolation_readiness(
    state: &AppState,
) -> Result<TenantIsolationReadinessReport, AppError> {
    let generated_at = Utc::now();
    let organizations = state.list_organizations().await?;
    let mut teams_count = 0usize;
    let mut projects_count = 0usize;
    let mut memberships_count = 0usize;
    let mut invitations_count = 0usize;
    for organization in &organizations {
        let teams = state.list_teams(organization.id).await?;
        teams_count += teams.len();
        memberships_count += state.list_memberships(organization.id).await?.len();
        invitations_count += state.list_tenant_invitations(organization.id).await?.len();
        for team in teams {
            projects_count += state.list_projects(team.id).await?.len();
        }
    }

    let rls_probe = probe_tenant_rls(state).await?;
    let table_coverage = tenant_isolation_table_coverage(&rls_probe.table_states);
    let rls = TenantIsolationRlsReadiness {
        required_for_production: true,
        enabled: rls_probe.enabled_table_count == rls_probe.tracked_table_count
            && rls_probe.tracked_table_count > 0,
        forced: rls_probe.forced_table_count == rls_probe.tracked_table_count
            && rls_probe.tracked_table_count > 0,
        migration_asset_present: rls_probe.migration_asset_present,
        tenant_context_configured: rls_probe.tenant_context_configured,
        enabled_table_count: rls_probe.enabled_table_count,
        forced_table_count: rls_probe.forced_table_count,
        tracked_table_count: rls_probe.tracked_table_count,
        status: if rls_probe.enabled_table_count == rls_probe.tracked_table_count
            && rls_probe.forced_table_count == rls_probe.tracked_table_count
            && rls_probe.tenant_context_configured
            && rls_probe.tracked_table_count > 0
        {
            "configured".to_string()
        } else if rls_probe.migration_asset_present {
            "migration_ready".to_string()
        } else {
            "not_configured".to_string()
        },
    };
    let header_fail_closed = true;
    let membership_scope_enforced = true;
    let audit_logs = state.list_audit_logs(None).await?;
    let mut attention_items = Vec::new();
    let runtime_tenant_mode = state.tenant_runtime_mode.as_str();
    let production_routing = build_tenant_production_routing_readiness(
        runtime_tenant_mode,
        header_fail_closed,
        membership_scope_enforced,
        &rls,
        &audit_logs,
        tenant_production_routing_controller_required(&|key| std::env::var(key).ok()),
        tenant_production_routing_controller_configured(&|key| std::env::var(key).ok()),
        generated_at,
    );
    if state.tenant_runtime_mode == TenantRuntimeMode::SingleRuntimeTenant {
        attention_items.push(TenantIsolationAttentionItem {
            kind: "single_runtime_tenant".to_string(),
            severity: "warning".to_string(),
            message: "runtime currently binds every request to one configured tenant_id; cross-tenant serving is intentionally disabled".to_string(),
        });
    }
    if production_routing.production_blocked {
        attention_items.push(TenantIsolationAttentionItem {
            kind: "tenant_production_routing_blocked".to_string(),
            severity: "critical".to_string(),
            message: production_routing.message.clone(),
        });
    }
    if !rls.enabled || !rls.forced || !rls.tenant_context_configured {
        attention_items.push(TenantIsolationAttentionItem {
            kind: "postgres_rls_incomplete".to_string(),
            severity: "warning".to_string(),
            message: "tenant-scoped store queries filter tenant_id; Postgres RLS is now tracked by migration/readiness but is not fully enabled, forced, and tenant-context configured for every tracked table".to_string(),
        });
    }

    let runbook_actions = vec![
        if state.tenant_runtime_mode == TenantRuntimeMode::TenantRouted {
            "keep x-mandoforge-tenant-id required for tenant-routed production clients and verify cross-tenant negative tests".to_string()
        } else {
            "keep x-mandoforge-tenant-id fail-closed until runtime tenant switching is implemented"
                .to_string()
        },
        "apply and verify db/migrations/0024_tenant_rls_policies.sql in Postgres-backed environments"
            .to_string(),
        "keep mandoforge.tenant_id configured on every acquired database connection before production multi-tenant serving"
            .to_string(),
        "run cross-tenant access tests for agents, sessions, tools, approvals, jobs, audit, and governance resources"
            .to_string(),
    ];
    let critical_count = attention_items
        .iter()
        .filter(|item| item.severity == "critical")
        .count() as i64;
    let warning_count = attention_items
        .iter()
        .filter(|item| item.severity == "warning")
        .count() as i64;
    let readiness_score = (100_i64 - critical_count * 25 - warning_count * 15).clamp(0, 100);

    Ok(TenantIsolationReadinessReport {
        generated_at,
        status: if critical_count > 0 {
            "critical"
        } else if warning_count > 0 {
            "attention"
        } else {
            "ready"
        }
        .to_string(),
        readiness_score,
        runtime_tenant_id: state.current_tenant_id(),
        runtime_tenant_mode: runtime_tenant_mode.to_string(),
        header_fail_closed,
        membership_scope_enforced,
        production_routing,
        scoped_counts: TenantIsolationScopedCounts {
            organizations: organizations.len(),
            teams: teams_count,
            projects: projects_count,
            memberships: memberships_count,
            invitations: invitations_count,
        },
        table_coverage,
        rls,
        attention_items,
        runbook_actions,
    })
}

pub(crate) fn build_tenant_production_routing_readiness(
    runtime_tenant_mode: &str,
    header_fail_closed: bool,
    membership_scope_enforced: bool,
    rls: &TenantIsolationRlsReadiness,
    audit_logs: &[AuditLog],
    controller_required: bool,
    controller_configured: bool,
    generated_at: DateTime<Utc>,
) -> TenantProductionRoutingReadiness {
    let cross_tenant_routing_supported = runtime_tenant_mode == "tenant_routed";
    let rls_ready = rls.enabled && rls.forced && rls.tenant_context_configured;
    let latest_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "tenant.production_routing_validation_run")
        .max_by_key(|log| log.created_at);
    let latest_controller_status = latest_controller_log
        .and_then(|log| {
            log.details["controller_execution"]["status"]
                .as_str()
                .or_else(|| log.details["status"].as_str())
        })
        .map(str::to_string);
    let latest_controller_age_hours =
        latest_controller_log.map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();
    if !cross_tenant_routing_supported {
        blocking_reasons.push(
            "runtime still serves one configured tenant instead of routing per tenant".to_string(),
        );
    }
    if !header_fail_closed {
        blocking_reasons.push("tenant header mismatch is not fail-closed".to_string());
    }
    if !membership_scope_enforced {
        blocking_reasons.push("membership scope enforcement is missing".to_string());
    }
    if !rls_ready {
        blocking_reasons.push(
            "Postgres RLS is not fully enabled, forced, and tenant-context configured".to_string(),
        );
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "tenant production routing controller is required but not configured".to_string(),
        );
    }
    if controller_required && !latest_controller_validated {
        blocking_reasons.push(
            "tenant production routing controller has no recent validated evidence".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("tenant production routing controller evidence is stale".to_string());
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
            "Tenant production routing is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Tenant production routing has runtime tenant routing, fail-closed headers, membership scope, and RLS ready".to_string()
    };
    TenantProductionRoutingReadiness {
        status,
        production_blocked,
        cross_tenant_routing_supported,
        runtime_tenant_mode: runtime_tenant_mode.to_string(),
        header_fail_closed,
        membership_scope_enforced,
        rls_ready,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        message,
        blocking_reasons,
    }
}

pub(crate) fn tenant_production_routing_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn tenant_production_routing_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) async fn execute_tenant_production_routing_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    checked_at: DateTime<Utc>,
    readiness: &TenantIsolationReadinessReport,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_TENANT_ROUTING_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.tenant_production_routing_validation",
        "subject": subject,
        "checked_at": checked_at,
        "runtime_tenant_id": readiness.runtime_tenant_id,
        "runtime_tenant_mode": readiness.runtime_tenant_mode,
        "header_fail_closed": readiness.header_fail_closed,
        "membership_scope_enforced": readiness.membership_scope_enforced,
        "production_routing": readiness.production_routing,
        "scoped_counts": readiness.scoped_counts,
        "rls": readiness.rls,
        "table_coverage": {
            "tracked_table_count": readiness.table_coverage.len(),
            "missing_rls_tables": readiness.table_coverage.iter()
                .filter(|table| !table.rls_enabled || !table.rls_forced)
                .map(|table| table.table.clone())
                .collect::<Vec<_>>(),
        },
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (http_status, body) =
        controller_response_json(response, "tenant production routing controller").await?;
    let controller_status = required_controller_status(&body)?;
    let validated = matches!(controller_status, "validated" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "validation_id": body.get("validation_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "target_kind": body.get("target_kind").and_then(Value::as_str),
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "environment": body.get("environment").and_then(Value::as_str),
        "tenant_count": body.get("tenant_count").and_then(Value::as_u64),
        "tenant_samples": body
            .get("tenant_samples")
            .or_else(|| body.get("tenant_ids_sample"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "rls_enforced": body.get("rls_enforced").and_then(Value::as_bool),
        "rls_table_count": body
            .get("rls_table_count")
            .or_else(|| body.get("rls_enabled_table_count"))
            .and_then(Value::as_u64),
        "rls_forced_table_count": body
            .get("rls_forced_table_count")
            .or_else(|| body.get("forced_rls_table_count"))
            .and_then(Value::as_u64),
        "tenant_context_validated": body.get("tenant_context_validated").and_then(Value::as_bool),
        "cross_tenant_negative_tests": body
            .get("cross_tenant_negative_tests")
            .and_then(Value::as_bool),
        "cross_tenant_negative_test_count": body
            .get("cross_tenant_negative_test_count")
            .or_else(|| body.get("negative_test_count"))
            .and_then(Value::as_u64),
        "checks": body.get("checks").cloned().unwrap_or_else(|| json!([])),
    }))
}

#[derive(Debug, Clone)]
struct TenantRlsProbe {
    migration_asset_present: bool,
    tenant_context_configured: bool,
    enabled_table_count: usize,
    forced_table_count: usize,
    tracked_table_count: usize,
    table_states: HashMap<String, (bool, bool)>,
}

async fn probe_tenant_rls(state: &AppState) -> Result<TenantRlsProbe, AppError> {
    let tracked_tables = tenant_isolation_tracked_tables();
    let tracked_table_count = tracked_tables.len();
    let migration_asset_present =
        include_str!("../../../db/migrations/0024_tenant_rls_policies.sql")
            .contains("mandoforge_current_tenant_id");
    let StoreBackend::Postgres(pool) = &state.store else {
        return Ok(TenantRlsProbe {
            migration_asset_present,
            tenant_context_configured: false,
            enabled_table_count: 0,
            forced_table_count: 0,
            tracked_table_count,
            table_states: HashMap::new(),
        });
    };
    let table_names: Vec<String> = tracked_tables
        .iter()
        .map(|table| (*table).to_string())
        .collect();
    let rows: Vec<(String, bool, bool)> = sqlx::query_as(
        "SELECT c.relname::text, c.relrowsecurity, c.relforcerowsecurity
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND c.relkind = 'r'
           AND c.relname = ANY($1)",
    )
    .bind(&table_names)
    .fetch_all(pool)
    .await?;
    let mut table_states = HashMap::new();
    for (table, enabled, forced) in rows {
        table_states.insert(table, (enabled, forced));
    }
    let enabled_table_count = tracked_tables
        .iter()
        .filter(|table| table_states.get(**table).is_some_and(|state| state.0))
        .count();
    let forced_table_count = tracked_tables
        .iter()
        .filter(|table| table_states.get(**table).is_some_and(|state| state.1))
        .count();
    let tenant_context_configured: bool =
        sqlx::query_scalar("SELECT NULLIF(current_setting('mandoforge.tenant_id', true), '') = $1")
            .bind(state.current_tenant_id().to_string())
            .fetch_one(pool)
            .await
            .unwrap_or(false);
    Ok(TenantRlsProbe {
        migration_asset_present,
        tenant_context_configured,
        enabled_table_count,
        forced_table_count,
        tracked_table_count,
        table_states,
    })
}

pub(crate) fn tenant_isolation_tracked_tables() -> Vec<&'static str> {
    vec![
        "tenants",
        "agents",
        "agent_versions",
        "sessions",
        "session_events",
        "tool_calls",
        "approvals",
        "audit_logs",
        "artifacts",
        "providers",
        "tool_definitions",
        "workspaces",
        "secret_records",
        "execution_jobs",
        "session_loop_jobs",
        "session_threads",
        "organizations",
        "teams",
        "projects",
        "memberships",
        "tenant_invitations",
        "provider_access",
        "remote_computers",
        "remote_computer_leases",
        "remote_computer_session_attachments",
        "remote_computer_job_assignments",
        "remote_computer_state_locks",
        "remote_computer_sidecar_heartbeats",
        "agent_handoff_events",
        "agent_handoff_assignments",
        "manager_agent_plans",
        "dynamic_workflow_plans",
        "workflow_pack_installations",
        "workflow_pack_profile_assets",
        "workflow_pack_bindings",
        "workflow_pack_runtime_objects",
        "workflow_definitions",
        "workflow_runs",
        "workflow_step_runs",
        "workflow_transitions",
        "task_grants",
        "approval_commit_tokens",
        "agent_runtime_profiles",
        "environments",
        "mcp_servers",
        "eval_datasets",
        "eval_cases",
        "eval_runs",
        "usage_rollups",
        "agent_releases",
        "policy_revisions",
        "approval_groups",
        "approval_escalation_rules",
        "cost_alert_routes",
        "codex_app_server_runs",
        "approval_notification_channel_policies",
        "semantic_sources",
        "semantic_objects",
        "semantic_links",
        "context_packets",
        "memory_writeback_candidates",
        "ontology_releases",
        "ontology_release_workflow_triggers",
        "ontology_onboarding_runs",
        "workflow_schedules",
    ]
}

pub(crate) fn tenant_isolation_table_coverage(
    rls_table_states: &HashMap<String, (bool, bool)>,
) -> Vec<TenantIsolationTableCoverage> {
    tenant_isolation_tracked_tables()
        .into_iter()
        .map(|table| TenantIsolationTableCoverage {
            table: table.to_string(),
            tenant_id_required: table != "agent_versions",
            store_filters_tenant: true,
            rls_required_for_production: true,
            rls_enabled: rls_table_states.get(table).is_some_and(|state| state.0),
            rls_forced: rls_table_states.get(table).is_some_and(|state| state.1),
        })
        .collect()
}

pub(crate) async fn project_work_item_semantic_object(
    state: &AppState,
    work_item: &WorkItem,
) -> Result<(), AppError> {
    let Some(semantic_scopes) = validate_work_item_semantic_scopes(&work_item.metadata)? else {
        return Ok(());
    };
    let source_uri = format!("mandoforge://work-items/{}", work_item.id);
    let object_key = format!("work_item:{}", work_item.id);
    let result = state
        .create_semantic_object(CreateSemanticObject {
            source_id: None,
            object_type: "work_item".to_string(),
            object_key: object_key.clone(),
            title: work_item.title.clone(),
            summary: work_item.description.clone().unwrap_or_else(|| {
                format!(
                    "WorkItem {} from {} with {} priority.",
                    work_item.id, work_item.source, work_item.priority
                )
            }),
            content: json!({
                "work_item_id": work_item.id,
                "organization_id": work_item.organization_id,
                "team_id": work_item.team_id,
                "project_id": work_item.project_id,
                "title": work_item.title.clone(),
                "description": work_item.description.clone(),
                "source": work_item.source.clone(),
                "source_url": work_item.source_url.clone(),
                "status": work_item.status.clone(),
                "priority": work_item.priority.clone(),
                "assignee": work_item.assignee.clone(),
                "metadata": work_item.metadata.clone(),
            }),
            semantic_scopes,
            source_uri: Some(source_uri.clone()),
            provenance: json!({
                "source": "work_item.created",
                "work_item_id": work_item.id,
                "observed_at": work_item.created_at,
            }),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        })
        .await;
    let semantic_object = match result {
        Ok(object) => object,
        Err(error) if error.message.contains("already exists") => return Ok(()),
        Err(error) => return Err(error),
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            Some(work_item.id),
            "work_item.semantic_object_projected",
            "semantic_object",
            Some(semantic_object.id),
            json!({
                "work_item_id": work_item.id,
                "semantic_object_id": semantic_object.id,
                "object_key": object_key,
                "source_uri": source_uri,
                "trust_level": semantic_object.trust_level,
                "freshness": semantic_object.freshness,
            }),
        ))
        .await?;
    Ok(())
}

pub(crate) fn validate_work_item_semantic_scopes(
    metadata: &Value,
) -> Result<Option<Value>, AppError> {
    let Some(semantic_scopes) = metadata.get("semantic_scopes") else {
        return Ok(None);
    };
    if !semantic_scopes.is_object() {
        return Err(AppError::bad_request(
            "work item metadata.semantic_scopes must be a JSON object",
        ));
    }
    let missing = missing_semantic_scope_keys(semantic_scopes);
    if !missing.is_empty() {
        return Err(AppError::bad_request(format!(
            "work item metadata.semantic_scopes missing required scope keys: {}",
            missing.join(", ")
        )));
    }
    Ok(Some(semantic_scopes.clone()))
}
