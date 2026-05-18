use anyhow::Result;
use chrono::Utc;
use sqlx::error::DatabaseError;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::memory_writeback_candidate_from_row;
use crate::{AppError, AppState, MemoryWritebackCandidate};

impl AppState {
    pub(crate) async fn create_memory_writeback_candidate(
        &self,
        candidate: MemoryWritebackCandidate,
    ) -> Result<MemoryWritebackCandidate, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.memory_writeback_candidates.values().any(|existing| {
                    existing.status == "pending"
                        && existing.candidate_type == candidate.candidate_type
                        && existing.source_event_id == candidate.source_event_id
                        && existing.source_artifact_id == candidate.source_artifact_id
                        && existing.source_approval_id == candidate.source_approval_id
                        && existing.source_handoff_id == candidate.source_handoff_id
                }) {
                    return Err(AppError::bad_request(
                        "pending memory writeback candidate already exists for source",
                    ));
                }
                store
                    .memory_writeback_candidates
                    .insert(candidate.id, candidate.clone());
                Ok(candidate)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO memory_writeback_candidates
                        (id, tenant_id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26)
                     RETURNING id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at",
                )
                .bind(candidate.id)
                .bind(self.current_tenant_id())
                .bind(candidate.session_id)
                .bind(&candidate.candidate_type)
                .bind(candidate.source_event_id)
                .bind(candidate.source_artifact_id)
                .bind(candidate.source_approval_id)
                .bind(candidate.source_handoff_id)
                .bind(&candidate.proposed_object_type)
                .bind(&candidate.proposed_object_key)
                .bind(&candidate.title)
                .bind(&candidate.summary)
                .bind(&candidate.content)
                .bind(&candidate.semantic_scopes)
                .bind(&candidate.source_refs)
                .bind(&candidate.provenance)
                .bind(&candidate.trust_level)
                .bind(&candidate.freshness)
                .bind(&candidate.status)
                .bind(&candidate.reviewer_subject)
                .bind(&candidate.review_reason)
                .bind(candidate.semantic_object_id)
                .bind(candidate.audit_trace_id)
                .bind(candidate.created_at)
                .bind(candidate.updated_at)
                .bind(candidate.decided_at)
                .fetch_one(pool)
                .await
                .map_err(memory_writeback_insert_error)?;
                memory_writeback_candidate_from_row(row)
            }
        }
    }

    pub(crate) async fn list_memory_writeback_candidates(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<MemoryWritebackCandidate>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut candidates: Vec<_> = inner
                    .read()
                    .await
                    .memory_writeback_candidates
                    .values()
                    .filter(|candidate| {
                        session_id
                            .map(|session_id| candidate.session_id == session_id)
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect();
                candidates.sort_by_key(|candidate| candidate.created_at);
                Ok(candidates)
            }
            StoreBackend::Postgres(pool) => {
                let rows = if let Some(session_id) = session_id {
                    sqlx::query(
                        "SELECT id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at
                         FROM memory_writeback_candidates
                         WHERE tenant_id = $1 AND session_id = $2
                         ORDER BY created_at ASC",
                    )
                    .bind(self.current_tenant_id())
                    .bind(session_id)
                    .fetch_all(pool)
                    .await?
                } else {
                    sqlx::query(
                        "SELECT id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at
                         FROM memory_writeback_candidates
                         WHERE tenant_id = $1
                         ORDER BY created_at ASC",
                    )
                    .bind(self.current_tenant_id())
                    .fetch_all(pool)
                    .await?
                };
                rows.into_iter()
                    .map(memory_writeback_candidate_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_memory_writeback_candidate(
        &self,
        id: Uuid,
    ) -> Result<MemoryWritebackCandidate, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .memory_writeback_candidates
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("memory writeback candidate not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at
                     FROM memory_writeback_candidates
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("memory writeback candidate not found"))?;
                memory_writeback_candidate_from_row(row)
            }
        }
    }

    pub(crate) async fn update_memory_writeback_candidate_audit_trace(
        &self,
        id: Uuid,
        audit_trace_id: Uuid,
    ) -> Result<MemoryWritebackCandidate, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let candidate = store
                    .memory_writeback_candidates
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("memory writeback candidate not found"))?;
                candidate.audit_trace_id = Some(audit_trace_id);
                Ok(candidate.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE memory_writeback_candidates
                     SET audit_trace_id = $3, updated_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(audit_trace_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("memory writeback candidate not found"))?;
                memory_writeback_candidate_from_row(row)
            }
        }
    }

    pub(crate) async fn decide_memory_writeback_candidate(
        &self,
        id: Uuid,
        status: &str,
        reviewer_subject: Option<String>,
        review_reason: Option<String>,
        semantic_object_id: Option<Uuid>,
    ) -> Result<MemoryWritebackCandidate, AppError> {
        let decided_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let candidate = store
                    .memory_writeback_candidates
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("memory writeback candidate not found"))?;
                if candidate.status != "pending" {
                    return Err(AppError::bad_request(
                        "only pending memory writeback candidates can be reviewed",
                    ));
                }
                candidate.status = status.to_string();
                candidate.reviewer_subject = reviewer_subject;
                candidate.review_reason = review_reason;
                candidate.semantic_object_id = semantic_object_id;
                candidate.updated_at = decided_at;
                candidate.decided_at = Some(decided_at);
                Ok(candidate.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE memory_writeback_candidates
                     SET status = $3,
                         reviewer_subject = $4,
                         review_reason = $5,
                         semantic_object_id = $6,
                         updated_at = $7,
                         decided_at = $7
                     WHERE tenant_id = $1 AND id = $2 AND status = 'pending'
                     RETURNING id, session_id, candidate_type, source_event_id, source_artifact_id, source_approval_id, source_handoff_id, proposed_object_type, proposed_object_key, title, summary, content, semantic_scopes, source_refs, provenance, trust_level, freshness, status, reviewer_subject, review_reason, semantic_object_id, audit_trace_id, created_at, updated_at, decided_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(status)
                .bind(&reviewer_subject)
                .bind(&review_reason)
                .bind(semantic_object_id)
                .bind(decided_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("pending memory writeback candidate not found"))?;
                memory_writeback_candidate_from_row(row)
            }
        }
    }
}

fn memory_writeback_insert_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database_error) = &error
        && is_unique_violation(database_error.as_ref())
    {
        return AppError::bad_request(
            "pending memory writeback candidate already exists for source",
        );
    }
    error.into()
}

fn is_unique_violation(error: &(dyn DatabaseError + '_)) -> bool {
    error.code().as_deref() == Some("23505")
}
