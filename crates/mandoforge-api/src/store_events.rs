use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::{
    PgPool,
    postgres::{PgListener, PgPoolOptions},
};
use tokio::sync::{OnceCell, broadcast};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::event_from_row;
use crate::{AppError, AppState, SessionEvent};

const SESSION_EVENT_BROADCAST_CAPACITY: usize = 1024;
const SESSION_EVENT_CATCH_UP_INTERVAL: Duration = Duration::from_secs(15);
pub(crate) const POSTGRES_SESSION_EVENT_CHANNEL: &str = "mf_session_events";

#[derive(Clone)]
struct ExecutionClaimFence {
    job_id: Uuid,
    worker_id: String,
    claim_generation: i64,
    status: crate::ExecutionJobStatus,
}

pub(crate) enum SessionEventSubscription {
    Memory(broadcast::Receiver<SessionEvent>),
    Postgres(PostgresSessionEventSubscription),
}

#[derive(Clone)]
struct PostgresSessionEventNotification {
    tenant_id: Option<Uuid>,
    session_id: Option<Uuid>,
}

pub(crate) struct PostgresSessionEventSubscription {
    _hub: Arc<PostgresSessionEventHub>,
    tenant_id: Uuid,
    receiver: broadcast::Receiver<PostgresSessionEventNotification>,
}

struct PostgresSessionEventHub {
    notifications: broadcast::Sender<PostgresSessionEventNotification>,
}

pub(crate) async fn subscribe_session_events(
    state: &AppState,
) -> Result<SessionEventSubscription, AppError> {
    match &state.store {
        StoreBackend::Memory(_) => Ok(SessionEventSubscription::Memory(
            session_event_broadcaster().subscribe(),
        )),
        StoreBackend::Postgres(pool) => {
            let tenant_id = state.current_tenant_id();
            let hub = postgres_session_event_hub(pool).await?;
            let receiver = hub.notifications.subscribe();
            Ok(SessionEventSubscription::Postgres(
                PostgresSessionEventSubscription {
                    _hub: hub,
                    tenant_id,
                    receiver,
                },
            ))
        }
    }
}

async fn postgres_session_event_hub(
    pool: &PgPool,
) -> Result<Arc<PostgresSessionEventHub>, AppError> {
    static HUB: OnceCell<Arc<PostgresSessionEventHub>> = OnceCell::const_new();
    HUB.get_or_try_init(|| async {
        let listener_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with((*pool.connect_options()).clone())
            .await?;
        let mut listener = PgListener::connect_with(&listener_pool).await?;
        listener.listen(POSTGRES_SESSION_EVENT_CHANNEL).await?;
        let (notifications, _) = broadcast::channel(SESSION_EVENT_BROADCAST_CAPACITY);
        tokio::spawn(run_postgres_session_event_hub(
            listener,
            notifications.clone(),
        ));
        Ok(Arc::new(PostgresSessionEventHub { notifications }))
    })
    .await
    .cloned()
}

async fn run_postgres_session_event_hub(
    mut listener: PgListener,
    notifications: broadcast::Sender<PostgresSessionEventNotification>,
) {
    loop {
        match listener.recv().await {
            Ok(notification) => {
                let (tenant_id, session_id) = notified_event_scope(notification.payload())
                    .map(|(tenant_id, session_id)| (Some(tenant_id), Some(session_id)))
                    .unwrap_or_default();
                let _ = notifications.send(PostgresSessionEventNotification {
                    tenant_id,
                    session_id,
                });
            }
            Err(error) => {
                tracing::warn!(%error, "Postgres session event listener reconnect failed");
                let _ = notifications.send(PostgresSessionEventNotification {
                    tenant_id: None,
                    session_id: None,
                });
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

fn publish_session_event(event: &SessionEvent) {
    let _ = session_event_broadcaster().send(event.clone());
}

pub(crate) fn session_event_notify_payload(tenant_id: Uuid, event: &SessionEvent) -> String {
    format!("{}:{}:{}", tenant_id, event.session_id, event.seq)
}

fn notified_event_scope(payload: &str) -> Option<(Uuid, Uuid)> {
    let mut fields = payload.split(':');
    let tenant_id = Uuid::parse_str(fields.next()?).ok()?;
    let session_id = Uuid::parse_str(fields.next()?).ok()?;
    fields.next()?;
    fields.next().is_none().then_some((tenant_id, session_id))
}

fn session_event_broadcaster() -> &'static broadcast::Sender<SessionEvent> {
    static BROADCASTER: OnceLock<broadcast::Sender<SessionEvent>> = OnceLock::new();
    BROADCASTER.get_or_init(|| {
        let (sender, _) = broadcast::channel(SESSION_EVENT_BROADCAST_CAPACITY);
        sender
    })
}

impl AppState {
    pub(crate) async fn has_unresolved_execution_result_at_or_before(
        &self,
        session_id: Uuid,
        event_seq: i64,
    ) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let jobs = self.execution_queue.list().await?;
                let store = inner.read().await;
                let Some(events) = store.events.get(&session_id) else {
                    return Ok(false);
                };
                let hidden_tool_call_ids =
                    jobs.iter()
                        .filter(|job| {
                            job.session_id == session_id && !events.iter().any(|event| {
                                crate::session_loop_runtime::execution_completion_event_matches_job(
                                    event, job,
                                )
                            })
                        })
                        .map(|job| job.tool_call_id)
                        .collect::<std::collections::HashSet<_>>();
                Ok(events.iter().any(|event| {
                    event.seq <= event_seq
                        && event.event_type == "tool.result"
                        && event
                            .actor_id
                            .is_some_and(|actor_id| hidden_tool_call_ids.contains(&actor_id))
                }))
            }
            StoreBackend::Postgres(pool) => {
                let blocked = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(
                         SELECT 1
                         FROM session_events AS event
                         JOIN execution_jobs AS job
                           ON job.tenant_id = event.tenant_id
                          AND job.session_id = event.session_id
                          AND job.tool_call_id = event.actor_id
                         WHERE event.tenant_id = $1
                           AND event.session_id = $2
                           AND event.event_type = 'tool.result'
                           AND event.seq <= $3
                           AND (
                               job.status <> 'completed'
                               OR NOT EXISTS (
                                   SELECT 1
                                   FROM session_events AS completion
                                   WHERE completion.tenant_id = job.tenant_id
                                     AND completion.session_id = job.session_id
                                     AND completion.event_type = 'execution.completed'
                                     AND completion.actor_type = 'worker'
                                     AND completion.actor_id = job.id
                                     AND completion.payload ->> 'status' = 'completed'
                                     AND completion.payload ->> 'execution_job_id' = job.id::text
                                     AND completion.payload ->> 'tool_call_id' = job.tool_call_id::text
                                     AND completion.payload ->> 'attempt_count' = job.attempt_count::text
                                     AND completion.payload ->> 'claim_generation' = job.claim_generation::text
                               )
                           )
                     )",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .bind(event_seq)
                .fetch_one(pool)
                .await?;
                Ok(blocked)
            }
        }
    }

    pub(crate) async fn list_events(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<SessionEvent>, AppError> {
        self.list_events_after(session_id, None).await
    }

    pub(crate) async fn list_events_after(
        &self,
        session_id: Uuid,
        after_seq: Option<i64>,
    ) -> Result<Vec<SessionEvent>, AppError> {
        self.list_events_after_for_tenant(self.current_tenant_id(), session_id, after_seq)
            .await
    }

    pub(crate) async fn list_events_after_for_tenant(
        &self,
        tenant_id: Uuid,
        session_id: Uuid,
        after_seq: Option<i64>,
    ) -> Result<Vec<SessionEvent>, AppError> {
        let after_seq = after_seq.unwrap_or(0);
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .events
                .get(&session_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|event| event.seq > after_seq)
                .collect()),
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2 AND seq > $3
                     ORDER BY seq ASC",
                )
                .bind(tenant_id)
                .bind(session_id)
                .bind(after_seq)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(event_from_row).collect()
            }
        }
    }

    pub(crate) async fn append_event(
        &self,
        actor_type: &str,
        actor_id: Option<Uuid>,
        session_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<SessionEvent, AppError> {
        self.append_event_with_id(
            Uuid::new_v4(),
            false,
            None,
            actor_type,
            actor_id,
            session_id,
            event_type,
            payload,
        )
        .await
    }

    pub(crate) async fn append_event_once(
        &self,
        event_id: Uuid,
        actor_type: &str,
        actor_id: Option<Uuid>,
        session_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<SessionEvent, AppError> {
        self.append_event_with_id(
            event_id, true, None, actor_type, actor_id, session_id, event_type, payload,
        )
        .await
    }

    pub(crate) async fn append_event_once_for_execution_claim(
        &self,
        job: &crate::execution_queue::ExecutionJob,
        status: crate::ExecutionJobStatus,
        event_id: Uuid,
        actor_type: &str,
        actor_id: Option<Uuid>,
        session_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<SessionEvent, AppError> {
        let worker_id = job
            .worker_id
            .clone()
            .ok_or_else(|| AppError::not_found("execution job claim has no worker"))?;
        self.append_event_with_id(
            event_id,
            true,
            Some(ExecutionClaimFence {
                job_id: job.id,
                worker_id,
                claim_generation: job.claim_generation,
                status,
            }),
            actor_type,
            actor_id,
            session_id,
            event_type,
            payload,
        )
        .await
    }

    async fn append_event_with_id(
        &self,
        event_id: Uuid,
        idempotent: bool,
        execution_claim: Option<ExecutionClaimFence>,
        actor_type: &str,
        actor_id: Option<Uuid>,
        session_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<SessionEvent, AppError> {
        let event = match &self.store {
            StoreBackend::Memory(inner) => {
                let _claim_guard = match execution_claim.as_ref() {
                    Some(claim) => Some(
                        self.execution_queue
                            .lock_owned_claim(
                                claim.job_id,
                                &claim.worker_id,
                                claim.claim_generation,
                                claim.status.clone(),
                            )
                            .await?,
                    ),
                    None => None,
                };
                let mut store = inner.write().await;
                if !store.sessions.contains_key(&session_id) {
                    return Err(AppError::not_found("session not found"));
                }
                if idempotent
                    && let Some(existing) = store
                        .events
                        .values()
                        .flatten()
                        .find(|event| event.id == event_id)
                {
                    validate_idempotent_event_identity(
                        existing, actor_type, actor_id, session_id, event_type, &payload,
                    )?;
                    return Ok(existing.clone());
                }
                let seq = store
                    .events
                    .get(&session_id)
                    .map_or(1, |events| events.len() as i64 + 1);
                let event = SessionEvent {
                    id: event_id,
                    session_id,
                    seq,
                    parent_event_id: None,
                    actor_type: actor_type.to_string(),
                    actor_id,
                    event_type: event_type.to_string(),
                    payload,
                    created_at: Utc::now(),
                };
                store
                    .events
                    .entry(session_id)
                    .or_default()
                    .push(event.clone());
                event
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                if self.get_session(session_id).await.is_err() {
                    return Err(AppError::not_found("session not found"));
                }
                let mut tx = pool.begin().await?;
                if let Some(claim) = execution_claim.as_ref() {
                    let owned = sqlx::query_scalar::<_, i32>(
                        "SELECT 1
                         FROM execution_jobs
                         WHERE tenant_id = $1
                           AND id = $2
                           AND status = $3
                           AND worker_id = $4
                           AND claim_generation = $5
                           AND lease_expires_at > now()
                         FOR UPDATE",
                    )
                    .bind(tenant_id)
                    .bind(claim.job_id)
                    .bind(claim.status.as_str())
                    .bind(&claim.worker_id)
                    .bind(claim.claim_generation)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if owned.is_none() {
                        return Err(AppError::not_found(
                            "execution job claim is no longer owned",
                        ));
                    }
                }
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
                if idempotent {
                    let existing = sqlx::query(
                        "SELECT id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at
                         FROM session_events
                         WHERE tenant_id = $1 AND id = $2",
                    )
                    .bind(tenant_id)
                    .bind(event_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if let Some(row) = existing {
                        let event = event_from_row(row)?;
                        validate_idempotent_event_identity(
                            &event, actor_type, actor_id, session_id, event_type, &payload,
                        )?;
                        tx.commit().await?;
                        return Ok(event);
                    }
                }
                let row = sqlx::query(
                    "WITH next_seq AS (
                        SELECT COALESCE(MAX(seq), 0) + 1 AS seq
                        FROM session_events
                        WHERE tenant_id = $1 AND session_id = $2
                     )
                     INSERT INTO session_events
                        (id, tenant_id, session_id, seq, actor_type, actor_id, event_type, payload, created_at)
                     SELECT $3, $1, $2, next_seq.seq, $4, $5, $6, $7, $8
                     FROM next_seq
                     RETURNING id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at",
                )
                .bind(tenant_id)
                .bind(session_id)
                .bind(event_id)
                .bind(actor_type)
                .bind(actor_id)
                .bind(event_type)
                .bind(payload)
                .bind(Utc::now())
                .fetch_one(&mut *tx)
                .await?;
                let event = event_from_row(row)?;
                sqlx::query("SELECT pg_notify($1, $2)")
                    .bind(POSTGRES_SESSION_EVENT_CHANNEL)
                    .bind(session_event_notify_payload(tenant_id, &event))
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                event
            }
        };
        self.emit_committed_session_events(std::slice::from_ref(&event))
            .await;
        Ok(event)
    }

    pub(crate) async fn emit_committed_session_events(&self, events: &[SessionEvent]) {
        for event in events {
            self.emit_telemetry_event(event).await;
            if matches!(&self.store, StoreBackend::Memory(_)) {
                publish_session_event(event);
            }
        }
    }
}

pub(crate) fn validate_idempotent_event_identity(
    event: &SessionEvent,
    actor_type: &str,
    actor_id: Option<Uuid>,
    session_id: Uuid,
    event_type: &str,
    payload: &Value,
) -> Result<(), AppError> {
    if event.actor_type == actor_type
        && event.actor_id == actor_id
        && event.session_id == session_id
        && event.event_type == event_type
        && event.payload == *payload
    {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "idempotent session event identity collision",
        ))
    }
}

impl SessionEventSubscription {
    pub(crate) async fn wait_for_session_change(
        &mut self,
        session_id: Uuid,
    ) -> Result<bool, AppError> {
        match self {
            SessionEventSubscription::Memory(receiver) => loop {
                match receiver.recv().await {
                    Ok(event) if event.session_id == session_id => return Ok(true),
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Ok(true),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(false),
                }
            },
            SessionEventSubscription::Postgres(PostgresSessionEventSubscription {
                tenant_id,
                receiver,
                ..
            }) => {
                let catch_up_deadline =
                    tokio::time::Instant::now() + SESSION_EVENT_CATCH_UP_INTERVAL;
                loop {
                    let notification =
                        match tokio::time::timeout_at(catch_up_deadline, receiver.recv()).await {
                            Ok(Ok(notification)) => notification,
                            Ok(Err(broadcast::error::RecvError::Lagged(_))) => return Ok(true),
                            Ok(Err(broadcast::error::RecvError::Closed)) => return Ok(false),
                            Err(_) => return Ok(true),
                        };
                    if notification
                        .tenant_id
                        .is_some_and(|notified_tenant_id| notified_tenant_id != *tenant_id)
                        || notification
                            .session_id
                            .is_some_and(|notified_session_id| notified_session_id != session_id)
                    {
                        continue;
                    }
                    return Ok(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notified_event_scope_parses_tenant_and_session() {
        let tenant_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        assert_eq!(
            notified_event_scope(&format!("{tenant_id}:{session_id}:42")),
            Some((tenant_id, session_id))
        );
        assert_eq!(notified_event_scope(&format!("{session_id}:42")), None);
        assert_eq!(notified_event_scope("not-a-uuid"), None);
    }

    #[tokio::test]
    async fn memory_subscription_requests_catch_up_after_lag() {
        let (sender, _) = broadcast::channel(1);
        let session_id = Uuid::new_v4();
        let other_session_id = Uuid::new_v4();
        let mut subscription = SessionEventSubscription::Memory(sender.subscribe());

        let send_event = |session_id| SessionEvent {
            id: Uuid::new_v4(),
            session_id,
            seq: 1,
            parent_event_id: None,
            actor_type: "user".to_string(),
            actor_id: None,
            event_type: "user.message".to_string(),
            payload: Value::Null,
            created_at: Utc::now(),
        };

        let _ = sender.send(send_event(other_session_id));
        let _ = sender.send(send_event(session_id));

        assert!(
            subscription
                .wait_for_session_change(session_id)
                .await
                .expect("lag should request catch-up")
        );
    }
}
