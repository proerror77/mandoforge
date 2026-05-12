use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::policy_revision_from_row;
use crate::{AppError, AppState, CreatePolicyRevision, PolicyRevision, PolicyRevisionGate};

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
                    "SELECT id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at
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
            gate_status: None,
            gate_result: json!({}),
            gated_at: None,
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
                    "INSERT INTO policy_revisions (id, tenant_id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(revision.id)
                .bind(self.tenant_id)
                .bind(&revision.name)
                .bind(&revision.body)
                .bind(&revision.status)
                .bind(&revision.created_by)
                .bind(revision.created_at)
                .bind(revision.activated_at)
                .bind(&revision.gate_status)
                .bind(&revision.gate_result)
                .bind(revision.gated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(revision)
    }

    pub(crate) async fn get_policy_revision(&self, id: Uuid) -> Result<PolicyRevision, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .policy_revisions
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("policy revision not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at
                     FROM policy_revisions
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("policy revision not found"))?;
                policy_revision_from_row(row)
            }
        }
    }

    pub(crate) async fn update_policy_revision_gate(
        &self,
        gate: &PolicyRevisionGate,
    ) -> Result<PolicyRevision, AppError> {
        let gate_result = serde_json::to_value(gate)?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let revision = store
                    .policy_revisions
                    .get_mut(&gate.revision_id)
                    .ok_or_else(|| AppError::not_found("policy revision not found"))?;
                revision.gate_status = Some(gate.status.clone());
                revision.gate_result = gate_result;
                revision.gated_at = Some(gate.checked_at);
                Ok(revision.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE policy_revisions
                     SET gate_status = $3, gate_result = $4, gated_at = $5
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at",
                )
                .bind(self.tenant_id)
                .bind(gate.revision_id)
                .bind(&gate.status)
                .bind(&gate_result)
                .bind(gate.checked_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("policy revision not found"))?;
                policy_revision_from_row(row)
            }
        }
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
                if store
                    .policy_revisions
                    .get(&id)
                    .and_then(|revision| revision.gate_status.as_deref())
                    != Some("passed")
                {
                    return Err(AppError::bad_request(
                        "policy revision must pass rollout gate before activation",
                    ));
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
                    "SELECT id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at
                     FROM policy_revisions
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("policy revision not found"))
                .and_then(policy_revision_from_row)?;
                if existing.gate_status.as_deref() != Some("passed") {
                    return Err(AppError::bad_request(
                        "policy revision must pass rollout gate before activation",
                    ));
                }
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
                     RETURNING id, name, body, status, created_by, created_at, activated_at, gate_status, gate_result, gated_at",
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
