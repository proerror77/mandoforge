use anyhow::Result;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::audit_log_from_row;
use crate::{AppError, AppState, AuditLog};

impl AppState {
    pub(crate) async fn append_audit_log(&self, audit_log: AuditLog) -> Result<AuditLog, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .audit_logs
                    .insert(audit_log.id, audit_log.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO audit_logs
                        (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(audit_log.id)
                .bind(self.tenant_id)
                .bind(audit_log.session_id)
                .bind(&audit_log.actor_type)
                .bind(audit_log.actor_id)
                .bind(&audit_log.action)
                .bind(&audit_log.resource_type)
                .bind(audit_log.resource_id)
                .bind(&audit_log.details)
                .bind(audit_log.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(audit_log)
    }

    pub(crate) async fn list_audit_logs(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<AuditLog>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut logs: Vec<_> = inner
                    .read()
                    .await
                    .audit_logs
                    .values()
                    .filter(|log| session_id.is_none_or(|id| log.session_id == Some(id)))
                    .cloned()
                    .collect();
                logs.sort_by_key(|log| log.created_at);
                logs.reverse();
                Ok(logs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                             FROM audit_logs
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                             FROM audit_logs
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(audit_log_from_row).collect()
            }
        }
    }
}
