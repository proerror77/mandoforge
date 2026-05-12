use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::policy_revision_from_row;
use crate::{AppError, AppState, CreatePolicyRevision, PolicyRevision};

impl AppState {
    pub(crate) async fn list_policy_revisions(&self) -> Result<Vec<PolicyRevision>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut revisions: Vec<_> = inner
                    .read()
                    .await
                    .policy_revisions
                    .values()
                    .cloned()
                    .collect();
                revisions.sort_by_key(|revision| revision.created_at);
                revisions.reverse();
                Ok(revisions)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, body, status, created_by, created_at, activated_at
                     FROM policy_revisions
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(policy_revision_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_policy_revision(
        &self,
        input: CreatePolicyRevision,
        created_by: String,
    ) -> Result<PolicyRevision, AppError> {
        let revision = PolicyRevision {
            id: Uuid::new_v4(),
            name: input.name,
            body: input.body,
            status: "draft".to_string(),
            created_by: Some(created_by),
            created_at: Utc::now(),
            activated_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .policy_revisions
                    .values()
                    .any(|existing| existing.name == revision.name)
                {
                    return Err(AppError::bad_request("policy revision name already exists"));
                }
                store.policy_revisions.insert(revision.id, revision.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO policy_revisions (id, tenant_id, name, body, status, created_by, created_at, activated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(revision.id)
                .bind(self.tenant_id)
                .bind(&revision.name)
                .bind(&revision.body)
                .bind(&revision.status)
                .bind(&revision.created_by)
                .bind(revision.created_at)
                .bind(revision.activated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(revision)
    }

    pub(crate) async fn activate_policy_revision(
        &self,
        id: Uuid,
    ) -> Result<PolicyRevision, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store.policy_revisions.contains_key(&id) {
                    return Err(AppError::not_found("policy revision not found"));
                }
                for revision in store.policy_revisions.values_mut() {
                    if revision.status == "active" {
                        revision.status = "archived".to_string();
                    }
                }
                let revision = store
                    .policy_revisions
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("policy revision not found"))?;
                revision.status = "active".to_string();
                revision.activated_at = Some(Utc::now());
                Ok(revision.clone())
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let existing = sqlx::query(
                    "SELECT id, name, body, status, created_by, created_at, activated_at
                     FROM policy_revisions
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("policy revision not found"))
                .and_then(policy_revision_from_row)?;
                sqlx::query(
                    "UPDATE policy_revisions
                     SET status = 'archived'
                     WHERE tenant_id = $1 AND status = 'active' AND id <> $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                let row = sqlx::query(
                    "UPDATE policy_revisions
                     SET status = 'active', activated_at = $3
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, name, body, status, created_by, created_at, activated_at",
                )
                .bind(self.tenant_id)
                .bind(existing.id)
                .bind(Utc::now())
                .fetch_one(&mut *tx)
                .await?;
                tx.commit().await?;
                policy_revision_from_row(row)
            }
        }
    }
}
