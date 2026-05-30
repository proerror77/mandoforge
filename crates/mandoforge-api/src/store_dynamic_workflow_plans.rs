use anyhow::Result;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::dynamic_workflow_plan_from_row;
use crate::{AppError, AppState, DynamicWorkflowPlan};

impl AppState {
    pub(crate) async fn list_dynamic_workflow_plans(
        &self,
    ) -> Result<Vec<DynamicWorkflowPlan>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut plans: Vec<_> = inner
                    .read()
                    .await
                    .dynamic_workflow_plans
                    .values()
                    .cloned()
                    .collect();
                plans.sort_by_key(|plan| plan.created_at);
                plans.reverse();
                Ok(plans)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at
                     FROM dynamic_workflow_plans
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(dynamic_workflow_plan_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_dynamic_workflow_plan(
        &self,
        id: Uuid,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .dynamic_workflow_plans
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("dynamic workflow plan not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at
                     FROM dynamic_workflow_plans
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn create_dynamic_workflow_plan(
        &self,
        plan: DynamicWorkflowPlan,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .dynamic_workflow_plans
                    .insert(plan.id, plan.clone());
                Ok(plan)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO dynamic_workflow_plans
                        (id, tenant_id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(plan.id)
                .bind(self.current_tenant_id())
                .bind(plan.source_work_item_id)
                .bind(plan.source_session_id)
                .bind(&plan.objective)
                .bind(&plan.status)
                .bind(&plan.phases)
                .bind(&plan.agent_fleet_policy)
                .bind(&plan.governance)
                .bind(&plan.validation)
                .bind(&plan.materialization)
                .bind(&plan.analysis)
                .bind(&plan.review)
                .bind(plan.workflow_definition_id)
                .bind(plan.workflow_run_id)
                .bind(plan.audit_trace_id)
                .bind(plan.created_at)
                .bind(plan.updated_at)
                .bind(plan.reviewed_at)
                .bind(plan.materialized_at)
                .fetch_one(pool)
                .await?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_dynamic_workflow_plan_review(
        &self,
        id: Uuid,
        status: String,
        review: serde_json::Value,
        audit_trace_id: Option<Uuid>,
        reviewed_at: DateTime<Utc>,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .dynamic_workflow_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                plan.status = status;
                plan.review = review;
                plan.audit_trace_id = audit_trace_id;
                plan.reviewed_at = Some(reviewed_at);
                plan.updated_at = reviewed_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET status = $3,
                         review = $4,
                         audit_trace_id = $5,
                         reviewed_at = $6,
                         updated_at = $6
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(&status)
                .bind(&review)
                .bind(audit_trace_id)
                .bind(reviewed_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_dynamic_workflow_plan_audit_trace(
        &self,
        id: Uuid,
        audit_trace_id: Option<Uuid>,
        updated_at: DateTime<Utc>,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .dynamic_workflow_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                plan.audit_trace_id = audit_trace_id;
                plan.updated_at = updated_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET audit_trace_id = $3,
                         updated_at = $4
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_dynamic_workflow_plan_materialized(
        &self,
        id: Uuid,
        workflow_definition_id: Uuid,
        workflow_run_id: Uuid,
        audit_trace_id: Option<Uuid>,
        materialized_at: DateTime<Utc>,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .dynamic_workflow_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                plan.status = "materialized".to_string();
                plan.workflow_definition_id = Some(workflow_definition_id);
                plan.workflow_run_id = Some(workflow_run_id);
                plan.audit_trace_id = audit_trace_id;
                plan.materialized_at = Some(materialized_at);
                plan.updated_at = materialized_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET status = 'materialized',
                         workflow_definition_id = $3,
                         workflow_run_id = $4,
                         audit_trace_id = $5,
                         materialized_at = $6,
                         updated_at = $6
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(workflow_definition_id)
                .bind(workflow_run_id)
                .bind(audit_trace_id)
                .bind(materialized_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }
}
