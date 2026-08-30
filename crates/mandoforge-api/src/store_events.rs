use std::{sync::OnceLock, time::Duration};

use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use sqlx::postgres::{PgListener, PgPoolOptions};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::event_from_row;
use crate::{AppError, AppState, SessionEvent};

const SESSION_EVENT_BROADCAST_CAPACITY: usize = 1024;
const SESSION_EVENT_CATCH_UP_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) enum SessionEventSubscription {
    Memory(broadcast::Receiver<SessionEvent>),
    Postgres(PgListener),
}

pub(crate) async fn subscribe_session_events(
    state: &AppState,
) -> Result<SessionEventSubscription, AppError> {
    match &state.store {
        StoreBackend::Memory(_) => Ok(SessionEventSubscription::Memory(
            session_event_broadcaster().subscribe(),
        )),
        StoreBackend::Postgres(pool) => {
            let listener_pool = PgPoolOptions::new()
                .max_connections(1)
                .connect_with((*pool.connect_options()).clone())
                .await?;
            let mut listener = PgListener::connect_with(&listener_pool).await?;
            listener
                .listen(&session_event_notify_channel(state.current_tenant_id()))
                .await?;
            Ok(SessionEventSubscription::Postgres(listener))
        }
    }
}

fn publish_session_event(event: &SessionEvent) {
    let _ = session_event_broadcaster().send(event.clone());
}

fn session_event_notify_channel(tenant_id: Uuid) -> String {
    format!("mf_session_events_{}", tenant_id.simple())
}

fn session_event_notify_payload(event: &SessionEvent) -> String {
    format!("{}:{}", event.session_id, event.seq)
}

fn notified_session_id(payload: &str) -> Option<Uuid> {
    payload
        .split_once(':')
        .map(|(session_id, _)| session_id)
        .or_else(|| (!payload.is_empty()).then_some(payload))
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

fn session_event_broadcaster() -> &'static broadcast::Sender<SessionEvent> {
    static BROADCASTER: OnceLock<broadcast::Sender<SessionEvent>> = OnceLock::new();
    BROADCASTER.get_or_init(|| {
        let (sender, _) = broadcast::channel(SESSION_EVENT_BROADCAST_CAPACITY);
        sender
    })
}

impl AppState {
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
        let event = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store.sessions.contains_key(&session_id) {
                    return Err(AppError::not_found("session not found"));
                }
                let seq = store
                    .events
                    .get(&session_id)
                    .map_or(1, |events| events.len() as i64 + 1);
                let event = SessionEvent {
                    id: Uuid::new_v4(),
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
                sqlx::query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text || ':' || $2::uuid::text, 0))",
                )
                .bind(tenant_id)
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
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
                .bind(Uuid::new_v4())
                .bind(actor_type)
                .bind(actor_id)
                .bind(event_type)
                .bind(payload)
                .bind(Utc::now())
                .fetch_one(&mut *tx)
                .await?;
                let event = event_from_row(row)?;
                sqlx::query("SELECT pg_notify($1, $2)")
                    .bind(session_event_notify_channel(tenant_id))
                    .bind(session_event_notify_payload(&event))
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                event
            }
        };
        self.emit_telemetry_event(&event).await;
        if matches!(&self.store, StoreBackend::Memory(_)) {
            publish_session_event(&event);
        }
        Ok(event)
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
            SessionEventSubscription::Postgres(listener) => {
                let catch_up_deadline =
                    tokio::time::Instant::now() + SESSION_EVENT_CATCH_UP_INTERVAL;
                loop {
                    let notification =
                        match tokio::time::timeout_at(catch_up_deadline, listener.recv()).await {
                            Ok(Ok(notification)) => notification,
                            Ok(Err(error)) => return Err(error.into()),
                            Err(_) => return Ok(true),
                        };
                    match notified_session_id(notification.payload()) {
                        Some(notified_session_id) if notified_session_id != session_id => {}
                        _ => return Ok(true),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notified_session_id_parses_session_prefix() {
        let session_id = Uuid::new_v4();
        assert_eq!(
            notified_session_id(&format!("{session_id}:42")),
            Some(session_id)
        );
        assert_eq!(
            notified_session_id(&session_id.to_string()),
            Some(session_id)
        );
        assert_eq!(notified_session_id("not-a-uuid"), None);
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
