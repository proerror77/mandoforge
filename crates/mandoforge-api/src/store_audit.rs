use anyhow::Result;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::audit_log_from_row;
use crate::{AppError, AppState, AuditLog};

struct ExecutionAuditClaimFence {
    job_id: Uuid,
    worker_id: String,
    claim_generation: i64,
    status: crate::ExecutionJobStatus,
}

impl AppState {
    pub(crate) async fn append_audit_log(&self, audit_log: AuditLog) -> Result<AuditLog, AppError> {
        self.append_audit_log_with_execution_claim(audit_log, None)
            .await
    }

    pub(crate) async fn append_audit_log_for_execution_claim(
        &self,
        job: &crate::execution_queue::ExecutionJob,
        status: crate::ExecutionJobStatus,
        audit_log: AuditLog,
    ) -> Result<AuditLog, AppError> {
        let worker_id = job
            .worker_id
            .clone()
            .ok_or_else(|| AppError::not_found("execution job claim has no worker"))?;
        self.append_audit_log_with_execution_claim(
            audit_log,
            Some(ExecutionAuditClaimFence {
                job_id: job.id,
                worker_id,
                claim_generation: job.claim_generation,
                status,
            }),
        )
        .await
    }

    async fn append_audit_log_with_execution_claim(
        &self,
        audit_log: AuditLog,
        execution_claim: Option<ExecutionAuditClaimFence>,
    ) -> Result<AuditLog, AppError> {
        match &self.store {
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
                if let Some(existing) = store.audit_logs.get(&audit_log.id) {
                    validate_idempotent_audit_identity(existing, &audit_log)?;
                    return Ok(existing.clone());
                }
                store.audit_logs.insert(audit_log.id, audit_log.clone());
            }
            StoreBackend::Postgres(pool) => {
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
                    .bind(self.current_tenant_id())
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
                let inserted = sqlx::query(
                    "INSERT INTO audit_logs
                        (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                     ON CONFLICT (id) DO NOTHING
                     RETURNING id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at",
                )
                .bind(audit_log.id)
                .bind(self.current_tenant_id())
                .bind(audit_log.session_id)
                .bind(&audit_log.actor_type)
                .bind(audit_log.actor_id)
                .bind(&audit_log.action)
                .bind(&audit_log.resource_type)
                .bind(audit_log.resource_id)
                .bind(&audit_log.details)
                .bind(audit_log.created_at)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(row) = inserted {
                    let inserted = audit_log_from_row(row)?;
                    tx.commit().await?;
                    return Ok(inserted);
                }
                let row = sqlx::query(
                    "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                     FROM audit_logs
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(audit_log.id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::bad_request("idempotent audit identity collision"))?;
                let existing = audit_log_from_row(row)?;
                validate_idempotent_audit_identity(&existing, &audit_log)?;
                tx.commit().await?;
                return Ok(existing);
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
                        .bind(self.current_tenant_id())
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
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(audit_log_from_row).collect()
            }
        }
    }
}

fn validate_idempotent_audit_identity(
    existing: &AuditLog,
    requested: &AuditLog,
) -> Result<(), AppError> {
    if existing.session_id == requested.session_id
        && existing.actor_type == requested.actor_type
        && existing.actor_id == requested.actor_id
        && existing.action == requested.action
        && existing.resource_type == requested.resource_type
        && existing.resource_id == requested.resource_id
        && existing.details == requested.details
    {
        Ok(())
    } else {
        Err(AppError::bad_request("idempotent audit identity collision"))
    }
}
