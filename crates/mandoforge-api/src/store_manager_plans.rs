use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::manager_agent_plan_from_row;
use crate::{AppError, AppState, ManagerAgentPlan};

impl AppState {
    pub(crate) async fn list_manager_agent_plans(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<ManagerAgentPlan>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut plans: Vec<_> = inner
                    .read()
                    .await
                    .manager_agent_plans
                    .values()
                    .filter(|plan| session_id.is_none_or(|id| plan.session_id == id))
                    .cloned()
                    .collect();
                plans.sort_by_key(|plan| plan.created_at);
                Ok(plans)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at
                             FROM manager_agent_plans
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at
                             FROM manager_agent_plans
                             WHERE tenant_id = $1
                             ORDER BY created_at ASC",
                        )
                        .bind(self.current_tenant_id())
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(manager_agent_plan_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_manager_agent_plan(
        &self,
        id: Uuid,
    ) -> Result<ManagerAgentPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .manager_agent_plans
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("manager agent plan not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at
                     FROM manager_agent_plans
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("manager agent plan not found"))?;
                manager_agent_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn create_manager_agent_plan(
        &self,
        plan: ManagerAgentPlan,
    ) -> Result<ManagerAgentPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .manager_agent_plans
                    .insert(plan.id, plan.clone());
                Ok(plan)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO manager_agent_plans
                        (id, tenant_id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     RETURNING id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at",
                )
                .bind(plan.id)
                .bind(self.current_tenant_id())
                .bind(plan.session_id)
                .bind(plan.manager_agent_id)
                .bind(plan.specialist_agent_id)
                .bind(&plan.task_intake)
                .bind(&plan.decomposition)
                .bind(&plan.specialist_selection)
                .bind(&plan.risk_classification)
                .bind(&plan.review)
                .bind(&plan.status)
                .bind(plan.audit_trace_id)
                .bind(plan.created_at)
                .bind(plan.updated_at)
                .fetch_one(pool)
                .await?;
                manager_agent_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_manager_agent_plan_review(
        &self,
        id: Uuid,
        review: serde_json::Value,
        status: String,
        audit_trace_id: Option<Uuid>,
    ) -> Result<ManagerAgentPlan, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .manager_agent_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("manager agent plan not found"))?;
                plan.review = review;
                plan.status = status;
                plan.audit_trace_id = audit_trace_id;
                plan.updated_at = updated_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE manager_agent_plans
                     SET review = $3,
                         status = $4,
                         audit_trace_id = $5,
                         updated_at = $6
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, session_id, manager_agent_id, specialist_agent_id, task_intake, decomposition, specialist_selection, risk_classification, review, status, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(&review)
                .bind(&status)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("manager agent plan not found"))?;
                manager_agent_plan_from_row(row)
            }
        }
    }
}
