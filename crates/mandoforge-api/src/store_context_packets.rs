use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::context_packet_from_row;
use crate::{AppError, AppState, ContextPacket};

impl AppState {
    pub(crate) async fn next_context_packet_version(
        &self,
        session_id: Uuid,
    ) -> Result<i64, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let max_version = inner
                    .read()
                    .await
                    .context_packets
                    .values()
                    .filter(|packet| packet.session_id == session_id)
                    .map(|packet| packet.version)
                    .max()
                    .unwrap_or(0);
                Ok(max_version + 1)
            }
            StoreBackend::Postgres(pool) => {
                let max_version: Option<i64> = sqlx::query_scalar(
                    "SELECT max(version)
                     FROM context_packets
                     WHERE tenant_id = $1 AND session_id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_one(pool)
                .await?;
                Ok(max_version.unwrap_or(0) + 1)
            }
        }
    }

    pub(crate) async fn create_context_packet(
        &self,
        packet: ContextPacket,
    ) -> Result<ContextPacket, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .context_packets
                    .insert(packet.id, packet.clone());
                Ok(packet)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO context_packets
                        (id, tenant_id, session_id, agent_id, agent_version_id, version, generated_at, task, agent, runtime_profile, semantic_scopes, tool_policy, policy_reminders, freshness_warnings, source_refs, retrieved_objects, replay_summary, audit_trace_id, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
                     RETURNING id, session_id, agent_id, agent_version_id, version, generated_at, task, agent, runtime_profile, semantic_scopes, tool_policy, policy_reminders, freshness_warnings, source_refs, retrieved_objects, replay_summary, audit_trace_id, created_at",
                )
                .bind(packet.id)
                .bind(self.current_tenant_id())
                .bind(packet.session_id)
                .bind(packet.agent_id)
                .bind(packet.agent_version_id)
                .bind(packet.version)
                .bind(packet.generated_at)
                .bind(&packet.task)
                .bind(json!(packet.agent))
                .bind(packet.runtime_profile.as_ref().map(|profile| json!(profile)))
                .bind(&packet.semantic_scopes)
                .bind(&packet.tool_policy)
                .bind(json!(packet.policy_reminders))
                .bind(json!(packet.freshness_warnings))
                .bind(json!(packet.source_refs))
                .bind(json!(packet.retrieved_objects))
                .bind(&packet.replay_summary)
                .bind(packet.audit_trace_id)
                .bind(packet.created_at)
                .fetch_one(pool)
                .await?;
                context_packet_from_row(row)
            }
        }
    }

    pub(crate) async fn update_context_packet_audit_trace(
        &self,
        id: Uuid,
        audit_trace_id: Uuid,
    ) -> Result<ContextPacket, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let packet = store
                    .context_packets
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("context packet not found"))?;
                packet.audit_trace_id = Some(audit_trace_id);
                Ok(packet.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE context_packets
                     SET audit_trace_id = $3
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, session_id, agent_id, agent_version_id, version, generated_at, task, agent, runtime_profile, semantic_scopes, tool_policy, policy_reminders, freshness_warnings, source_refs, retrieved_objects, replay_summary, audit_trace_id, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(audit_trace_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("context packet not found"))?;
                context_packet_from_row(row)
            }
        }
    }

    pub(crate) async fn list_context_packets(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ContextPacket>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut packets: Vec<_> = inner
                    .read()
                    .await
                    .context_packets
                    .values()
                    .filter(|packet| packet.session_id == session_id)
                    .cloned()
                    .collect();
                packets.sort_by_key(|packet| packet.version);
                Ok(packets)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, agent_id, agent_version_id, version, generated_at, task, agent, runtime_profile, semantic_scopes, tool_policy, policy_reminders, freshness_warnings, source_refs, retrieved_objects, replay_summary, audit_trace_id, created_at
                     FROM context_packets
                     WHERE tenant_id = $1 AND session_id = $2
                     ORDER BY version ASC",
                )
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(context_packet_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_context_packet(&self, id: Uuid) -> Result<ContextPacket, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .context_packets
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("context packet not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, agent_id, agent_version_id, version, generated_at, task, agent, runtime_profile, semantic_scopes, tool_policy, policy_reminders, freshness_warnings, source_refs, retrieved_objects, replay_summary, audit_trace_id, created_at
                     FROM context_packets
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("context packet not found"))?;
                context_packet_from_row(row)
            }
        }
    }
}
