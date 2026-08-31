use std::collections::HashSet;

use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

#[cfg(test)]
use crate::ontology_action_name_from_args;
use crate::store_backend::StoreBackend;
use crate::store_events::{POSTGRES_SESSION_EVENT_CHANNEL, session_event_notify_payload};
use crate::store_rows::{task_grant_from_row, tool_call_from_row};
use crate::store_workflows::{
    TASK_GRANT_COLUMNS, task_grant_runtime_denial, task_grant_tool_call_reservation_denial,
};
use crate::{
    AppError, AppState, Artifact, AuditLog, SessionEvent, SessionStatus, TaskGrant, ToolCall,
    new_audit_log, ontology_release_current_status,
};

fn invocation_event(
    id: Uuid,
    tool_call: &ToolCall,
    actor_type: &str,
    actor_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> SessionEvent {
    SessionEvent {
        id,
        session_id: tool_call.session_id,
        seq: 0,
        parent_event_id: None,
        actor_type: actor_type.to_string(),
        actor_id,
        event_type: event_type.to_string(),
        payload,
        created_at: Utc::now(),
    }
}

fn tool_invocation_records(
    tool_call: &ToolCall,
    task_grant: Option<&TaskGrant>,
    agent_version_id: Uuid,
    agent_version: i32,
) -> Result<(Vec<SessionEvent>, Option<AuditLog>), AppError> {
    let call_event_id = tool_call
        .event_id
        .ok_or_else(|| AppError::bad_request("tool call requires a call event id"))?;
    let mut events = Vec::with_capacity(if task_grant.is_some() { 3 } else { 2 });
    let audit = task_grant.map(|grant| {
        let details = json!({
            "task_grant_id": grant.id,
            "workflow_run_id": grant.workflow_run_id,
            "tool": tool_call.tool_name,
            "status": "allowed",
            "turns_used": grant.turns_used,
            "tool_calls_used": grant.tool_calls_used,
            "cost_usd_micros_used": grant.cost_usd_micros_used,
        });
        events.push(invocation_event(
            Uuid::new_v4(),
            tool_call,
            "system",
            Some(grant.id),
            "task_grant.checked",
            details.clone(),
        ));
        new_audit_log(
            Some(tool_call.session_id),
            "system",
            Some(grant.id),
            "task_grant.checked",
            "task_grant",
            Some(grant.id),
            details,
        )
    });
    events.push(invocation_event(
        call_event_id,
        tool_call,
        "tool",
        None,
        "tool.call",
        json!({
            "tool": tool_call.tool_name,
            "args": tool_call.args,
            "agent_version_id": agent_version_id,
            "agent_version": agent_version,
        }),
    ));
    events.push(invocation_event(
        Uuid::new_v4(),
        tool_call,
        "agent",
        Some(call_event_id),
        "agent.tool_use",
        json!({
            "event_id": call_event_id,
            "tool": tool_call.tool_name,
            "args": tool_call.args,
            "agent_version_id": agent_version_id,
            "agent_version": agent_version,
        }),
    ));
    Ok((events, audit))
}

fn tool_invocation_result_records(
    tool_call: &ToolCall,
    status: &str,
    result: &Value,
    origin: &str,
) -> (Vec<SessionEvent>, AuditLog) {
    let event_type = if status == "waiting_approval" {
        "policy.requires_approval"
    } else {
        "tool.result"
    };
    let events = vec![
        invocation_event(
            Uuid::new_v4(),
            tool_call,
            if status == "waiting_approval" {
                "system"
            } else {
                "tool"
            },
            Some(tool_call.id),
            event_type,
            json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "origin": origin,
                "content": result,
            }),
        ),
        invocation_event(
            Uuid::new_v4(),
            tool_call,
            "agent",
            Some(tool_call.id),
            "agent.tool_result",
            json!({
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "status": status,
                "content": result,
            }),
        ),
    ];
    let audit = new_audit_log(
        Some(tool_call.session_id),
        "tool",
        Some(tool_call.id),
        if status == "waiting_approval" {
            "tool.waiting_approval"
        } else {
            "tool.completed"
        },
        "tool_call",
        Some(tool_call.id),
        json!({
            "tool": tool_call.tool_name,
            "risk_level": tool_call.risk_level,
            "status": status,
        }),
    );
    (events, audit)
}

fn approved_ontology_action_proposal_records(
    job: &crate::execution_queue::ExecutionJob,
    approval_id: Uuid,
    tool_call: &ToolCall,
    artifact: &Artifact,
    proposal_details: Value,
    result: &Value,
) -> (Vec<SessionEvent>, Vec<AuditLog>) {
    let artifact_event_id =
        crate::deterministic_record_id(artifact.id, "ontology-action-event", &["artifact.created"]);
    let proposal_event_id =
        crate::deterministic_record_id(artifact.id, "ontology-action-event", &["proposal_created"]);
    let outcome_event_id = crate::execution::execution_attempt_event_id(job, "tool.result");
    let events = vec![
        SessionEvent {
            id: artifact_event_id,
            session_id: artifact.session_id,
            seq: 0,
            parent_event_id: None,
            actor_type: "system".to_string(),
            actor_id: Some(artifact.id),
            event_type: "artifact.created".to_string(),
            payload: json!({
                "artifact_id": artifact.id,
                "name": artifact.name,
                "artifact_type": artifact.artifact_type,
                "tool_call_id": tool_call.id,
            }),
            created_at: Utc::now(),
        },
        SessionEvent {
            id: proposal_event_id,
            session_id: artifact.session_id,
            seq: 0,
            parent_event_id: None,
            actor_type: "system".to_string(),
            actor_id: Some(artifact.id),
            event_type: "ontology_action.proposal_created".to_string(),
            payload: proposal_details.clone(),
            created_at: Utc::now(),
        },
        invocation_event(
            outcome_event_id,
            tool_call,
            "tool",
            Some(tool_call.id),
            "tool.result",
            json!({
                "execution_job_id": job.id,
                "tool_call_id": tool_call.id,
                "tool": tool_call.tool_name,
                "content": result,
                "execution_outcome_known": true,
                "attempt_count": job.attempt_count,
                "claim_generation": job.claim_generation,
            }),
        ),
    ];
    let mut artifact_audit = new_audit_log(
        Some(artifact.session_id),
        "tool",
        Some(tool_call.id),
        "artifact.created",
        "artifact",
        Some(artifact.id),
        json!({
            "name": artifact.name,
            "artifact_type": artifact.artifact_type,
            "source": "ontology_action_proposal",
        }),
    );
    artifact_audit.id =
        crate::deterministic_record_id(artifact_event_id, "audit", &["artifact.created"]);
    let mut proposal_audit = new_audit_log(
        Some(artifact.session_id),
        "tool",
        Some(tool_call.id),
        "ontology_action.proposal_created",
        "artifact",
        Some(artifact.id),
        proposal_details,
    );
    proposal_audit.id = crate::deterministic_record_id(
        proposal_event_id,
        "audit",
        &["ontology_action.proposal_created"],
    );
    let mut completed_audit = new_audit_log(
        Some(artifact.session_id),
        "worker",
        Some(tool_call.id),
        "tool.completed",
        "tool_call",
        Some(tool_call.id),
        json!({
            "tool": tool_call.tool_name,
            "approval_id": approval_id,
            "resumed_after_approval": true,
        }),
    );
    completed_audit.id =
        crate::deterministic_record_id(outcome_event_id, "audit", &["tool.completed"]);
    (
        events,
        vec![artifact_audit, proposal_audit, completed_audit],
    )
}

fn validate_approved_ontology_action_commit(
    job: &crate::execution_queue::ExecutionJob,
    approval_id: Uuid,
    tool_call: &ToolCall,
    artifact: &Artifact,
) -> Result<Uuid, AppError> {
    if job.approval_id != approval_id
        || job.tool_call_id != tool_call.id
        || job.session_id != tool_call.session_id
        || job.session_id != artifact.session_id
        || job.tool_name != "ontology.action.execute"
        || tool_call.tool_name != "ontology.action.execute"
    {
        return Err(AppError::bad_request(
            "ontology action proposal does not match the execution claim",
        ));
    }
    if tool_call.status != "waiting_approval" {
        return Err(AppError::bad_request(
            "only waiting approval ontology actions can commit a proposal",
        ));
    }
    artifact
        .content
        .get("ontology_release_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| AppError::bad_request("ontology action proposal release id is invalid"))
}

impl AppState {
    pub(crate) async fn commit_tool_invocation_start(
        &self,
        tool_call: ToolCall,
        agent_version_id: Uuid,
        agent_version: i32,
    ) -> Result<(ToolCall, Option<TaskGrant>), AppError> {
        let (task_grant, events) = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let session = store
                    .sessions
                    .get(&tool_call.session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(
                    session.status,
                    SessionStatus::Terminated | SessionStatus::Failed
                ) {
                    return Err(AppError::bad_request(
                        "terminal session cannot accept new tool calls",
                    ));
                }
                let task_grant = if let Some(grant_id) = tool_call.task_grant_id {
                    let now = Utc::now();
                    let mut lineage = Vec::new();
                    let mut seen = HashSet::new();
                    let mut current_id = Some(grant_id);
                    while let Some(id) = current_id {
                        if !seen.insert(id) {
                            return Err(AppError::bad_request("task grant parent cycle detected"));
                        }
                        let grant = store
                            .task_grants
                            .get(&id)
                            .cloned()
                            .ok_or_else(|| AppError::not_found("task grant not found"))?;
                        current_id = grant.parent_grant_id;
                        lineage.push(grant);
                    }
                    for grant in &lineage {
                        if let Some((message, expire)) =
                            task_grant_tool_call_reservation_denial(grant, now)
                        {
                            if expire
                                && let Some(stored) = store.task_grants.get_mut(&grant.id)
                                && stored.status == "active"
                            {
                                stored.status = "expired".to_string();
                                stored.updated_at = now;
                            }
                            return Err(AppError::forbidden(message));
                        }
                        grant.tool_calls_used.checked_add(1).ok_or_else(|| {
                            AppError::bad_request("task grant tool call counter overflow")
                        })?;
                    }
                    for grant in &lineage {
                        let stored = store
                            .task_grants
                            .get_mut(&grant.id)
                            .expect("validated task grant lineage");
                        stored.tool_calls_used += 1;
                        stored.updated_at = now;
                    }
                    Some(
                        store
                            .task_grants
                            .get(&grant_id)
                            .expect("reserved task grant")
                            .clone(),
                    )
                } else {
                    None
                };
                let (mut events, audit) = tool_invocation_records(
                    &tool_call,
                    task_grant.as_ref(),
                    agent_version_id,
                    agent_version,
                )?;
                let persisted_events = store.events.entry(tool_call.session_id).or_default();
                let next_seq = persisted_events.len() as i64 + 1;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    persisted_events.push(event.clone());
                }
                store.tool_calls.insert(tool_call.id, tool_call.clone());
                if let Some(audit) = audit {
                    store.audit_logs.insert(audit.id, audit);
                }
                (task_grant, events)
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .execute(&mut *tx)
                .await?;
                let session_status = sqlx::query_scalar::<_, String>(
                    "SELECT status
                     FROM sessions
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(session_status.as_str(), "terminated" | "failed") {
                    return Err(AppError::bad_request(
                        "terminal session cannot accept new tool calls",
                    ));
                }
                let task_grant = if let Some(grant_id) = tool_call.task_grant_id {
                    let now = Utc::now();
                    let select_sql = format!(
                        "SELECT {TASK_GRANT_COLUMNS}
                         FROM task_grants
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE"
                    );
                    let mut lineage = Vec::new();
                    let mut seen = HashSet::new();
                    let mut current_id = Some(grant_id);
                    while let Some(id) = current_id {
                        if !seen.insert(id) {
                            return Err(AppError::bad_request("task grant parent cycle detected"));
                        }
                        let row = sqlx::query(&select_sql)
                            .bind(tenant_id)
                            .bind(id)
                            .fetch_optional(&mut *tx)
                            .await?
                            .ok_or_else(|| AppError::not_found("task grant not found"))?;
                        let grant = task_grant_from_row(row)?;
                        current_id = grant.parent_grant_id;
                        lineage.push(grant);
                    }
                    for grant in &lineage {
                        if let Some((message, expire)) =
                            task_grant_tool_call_reservation_denial(grant, now)
                        {
                            if expire {
                                sqlx::query(
                                    "UPDATE task_grants
                                     SET status = 'expired', updated_at = $3
                                     WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
                                )
                                .bind(tenant_id)
                                .bind(grant.id)
                                .bind(now)
                                .execute(&mut *tx)
                                .await?;
                                tx.commit().await?;
                            }
                            return Err(AppError::forbidden(message));
                        }
                        grant.tool_calls_used.checked_add(1).ok_or_else(|| {
                            AppError::bad_request("task grant tool call counter overflow")
                        })?;
                    }
                    for grant in &lineage {
                        sqlx::query(
                            "UPDATE task_grants
                             SET tool_calls_used = tool_calls_used + 1, updated_at = $3
                             WHERE tenant_id = $1 AND id = $2",
                        )
                        .bind(tenant_id)
                        .bind(grant.id)
                        .bind(now)
                        .execute(&mut *tx)
                        .await?;
                    }
                    let mut grant = lineage
                        .into_iter()
                        .next()
                        .expect("requested task grant is in lineage");
                    grant.tool_calls_used += 1;
                    grant.updated_at = now;
                    Some(grant)
                } else {
                    None
                };
                let (mut events, audit) = tool_invocation_records(
                    &tool_call,
                    task_grant.as_ref(),
                    agent_version_id,
                    agent_version,
                )?;
                let next_seq = sqlx::query_scalar::<_, i64>(
                    "SELECT COALESCE(MAX(seq), 0) + 1
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .fetch_one(&mut *tx)
                .await?;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    sqlx::query(
                        "INSERT INTO session_events
                            (id, tenant_id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(event.id)
                    .bind(tenant_id)
                    .bind(event.session_id)
                    .bind(event.seq)
                    .bind(event.parent_event_id)
                    .bind(&event.actor_type)
                    .bind(event.actor_id)
                    .bind(&event.event_type)
                    .bind(&event.payload)
                    .bind(event.created_at)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query("SELECT pg_notify($1, $2)")
                        .bind(POSTGRES_SESSION_EVENT_CHANNEL)
                        .bind(session_event_notify_payload(tenant_id, event))
                        .execute(&mut *tx)
                        .await?;
                }
                sqlx::query(
                    "INSERT INTO tool_calls
                        (id, tenant_id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, result, status, risk_level, policy_decision, started_at, completed_at, error, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
                )
                .bind(tool_call.id)
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .bind(tool_call.event_id)
                .bind(&tool_call.tool_name)
                .bind(&tool_call.args)
                .bind(tool_call.task_grant_id)
                .bind(&tool_call.normalized_args_hash)
                .bind(&tool_call.target_binding)
                .bind(&tool_call.result)
                .bind(&tool_call.status)
                .bind(&tool_call.risk_level)
                .bind(&tool_call.policy_decision)
                .bind(tool_call.started_at)
                .bind(tool_call.completed_at)
                .bind(&tool_call.error)
                .bind(tool_call.created_at)
                .execute(&mut *tx)
                .await?;
                if let Some(audit) = audit {
                    sqlx::query(
                        "INSERT INTO audit_logs
                            (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(audit.id)
                    .bind(tenant_id)
                    .bind(audit.session_id)
                    .bind(&audit.actor_type)
                    .bind(audit.actor_id)
                    .bind(&audit.action)
                    .bind(&audit.resource_type)
                    .bind(audit.resource_id)
                    .bind(&audit.details)
                    .bind(audit.created_at)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                (task_grant, events)
            }
        };
        self.emit_committed_session_events(&events).await;
        Ok((tool_call, task_grant))
    }

    pub(crate) async fn commit_tool_invocation_result(
        &self,
        id: Uuid,
        status: &str,
        result: Value,
        origin: &str,
    ) -> Result<SessionEvent, AppError> {
        if !matches!(status, "completed" | "waiting_approval") {
            return Err(AppError::bad_request(
                "tool invocation result status must be completed or waiting_approval",
            ));
        }
        let (tool_call, events) = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut tool_call = store
                    .tool_calls
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                if tool_call.status != "running" {
                    return Err(AppError::bad_request(
                        "only running tool calls can commit an invocation result",
                    ));
                }
                tool_call.status = status.to_string();
                tool_call.result = Some(result.clone());
                tool_call.error = None;
                tool_call.completed_at = Some(Utc::now());
                let (mut events, audit) =
                    tool_invocation_result_records(&tool_call, status, &result, origin);
                let persisted_events = store.events.entry(tool_call.session_id).or_default();
                let next_seq = persisted_events.len() as i64 + 1;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    persisted_events.push(event.clone());
                }
                store.tool_calls.insert(tool_call.id, tool_call.clone());
                store.audit_logs.insert(audit.id, audit);
                (tool_call, events)
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool.begin().await?;
                let current = sqlx::query(
                    "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                     FROM tool_calls
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                let current = tool_call_from_row(current)?;
                if current.status != "running" {
                    return Err(AppError::bad_request(
                        "only running tool calls can commit an invocation result",
                    ));
                }
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET status = $1, result = $2, error = NULL, completed_at = now()
                     WHERE tenant_id = $3 AND id = $4 AND status = 'running'
                     RETURNING id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(status)
                .bind(&result)
                .bind(tenant_id)
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
                let tool_call = tool_call_from_row(row)?;
                let (mut events, audit) =
                    tool_invocation_result_records(&tool_call, status, &result, origin);
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .execute(&mut *tx)
                .await?;
                let next_seq = sqlx::query_scalar::<_, i64>(
                    "SELECT COALESCE(MAX(seq), 0) + 1
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .fetch_one(&mut *tx)
                .await?;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    sqlx::query(
                        "INSERT INTO session_events
                            (id, tenant_id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(event.id)
                    .bind(tenant_id)
                    .bind(event.session_id)
                    .bind(event.seq)
                    .bind(event.parent_event_id)
                    .bind(&event.actor_type)
                    .bind(event.actor_id)
                    .bind(&event.event_type)
                    .bind(&event.payload)
                    .bind(event.created_at)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query("SELECT pg_notify($1, $2)")
                        .bind(POSTGRES_SESSION_EVENT_CHANNEL)
                        .bind(session_event_notify_payload(tenant_id, event))
                        .execute(&mut *tx)
                        .await?;
                }
                sqlx::query(
                    "INSERT INTO audit_logs
                        (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(audit.id)
                .bind(tenant_id)
                .bind(audit.session_id)
                .bind(&audit.actor_type)
                .bind(audit.actor_id)
                .bind(&audit.action)
                .bind(&audit.resource_type)
                .bind(audit.resource_id)
                .bind(&audit.details)
                .bind(audit.created_at)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                (tool_call, events)
            }
        };
        self.emit_committed_session_events(&events).await;
        let result_event = events
            .into_iter()
            .next()
            .expect("tool invocation result always has a primary event");
        debug_assert_eq!(tool_call.status, status);
        Ok(result_event)
    }

    pub(crate) async fn commit_approved_ontology_action_proposal(
        &self,
        job: &crate::execution_queue::ExecutionJob,
        approval_id: Uuid,
        artifact: Artifact,
        proposal_details: Value,
        result: Value,
    ) -> Result<(), AppError> {
        if job.status != crate::ExecutionJobStatus::Executing {
            return Err(AppError::bad_request(
                "ontology action proposal requires an executing claim",
            )
            .with_known_execution_outcome());
        }
        let worker_id = job.worker_id.as_deref().ok_or_else(|| {
            AppError::bad_request("ontology action proposal claim has no worker")
                .with_known_execution_outcome()
        })?;
        let events = match &self.store {
            StoreBackend::Memory(inner) => {
                let _claim_guard = self
                    .execution_queue
                    .lock_owned_claim(
                        job.id,
                        worker_id,
                        job.claim_generation,
                        crate::ExecutionJobStatus::Executing,
                    )
                    .await
                    .map_err(AppError::with_retry_safe_execution)?;
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get(&job.tool_call_id)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::not_found("tool call not found").with_known_execution_outcome()
                    })?;
                let release_id = validate_approved_ontology_action_commit(
                    job,
                    approval_id,
                    &tool_call,
                    &artifact,
                )
                .map_err(AppError::with_known_execution_outcome)?;
                let release = store.ontology_releases.get(&release_id).ok_or_else(|| {
                    AppError::not_found("ontology release not found").with_known_execution_outcome()
                })?;
                if !ontology_release_current_status(&release.status) {
                    return Err(AppError::forbidden(
                        "pinned ontology release has been revoked for new action proposals",
                    )
                    .with_known_execution_outcome());
                }
                let grant_id = tool_call.task_grant_id.ok_or_else(|| {
                    AppError::forbidden("ontology action execution requires a workflow TaskGrant")
                        .with_known_execution_outcome()
                })?;
                let mut seen = HashSet::new();
                let mut current_id = Some(grant_id);
                while let Some(id) = current_id {
                    if !seen.insert(id) {
                        return Err(AppError::bad_request("task grant parent cycle detected")
                            .with_known_execution_outcome());
                    }
                    let grant = store.task_grants.get(&id).ok_or_else(|| {
                        AppError::not_found("task grant not found").with_known_execution_outcome()
                    })?;
                    if let Some((message, _)) = task_grant_runtime_denial(grant, Utc::now()) {
                        return Err(AppError::forbidden(message).with_known_execution_outcome());
                    }
                    current_id = grant.parent_grant_id;
                }
                if !store.sessions.contains_key(&artifact.session_id) {
                    return Err(
                        AppError::not_found("session not found").with_known_execution_outcome()
                    );
                }
                if store.artifacts.contains_key(&artifact.id) {
                    return Err(AppError::conflict("artifact already exists")
                        .with_known_execution_outcome());
                }
                let (mut events, audits) = approved_ontology_action_proposal_records(
                    job,
                    approval_id,
                    &tool_call,
                    &artifact,
                    proposal_details,
                    &result,
                );
                if events.iter().any(|candidate| {
                    store
                        .events
                        .values()
                        .flatten()
                        .any(|existing| existing.id == candidate.id)
                }) || audits
                    .iter()
                    .any(|audit| store.audit_logs.contains_key(&audit.id))
                {
                    return Err(
                        AppError::conflict("ontology proposal records already exist")
                            .with_known_execution_outcome(),
                    );
                }
                let persisted_events = store.events.entry(tool_call.session_id).or_default();
                let next_seq = persisted_events.len() as i64 + 1;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    persisted_events.push(event.clone());
                }
                store.artifacts.insert(artifact.id, artifact);
                let completed = store
                    .tool_calls
                    .get_mut(&tool_call.id)
                    .expect("validated tool call remains present");
                completed.status = "completed".to_string();
                completed.result = Some(result);
                completed.error = None;
                completed.completed_at = Some(Utc::now());
                for audit in audits {
                    store.audit_logs.insert(audit.id, audit);
                }
                events
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(AppError::from)
                    .map_err(AppError::with_retry_safe_execution)?;
                let precommit = async {
                    sqlx::query(
                        "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                    )
                    .bind(tenant_id)
                    .bind(job.session_id)
                    .execute(&mut *tx)
                    .await?;
                    let owned = sqlx::query_scalar::<_, i32>(
                        "SELECT 1
                         FROM execution_jobs
                         WHERE tenant_id = $1
                           AND id = $2
                           AND status = 'executing'
                           AND worker_id = $3
                           AND claim_generation = $4
                           AND lease_expires_at > now()
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(job.id)
                    .bind(worker_id)
                    .bind(job.claim_generation)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if owned.is_none() {
                        return Err(AppError::not_found(
                            "execution job claim is no longer owned",
                        ));
                    }
                    let row = sqlx::query(
                        "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                         FROM tool_calls
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(job.tool_call_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                    let tool_call = tool_call_from_row(row)?;
                    let release_id = validate_approved_ontology_action_commit(
                        job,
                        approval_id,
                        &tool_call,
                        &artifact,
                    )
                    .map_err(AppError::with_known_execution_outcome)?;
                    let release_status = sqlx::query_scalar::<_, String>(
                        "SELECT status
                         FROM ontology_releases
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(release_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| {
                        AppError::not_found("ontology release not found")
                            .with_known_execution_outcome()
                    })?;
                    if !ontology_release_current_status(&release_status) {
                        return Err(AppError::forbidden(
                            "pinned ontology release has been revoked for new action proposals",
                        )
                        .with_known_execution_outcome());
                    }
                    let grant_id = tool_call.task_grant_id.ok_or_else(|| {
                        AppError::forbidden(
                            "ontology action execution requires a workflow TaskGrant",
                        )
                        .with_known_execution_outcome()
                    })?;
                    let select_grant_sql = format!(
                        "SELECT {TASK_GRANT_COLUMNS}
                         FROM task_grants
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE"
                    );
                    let mut seen = HashSet::new();
                    let mut current_id = Some(grant_id);
                    while let Some(id) = current_id {
                        if !seen.insert(id) {
                            return Err(AppError::bad_request(
                                "task grant parent cycle detected",
                            )
                            .with_known_execution_outcome());
                        }
                        let row = sqlx::query(&select_grant_sql)
                            .bind(tenant_id)
                            .bind(id)
                            .fetch_optional(&mut *tx)
                            .await?
                            .ok_or_else(|| {
                                AppError::not_found("task grant not found")
                                    .with_known_execution_outcome()
                            })?;
                        let grant = task_grant_from_row(row)?;
                        if let Some((message, _)) = task_grant_runtime_denial(&grant, Utc::now()) {
                            return Err(
                                AppError::forbidden(message).with_known_execution_outcome()
                            );
                        }
                        current_id = grant.parent_grant_id;
                    }
                    let (mut events, audits) = approved_ontology_action_proposal_records(
                        job,
                        approval_id,
                        &tool_call,
                        &artifact,
                        proposal_details,
                        &result,
                    );
                    sqlx::query(
                        "INSERT INTO artifacts
                            (id, tenant_id, session_id, artifact_type, name, path, content, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    )
                    .bind(artifact.id)
                    .bind(tenant_id)
                    .bind(artifact.session_id)
                    .bind(&artifact.artifact_type)
                    .bind(&artifact.name)
                    .bind(&artifact.path)
                    .bind(&artifact.content)
                    .bind(artifact.created_at)
                    .execute(&mut *tx)
                    .await?;
                    let next_seq = sqlx::query_scalar::<_, i64>(
                        "SELECT COALESCE(MAX(seq), 0) + 1
                         FROM session_events
                         WHERE tenant_id = $1 AND session_id = $2",
                    )
                    .bind(tenant_id)
                    .bind(tool_call.session_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    for (offset, event) in events.iter_mut().enumerate() {
                        event.seq = next_seq + offset as i64;
                        sqlx::query(
                            "INSERT INTO session_events
                                (id, tenant_id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                        )
                        .bind(event.id)
                        .bind(tenant_id)
                        .bind(event.session_id)
                        .bind(event.seq)
                        .bind(event.parent_event_id)
                        .bind(&event.actor_type)
                        .bind(event.actor_id)
                        .bind(&event.event_type)
                        .bind(&event.payload)
                        .bind(event.created_at)
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query("SELECT pg_notify($1, $2)")
                            .bind(POSTGRES_SESSION_EVENT_CHANNEL)
                            .bind(session_event_notify_payload(tenant_id, event))
                            .execute(&mut *tx)
                            .await?;
                    }
                    sqlx::query(
                        "UPDATE tool_calls
                         SET status = 'completed', result = $1, error = NULL, completed_at = now()
                         WHERE tenant_id = $2 AND id = $3 AND status = 'waiting_approval'",
                    )
                    .bind(&result)
                    .bind(tenant_id)
                    .bind(tool_call.id)
                    .execute(&mut *tx)
                    .await?;
                    for audit in audits {
                        sqlx::query(
                            "INSERT INTO audit_logs
                                (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                        )
                        .bind(audit.id)
                        .bind(tenant_id)
                        .bind(audit.session_id)
                        .bind(&audit.actor_type)
                        .bind(audit.actor_id)
                        .bind(&audit.action)
                        .bind(&audit.resource_type)
                        .bind(audit.resource_id)
                        .bind(&audit.details)
                        .bind(audit.created_at)
                        .execute(&mut *tx)
                        .await?;
                    }
                    Ok::<_, AppError>(events)
                }
                .await;
                let events = precommit.map_err(|error| {
                    if error.execution_outcome_known {
                        error
                    } else {
                        error.with_retry_safe_execution()
                    }
                })?;
                tx.commit().await?;
                events
            }
        };
        self.emit_committed_session_events(&events).await;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_tool_call(&self, tool_call: ToolCall) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let session = store
                    .sessions
                    .get(&tool_call.session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(
                    session.status,
                    SessionStatus::Terminated | SessionStatus::Failed
                ) {
                    return Err(AppError::bad_request(
                        "terminal session cannot accept new tool calls",
                    ));
                }
                store.tool_calls.insert(tool_call.id, tool_call.clone());
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let session_status = sqlx::query_scalar::<_, String>(
                    "SELECT status
                     FROM sessions
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(tool_call.session_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(session_status.as_str(), "terminated" | "failed") {
                    return Err(AppError::bad_request(
                        "terminal session cannot accept new tool calls",
                    ));
                }
                sqlx::query(
                    "INSERT INTO tool_calls
                        (id, tenant_id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, result, status, risk_level, policy_decision, started_at, completed_at, error, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
                )
                .bind(tool_call.id)
                .bind(self.current_tenant_id())
                .bind(tool_call.session_id)
                .bind(tool_call.event_id)
                .bind(&tool_call.tool_name)
                .bind(&tool_call.args)
                .bind(tool_call.task_grant_id)
                .bind(&tool_call.normalized_args_hash)
                .bind(&tool_call.target_binding)
                .bind(&tool_call.result)
                .bind(&tool_call.status)
                .bind(&tool_call.risk_level)
                .bind(&tool_call.policy_decision)
                .bind(tool_call.started_at)
                .bind(tool_call.completed_at)
                .bind(&tool_call.error)
                .bind(tool_call.created_at)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
        }
        Ok(tool_call)
    }

    pub(crate) async fn update_tool_call_status(
        &self,
        id: Uuid,
        status: &str,
        result: Option<Value>,
        error: Option<Value>,
    ) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call.status = status.to_string();
                tool_call.completed_at = Some(Utc::now());
                tool_call.result = result;
                tool_call.error = error;
                Ok(tool_call.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET status = $1, result = $2, error = $3, completed_at = now()
                     WHERE tenant_id = $4 AND id = $5
                     RETURNING id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(status)
                .bind(result)
                .bind(error)
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn update_tool_call_args(
        &self,
        id: Uuid,
        args: Value,
    ) -> Result<ToolCall, AppError> {
        let current = self.get_tool_call(id).await?;
        if current.tool_name == "ontology.action.execute"
            && ontology_action_name_from_args(&current.args)?
                != ontology_action_name_from_args(&args)?
        {
            return Err(AppError::forbidden(
                "ontology action identity cannot be changed after approval is requested",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                if tool_call.status != "waiting_approval" {
                    return Err(AppError::bad_request(
                        "only waiting approval tool calls can be modified",
                    ));
                }
                tool_call.args = args;
                Ok(tool_call.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET args = $1
                     WHERE tenant_id = $2 AND id = $3 AND status = 'waiting_approval'
                     RETURNING id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(args)
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("waiting approval tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn get_tool_call(&self, id: Uuid) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .tool_calls
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("tool call not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                     FROM tool_calls
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn list_tool_calls(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<ToolCall>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut calls: Vec<_> = inner
                    .read()
                    .await
                    .tool_calls
                    .values()
                    .filter(|call| session_id.is_none_or(|id| call.session_id == id))
                    .cloned()
                    .collect();
                calls.sort_by_key(|call| call.created_at);
                calls.reverse();
                Ok(calls)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.current_tenant_id())
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, task_grant_id, normalized_args_hash, target_binding, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(tool_call_from_row).collect()
            }
        }
    }
}
