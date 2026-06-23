use std::{collections::HashSet, time::Duration};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

pub(crate) async fn poll_codex_app_server_run_inner(
    state: &AppState,
    run_id: Uuid,
    input: CodexAppServerPollRequest,
    actor_type: &str,
    subject: String,
) -> Result<CodexAppServerPollResponse, AppError> {
    let config = codex_app_server_config(state)?;
    let run = state.get_codex_app_server_run(run_id).await?;
    let turn_id = run
        .turn_id
        .clone()
        .ok_or_else(|| AppError::bad_request("Codex App Server run has no turn_id to poll"))?;
    let max_attempts = input.max_attempts.clamp(1, 10);
    let retry_interval_ms = input.retry_interval_ms.min(5_000);
    let mut attempts = 0;
    let mut last_status = run.status.clone();
    let mut terminal = false;
    let mut updated = run;

    while attempts < max_attempts && !terminal {
        attempts += 1;
        match state
            .codex_app_server_client
            .get_turn_status(config, &turn_id)
            .await
        {
            Ok(response) => {
                last_status = response
                    .status
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                terminal = codex_turn_status_is_terminal(&last_status);
                updated = state
                    .update_codex_app_server_run_status(
                        run_id,
                        last_status.clone(),
                        serde_json::to_value(&response)?,
                        None,
                    )
                    .await?;
            }
            Err(error) => {
                last_status = "poll_failed".to_string();
                updated = state
                    .update_codex_app_server_run_status(
                        run_id,
                        last_status.clone(),
                        updated.response.clone(),
                        Some(json!({"message": error.message, "attempt": attempts})),
                    )
                    .await?;
                terminal = attempts >= max_attempts;
            }
        }
        if attempts < max_attempts && !terminal && retry_interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
        }
    }

    let response = CodexAppServerPollResponse {
        run: updated,
        attempts,
        terminal,
        last_status,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            actor_type,
            None,
            "codex_app_server.run_polled",
            "codex_app_server_run",
            Some(run_id),
            json!({
                "subject": subject,
                "run_id": run_id,
                "turn_id": turn_id,
                "attempts": response.attempts,
                "terminal": response.terminal,
                "last_status": response.last_status,
            }),
        ))
        .await?;
    Ok(response)
}

pub(crate) async fn execute_stale_codex_app_server_polls(
    state: &AppState,
    input: CodexAppServerStalePollRequest,
    actor_type: &str,
    subject: &str,
) -> Result<CodexAppServerStalePollRun, AppError> {
    let checked_at = Utc::now();
    let stale_after_seconds = input.stale_after_seconds.min(86_400);
    let max_runs = input.max_runs.clamp(1, 100);
    let runs = state.list_codex_app_server_runs().await?;
    let candidates = select_stale_codex_app_server_runs(&runs, checked_at, stale_after_seconds);
    let candidate_count = candidates.len();
    let mut results = Vec::new();
    let mut polled_count = 0usize;
    let mut terminal_count = 0usize;
    let mut failed_count = 0usize;
    let mut skipped_count = candidate_count.saturating_sub(max_runs);

    if state.codex_app_server_config.is_none() {
        skipped_count = candidate_count;
        for run in candidates {
            results.push(json!({
                "run_id": run.id,
                "turn_id": run.turn_id,
                "status": "skipped",
                "reason": "codex_app_server_reserved",
            }));
        }
    } else {
        for run in candidates.into_iter().take(max_runs) {
            let poll_input = CodexAppServerPollRequest {
                max_attempts: input.max_attempts,
                retry_interval_ms: input.retry_interval_ms,
            };
            match poll_codex_app_server_run_inner(
                state,
                run.id,
                poll_input,
                actor_type,
                subject.to_string(),
            )
            .await
            {
                Ok(response) => {
                    polled_count += 1;
                    if response.terminal {
                        terminal_count += 1;
                    }
                    results.push(json!({
                        "run_id": response.run.id,
                        "turn_id": response.run.turn_id,
                        "status": "polled",
                        "last_status": response.last_status,
                        "attempts": response.attempts,
                        "terminal": response.terminal,
                    }));
                }
                Err(error) => {
                    failed_count += 1;
                    results.push(json!({
                        "run_id": run.id,
                        "turn_id": run.turn_id,
                        "status": "failed",
                        "error": error.message,
                    }));
                }
            }
        }
    }

    let run = CodexAppServerStalePollRun {
        checked_at,
        stale_after_seconds,
        candidate_count,
        polled_count,
        terminal_count,
        skipped_count,
        failed_count,
        results,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            actor_type,
            None,
            "codex_app_server.stale_poll_due_run",
            "codex_app_server",
            None,
            json!({
                "subject": subject,
                "stale_after_seconds": run.stale_after_seconds,
                "candidate_count": run.candidate_count,
                "polled_count": run.polled_count,
                "terminal_count": run.terminal_count,
                "skipped_count": run.skipped_count,
                "failed_count": run.failed_count,
            }),
        ))
        .await?;
    Ok(run)
}

pub(crate) fn select_stale_codex_app_server_runs(
    runs: &[CodexAppServerRun],
    now: DateTime<Utc>,
    stale_after_seconds: u64,
) -> Vec<CodexAppServerRun> {
    let cutoff = now - chrono::Duration::seconds(stale_after_seconds as i64);
    let mut latest_by_turn = BTreeMap::<String, &CodexAppServerRun>::new();
    for run in runs {
        let Some(turn_id) = run.turn_id.as_ref() else {
            continue;
        };
        if !run.operation.starts_with("turn.") {
            continue;
        }
        if codex_turn_status_is_terminal(&run.status) || run.created_at > cutoff {
            continue;
        }
        latest_by_turn
            .entry(turn_id.clone())
            .and_modify(|existing| {
                if existing.created_at < run.created_at {
                    *existing = run;
                }
            })
            .or_insert(run);
    }
    let mut candidates = latest_by_turn.into_values().cloned().collect::<Vec<_>>();
    candidates.sort_by_key(|run| run.created_at);
    candidates
}

pub(crate) fn codex_turn_status_is_terminal(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "completed" | "failed" | "cancelled" | "canceled" | "interrupted"
    )
}

pub(crate) fn build_codex_app_server_trace_summary(
    runs: &[CodexAppServerRun],
) -> CodexAppServerTraceSummary {
    let mut by_status = HashMap::new();
    let mut by_operation = HashMap::new();
    let mut by_failure_domain = HashMap::new();
    let mut grouped = BTreeMap::<String, Vec<&CodexAppServerRun>>::new();
    for run in runs {
        increment_count(&mut by_status, run.status.as_str());
        increment_count(&mut by_operation, run.operation.as_str());
        grouped
            .entry(codex_app_server_trace_key(run))
            .or_default()
            .push(run);
    }
    let mut traces = Vec::new();
    for (trace_key, mut group) in grouped {
        group.sort_by_key(|run| run.created_at);
        let first = group.first().expect("group has at least one run");
        let latest = group.last().expect("group has at least one run");
        let mut operations = BTreeSet::new();
        let mut command_ids = BTreeSet::new();
        let mut command_count = 0usize;
        let mut poll_count = 0usize;
        let mut error_count = 0usize;
        for run in &group {
            operations.insert(run.operation.clone());
            if run.operation.contains("command") || run.command_id.is_some() {
                command_count += 1;
            }
            if let Some(command_id) = run.command_id.as_ref() {
                command_ids.insert(command_id.clone());
            }
            if run.operation.contains("poll") {
                poll_count += 1;
            }
            if run.error.is_some() || codex_run_status_failed(&run.status) {
                error_count += 1;
            }
        }
        let latest_status = latest.status.clone();
        let terminal = codex_turn_status_is_terminal(&latest_status);
        let latest_error = group.iter().rev().find_map(|run| run.error.clone());
        let dashboard = build_codex_trace_dashboard(&group, &[], 0, terminal);
        increment_count(&mut by_failure_domain, dashboard.failure_domain.as_str());
        traces.push(CodexTurnTrace {
            trace_key,
            turn_id: latest.turn_id.clone().or_else(|| first.turn_id.clone()),
            thread_id: latest.thread_id.clone().or_else(|| first.thread_id.clone()),
            latest_run_id: latest.id,
            latest_status: latest_status.clone(),
            terminal,
            run_count: group.len(),
            command_count,
            poll_count,
            error_count,
            duration_seconds: (latest.created_at - first.created_at).num_seconds(),
            command_ids: command_ids.into_iter().collect(),
            operations: operations.into_iter().collect(),
            next_action: dashboard.operator_action.clone(),
            latest_error,
            dashboard,
            first_seen_at: first.created_at,
            last_seen_at: latest.created_at,
        });
    }
    traces.sort_by_key(|trace| std::cmp::Reverse(trace.last_seen_at));
    let active_turn_count = traces
        .iter()
        .filter(|trace| trace.turn_id.is_some() && !trace.terminal)
        .count();
    let failed_turn_count = traces
        .iter()
        .filter(|trace| codex_run_status_failed(&trace.latest_status) || trace.error_count > 0)
        .count();
    CodexAppServerTraceSummary {
        generated_at: Utc::now(),
        run_count: runs.len(),
        turn_count: traces
            .iter()
            .filter(|trace| trace.turn_id.is_some())
            .count(),
        active_turn_count,
        failed_turn_count,
        by_status,
        by_operation,
        by_failure_domain,
        traces,
    }
}

pub(crate) fn build_codex_app_server_control_plane_summary(
    config: Option<&CodexAppServerConfig>,
    runs: &[CodexAppServerRun],
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
) -> CodexAppServerControlPlaneSummary {
    let trace_summary = build_codex_app_server_trace_summary(runs);
    let stale_candidates =
        select_stale_codex_app_server_runs(runs, generated_at, default_codex_stale_after_seconds());
    let stale_candidate_count = stale_candidates.len();
    let pollable_turn_count = trace_summary
        .traces
        .iter()
        .filter(|trace| trace.turn_id.is_some() && !trace.terminal)
        .count();
    let latest_seen_at = trace_summary
        .traces
        .iter()
        .map(|trace| trace.last_seen_at)
        .max()
        .or_else(|| runs.iter().map(|run| run.created_at).max());
    let mut attention_items = Vec::new();

    if config.is_none() {
        attention_items.push(CodexAppServerControlPlaneAttentionItem {
            kind: "reserved_adapter".to_string(),
            severity: "warning".to_string(),
            message:
                "Codex App Server is reserved until MANDOFORGE_CODEX_APP_SERVER_URL is configured"
                    .to_string(),
            trace_key: None,
            turn_id: None,
        });
    }
    for trace in &trace_summary.traces {
        if codex_run_status_failed(&trace.latest_status) || trace.error_count > 0 {
            attention_items.push(CodexAppServerControlPlaneAttentionItem {
                kind: "failed_trace".to_string(),
                severity: "critical".to_string(),
                message: format!(
                    "trace latest status is {} with {} error signals",
                    trace.latest_status, trace.error_count
                ),
                trace_key: Some(trace.trace_key.clone()),
                turn_id: trace.turn_id.clone(),
            });
        } else if trace.turn_id.is_some() && !trace.terminal {
            attention_items.push(CodexAppServerControlPlaneAttentionItem {
                kind: "active_turn".to_string(),
                severity: "warning".to_string(),
                message: "turn is still non-terminal and should be polled or interrupted"
                    .to_string(),
                trace_key: Some(trace.trace_key.clone()),
                turn_id: trace.turn_id.clone(),
            });
        }
    }
    for run in stale_candidates {
        attention_items.push(CodexAppServerControlPlaneAttentionItem {
            kind: "stale_turn".to_string(),
            severity: if config.is_some() {
                "warning".to_string()
            } else {
                "critical".to_string()
            },
            message: format!(
                "turn has not reached a terminal state within {} seconds",
                default_codex_stale_after_seconds()
            ),
            trace_key: Some(codex_app_server_trace_key(&run)),
            turn_id: run.turn_id,
        });
    }

    let timeout_seconds = config.map(|config| config.timeout_seconds);
    let lookup = |key: &str| std::env::var(key).ok();
    let production_ops = build_codex_app_server_production_ops_readiness(
        config.is_some(),
        &trace_summary,
        stale_candidate_count,
        audit_logs,
        generated_at,
        codex_app_server_ops_controller_required(&lookup),
        codex_app_server_ops_controller_configured(&lookup),
    );
    let deployment_readiness = build_codex_app_server_deployment_readiness(
        config.is_some(),
        audit_logs,
        generated_at,
        codex_app_server_deployment_controller_required(&lookup),
        codex_app_server_deployment_controller_configured(&lookup),
    );
    if production_ops.production_blocked {
        attention_items.push(CodexAppServerControlPlaneAttentionItem {
            kind: "production_ops_blocked".to_string(),
            severity: if production_ops.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: production_ops.message.clone(),
            trace_key: None,
            turn_id: None,
        });
    }
    if deployment_readiness.production_blocked {
        attention_items.push(CodexAppServerControlPlaneAttentionItem {
            kind: "deployment_validation_blocked".to_string(),
            severity: if deployment_readiness.status == "blocked" {
                "critical".to_string()
            } else {
                "warning".to_string()
            },
            message: deployment_readiness.message.clone(),
            trace_key: None,
            turn_id: None,
        });
    }
    let status = if config.is_none() {
        "reserved"
    } else if attention_items
        .iter()
        .any(|item| item.severity == "critical")
    {
        "critical"
    } else if attention_items.is_empty() {
        "ready"
    } else {
        "attention"
    }
    .to_string();

    CodexAppServerControlPlaneSummary {
        generated_at,
        configured: config.is_some(),
        status,
        endpoint_configured: config.is_some(),
        timeout_seconds,
        run_count: trace_summary.run_count,
        turn_count: trace_summary.turn_count,
        active_turn_count: trace_summary.active_turn_count,
        failed_turn_count: trace_summary.failed_turn_count,
        stale_candidate_count,
        pollable_turn_count,
        latest_seen_at,
        by_status: trace_summary.by_status,
        by_operation: trace_summary.by_operation,
        production_ops,
        deployment_readiness,
        attention_items,
    }
}

pub(crate) fn build_codex_trace_dashboard(
    runs: &[&CodexAppServerRun],
    evidence: &[CodexTraceEvidence],
    artifact_lineage_count: usize,
    terminal: bool,
) -> CodexTraceDashboard {
    let mut dashboard = CodexTraceDashboard::default();
    for run in runs {
        if run.operation.contains("command") || run.command_id.is_some() {
            dashboard.command_count += 1;
        }
        if run.operation.contains("poll") {
            dashboard.poll_count += 1;
        }
        if run.operation.contains("interrupt") {
            dashboard.interrupt_count += 1;
        }
        if run.error.is_some() || codex_run_status_failed(&run.status) {
            dashboard.failed_operation_count += 1;
        }
    }
    for item in evidence {
        match item.kind.as_str() {
            "poll" => dashboard.poll_count += 1,
            "interrupt" => dashboard.interrupt_count += 1,
            "worker_lease" => dashboard.worker_lease_count += 1,
            "retry" => dashboard.retry_count += 1,
            "fallback" => dashboard.fallback_count += 1,
            "artifact_sync" => dashboard.artifact_sync_count += 1,
            "failed" => dashboard.failed_operation_count += 1,
            _ => {}
        }
    }
    dashboard.artifact_sync_count = dashboard.artifact_sync_count.max(artifact_lineage_count);
    dashboard.stuck = runs
        .last()
        .is_some_and(|run| run.turn_id.is_some() && !terminal);
    dashboard.failure_domain =
        if dashboard.fallback_count > 0 && dashboard.failed_operation_count > 0 {
            "fallback"
        } else if dashboard.retry_count > 0 && dashboard.failed_operation_count > 0 {
            "worker_lease"
        } else if dashboard.failed_operation_count > 0
            && runs
                .last()
                .is_some_and(|run| run.operation.contains("poll") || run.status == "poll_failed")
        {
            "poll"
        } else if dashboard.failed_operation_count > 0 && dashboard.interrupt_count > 0 {
            "interrupt"
        } else if dashboard.failed_operation_count > 0 && dashboard.command_count > 0 {
            "command"
        } else if dashboard.failed_operation_count > 0 {
            "provider"
        } else if dashboard.stuck {
            "non_terminal_poll"
        } else {
            "none"
        }
        .to_string();
    dashboard.operator_action = match dashboard.failure_domain.as_str() {
        "none" if terminal && dashboard.artifact_sync_count > 0 => "inspect_artifact_lineage",
        "none" if terminal => "complete",
        "none" => "observe",
        "non_terminal_poll" => "poll_or_interrupt",
        "fallback" => "inspect_fallback_reason",
        "worker_lease" => "inspect_worker_retry",
        "poll" => "inspect_poll_failure",
        "interrupt" => "inspect_interrupt_reason",
        "command" => "inspect_command_failure",
        _ => "inspect_provider_error",
    }
    .to_string();
    dashboard
}

pub(crate) fn codex_trace_evidence(
    runs: &[CodexAppServerRun],
    events: &[SessionEvent],
    audit_logs: &[AuditLog],
) -> Vec<CodexTraceEvidence> {
    let run_ids = runs.iter().map(|run| run.id).collect::<HashSet<_>>();
    let turn_ids = runs
        .iter()
        .filter_map(|run| run.turn_id.clone())
        .collect::<HashSet<_>>();
    let command_ids = runs
        .iter()
        .filter_map(|run| run.command_id.clone())
        .collect::<HashSet<_>>();
    let session_ids = codex_trace_session_ids(runs);
    let mut evidence = Vec::new();
    for event in events {
        let Some(kind) = codex_trace_event_kind(event) else {
            continue;
        };
        if !codex_trace_event_matches(event, &run_ids, &turn_ids, &command_ids, &session_ids) {
            continue;
        }
        evidence.push(CodexTraceEvidence {
            source: "session_event".to_string(),
            kind,
            message: codex_trace_event_message(event),
            run_id: value_uuid(&event.payload, "run_id"),
            event_id: Some(event.id),
            audit_log_id: None,
            session_id: Some(event.session_id),
            created_at: event.created_at,
            details: json!({
                "event_type": event.event_type,
                "actor_type": event.actor_type,
                "actor_id": event.actor_id,
                "payload": event.payload,
            }),
        });
    }
    for log in audit_logs {
        let Some(kind) = codex_trace_audit_kind(log) else {
            continue;
        };
        if !codex_trace_audit_matches(log, &run_ids, &turn_ids, &command_ids, &session_ids) {
            continue;
        }
        evidence.push(CodexTraceEvidence {
            source: "audit_log".to_string(),
            kind,
            message: codex_trace_audit_message(log),
            run_id: value_uuid(&log.details, "run_id"),
            event_id: None,
            audit_log_id: Some(log.id),
            session_id: log.session_id,
            created_at: log.created_at,
            details: json!({
                "action": log.action,
                "actor_type": log.actor_type,
                "actor_id": log.actor_id,
                "resource_type": log.resource_type,
                "resource_id": log.resource_id,
                "details": log.details,
            }),
        });
    }
    evidence.sort_by_key(|item| item.created_at);
    evidence
}

pub(crate) fn codex_trace_artifact_lineage(
    runs: &[CodexAppServerRun],
    audit_logs: &[AuditLog],
) -> Vec<CodexTraceArtifactLineage> {
    let turn_ids = runs
        .iter()
        .filter_map(|run| run.turn_id.clone())
        .collect::<HashSet<_>>();
    let command_ids = runs
        .iter()
        .filter_map(|run| run.command_id.clone())
        .collect::<HashSet<_>>();
    let session_ids = codex_trace_session_ids(runs);
    let mut lineage = audit_logs
        .iter()
        .filter(|log| log.action == "codex_app_server.artifact_synced")
        .filter(|log| {
            codex_trace_audit_matches(log, &HashSet::new(), &turn_ids, &command_ids, &session_ids)
        })
        .filter_map(|log| {
            let artifact_id = log
                .resource_id
                .or_else(|| value_uuid(&log.details, "artifact_id"))?;
            Some(CodexTraceArtifactLineage {
                artifact_id,
                session_id: log.session_id,
                turn_id: value_string(&log.details, "turn_id"),
                command_id: value_string(&log.details, "command_id"),
                name: value_string(&log.details, "name"),
                path: value_string(&log.details, "path"),
                artifact_type: value_string(&log.details, "artifact_type"),
                created_at: log.created_at,
            })
        })
        .collect::<Vec<_>>();
    lineage.sort_by_key(|item| item.created_at);
    lineage
}

pub(crate) fn codex_trace_session_ids(runs: &[CodexAppServerRun]) -> HashSet<Uuid> {
    let mut session_ids = HashSet::new();
    for run in runs {
        for value in [
            run.request.get("session_id"),
            run.response.get("session_id"),
            run.request.pointer("/metadata/session_id"),
            run.response.pointer("/metadata/session_id"),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(session_id) = value.as_str().and_then(|value| Uuid::parse_str(value).ok()) {
                session_ids.insert(session_id);
            }
        }
    }
    session_ids
}

pub(crate) fn codex_trace_event_kind(event: &SessionEvent) -> Option<String> {
    let kind = match event.event_type.as_str() {
        "codex.task.event" => "poll",
        "codex.task.fallback" => "fallback",
        "codex.task.failed" => "failed",
        "codex.task.started" | "codex.task.completed" => "worker_lease",
        "execution.queued" => "worker_lease",
        "execution.retry_queued" => "retry",
        "artifact.created"
            if event.payload.get("source").and_then(Value::as_str) == Some("codex_app_server") =>
        {
            "artifact_sync"
        }
        _ => return None,
    };
    Some(kind.to_string())
}

pub(crate) fn codex_trace_audit_kind(log: &AuditLog) -> Option<String> {
    let kind = match log.action.as_str() {
        "codex_app_server.run_polled" => "poll",
        "codex_app_server.artifact_synced" => "artifact_sync",
        _ => return None,
    };
    Some(kind.to_string())
}

pub(crate) fn codex_trace_event_matches(
    event: &SessionEvent,
    run_ids: &HashSet<Uuid>,
    turn_ids: &HashSet<String>,
    command_ids: &HashSet<String>,
    session_ids: &HashSet<Uuid>,
) -> bool {
    value_uuid(&event.payload, "run_id").is_some_and(|run_id| run_ids.contains(&run_id))
        || value_string(&event.payload, "turn_id")
            .is_some_and(|turn_id| turn_ids.contains(&turn_id))
        || value_string(&event.payload, "command_id")
            .is_some_and(|command_id| command_ids.contains(&command_id))
        || (!session_ids.is_empty() && session_ids.contains(&event.session_id))
}

pub(crate) fn codex_trace_audit_matches(
    log: &AuditLog,
    run_ids: &HashSet<Uuid>,
    turn_ids: &HashSet<String>,
    command_ids: &HashSet<String>,
    session_ids: &HashSet<Uuid>,
) -> bool {
    value_uuid(&log.details, "run_id").is_some_and(|run_id| run_ids.contains(&run_id))
        || value_string(&log.details, "turn_id").is_some_and(|turn_id| turn_ids.contains(&turn_id))
        || value_string(&log.details, "command_id")
            .is_some_and(|command_id| command_ids.contains(&command_id))
        || log
            .session_id
            .is_some_and(|session_id| session_ids.contains(&session_id))
}

pub(crate) fn codex_trace_event_message(event: &SessionEvent) -> String {
    if let Some(error) = event.payload.get("error").and_then(Value::as_str) {
        return error.to_string();
    }
    if let Some(reason) = event.payload.get("reason").and_then(Value::as_str) {
        return reason.to_string();
    }
    if let Some(status) = event.payload.get("status").and_then(Value::as_str) {
        return format!("{} status {status}", event.event_type);
    }
    event.event_type.clone()
}

pub(crate) fn codex_trace_audit_message(log: &AuditLog) -> String {
    if let Some(status) = log
        .details
        .get("last_status")
        .or_else(|| log.details.get("status"))
        .and_then(Value::as_str)
    {
        return format!("{} status {status}", log.action);
    }
    if let Some(name) = log.details.get("name").and_then(Value::as_str) {
        return format!("{} {name}", log.action);
    }
    log.action.clone()
}

pub(crate) fn value_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn value_uuid(value: &Value, key: &str) -> Option<Uuid> {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

pub(crate) fn build_codex_app_server_production_ops_readiness(
    configured: bool,
    trace_summary: &CodexAppServerTraceSummary,
    stale_candidate_count: usize,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> CodexAppServerProductionOpsReadiness {
    let latest_stale_poll = audit_logs
        .iter()
        .filter(|log| log.action == "codex_app_server.stale_poll_due_run")
        .max_by_key(|log| log.created_at);
    let latest_stale_poll_at = latest_stale_poll.map(|log| log.created_at);
    let latest_stale_poll_age_hours =
        latest_stale_poll_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_stale_poll_candidate_count = latest_stale_poll
        .and_then(|log| log.details.get("candidate_count"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let latest_stale_poll_failed_count = latest_stale_poll
        .and_then(|log| log.details.get("failed_count"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let latest_controller_log = audit_logs
        .iter()
        .filter(|log| log.action == "codex_app_server.ops_validation")
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
    if !configured {
        blocking_reasons.push("adapter endpoint is not configured".to_string());
    }
    if trace_summary.failed_turn_count > 0 {
        blocking_reasons.push("failed/interrupted turn traces are present".to_string());
    }
    if stale_candidate_count > 0 {
        blocking_reasons.push("stale non-terminal turns are present".to_string());
    }
    if latest_stale_poll.is_none() {
        blocking_reasons.push("stale-turn supervision has not run".to_string());
    }
    if latest_stale_poll_failed_count > 0 {
        blocking_reasons.push("latest stale-turn poll run failed".to_string());
    }
    if latest_stale_poll_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("stale-turn supervision is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons
            .push("Codex App Server ops controller is required but not configured".to_string());
    }
    if controller_required && !latest_controller_validated {
        blocking_reasons.push(
            "Codex App Server ops controller evidence is missing or not validated".to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons.push("Codex App Server ops controller evidence is stale".to_string());
    }
    let production_blocked = !blocking_reasons.is_empty();
    let status = if production_blocked {
        "blocked"
    } else if trace_summary.active_turn_count > 0 {
        "attention"
    } else {
        "ready"
    };
    let message = if production_blocked {
        format!(
            "Codex App Server production ops are blocked: {}",
            blocking_reasons.join("; ")
        )
    } else if trace_summary.active_turn_count > 0 {
        "Codex App Server has active non-terminal turns, but stale-turn supervision and required ops controller evidence are fresh".to_string()
    } else {
        "Codex App Server stale-turn supervision, turn traces, and required ops controller evidence are ready".to_string()
    };
    CodexAppServerProductionOpsReadiness {
        status: status.to_string(),
        production_blocked,
        configured,
        run_count: trace_summary.run_count,
        active_turn_count: trace_summary.active_turn_count,
        failed_turn_count: trace_summary.failed_turn_count,
        stale_candidate_count,
        latest_stale_poll_at,
        latest_stale_poll_age_hours,
        latest_stale_poll_candidate_count,
        latest_stale_poll_failed_count,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        message,
    }
}

pub(crate) fn build_codex_app_server_deployment_readiness(
    configured: bool,
    audit_logs: &[AuditLog],
    generated_at: DateTime<Utc>,
    controller_required: bool,
    controller_configured: bool,
) -> CodexAppServerDeploymentReadiness {
    let latest_validation = audit_logs
        .iter()
        .filter(|log| log.action == "codex_app_server.deployment_validation")
        .max_by_key(|log| log.created_at);
    let controller_validation_logs = audit_logs
        .iter()
        .filter(|log| {
            log.action == "codex_app_server.deployment_validation"
                && log
                    .details
                    .get("controller_execution")
                    .and_then(|execution| execution.get("attempted"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let controller_execution_count = controller_validation_logs.len();
    let controller_failed_count = controller_validation_logs
        .iter()
        .filter(|log| {
            log.details
                .get("controller_execution")
                .and_then(|execution| execution.get("status"))
                .and_then(Value::as_str)
                .is_some_and(|status| status != "validated")
        })
        .count();
    let latest_validation_at = latest_validation.map(|log| log.created_at);
    let latest_validation_age_hours =
        latest_validation_at.map(|created_at| (generated_at - created_at).num_hours());
    let latest_validation_status = latest_validation
        .and_then(|log| log.details.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_validation_healthy = latest_validation
        .and_then(|log| log.details.get("healthy"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let latest_controller_status = latest_validation
        .and_then(|log| log.details.get("controller_execution"))
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let latest_controller_age_hours = latest_validation
        .filter(|_| latest_controller_status.is_some())
        .map(|log| (generated_at - log.created_at).num_hours());
    let controller_evidence_fresh =
        latest_controller_age_hours.is_some_and(|age_hours| age_hours < 24);
    let latest_controller_validated = latest_controller_status.as_deref() == Some("validated");
    let mut blocking_reasons = Vec::new();

    if !configured {
        blocking_reasons.push("Codex App Server endpoint is not configured".to_string());
    }
    if latest_validation.is_none() {
        blocking_reasons.push("deployment validation has not run".to_string());
    }
    if latest_validation.is_some() && !latest_validation_healthy {
        blocking_reasons.push("latest deployment validation was not healthy".to_string());
    }
    if latest_validation_age_hours.is_some_and(|hours| hours >= 24) {
        blocking_reasons.push("deployment validation evidence is stale".to_string());
    }
    if controller_required && !controller_configured {
        blocking_reasons.push(
            "Codex App Server deployment controller is required but not configured".to_string(),
        );
    }
    if controller_required && controller_configured && !latest_controller_validated {
        blocking_reasons.push(
            "Codex App Server deployment controller evidence is missing or not validated"
                .to_string(),
        );
    }
    if controller_required && latest_controller_validated && !controller_evidence_fresh {
        blocking_reasons
            .push("Codex App Server deployment controller evidence is stale".to_string());
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
            "Codex App Server deployment validation is blocked: {}",
            blocking_reasons.join("; ")
        )
    } else {
        "Codex App Server deployment has a recent healthy validation run".to_string()
    };

    CodexAppServerDeploymentReadiness {
        status,
        production_blocked,
        configured,
        endpoint_configured: configured,
        deployment_validated: latest_validation_healthy && !production_blocked,
        latest_validation_at,
        latest_validation_age_hours,
        latest_validation_status,
        latest_validation_healthy,
        controller_required,
        controller_configured,
        latest_controller_status,
        latest_controller_age_hours,
        controller_evidence_fresh,
        latest_controller_validated,
        controller_execution_count,
        controller_failed_count,
        blocking_reasons,
        message,
    }
}

pub(crate) fn codex_app_server_deployment_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn codex_app_server_deployment_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn codex_app_server_ops_controller_required<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn codex_app_server_ops_controller_configured<F>(lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup("MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) async fn execute_codex_app_server_ops_controller<F>(
    lookup: &F,
    subject: Option<&str>,
    checked_at: DateTime<Utc>,
    summary: &CodexAppServerControlPlaneSummary,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL is required")
        })?;
    let timeout_seconds = lookup("MANDOFORGE_CODEX_APP_SERVER_OPS_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.codex_app_server_ops",
        "subject": subject,
        "checked_at": checked_at,
        "control_plane": {
            "configured": summary.configured,
            "status": summary.status,
            "endpoint_configured": summary.endpoint_configured,
            "timeout_seconds": summary.timeout_seconds,
            "run_count": summary.run_count,
            "turn_count": summary.turn_count,
            "active_turn_count": summary.active_turn_count,
            "failed_turn_count": summary.failed_turn_count,
            "stale_candidate_count": summary.stale_candidate_count,
            "pollable_turn_count": summary.pollable_turn_count,
            "production_ops": summary.production_ops,
            "deployment_readiness": summary.deployment_readiness,
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
    let http_status = response.status();
    let body = response.json::<Value>().await.unwrap_or_else(|_| json!({}));
    if !http_status.is_success() {
        return Err(AppError::bad_request(format!(
            "Codex App Server ops controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(controller_status, "validated" | "success" | "ok");
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "ops_id": body.get("ops_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "checks": body.get("checks").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) async fn execute_codex_app_server_deployment_controller<F>(
    lookup: &F,
    subject: &str,
    checked_at: DateTime<Utc>,
    config: &CodexAppServerConfig,
) -> Result<Value, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let endpoint = lookup("MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL is required",
            )
        })?;
    let timeout_seconds = lookup("MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_TIMEOUT_SECONDS")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(15);
    let token = lookup("MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_TOKEN")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let payload = json!({
        "type": "mandoforge.codex_app_server_deployment",
        "subject": subject,
        "checked_at": checked_at,
        "app_server": {
            "endpoint_configured": true,
            "timeout_seconds": config.timeout_seconds,
            "endpoint": config.endpoint.as_str(),
        }
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
            "Codex App Server deployment controller failed with status {http_status}"
        )));
    }
    let controller_status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("validated");
    let validated = matches!(
        controller_status,
        "validated" | "deployed" | "healthy" | "success" | "ok"
    );
    Ok(json!({
        "attempted": true,
        "status": if validated { "validated" } else { "failed" },
        "http_status": http_status.as_u16(),
        "provider_status": controller_status,
        "deployment_id": body.get("deployment_id").and_then(Value::as_str),
        "message": body.get("message").and_then(Value::as_str),
        "steps": body.get("steps").cloned().unwrap_or_else(|| json!([])),
    }))
}

pub(crate) fn build_codex_app_server_trace_detail(
    runs: &[CodexAppServerRun],
    trace_key: &str,
    events: &[SessionEvent],
    audit_logs: &[AuditLog],
) -> Result<CodexAppServerTraceDetail, AppError> {
    let mut matching = runs
        .iter()
        .filter(|run| codex_app_server_trace_key(run) == trace_key)
        .cloned()
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(AppError::not_found("Codex App Server trace not found"));
    }
    matching.sort_by_key(|run| run.created_at);
    let summary = build_codex_app_server_trace_summary(&matching);
    let trace = summary
        .traces
        .into_iter()
        .find(|trace| trace.trace_key == trace_key)
        .ok_or_else(|| AppError::not_found("Codex App Server trace not found"))?;
    let evidence = codex_trace_evidence(&matching, events, audit_logs);
    let artifact_lineage = codex_trace_artifact_lineage(&matching, audit_logs);
    let matching_refs = matching.iter().collect::<Vec<_>>();
    let dashboard = build_codex_trace_dashboard(
        &matching_refs,
        &evidence,
        artifact_lineage.len(),
        trace.terminal,
    );
    let status_timeline = matching
        .iter()
        .map(|run| CodexAppServerStatusPoint {
            run_id: run.id,
            operation: run.operation.clone(),
            status: run.status.clone(),
            terminal: codex_turn_status_is_terminal(&run.status),
            created_at: run.created_at,
            error: run.error.clone(),
        })
        .collect::<Vec<_>>();
    let mut by_status = HashMap::new();
    let mut by_operation = HashMap::new();
    let mut command_ids = BTreeSet::new();
    let mut terminal_count = 0usize;
    let mut non_terminal_count = 0usize;
    for run in &matching {
        increment_count(&mut by_status, run.status.as_str());
        increment_count(&mut by_operation, run.operation.as_str());
        if let Some(command_id) = run.command_id.as_ref() {
            command_ids.insert(command_id.clone());
        }
        if codex_turn_status_is_terminal(&run.status) {
            terminal_count += 1;
        } else {
            non_terminal_count += 1;
        }
    }
    let errors = matching
        .iter()
        .filter_map(|run| run.error.clone())
        .collect::<Vec<_>>();
    let latest_response = matching
        .last()
        .map(|run| run.response.clone())
        .unwrap_or_else(|| json!({}));
    Ok(CodexAppServerTraceDetail {
        generated_at: Utc::now(),
        trace,
        runs: matching,
        status_timeline,
        by_status,
        by_operation,
        terminal_count,
        non_terminal_count,
        command_ids: command_ids.into_iter().collect(),
        errors,
        dashboard,
        evidence,
        artifact_lineage,
        latest_response,
    })
}

pub(crate) fn codex_app_server_trace_key(run: &CodexAppServerRun) -> String {
    run.turn_id
        .clone()
        .or_else(|| run.command_id.clone())
        .unwrap_or_else(|| format!("run:{}", run.id))
}

pub(crate) fn codex_run_status_failed(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "failed" | "poll_failed" | "cancelled" | "canceled" | "interrupted"
    )
}

pub(crate) fn codex_app_server_config(state: &AppState) -> Result<&CodexAppServerConfig, AppError> {
    state
        .codex_app_server_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("Codex App Server is not configured"))
}
