use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_events::{POSTGRES_SESSION_EVENT_CHANNEL, session_event_notify_payload};
use crate::store_rows::{session_from_row, session_thread_from_row};
use crate::{
    AppError, AppState, AuditLog, Session, SessionEvent, SessionStatus, SessionThread, ToolCall,
    managed_session_status_event, new_audit_log, session_thread_event_payload,
};

fn provider_completion_event(
    id: Uuid,
    session_id: Uuid,
    actor_type: &str,
    actor_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> SessionEvent {
    SessionEvent {
        id,
        session_id,
        seq: 0,
        parent_event_id: None,
        actor_type: actor_type.to_string(),
        actor_id,
        event_type: event_type.to_string(),
        payload,
        created_at: Utc::now(),
    }
}

fn provider_completion_records(
    tool_call: &ToolCall,
    session: &Session,
    updated_thread: Option<&SessionThread>,
    completion_status: &str,
    goal_event_type: &str,
    summary: &str,
) -> (Vec<SessionEvent>, AuditLog) {
    let call_event_id = tool_call
        .event_id
        .expect("validated complete_task event id");
    let args = tool_call.args.clone();
    let mut events = vec![
        provider_completion_event(
            call_event_id,
            session.id,
            "tool",
            None,
            "tool.call",
            json!({"tool": "complete_task", "args": args}),
        ),
        provider_completion_event(
            Uuid::new_v4(),
            session.id,
            "agent",
            Some(call_event_id),
            "agent.tool_use",
            json!({"event_id": call_event_id, "tool_call_id": tool_call.id, "tool": "complete_task", "args": args}),
        ),
        provider_completion_event(
            Uuid::new_v4(),
            session.id,
            "tool",
            Some(tool_call.id),
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": "complete_task", "origin": "session_loop", "content": args}),
        ),
        provider_completion_event(
            Uuid::new_v4(),
            session.id,
            "agent",
            None,
            goal_event_type,
            json!({"objective": summary, "summary": summary, "reason": summary}),
        ),
    ];
    if let Some(thread) = updated_thread {
        events.push(provider_completion_event(
            Uuid::new_v4(),
            session.id,
            "system",
            Some(thread.id),
            "thread.status_changed",
            session_thread_event_payload(thread),
        ));
    }
    events.push(provider_completion_event(
        Uuid::new_v4(),
        session.id,
        "system",
        None,
        managed_session_status_event(&session.status),
        json!({
            "status": session.status,
            "reason": summary,
            "environment_id": session.environment_id
        }),
    ));
    let audit = new_audit_log(
        Some(session.id),
        "agent",
        None,
        goal_event_type,
        "session",
        Some(session.id),
        json!({"status": completion_status, "summary": summary}),
    );
    (events, audit)
}

impl AppState {
    pub(crate) async fn set_session_and_primary_thread_status(
        &self,
        session_id: Uuid,
        thread_id: Uuid,
        session_status: SessionStatus,
        thread_status: &str,
    ) -> Result<(Session, Option<SessionThread>), AppError> {
        let updated_at = Utc::now();
        let next_is_terminal = matches!(
            session_status,
            SessionStatus::Terminated | SessionStatus::Failed
        );
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let session = store
                    .sessions
                    .get(&session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(
                    session.status,
                    SessionStatus::Terminated | SessionStatus::Failed
                ) && !next_is_terminal
                {
                    return Err(AppError::bad_request(
                        "session is terminal and cannot run session loop work",
                    ));
                }
                let session = store
                    .sessions
                    .get_mut(&session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                session.status = session_status;
                session.updated_at = updated_at;
                let session = session.clone();
                let thread = store
                    .session_threads
                    .get_mut(&thread_id)
                    .filter(|thread| thread.session_id == session_id)
                    .ok_or_else(|| AppError::not_found("session thread not found"))?;
                let updated_thread = if thread.status == thread_status {
                    None
                } else {
                    thread.status = thread_status.to_string();
                    thread.updated_at = updated_at;
                    Some(thread.clone())
                };
                Ok((session, updated_thread))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let current_status = sqlx::query_scalar::<_, String>(
                    "SELECT status
                     FROM sessions
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                if matches!(current_status.as_str(), "terminated" | "failed") && !next_is_terminal {
                    return Err(AppError::bad_request(
                        "session is terminal and cannot run session loop work",
                    ));
                }
                let session_row = sqlx::query(
                    "UPDATE sessions
                     SET status = $1, updated_at = $2
                     WHERE tenant_id = $3 AND id = $4
                     RETURNING id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at",
                )
                .bind(session_status.as_str())
                .bind(updated_at)
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_one(&mut *tx)
                .await?;
                let thread_row = sqlx::query(
                    "UPDATE session_threads
                     SET status = $1, updated_at = $2
                     WHERE tenant_id = $3 AND id = $4 AND session_id = $5 AND status IS DISTINCT FROM $1
                     RETURNING id, session_id, parent_thread_id, thread_kind, agent_id,
                               agent_version_id, environment_id, source_handoff_id,
                               specialist_session_id, status, title, context, created_at, updated_at",
                )
                .bind(thread_status)
                .bind(updated_at)
                .bind(self.current_tenant_id())
                .bind(thread_id)
                .bind(session_id)
                .fetch_optional(&mut *tx)
                .await?;
                let thread_exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(
                         SELECT 1 FROM session_threads
                         WHERE tenant_id = $1 AND id = $2 AND session_id = $3
                     )",
                )
                .bind(self.current_tenant_id())
                .bind(thread_id)
                .bind(session_id)
                .fetch_one(&mut *tx)
                .await?;
                if !thread_exists {
                    return Err(AppError::not_found("session thread not found"));
                }
                let session = session_from_row(session_row)?;
                let updated_thread = thread_row.map(session_thread_from_row).transpose()?;
                tx.commit().await?;
                Ok((session, updated_thread))
            }
        }
    }

    pub(crate) async fn commit_provider_completion(
        &self,
        tool_call: ToolCall,
        thread_id: Uuid,
        session_status: SessionStatus,
        thread_status: &str,
        completion_status: &str,
        summary: &str,
    ) -> Result<(ToolCall, Session, Vec<SessionEvent>), AppError> {
        if tool_call.event_id.is_none() {
            return Err(AppError::bad_request(
                "complete_task requires a call event id",
            ));
        }
        let goal_event_type = match completion_status {
            "completed" => "session.goal.completed",
            "blocked" => "session.goal.blocked",
            _ => {
                return Err(AppError::bad_request(
                    "complete_task status must be completed or blocked",
                ));
            }
        };
        let updated_at = Utc::now();
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
                        "session is terminal and cannot complete again",
                    ));
                }
                if store.approvals.values().any(|approval| {
                    approval.session_id == tool_call.session_id && approval.status == "pending"
                }) || store.tool_calls.values().any(|call| {
                    call.session_id == tool_call.session_id
                        && matches!(call.status.as_str(), "running" | "waiting_approval")
                }) {
                    return Err(AppError::conflict(
                        "complete_task cannot finish while durable actions are unresolved",
                    ));
                }
                if !store
                    .session_threads
                    .get(&thread_id)
                    .is_some_and(|thread| thread.session_id == tool_call.session_id)
                {
                    return Err(AppError::not_found("session thread not found"));
                }
                let session = store
                    .sessions
                    .get_mut(&tool_call.session_id)
                    .expect("validated session");
                session.status = session_status;
                session.updated_at = updated_at;
                let session = session.clone();
                let thread = store
                    .session_threads
                    .get_mut(&thread_id)
                    .expect("validated session thread");
                let updated_thread = if thread.status == thread_status {
                    None
                } else {
                    thread.status = thread_status.to_string();
                    thread.updated_at = updated_at;
                    Some(thread.clone())
                };
                let (mut events, audit) = provider_completion_records(
                    &tool_call,
                    &session,
                    updated_thread.as_ref(),
                    completion_status,
                    goal_event_type,
                    summary,
                );
                let persisted_events = store.events.entry(tool_call.session_id).or_default();
                let next_seq = persisted_events.len() as i64 + 1;
                for (offset, event) in events.iter_mut().enumerate() {
                    event.seq = next_seq + offset as i64;
                    persisted_events.push(event.clone());
                }
                store.tool_calls.insert(tool_call.id, tool_call.clone());
                store.audit_logs.insert(audit.id, audit);
                Ok((tool_call, session, events))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let tenant_id = self.current_tenant_id();
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .execute(&mut *tx)
                .await?;
                let current_status = sqlx::query_scalar::<_, String>(
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
                if matches!(current_status.as_str(), "terminated" | "failed") {
                    return Err(AppError::bad_request(
                        "session is terminal and cannot complete again",
                    ));
                }
                let unresolved = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(
                         SELECT 1 FROM approvals
                         WHERE tenant_id = $1 AND session_id = $2 AND status = 'pending'
                     ) OR EXISTS(
                         SELECT 1 FROM tool_calls
                         WHERE tenant_id = $1 AND session_id = $2
                           AND status IN ('running', 'waiting_approval')
                     )",
                )
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .fetch_one(&mut *tx)
                .await?;
                if unresolved {
                    return Err(AppError::conflict(
                        "complete_task cannot finish while durable actions are unresolved",
                    ));
                }
                let session_row = sqlx::query(
                    "UPDATE sessions
                     SET status = $1, updated_at = $2
                     WHERE tenant_id = $3 AND id = $4
                     RETURNING id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at",
                )
                .bind(session_status.as_str())
                .bind(updated_at)
                .bind(tenant_id)
                .bind(tool_call.session_id)
                .fetch_one(&mut *tx)
                .await?;
                let thread_row = sqlx::query(
                    "UPDATE session_threads
                     SET status = $1, updated_at = $2
                     WHERE tenant_id = $3 AND id = $4 AND session_id = $5 AND status IS DISTINCT FROM $1
                     RETURNING id, session_id, parent_thread_id, thread_kind, agent_id,
                               agent_version_id, environment_id, source_handoff_id,
                               specialist_session_id, status, title, context, created_at, updated_at",
                )
                .bind(thread_status)
                .bind(updated_at)
                .bind(tenant_id)
                .bind(thread_id)
                .bind(tool_call.session_id)
                .fetch_optional(&mut *tx)
                .await?;
                let thread_exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(
                         SELECT 1 FROM session_threads
                         WHERE tenant_id = $1 AND id = $2 AND session_id = $3
                     )",
                )
                .bind(tenant_id)
                .bind(thread_id)
                .bind(tool_call.session_id)
                .fetch_one(&mut *tx)
                .await?;
                if !thread_exists {
                    return Err(AppError::not_found("session thread not found"));
                }
                let session = session_from_row(session_row)?;
                let updated_thread = thread_row.map(session_thread_from_row).transpose()?;
                let (mut events, audit) = provider_completion_records(
                    &tool_call,
                    &session,
                    updated_thread.as_ref(),
                    completion_status,
                    goal_event_type,
                    summary,
                );
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
                Ok((tool_call, session, events))
            }
        }
    }

    pub(crate) async fn list_session_threads(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<SessionThread>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut threads: Vec<_> = inner
                    .read()
                    .await
                    .session_threads
                    .values()
                    .filter(|thread| {
                        session_id.is_none_or(|session_id| {
                            thread.session_id == session_id
                                || thread.specialist_session_id == Some(session_id)
                        })
                    })
                    .cloned()
                    .collect();
                threads.sort_by_key(|thread| thread.created_at);
                Ok(threads)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => sqlx::query(
                        "SELECT id, session_id, parent_thread_id, thread_kind, agent_id,
                                agent_version_id, environment_id, source_handoff_id,
                                specialist_session_id, status, title, context, created_at, updated_at
                         FROM session_threads
                         WHERE tenant_id = $1
                           AND (session_id = $2 OR specialist_session_id = $2)
                         ORDER BY created_at ASC",
                    )
                    .bind(self.current_tenant_id())
                    .bind(session_id)
                    .fetch_all(pool)
                    .await?,
                    None => sqlx::query(
                        "SELECT id, session_id, parent_thread_id, thread_kind, agent_id,
                                agent_version_id, environment_id, source_handoff_id,
                                specialist_session_id, status, title, context, created_at, updated_at
                         FROM session_threads
                         WHERE tenant_id = $1
                         ORDER BY created_at ASC",
                    )
                    .bind(self.current_tenant_id())
                    .fetch_all(pool)
                    .await?,
                };
                rows.into_iter().map(session_thread_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_session_thread(&self, id: Uuid) -> Result<SessionThread, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .session_threads
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("session thread not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, parent_thread_id, thread_kind, agent_id,
                            agent_version_id, environment_id, source_handoff_id,
                            specialist_session_id, status, title, context, created_at, updated_at
                     FROM session_threads
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session thread not found"))?;
                session_thread_from_row(row)
            }
        }
    }

    pub(crate) async fn primary_session_thread(
        &self,
        session_id: Uuid,
    ) -> Result<Option<SessionThread>, AppError> {
        Ok(self
            .list_session_threads(Some(session_id))
            .await?
            .into_iter()
            .find(|thread| thread.thread_kind == "primary"))
    }

    pub(crate) async fn session_thread_for_handoff(
        &self,
        source_handoff_id: Uuid,
    ) -> Result<Option<SessionThread>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .session_threads
                .values()
                .find(|thread| thread.source_handoff_id == Some(source_handoff_id))
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, parent_thread_id, thread_kind, agent_id,
                            agent_version_id, environment_id, source_handoff_id,
                            specialist_session_id, status, title, context, created_at, updated_at
                     FROM session_threads
                     WHERE tenant_id = $1 AND source_handoff_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(source_handoff_id)
                .fetch_optional(pool)
                .await?;
                row.map(session_thread_from_row).transpose()
            }
        }
    }

    pub(crate) async fn create_session_thread(
        &self,
        thread: SessionThread,
    ) -> Result<SessionThread, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if thread.thread_kind == "primary"
                    && store.session_threads.values().any(|existing| {
                        existing.session_id == thread.session_id
                            && existing.thread_kind == "primary"
                    })
                {
                    return Err(AppError::bad_request(
                        "session already has a primary thread",
                    ));
                }
                if thread.source_handoff_id.is_some()
                    && store
                        .session_threads
                        .values()
                        .any(|existing| existing.source_handoff_id == thread.source_handoff_id)
                {
                    return Err(AppError::bad_request(
                        "agent handoff already has a session thread",
                    ));
                }
                store.session_threads.insert(thread.id, thread.clone());
                Ok(thread)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO session_threads
                        (id, tenant_id, session_id, parent_thread_id, thread_kind, agent_id,
                         agent_version_id, environment_id, source_handoff_id,
                         specialist_session_id, status, title, context, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                     RETURNING id, session_id, parent_thread_id, thread_kind, agent_id,
                               agent_version_id, environment_id, source_handoff_id,
                               specialist_session_id, status, title, context, created_at, updated_at",
                )
                .bind(thread.id)
                .bind(self.current_tenant_id())
                .bind(thread.session_id)
                .bind(thread.parent_thread_id)
                .bind(&thread.thread_kind)
                .bind(thread.agent_id)
                .bind(thread.agent_version_id)
                .bind(thread.environment_id)
                .bind(thread.source_handoff_id)
                .bind(thread.specialist_session_id)
                .bind(&thread.status)
                .bind(&thread.title)
                .bind(&thread.context)
                .bind(thread.created_at)
                .bind(thread.updated_at)
                .fetch_one(pool)
                .await?;
                session_thread_from_row(row)
            }
        }
    }

    pub(crate) async fn update_session_thread_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<SessionThread, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let thread = store
                    .session_threads
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("session thread not found"))?;
                thread.status = status.to_string();
                thread.updated_at = updated_at;
                Ok(thread.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE session_threads
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, session_id, parent_thread_id, thread_kind, agent_id,
                               agent_version_id, environment_id, source_handoff_id,
                               specialist_session_id, status, title, context, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(status)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session thread not found"))?;
                session_thread_from_row(row)
            }
        }
    }
}
