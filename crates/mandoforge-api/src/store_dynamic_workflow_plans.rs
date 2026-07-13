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
        expected_status: &str,
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
                if plan.status != expected_status {
                    return Err(AppError::conflict(
                        "dynamic workflow plan review changed concurrently",
                    ));
                }
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
                     SET status = $4,
                         review = $5,
                         audit_trace_id = $6,
                         reviewed_at = $7,
                         updated_at = $7
                     WHERE tenant_id = $1 AND id = $2 AND status = $3
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(expected_status)
                .bind(&status)
                .bind(&review)
                .bind(audit_trace_id)
                .bind(reviewed_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::conflict("dynamic workflow plan review changed concurrently")
                })?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_dynamic_workflow_plan_audit_trace_if_unchanged(
        &self,
        id: Uuid,
        expected_status: &str,
        expected_updated_at: DateTime<Utc>,
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
                if plan.status != expected_status || plan.updated_at != expected_updated_at {
                    return Ok(plan.clone());
                }
                plan.audit_trace_id = audit_trace_id;
                plan.updated_at = updated_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET audit_trace_id = $5,
                         updated_at = $6
                     WHERE tenant_id = $1
                       AND id = $2
                       AND status = $3
                       AND updated_at = $4
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(expected_status)
                .bind(expected_updated_at)
                .bind(audit_trace_id)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?;
                match row {
                    Some(row) => dynamic_workflow_plan_from_row(row),
                    None => self.get_dynamic_workflow_plan(id).await,
                }
            }
        }
    }

    pub(crate) async fn claim_dynamic_workflow_plan_materialization(
        &self,
        id: Uuid,
        claim_audit_id: Uuid,
        claimed_at: DateTime<Utc>,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .dynamic_workflow_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                if plan.status != "approved"
                    || plan.workflow_definition_id.is_some()
                    || plan.workflow_run_id.is_some()
                {
                    return Err(AppError::conflict(
                        "dynamic workflow plan materialization is already claimed",
                    ));
                }
                plan.status = "materializing".to_string();
                plan.audit_trace_id = Some(claim_audit_id);
                plan.updated_at = claimed_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET status = 'materializing',
                         audit_trace_id = $3,
                         updated_at = $4
                     WHERE tenant_id = $1
                       AND id = $2
                       AND status = 'approved'
                       AND workflow_definition_id IS NULL
                       AND workflow_run_id IS NULL
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(claim_audit_id)
                .bind(claimed_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::conflict(
                        "dynamic workflow plan materialization is already claimed",
                    )
                })?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn fail_dynamic_workflow_plan_materialization(
        &self,
        id: Uuid,
        claim_audit_id: Uuid,
        audit_trace_id: Option<Uuid>,
        failed_at: DateTime<Utc>,
    ) -> Result<DynamicWorkflowPlan, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let plan = store
                    .dynamic_workflow_plans
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("dynamic workflow plan not found"))?;
                if plan.status != "materializing" || plan.audit_trace_id != Some(claim_audit_id) {
                    return Err(AppError::conflict(
                        "dynamic workflow materialization failure does not match the active claim",
                    ));
                }
                plan.status = "materialization_failed".to_string();
                plan.audit_trace_id = audit_trace_id;
                plan.updated_at = failed_at;
                Ok(plan.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE dynamic_workflow_plans
                     SET status = 'materialization_failed',
                         audit_trace_id = $4,
                         updated_at = $5
                     WHERE tenant_id = $1
                       AND id = $2
                       AND status = 'materializing'
                       AND audit_trace_id = $3
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(claim_audit_id)
                .bind(audit_trace_id)
                .bind(failed_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::conflict(
                        "dynamic workflow materialization failure does not match the active claim",
                    )
                })?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }

    pub(crate) async fn update_dynamic_workflow_plan_materialized(
        &self,
        id: Uuid,
        claim_audit_id: Uuid,
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
                if plan.status != "materializing" || plan.audit_trace_id != Some(claim_audit_id) {
                    return Err(AppError::conflict(
                        "dynamic workflow materialization completion does not match the active claim",
                    ));
                }
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
                         workflow_definition_id = $4,
                         workflow_run_id = $5,
                         audit_trace_id = $6,
                         materialized_at = $7,
                         updated_at = $7
                     WHERE tenant_id = $1
                       AND id = $2
                       AND status = 'materializing'
                       AND audit_trace_id = $3
                     RETURNING id, source_work_item_id, source_session_id, objective, status, phases, agent_fleet_policy, governance, validation, materialization, analysis, review, workflow_definition_id, workflow_run_id, audit_trace_id, created_at, updated_at, reviewed_at, materialized_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(claim_audit_id)
                .bind(workflow_definition_id)
                .bind(workflow_run_id)
                .bind(audit_trace_id)
                .bind(materialized_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::conflict(
                        "dynamic workflow materialization completion does not match the active claim",
                    )
                })?;
                dynamic_workflow_plan_from_row(row)
            }
        }
    }
}
