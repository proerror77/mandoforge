use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::agent_handoff_event_from_row;
use crate::{AgentHandoffEvent, AppError, AppState};

impl AppState {
    pub(crate) async fn list_agent_handoff_events(
        &self,
        source_session_id: Option<Uuid>,
    ) -> Result<Vec<AgentHandoffEvent>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut handoffs: Vec<_> = inner
                    .read()
                    .await
                    .agent_handoff_events
                    .values()
                    .filter(|event| {
                        source_session_id
                            .is_none_or(|session_id| event.source_session_id == session_id)
                    })
                    .cloned()
                    .collect();
                handoffs.sort_by_key(|event| event.created_at);
                Ok(handoffs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match source_session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_events
                             WHERE tenant_id = $1 AND source_session_id = $2
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at
                             FROM agent_handoff_events
                             WHERE tenant_id = $1
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(agent_handoff_event_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_agent_handoff_event(
        &self,
        id: Uuid,
    ) -> Result<AgentHandoffEvent, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agent_handoff_events
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("agent handoff event not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at
                     FROM agent_handoff_events
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                agent_handoff_event_from_row(row)
            }
        }
    }

    pub(crate) async fn create_agent_handoff_event(
        &self,
        event: AgentHandoffEvent,
    ) -> Result<AgentHandoffEvent, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .agent_handoff_events
                    .insert(event.id, event.clone());
                Ok(event)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO agent_handoff_events
                        (id, tenant_id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     RETURNING id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at",
                )
                .bind(event.id)
                .bind(self.current_tenant_id())
                .bind(event.source_session_id)
                .bind(event.source_agent_id)
                .bind(event.target_agent_id)
                .bind(&event.intent)
                .bind(&event.payload)
                .bind(&event.schema_version)
                .bind(&event.risk_level)
                .bind(event.approval_required)
                .bind(&event.status)
                .bind(event.audit_trace_id)
                .bind(event.created_at)
                .bind(event.updated_at)
                .fetch_one(pool)
                .await?;
                agent_handoff_event_from_row(row)
            }
        }
    }

    pub(crate) async fn update_agent_handoff_event_status(
        &self,
        id: Uuid,
        status: &str,
        audit_trace_id: Option<Uuid>,
    ) -> Result<AgentHandoffEvent, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let event = store
                    .agent_handoff_events
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                event.status = status.to_string();
                event.audit_trace_id = audit_trace_id;
                event.updated_at = updated_at;
                Ok(event.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_handoff_events
                     SET status = $3, audit_trace_id = $4, updated_at = $5
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, source_session_id, source_agent_id, target_agent_id, intent, payload, schema_version, risk_level, approval_required, status, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(status)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent handoff event not found"))?;
                agent_handoff_event_from_row(row)
            }
        }
    }
}
