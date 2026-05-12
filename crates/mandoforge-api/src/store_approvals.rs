use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::approval_from_row;
use crate::{AppError, AppState, Approval};

impl AppState {
    pub(crate) async fn insert_approval(&self, approval: Approval) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .approvals
                    .insert(approval.id, approval.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approvals (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, created_at, decided_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(approval.id)
                .bind(self.tenant_id)
                .bind(approval.session_id)
                .bind(approval.tool_call_id)
                .bind(&approval.action)
                .bind(&approval.risk_level)
                .bind(&approval.reason)
                .bind(&approval.evidence)
                .bind(&approval.decision_payload)
                .bind(&approval.status)
                .bind(approval.created_at)
                .bind(approval.decided_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(approval)
    }

    pub(crate) async fn list_approvals(&self) -> Result<Vec<Approval>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.approvals.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(approval_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_approval(&self, approval_id: Uuid) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .approvals
                .get(&approval_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("approval not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval_from_row(row)
            }
        }
    }

    pub(crate) async fn decide_approval(
        &self,
        approval_id: Uuid,
        status: &str,
    ) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval.status = status.to_string();
                approval.decided_at = Some(Utc::now());
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approvals
                     SET status = $1, decided_at = now()
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, created_at, decided_at",
                )
                .bind(status)
                .bind(self.tenant_id)
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval_from_row(row)
            }
        }
    }

    pub(crate) async fn modify_approval(
        &self,
        approval_id: Uuid,
        modified_args: Value,
        comment: Option<String>,
    ) -> Result<Approval, AppError> {
        let decision_payload = serde_json::json!({
            "modified_args": modified_args,
            "comment": comment,
        });
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                if approval.status != "pending" {
                    return Err(AppError::bad_request(
                        "only pending approvals can be modified",
                    ));
                }
                approval.decision_payload = decision_payload;
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approvals
                     SET decision_payload = $1
                     WHERE tenant_id = $2 AND id = $3 AND status = 'pending'
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, decision_payload, status, created_at, decided_at",
                )
                .bind(&decision_payload)
                .bind(self.tenant_id)
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("pending approval not found"))?;
                approval_from_row(row)
            }
        }
    }
}
