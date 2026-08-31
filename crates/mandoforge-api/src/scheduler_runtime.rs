use axum::http::HeaderMap;
use chrono::Utc;
use serde_json::{Value, json};

use crate::*;

pub(crate) async fn build_scheduler_orchestration_summary(
    state: &AppState,
) -> Result<SchedulerOrchestrationSummary, AppError> {
    let generated_at = Utc::now();
    let plan = build_scheduler_due_plan(state).await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let deployment_readiness =
        scheduler_deployment_readiness_from_manifests(&audit_logs, generated_at, &|key| {
            std::env::var(key).ok()
        });
    let mut recent_runs: Vec<_> = audit_logs
        .into_iter()
        .filter(|log| log.action == "scheduler.run_due")
        .filter_map(scheduler_run_history_item)
        .collect();
    recent_runs.sort_by_key(|run| std::cmp::Reverse(run.created_at));
    recent_runs.truncate(10);
    let last_run = recent_runs.first();
    let mut attention_items = Vec::new();
    if plan.status == "blocked" {
        attention_items.push(SchedulerAttentionItem {
            severity: "warning".to_string(),
            kind: "blocked_plan".to_string(),
            message: "scheduler has a blocked due-plan item that needs configuration".to_string(),
        });
    }
    if plan.actionable_count > 0 {
        attention_items.push(SchedulerAttentionItem {
            severity: "warning".to_string(),
            kind: "due_actions".to_string(),
            message: format!(
                "{} scheduler item(s) are ready for due-run execution",
                plan.actionable_count
            ),
        });
    }
    if let Some(run) = last_run
        && run.status != "completed"
        && run.status != "noop"
    {
        attention_items.push(SchedulerAttentionItem {
            severity: "critical".to_string(),
            kind: "last_run_unhealthy".to_string(),
            message: format!("last scheduler due-run ended with status {}", run.status),
        });
    }
    if deployment_readiness.production_blocked {
        attention_items.push(SchedulerAttentionItem {
            severity: "critical".to_string(),
            kind: "scheduler_deployment_blocked".to_string(),
            message: deployment_readiness.message.clone(),
        });
    }
    let status = if attention_items
        .iter()
        .any(|item| item.severity == "critical")
    {
        "critical"
    } else if !attention_items.is_empty() {
        "attention"
    } else {
        "ok"
    }
    .to_string();
    Ok(SchedulerOrchestrationSummary {
        generated_at,
        status,
        plan,
        deployment_readiness,
        recent_run_count: recent_runs.len(),
        last_run_at: last_run.map(|run| run.created_at),
        last_run_status: last_run.map(|run| run.status.clone()),
        last_run_action_count: last_run.map(|run| run.action_count).unwrap_or(0),
        recent_runs,
        attention_items,
    })
}

pub(crate) fn validate_scheduler_shared_token(headers: &HeaderMap) -> Result<(), AppError> {
    let Some(expected_token) = scheduler_shared_token_from_env() else {
        return Ok(());
    };
    let provided_token = header_value(headers, "x-mandoforge-scheduler-token")
        .map(str::trim)
        .unwrap_or_default();
    if provided_token != expected_token {
        return Err(AppError::forbidden("scheduler token is invalid"));
    }
    Ok(())
}

pub(crate) fn scheduler_shared_token_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_SCHEDULER_TOKEN")
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
}

pub(crate) fn scheduler_deployment_readiness_from_manifests<F>(
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    lookup: &F,
) -> SchedulerDeploymentReadiness
where
    F: Fn(&str) -> Option<String>,
{
    let scheduler_manifest_path = "deploy/k8s/scheduler.yaml";
    let service_account_manifest_path = "deploy/k8s/scheduler-serviceaccount.yaml";
    let secret_manifest_path = "deploy/k8s/secret.example.yaml";
    let scheduler_manifest =
        read_yaml_manifest_value(scheduler_manifest_path).and_then(|manifest| {
            (manifest.get("kind").and_then(Value::as_str) == Some("CronJob")).then_some(manifest)
        });
    let scheduler_manifest_present = scheduler_manifest.is_some();
    let service_account_name = scheduler_manifest
        .as_ref()
        .and_then(|manifest| {
            manifest.pointer("/spec/jobTemplate/spec/template/spec/serviceAccountName")
        })
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let service_account_manifest_present = service_account_name.as_deref().is_some_and(|name| {
        manifest_has_kind_name(service_account_manifest_path, "ServiceAccount", name)
    });
    let automount_service_account_token_disabled = scheduler_manifest
        .as_ref()
        .and_then(|manifest| {
            manifest.pointer("/spec/jobTemplate/spec/template/spec/automountServiceAccountToken")
        })
        .and_then(Value::as_bool)
        == Some(false);
    let scheduler_container = scheduler_manifest
        .as_ref()
        .and_then(|manifest| manifest.pointer("/spec/jobTemplate/spec/template/spec/containers"))
        .and_then(Value::as_array)
        .and_then(|containers| {
            containers.iter().find(|container| {
                container.get("name").and_then(Value::as_str) == Some("scheduler")
            })
        });
    let subject_from_secret =
        scheduler_container_env_uses_secret(
            scheduler_container,
            "MANDOFORGE_SCHEDULER_SUBJECT",
            "MANDOFORGE_SCHEDULER_SUBJECT",
        ) && secret_manifest_defines_key(secret_manifest_path, "MANDOFORGE_SCHEDULER_SUBJECT");
    let roles_from_secret =
        scheduler_container_env_uses_secret(
            scheduler_container,
            "MANDOFORGE_SCHEDULER_ROLES",
            "MANDOFORGE_SCHEDULER_ROLES",
        ) && secret_manifest_defines_key(secret_manifest_path, "MANDOFORGE_SCHEDULER_ROLES");
    let token_from_secret =
        scheduler_container_env_uses_secret(
            scheduler_container,
            "MANDOFORGE_SCHEDULER_TOKEN",
            "MANDOFORGE_SCHEDULER_TOKEN",
        ) && secret_manifest_defines_key(secret_manifest_path, "MANDOFORGE_SCHEDULER_TOKEN");
    let args_text = scheduler_container
        .and_then(|container| container.get("args"))
        .and_then(Value::as_array)
        .map(|args| {
            args.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let token_header_present = args_text.contains("x-mandoforge-scheduler-token")
        && args_text.contains("MANDOFORGE_SCHEDULER_TOKEN");
    let hardcoded_admin_headers_absent = !args_text.contains("x-mandoforge-roles: admin")
        && !args_text.contains("x-mandoforge-subject: scheduler");
    let shared_token_runtime_configured = lookup("MANDOFORGE_SCHEDULER_TOKEN")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let controller_required = scheduler_deployment_controller_required(lookup);
    let controller_configured = scheduler_deployment_controller_configured(lookup);
    let latest_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "scheduler.deployment_validation_run")
        .max_by_key(|log| log.created_at);
    let latest_controller_status = latest_controller_log
        .and_then(|log| {
            log.details["controller_execution"]["status"]
                .as_str()
                .or_else(|| log.details["status"].as_str())
        })
        .map(str::to_string);
    let latest_controller_age_hours = latest_controller_log
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();

    if !scheduler_manifest_present {
        blocking_reasons.push("scheduler CronJob manifest is missing".to_string());
    }
    if !service_account_manifest_present {
        blocking_reasons.push("scheduler ServiceAccount manifest is missing".to_string());
    }
    if !automount_service_account_token_disabled {
        blocking_reasons
            .push("scheduler ServiceAccount token automount is not disabled".to_string());
    }
    if !subject_from_secret {
        blocking_reasons.push("scheduler subject is not sourced from Secret".to_string());
    }
    if !roles_from_secret {
        blocking_reasons.push("scheduler roles are not sourced from Secret".to_string());
    }
    if !token_from_secret {
        blocking_reasons.push("scheduler shared token is not sourced from Secret".to_string());
    }
    if !token_header_present {
        blocking_reasons.push("scheduler token header is not sent by the CronJob".to_string());
    }
    if !hardcoded_admin_headers_absent {
        blocking_reasons
            .push("scheduler CronJob still contains hardcoded demo admin headers".to_string());
    }
    if !shared_token_runtime_configured {
        blocking_reasons
            .push("MANDOFORGE_SCHEDULER_TOKEN is not configured in the API runtime".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("scheduler deployment controller is required but not configured".to_string());
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons
            .push("scheduler deployment controller has no recent validated evidence".to_string());
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("scheduler deployment controller evidence is stale".to_string());
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
            "Scheduler deployment is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Scheduler deployment uses Secret-backed identity and shared-token auth".to_string()
    };

    SchedulerDeploymentReadiness {
        status,
        production_blocked,
        scheduler_manifest_present,
        service_account_manifest_present,
        service_account_name,
        automount_service_account_token_disabled,
        subject_from_secret,
        roles_from_secret,
        token_from_secret,
        token_header_present,
        hardcoded_admin_headers_absent,
        shared_token_runtime_configured,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        blocking_reasons,
        message,
    }
}

pub(crate) fn scheduler_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn scheduler_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) async fn execute_scheduler_deployment_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    checked_at: DateTime<Utc>,
    readiness: &SchedulerDeploymentReadiness,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_SCHEDULER_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.scheduler_deployment_validation",
        "subject": subject,
        "checked_at": checked_at,
        "readiness": readiness,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;
    let mut request = client.post(endpoint).json(&payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let (_http_status, body) =
        controller_response_json(response, "scheduler deployment controller").await?;
    Ok(json!({
        "attempted": true,
        "status": required_controller_status(&body)?,
        "controller_response": body,
    }))
}

pub(crate) fn scheduler_container_env_uses_secret(
    container: Option<&Value>,
    name: &str,
    secret_key: &str,
) -> bool {
    container
        .and_then(|container| container.get("env"))
        .and_then(Value::as_array)
        .and_then(|env| {
            env.iter()
                .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|entry| entry.pointer("/valueFrom/secretKeyRef/key"))
        .and_then(Value::as_str)
        == Some(secret_key)
}

pub(crate) fn secret_manifest_defines_key(relative_path: &str, key: &str) -> bool {
    read_yaml_manifest_value(relative_path)
        .and_then(|manifest| manifest.pointer("/stringData").cloned())
        .and_then(|string_data| string_data.as_object().cloned())
        .is_some_and(|string_data| string_data.contains_key(key))
}

pub(crate) fn scheduler_run_history_item(log: AuditLog) -> Option<SchedulerRunHistoryItem> {
    let run_id = log
        .details
        .get("run_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let idempotency_key = log
        .details
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::to_string);
    let owner = log
        .details
        .get("owner")
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = log
        .details
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let team_count = log
        .details
        .get("team_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let actions: Vec<String> = log
        .details
        .get("actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(SchedulerRunHistoryItem {
        audit_log_id: log.id,
        run_id,
        idempotency_key,
        owner,
        status,
        team_count,
        action_count: actions.len(),
        actions,
        created_at: log.created_at,
    })
}

pub(crate) async fn build_scheduler_due_plan(
    state: &AppState,
) -> Result<SchedulerDuePlan, AppError> {
    let generated_at = Utc::now();
    let mut actions = Vec::new();
    let policy_revisions = state.list_policy_revisions().await?;
    let policy_due_count = policy_revisions
        .iter()
        .filter(|revision| policy_revision_is_due_for_scheduled_activation(revision, generated_at))
        .count();
    actions.push(scheduler_due_plan_item(
        "policy",
        "policy_rollout_activation",
        "auto",
        policy_due_count,
        policy_revisions.len().saturating_sub(policy_due_count),
        policy_revisions.len(),
        if policy_due_count > 0 {
            "activate earliest due passed policy revision"
        } else {
            "no passed draft policy revision is inside its activation window"
        },
    ));

    let providers = state.list_providers().await?;
    let audit_logs = state.list_audit_logs(None).await?;
    let provider_gate_due = provider_policy_gate_is_due(&providers, &audit_logs, generated_at);
    actions.push(scheduler_due_plan_item(
        "providers",
        "provider_policy_gate",
        "auto",
        usize::from(provider_gate_due),
        providers
            .len()
            .saturating_sub(usize::from(provider_gate_due)),
        providers.len(),
        if provider_gate_due {
            "run provider policy gate because no fresh gate run covers configured providers"
        } else if providers.is_empty() {
            "no providers are configured for policy gate evaluation"
        } else {
            "provider policy gate has fresh run history"
        },
    ));

    let approvals = state.list_approvals().await?;
    let approval_rules = state.list_approval_escalation_rules().await?;
    let pending_approvals: Vec<_> = approvals
        .iter()
        .filter(|approval| approval.status == "pending")
        .collect();
    let expired_approval_count = pending_approvals
        .iter()
        .filter(|approval| approval_is_expired_at(approval, generated_at))
        .count();
    let escalation_due_count = pending_approvals
        .iter()
        .filter(|approval| {
            !approval_is_expired_at(approval, generated_at)
                && next_due_escalation_rule(approval, &approval_rules, generated_at).is_some()
        })
        .count();
    let approval_due_count = expired_approval_count + escalation_due_count;
    actions.push(scheduler_due_plan_item(
        "approvals",
        "approval_expiration_and_escalation",
        "auto",
        approval_due_count,
        pending_approvals.len().saturating_sub(approval_due_count),
        pending_approvals.len(),
        if approval_due_count > 0 {
            "expire overdue approvals and escalate pending approvals whose rules are due"
        } else {
            "no pending approval is expired or due for escalation"
        },
    ));

    let pending_releases = state.list_pending_agent_releases().await?;
    let release_due_count = pending_releases
        .iter()
        .filter(|release| {
            release_automation_is_expired(release, generated_at)
                || matches!(
                    release_automation_due_decision(release, generated_at),
                    ReleaseAutomationDecision::Promote
                )
        })
        .count();
    actions.push(scheduler_due_plan_item(
        "agent_releases",
        "release_promotion_automation",
        "auto",
        release_due_count,
        pending_releases.len().saturating_sub(release_due_count),
        pending_releases.len(),
        if release_due_count > 0 {
            "promote auto-approved releases or reject expired pending releases"
        } else {
            "no pending release is due for automated decision"
        },
    ));

    let mut team_count = 0usize;
    let mut mcp_server_count = 0usize;
    let mut mcp_health_due_count = 0usize;
    let mut mcp_rollout_due_count = 0usize;
    let mut mcp_rollout_skipped_count = 0usize;
    for organization in state
        .list_organizations()
        .await?
        .into_iter()
        .filter(|organization| organization.archived_at.is_none())
    {
        for team in state
            .list_teams(organization.id)
            .await?
            .into_iter()
            .filter(|team| team.archived_at.is_none())
        {
            team_count += 1;
            let servers = state.list_mcp_servers(team.id).await?;
            mcp_server_count += servers.len();
            for server in servers {
                if mcp_server_health_check_is_due(&server, generated_at) {
                    mcp_health_due_count += 1;
                }
                match mcp_pending_rollout(&server) {
                    Some(rollout)
                        if mcp_rollout_is_due(rollout, generated_at)
                            || mcp_rollout_is_expired(rollout, generated_at) =>
                    {
                        mcp_rollout_due_count += 1;
                    }
                    Some(_) => mcp_rollout_skipped_count += 1,
                    None => {}
                }
            }
        }
    }
    actions.push(scheduler_due_plan_item(
        "mcp",
        "mcp_scheduled_health_checks",
        "auto",
        mcp_health_due_count,
        mcp_server_count.saturating_sub(mcp_health_due_count),
        mcp_server_count,
        if mcp_health_due_count > 0 {
            "run scheduled MCP health checks for due connectors"
        } else {
            "no active MCP connector health check is due"
        },
    ));
    actions.push(scheduler_due_plan_item(
        "mcp",
        "mcp_connector_rollouts",
        "auto",
        mcp_rollout_due_count,
        mcp_rollout_skipped_count,
        mcp_rollout_due_count + mcp_rollout_skipped_count,
        if mcp_rollout_due_count > 0 {
            "apply due MCP connector rollouts and expire overdue rollout windows"
        } else {
            "no pending MCP connector rollout is due"
        },
    ));

    let codex_runs = state.list_codex_app_server_runs().await?;
    let codex_stale_candidates = select_stale_codex_app_server_runs(
        &codex_runs,
        generated_at,
        default_codex_stale_after_seconds(),
    )
    .len();
    actions.push(scheduler_due_plan_item(
        "codex_app_server",
        "stale_turn_polling",
        "auto",
        codex_stale_candidates,
        codex_runs.len().saturating_sub(codex_stale_candidates),
        codex_runs.len(),
        if codex_stale_candidates > 0 {
            "poll stale non-terminal Codex App Server turns"
        } else {
            "no stale non-terminal Codex App Server turn is due"
        },
    ));

    let mut workflow_scheduled_total = 0usize;
    let mut workflow_scheduled_due_count = 0usize;
    for run in state.list_workflow_runs().await? {
        if workflow_step_status_terminal(&run.status) {
            continue;
        }
        let scheduled_steps = state
            .list_workflow_step_runs(run.id)
            .await?
            .into_iter()
            .filter(|step| step.status == "scheduled")
            .collect::<Vec<_>>();
        if scheduled_steps.is_empty() {
            continue;
        }
        workflow_scheduled_total += scheduled_steps.len();
        workflow_scheduled_due_count += scheduled_steps
            .iter()
            .filter(|step| {
                step.scheduled_at
                    .is_some_and(|scheduled_at| scheduled_at <= generated_at)
            })
            .count();
    }
    actions.push(scheduler_due_plan_item(
        "workflows",
        "workflow_scheduled_step_activation",
        "auto",
        workflow_scheduled_due_count,
        workflow_scheduled_total.saturating_sub(workflow_scheduled_due_count),
        workflow_scheduled_total,
        if workflow_scheduled_due_count > 0 {
            "activate scheduled workflow retry/delay steps whose due time has passed"
        } else if workflow_scheduled_total > 0 {
            "scheduled workflow steps exist but none are due yet"
        } else {
            "no scheduled workflow steps are pending"
        },
    ));
    if let Some(item) = actions.last_mut()
        && workflow_scheduled_due_count == 0
        && workflow_scheduled_total > 0
    {
        item.status = "waiting".to_string();
        item.severity = "info".to_string();
        item.skipped_count = workflow_scheduled_total;
    }

    let (
        semantic_synthesis_scheduled_count,
        semantic_synthesis_due_count,
        semantic_synthesis_skipped_count,
    ) = build_semantic_synthesis_schedule_due_counts(state, generated_at).await?;
    actions.push(scheduler_due_plan_item(
        "memory",
        "semantic_synthesis_schedule_run",
        "auto",
        semantic_synthesis_due_count,
        semantic_synthesis_skipped_count,
        semantic_synthesis_scheduled_count,
        if semantic_synthesis_due_count > 0 {
            "create due reflection/dreaming artifacts and review-gated memory writeback candidates"
        } else if semantic_synthesis_scheduled_count > 0 {
            "semantic synthesis schedules exist but none are due"
        } else {
            "no semantic synthesis schedules are registered"
        },
    ));
    let (semantic_aging_policy_count, semantic_aging_due_count, semantic_aging_skipped_count) =
        build_scheduled_runtime_object_due_counts(
            state,
            generated_at,
            "semantic_aging_policy",
            "semantic_aging.policy_run",
        )
        .await?;
    actions.push(scheduler_due_plan_item(
        "memory",
        "semantic_aging_policy_run",
        "auto",
        semantic_aging_due_count,
        semantic_aging_skipped_count,
        semantic_aging_policy_count,
        if semantic_aging_due_count > 0 {
            "archive or flag stale semantic memory according to due aging policies"
        } else if semantic_aging_policy_count > 0 {
            "semantic aging policies exist but none are due"
        } else {
            "no semantic aging policies are registered"
        },
    ));

    let retryable_ontology_release_triggers = state
        .retryable_ontology_release_workflow_triggers(100)
        .await?;
    actions.push(scheduler_due_plan_item(
        "ontology",
        "ontology_release_workflow_trigger_retry",
        "auto",
        retryable_ontology_release_triggers.len(),
        0,
        retryable_ontology_release_triggers.len(),
        if retryable_ontology_release_triggers.is_empty() {
            "no failed or stale ontology release workflow trigger needs retry"
        } else {
            "retry failed or stale ontology release workflow triggers"
        },
    ));

    let stale_remote_computer_attachments = state.list_stale_remote_computer_attachments().await?;
    let remote_computer_leases = state.list_remote_computer_leases().await?;
    let expired_remote_computer_leases = remote_computer_leases
        .iter()
        .filter(|lease| {
            lease.status == "leased"
                && lease
                    .lease_expires_at
                    .is_some_and(|lease_expires_at| lease_expires_at <= generated_at)
        })
        .count();
    let remote_computer_due_count =
        stale_remote_computer_attachments.len() + expired_remote_computer_leases;
    actions.push(scheduler_due_plan_item(
        "remote_computers",
        "remote_computer_stale_reclaim",
        "auto",
        remote_computer_due_count,
        remote_computer_leases
            .len()
            .saturating_sub(expired_remote_computer_leases),
        remote_computer_leases.len() + stale_remote_computer_attachments.len(),
        if remote_computer_due_count > 0 {
            "reclaim stale Remote Computer attachments and expired leases"
        } else {
            "no stale Remote Computer attachment or expired lease is due"
        },
    ));
    let remote_computer_readiness = build_remote_computer_readiness(state).await?;
    let sidecar_supervision = remote_computer_readiness.sidecar_supervision;
    let unhealthy_sidecar_count =
        sidecar_supervision.missing_heartbeat_count + sidecar_supervision.stale_heartbeat_count;
    actions.push(scheduler_due_plan_item(
        "remote_computers",
        "remote_computer_sidecar_supervision",
        "auto",
        unhealthy_sidecar_count,
        sidecar_supervision
            .active_remote_computer_count
            .saturating_sub(unhealthy_sidecar_count),
        sidecar_supervision.active_remote_computer_count,
        if unhealthy_sidecar_count > 0 {
            "record missing or stale Remote Computer artifact-discovery sidecar supervision evidence"
        } else if sidecar_supervision.active_remote_computer_count == 0 {
            "no active Remote Computer sidecar needs supervision"
        } else {
            "Remote Computer sidecar heartbeats are within the configured threshold"
        },
    ));

    let usage_export_enabled = usage_finance_export_schedule_enabled();
    let usage_export_ready = usage_finance_export_webhook_url().is_some();
    let usage_export_due_count = usize::from(usage_export_enabled);
    let mut usage_item = scheduler_due_plan_item(
        "usage",
        "usage_finance_export_delivery",
        "auto",
        usage_export_due_count,
        usize::from(!usage_export_enabled),
        1,
        if usage_export_enabled && usage_export_ready {
            "deliver the scheduled usage finance export to the configured webhook"
        } else if usage_export_enabled {
            "scheduled usage finance export is enabled but no target webhook is configured"
        } else {
            "scheduled usage finance export is disabled"
        },
    );
    if !usage_export_enabled {
        usage_item.status = "disabled".to_string();
        usage_item.severity = "info".to_string();
    } else if !usage_export_ready {
        usage_item.status = "blocked".to_string();
        usage_item.severity = "warning".to_string();
    }
    actions.push(usage_item);

    let usage = build_usage_summary(state).await?;
    let cost_alerts = build_cost_alerts(&usage.provider_budgets, generated_at);
    let alert_routes = state.list_cost_alert_routes().await?;
    let active_alert_route_count = alert_routes
        .iter()
        .filter(|route| route.status == "active")
        .count();
    let alert_delivery_ready =
        active_alert_route_count > 0 || state.cost_alert_webhook_url.is_some();
    let mut alert_delivery_item = scheduler_due_plan_item(
        "usage",
        "usage_cost_alert_delivery",
        "auto",
        usize::from(!cost_alerts.is_empty()),
        usize::from(cost_alerts.is_empty()),
        cost_alerts.len().max(1),
        if !cost_alerts.is_empty() && alert_delivery_ready {
            "deliver current provider budget alerts through active alert routes or fallback webhook"
        } else if !cost_alerts.is_empty() {
            "current provider budget alerts exist but no active alert route or fallback webhook is configured"
        } else {
            "no current provider budget alert requires delivery"
        },
    );
    if cost_alerts.is_empty() {
        alert_delivery_item.status = "idle".to_string();
        alert_delivery_item.severity = "info".to_string();
    } else if !alert_delivery_ready {
        alert_delivery_item.status = "blocked".to_string();
        alert_delivery_item.severity = "critical".to_string();
    }
    actions.push(alert_delivery_item);

    let actionable_count = actions
        .iter()
        .filter(|item| item.due_count > 0 && item.status != "blocked")
        .count();
    let item_count = actions.len();
    let status = if actions.iter().any(|item| item.status == "blocked") {
        "blocked"
    } else if actionable_count > 0 {
        "ready"
    } else {
        "idle"
    }
    .to_string();
    Ok(SchedulerDuePlan {
        status,
        generated_at,
        team_count,
        item_count,
        actionable_count,
        actions,
    })
}

pub(crate) fn scheduler_due_plan_item(
    area: &str,
    action: &str,
    mode: &str,
    due_count: usize,
    skipped_count: usize,
    target_count: usize,
    reason: &str,
) -> SchedulerDuePlanItem {
    SchedulerDuePlanItem {
        area: area.to_string(),
        action: action.to_string(),
        mode: mode.to_string(),
        status: if due_count > 0 { "due" } else { "idle" }.to_string(),
        due_count,
        skipped_count,
        target_count,
        severity: if due_count > 0 { "warning" } else { "info" }.to_string(),
        reason: reason.to_string(),
    }
}

pub(crate) async fn execute_scheduler_due_tasks(
    state: &AppState,
    input: Option<SchedulerRunDueRequest>,
) -> Result<SchedulerDueRun, AppError> {
    let checked_at = Utc::now();
    let mut task_errors = Vec::new();
    let request = normalize_scheduler_run_due_request(input, checked_at)?;
    if let Some(existing_run) =
        scheduler_replay_due_run(state, request.idempotency_key.as_deref()).await?
    {
        return Ok(existing_run);
    }
    let providers = match state.list_providers().await {
        Ok(providers) => providers,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "provider_policy_gate.inputs", error);
            Vec::new()
        }
    };
    let audit_logs = match state.list_audit_logs(None).await {
        Ok(audit_logs) => audit_logs,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "provider_policy_gate.audit_logs", error);
            Vec::new()
        }
    };
    let provider_policy_gate = if provider_policy_gate_is_due(&providers, &audit_logs, checked_at) {
        match execute_provider_policy_gate(state, Some("system".to_string()), "system").await {
            Ok(response) => Some(response.run),
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "provider_policy_gate", error);
                None
            }
        }
    } else {
        None
    };
    let policy_rollout_result =
        match scheduler_forced_task_failure("policy_rollout", request.owner.as_deref()) {
            Some(error) => Err(error),
            None => execute_due_policy_rollouts(state, "scheduler", "system").await,
        };
    let policy_rollout = match policy_rollout_result {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "policy_rollout", error);
            failed_policy_rollout_run(checked_at)
        }
    };
    let approval_escalations = match execute_due_approval_escalations(state).await {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "approval_escalations", error);
            failed_approval_escalation_run(checked_at)
        }
    };
    let agent_releases = match execute_due_agent_release_promotions(state).await {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "agent_releases", error);
            failed_agent_release_run(checked_at)
        }
    };
    let stale_poll_request = CodexAppServerStalePollRequest::default();
    let codex_app_server_stale_polls = match execute_stale_codex_app_server_polls(
        state,
        stale_poll_request.clone(),
        "system",
        "system",
    )
    .await
    {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "codex_app_server_stale_polls", error);
            failed_codex_app_server_stale_poll_run(checked_at, &stale_poll_request)
        }
    };
    let workflow_scheduled_steps =
        match execute_due_workflow_scheduled_steps(state, checked_at).await {
            Ok(run) => run,
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "workflow_scheduled_steps", error);
                failed_workflow_scheduled_step_sweep(checked_at)
            }
        };
    let semantic_synthesis_schedules =
        match execute_due_semantic_synthesis_schedules(state, checked_at).await {
            Ok(run) => run,
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "semantic_synthesis_schedules", error);
                failed_semantic_synthesis_schedule_sweep(checked_at)
            }
        };
    let semantic_aging_policies = match execute_due_semantic_aging_policies(state, checked_at).await
    {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "semantic_aging_policies", error);
            failed_semantic_aging_policy_sweep(checked_at)
        }
    };
    let ontology_release_workflow_triggers =
        match drain_due_ontology_release_workflow_triggers(state, "system", 100).await {
            Ok(run) => run,
            Err(error) => {
                scheduler_task_failed(
                    &mut task_errors,
                    "ontology_release_workflow_triggers",
                    error,
                );
                failed_ontology_release_workflow_trigger_drain(checked_at)
            }
        };
    let usage = match build_usage_summary(state).await {
        Ok(usage) => Some(usage),
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "usage_summary", error);
            None
        }
    };
    let cost_alerts = usage
        .as_ref()
        .map(|usage| build_cost_alerts(&usage.provider_budgets, checked_at))
        .unwrap_or_default();
    let active_alert_route_count = match state.list_cost_alert_routes().await {
        Ok(routes) => routes
            .iter()
            .filter(|route| route.status == "active")
            .count(),
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "cost_alert_routes", error);
            0
        }
    };
    let cost_alert_delivery = if !cost_alerts.is_empty()
        && (active_alert_route_count > 0 || state.cost_alert_webhook_url.is_some())
    {
        match execute_cost_alert_delivery(state, checked_at).await {
            Ok(delivery) => Some(delivery),
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "cost_alert_delivery", error);
                Some(failed_cost_alert_delivery(checked_at))
            }
        }
    } else {
        None
    };
    let usage_finance_export =
        match execute_usage_finance_export_delivery(state, true, "system", Some("system")).await {
            Ok(run) => run,
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "usage_finance_export", error);
                failed_usage_finance_export_delivery(checked_at)
            }
        };
    let remote_computer_sidecar_supervision =
        match execute_remote_computer_sidecar_supervision(state).await {
            Ok(run) => run,
            Err(error) => {
                scheduler_task_failed(
                    &mut task_errors,
                    "remote_computer_sidecar_supervision",
                    error,
                );
                failed_remote_computer_sidecar_supervision_run(checked_at)
            }
        };
    let remote_computer_reclaim = match execute_remote_computer_stale_reclaim(state).await {
        Ok(run) => run,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "remote_computer_reclaim", error);
            failed_remote_computer_reclaim_run(checked_at)
        }
    };
    let mut mcp_health_runs = Vec::new();
    let mut mcp_rollout_runs = Vec::new();
    let mut team_count = 0usize;
    let organizations = match state.list_organizations().await {
        Ok(organizations) => organizations,
        Err(error) => {
            scheduler_task_failed(&mut task_errors, "organizations", error);
            Vec::new()
        }
    };
    for organization in organizations
        .into_iter()
        .filter(|organization| organization.archived_at.is_none())
    {
        let teams = match state.list_teams(organization.id).await {
            Ok(teams) => teams,
            Err(error) => {
                scheduler_task_failed(&mut task_errors, "teams", error);
                Vec::new()
            }
        };
        for team in teams.into_iter().filter(|team| team.archived_at.is_none()) {
            team_count += 1;
            match execute_due_mcp_server_health_checks(state, team.id).await {
                Ok(run) => mcp_health_runs.push(run),
                Err(error) => {
                    scheduler_task_failed(&mut task_errors, "mcp_health_checks", error);
                    mcp_health_runs.push(failed_mcp_health_run(team.id, checked_at));
                }
            }
            match execute_due_mcp_server_rollouts(state, team.id).await {
                Ok(run) => mcp_rollout_runs.push(run),
                Err(error) => {
                    scheduler_task_failed(&mut task_errors, "mcp_rollouts", error);
                    mcp_rollout_runs.push(failed_mcp_rollout_run(team.id, checked_at));
                }
            }
        }
    }
    let mut actions = Vec::new();
    if policy_rollout.status == "activated" {
        actions.push("policy_rollout_activated".to_string());
    }
    if provider_policy_gate.is_some() {
        actions.push("provider_policy_gate_processed".to_string());
    }
    if approval_escalations.expired_count > 0 || approval_escalations.escalated_count > 0 {
        actions.push("approval_escalations_processed".to_string());
    }
    if agent_releases.promoted_count > 0 || agent_releases.rejected_count > 0 {
        actions.push("agent_release_automation_processed".to_string());
    }
    if mcp_health_runs.iter().any(|run| run.due_count > 0) {
        actions.push("mcp_health_checks_processed".to_string());
    }
    if mcp_rollout_runs
        .iter()
        .any(|run| run.applied_count > 0 || run.expired_count > 0 || run.failed_count > 0)
    {
        actions.push("mcp_rollouts_processed".to_string());
    }
    if codex_app_server_stale_polls.polled_count > 0
        || codex_app_server_stale_polls.failed_count > 0
    {
        actions.push("codex_app_server_stale_polls_processed".to_string());
    }
    if workflow_scheduled_steps.activated_count > 0 {
        actions.push("workflow_scheduled_steps_activated".to_string());
    }
    if semantic_synthesis_schedules.created_count > 0
        || semantic_synthesis_schedules.failed_count > 0
    {
        actions.push("semantic_synthesis_schedules_processed".to_string());
    }
    if semantic_aging_policies.archived_count > 0 || semantic_aging_policies.failed_count > 0 {
        actions.push("semantic_aging_policies_processed".to_string());
    }
    if ontology_release_workflow_triggers.triggered_count > 0
        || ontology_release_workflow_triggers.failed_count > 0
    {
        actions.push("ontology_release_workflow_triggers_processed".to_string());
    }
    if cost_alert_delivery.is_some() {
        actions.push("usage_cost_alert_delivery_processed".to_string());
    }
    if usage_finance_export.status != "disabled" {
        actions.push("usage_finance_export_processed".to_string());
    }
    if remote_computer_reclaim.reclaimed_attachment_count > 0
        || remote_computer_reclaim.reclaimed_lease_count > 0
    {
        actions.push("remote_computer_reclaim_processed".to_string());
    }
    if remote_computer_sidecar_supervision.missing_heartbeat_count > 0
        || remote_computer_sidecar_supervision.stale_heartbeat_count > 0
    {
        actions.push("remote_computer_sidecar_supervision_processed".to_string());
    }
    let status = if !task_errors.is_empty() {
        "failed"
    } else if actions.is_empty() {
        "noop"
    } else {
        "completed"
    }
    .to_string();
    let run = SchedulerDueRun {
        run_id: Uuid::new_v4(),
        idempotency_key: request.idempotency_key.clone(),
        owner: request.owner.clone().expect("normalized owner"),
        run_window_start: request
            .run_window_start
            .expect("normalized run window start"),
        run_window_end: request.run_window_end.expect("normalized run window end"),
        retry_policy: request
            .retry_policy
            .clone()
            .expect("normalized retry policy"),
        replayed: false,
        status,
        checked_at,
        team_count,
        actions,
        task_errors,
        provider_policy_gate,
        policy_rollout,
        approval_escalations,
        agent_releases,
        workflow_scheduled_steps: Some(workflow_scheduled_steps),
        semantic_synthesis_schedules: Some(semantic_synthesis_schedules),
        semantic_aging_policies: Some(semantic_aging_policies),
        ontology_release_workflow_triggers: Some(ontology_release_workflow_triggers),
        mcp_health_runs,
        mcp_rollout_runs,
        codex_app_server_stale_polls,
        cost_alert_delivery,
        usage_finance_export,
        remote_computer_reclaim,
        remote_computer_sidecar_supervision,
    };
    let run_value = serde_json::to_value(&run)?;
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            None,
            "scheduler.run_due",
            "scheduler",
            None,
            json!({
                "status": run.status,
                "run_id": run.run_id,
                "idempotency_key": run.idempotency_key,
                "owner": run.owner,
                "run_window_start": run.run_window_start,
                "run_window_end": run.run_window_end,
                "retry_policy": run.retry_policy,
                "team_count": run.team_count,
                "actions": run.actions,
                "task_errors": run.task_errors,
                "task_error_count": run.task_errors.len(),
                "provider_policy_gate_status": run.provider_policy_gate.as_ref().map(|gate| gate.status.clone()),
                "workflow_scheduled_step_status": run.workflow_scheduled_steps.as_ref().map(|workflow| workflow.status.clone()),
                "workflow_scheduled_step_activated_count": run.workflow_scheduled_steps.as_ref().map(|workflow| workflow.activated_count).unwrap_or(0),
                "semantic_synthesis_schedule_status": run.semantic_synthesis_schedules.as_ref().map(|semantic| semantic.status.clone()),
                "semantic_synthesis_schedule_created_count": run.semantic_synthesis_schedules.as_ref().map(|semantic| semantic.created_count).unwrap_or(0),
                "semantic_synthesis_schedule_failed_count": run.semantic_synthesis_schedules.as_ref().map(|semantic| semantic.failed_count).unwrap_or(0),
                "semantic_aging_policy_status": run.semantic_aging_policies.as_ref().map(|semantic| semantic.status.clone()),
                "semantic_aging_policy_archived_count": run.semantic_aging_policies.as_ref().map(|semantic| semantic.archived_count).unwrap_or(0),
                "ontology_release_workflow_trigger_status": run.ontology_release_workflow_triggers.as_ref().map(|trigger| trigger.status.clone()),
                "ontology_release_workflow_trigger_triggered_count": run.ontology_release_workflow_triggers.as_ref().map(|trigger| trigger.triggered_count).unwrap_or(0),
                "ontology_release_workflow_trigger_failed_count": run.ontology_release_workflow_triggers.as_ref().map(|trigger| trigger.failed_count).unwrap_or(0),
                "cost_alert_delivery_status": run.cost_alert_delivery.as_ref().map(|delivery| delivery.status.clone()),
                "remote_computer_sidecar_supervision_status": run.remote_computer_sidecar_supervision.status,
                "remote_computer_sidecar_missing_heartbeat_count": run.remote_computer_sidecar_supervision.missing_heartbeat_count,
                "remote_computer_sidecar_stale_heartbeat_count": run.remote_computer_sidecar_supervision.stale_heartbeat_count,
                "checked_at": run.checked_at,
                "run": run_value,
            }),
        ))
        .await?;
    Ok(run)
}

fn scheduler_task_failed(task_errors: &mut Vec<SchedulerTaskError>, task: &str, error: AppError) {
    task_errors.push(SchedulerTaskError {
        task: task.to_string(),
        message: error.message,
    });
}

fn scheduler_forced_task_failure(task: &str, owner: Option<&str>) -> Option<AppError> {
    #[cfg(test)]
    {
        if owner == Some("__force_policy_rollout_failure") && task == "policy_rollout" {
            return Some(AppError::bad_request(format!(
                "forced scheduler task failure: {task}"
            )));
        }
    }
    #[cfg(not(test))]
    {
        let _ = task;
        let _ = owner;
    }
    None
}

fn failed_policy_rollout_run(checked_at: DateTime<Utc>) -> PolicyScheduledRolloutRun {
    PolicyScheduledRolloutRun {
        status: "failed".to_string(),
        activated_revision_id: None,
        activated_revision: None,
        controller_id: None,
        policy_store_id: None,
        deployment_id: None,
        scanned_count: 0,
        skipped_count: 0,
        scanned_revisions: Vec::new(),
        checked_at,
        reason: "scheduler task failed before policy rollout completed".to_string(),
    }
}

fn failed_approval_escalation_run(checked_at: DateTime<Utc>) -> ApprovalEscalationDueRun {
    ApprovalEscalationDueRun {
        status: "failed".to_string(),
        checked_at,
        expired_count: 0,
        escalated_count: 0,
        skipped_count: 0,
        notification_deliveries: Vec::new(),
    }
}

fn failed_agent_release_run(checked_at: DateTime<Utc>) -> AgentReleaseAutomationRun {
    AgentReleaseAutomationRun {
        checked_at,
        pending_count: 0,
        promoted_count: 0,
        rejected_count: 0,
        skipped_count: 0,
        controller_required: false,
        controller_configured: false,
        controller_execution_count: 0,
        controller_failed_count: 1,
        results: Vec::new(),
    }
}

fn failed_codex_app_server_stale_poll_run(
    checked_at: DateTime<Utc>,
    request: &CodexAppServerStalePollRequest,
) -> CodexAppServerStalePollRun {
    CodexAppServerStalePollRun {
        checked_at,
        stale_after_seconds: request.stale_after_seconds,
        candidate_count: 0,
        polled_count: 0,
        terminal_count: 0,
        skipped_count: 0,
        failed_count: 1,
        results: Vec::new(),
    }
}

fn failed_workflow_scheduled_step_sweep(
    checked_at: DateTime<Utc>,
) -> WorkflowScheduledStepActivationSweep {
    WorkflowScheduledStepActivationSweep {
        status: "failed".to_string(),
        checked_at,
        workflow_run_count: 0,
        scheduled_step_count: 0,
        due_step_count: 0,
        activated_count: 0,
        activated_step_ids: Vec::new(),
        remaining_scheduled_count: 0,
        actions: Vec::new(),
    }
}

fn failed_semantic_synthesis_schedule_sweep(
    checked_at: DateTime<Utc>,
) -> SemanticSynthesisScheduleSweep {
    SemanticSynthesisScheduleSweep {
        status: "failed".to_string(),
        checked_at,
        scheduled_count: 0,
        due_count: 0,
        created_count: 0,
        skipped_count: 0,
        failed_count: 1,
        runs: Vec::new(),
        actions: Vec::new(),
    }
}

fn failed_semantic_aging_policy_sweep(checked_at: DateTime<Utc>) -> SemanticAgingPolicySweep {
    SemanticAgingPolicySweep {
        status: "failed".to_string(),
        checked_at,
        policy_count: 0,
        due_count: 0,
        archived_count: 0,
        skipped_count: 0,
        failed_count: 1,
        archived_object_ids: Vec::new(),
        runs: Vec::new(),
        actions: Vec::new(),
    }
}

fn failed_ontology_release_workflow_trigger_drain(
    checked_at: DateTime<Utc>,
) -> OntologyReleaseWorkflowTriggerDrain {
    OntologyReleaseWorkflowTriggerDrain {
        status: "failed".to_string(),
        checked_at,
        retryable_count: 0,
        triggered_count: 0,
        skipped_count: 0,
        failed_count: 1,
        trigger_ids: Vec::new(),
    }
}

fn failed_cost_alert_delivery(checked_at: DateTime<Utc>) -> CostAlertDelivery {
    CostAlertDelivery {
        status: "failed".to_string(),
        delivered: false,
        channel: "scheduler".to_string(),
        webhook_configured: false,
        alerts: Vec::new(),
        route_deliveries: Vec::new(),
        delivered_at: checked_at,
    }
}

fn failed_usage_finance_export_delivery(checked_at: DateTime<Utc>) -> UsageFinanceExportDelivery {
    UsageFinanceExportDelivery {
        status: "failed".to_string(),
        delivered: false,
        channel: "scheduler".to_string(),
        scheduled: true,
        target_configured: false,
        delivery_id: Uuid::new_v4(),
        file_name: "usage-finance-export-failed.csv".to_string(),
        bytes: 0,
        export_bytes: 0,
        record_count: 0,
        provider_count: 0,
        budget_pressure_count: 0,
        rollup_count: 0,
        delivered_at: checked_at,
    }
}

fn failed_remote_computer_sidecar_supervision_run(
    checked_at: DateTime<Utc>,
) -> RemoteComputerSidecarSupervisionRun {
    RemoteComputerSidecarSupervisionRun {
        status: "failed".to_string(),
        checked_at,
        active_remote_computer_count: 0,
        heartbeat_count: 0,
        missing_heartbeat_count: 0,
        stale_heartbeat_count: 0,
        stale_after_seconds: 0,
        actions: Vec::new(),
    }
}

fn failed_remote_computer_reclaim_run(checked_at: DateTime<Utc>) -> RemoteComputerReclaimRun {
    RemoteComputerReclaimRun {
        generated_at: checked_at,
        status: "failed".to_string(),
        stale_attachment_count: 0,
        reclaimed_attachment_count: 0,
        expired_lease_count: 0,
        reclaimed_lease_count: 0,
        replayed_cleanup_evidence_count: 0,
        attachments: Vec::new(),
        leases: Vec::new(),
        execution_enabled: false,
    }
}

fn failed_mcp_health_run(team_id: Uuid, checked_at: DateTime<Utc>) -> McpServerScheduledHealthRun {
    McpServerScheduledHealthRun {
        team_id,
        due_count: 0,
        skipped_count: 0,
        healthy_count: 0,
        unhealthy_count: 1,
        results: Vec::new(),
        checked_at,
    }
}

fn failed_mcp_rollout_run(team_id: Uuid, checked_at: DateTime<Utc>) -> McpServerRolloutDueRun {
    McpServerRolloutDueRun {
        team_id,
        applied_count: 0,
        skipped_count: 0,
        expired_count: 0,
        failed_count: 1,
        controller_required: false,
        controller_configured: false,
        controller_execution_count: 0,
        controller_failed_count: 0,
        results: Vec::new(),
        checked_at,
    }
}

pub(crate) fn normalize_scheduler_run_due_request(
    input: Option<SchedulerRunDueRequest>,
    checked_at: DateTime<Utc>,
) -> Result<SchedulerRunDueRequest, AppError> {
    let input = input.unwrap_or(SchedulerRunDueRequest {
        idempotency_key: None,
        owner: None,
        run_window_start: None,
        run_window_end: None,
        retry_policy: None,
    });
    let idempotency_key = input
        .idempotency_key
        .map(|value| validate_scheduler_slug("idempotency_key", &value))
        .transpose()?;
    let owner = input
        .owner
        .map(|value| validate_scheduler_slug("owner", &value))
        .transpose()?
        .unwrap_or_else(|| "manual".to_string());
    let run_window_start = input.run_window_start.unwrap_or(checked_at);
    let run_window_end = input
        .run_window_end
        .unwrap_or_else(|| checked_at + ChronoDuration::minutes(5));
    if run_window_end <= run_window_start {
        return Err(AppError::bad_request(
            "scheduler run_window_end must be after run_window_start",
        ));
    }
    let retry_policy = input.retry_policy.unwrap_or(SchedulerRetryPolicy {
        max_attempts: 1,
        backoff_seconds: 0,
    });
    if retry_policy.max_attempts == 0 || retry_policy.max_attempts > 10 {
        return Err(AppError::bad_request(
            "scheduler retry_policy.max_attempts must be between 1 and 10",
        ));
    }
    if retry_policy.backoff_seconds > 3600 {
        return Err(AppError::bad_request(
            "scheduler retry_policy.backoff_seconds must be 3600 or less",
        ));
    }
    Ok(SchedulerRunDueRequest {
        idempotency_key,
        owner: Some(owner),
        run_window_start: Some(run_window_start),
        run_window_end: Some(run_window_end),
        retry_policy: Some(retry_policy),
    })
}

pub(crate) async fn scheduler_replay_due_run(
    state: &AppState,
    idempotency_key: Option<&str>,
) -> Result<Option<SchedulerDueRun>, AppError> {
    let Some(idempotency_key) = idempotency_key else {
        return Ok(None);
    };
    let audit_logs = state.list_audit_logs(None).await?;
    let Some(log) = audit_logs
        .into_iter()
        .filter(|log| log.action == "scheduler.run_due")
        .find(|log| log.details["idempotency_key"].as_str() == Some(idempotency_key))
    else {
        return Ok(None);
    };
    let mut run: SchedulerDueRun = serde_json::from_value(log.details["run"].clone())
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    run.replayed = true;
    Ok(Some(run))
}

pub(crate) fn validate_scheduler_slug(field: &str, value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err(AppError::bad_request(format!(
            "scheduler {field} must be non-empty token text"
        )));
    }
    Ok(trimmed.to_string())
}
