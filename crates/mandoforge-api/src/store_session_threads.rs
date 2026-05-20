use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::session_thread_from_row;
use crate::{AppError, AppState, SessionThread};

impl AppState {
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
