use chrono::{Duration, Utc};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::workflow_run_from_row;
use crate::{
    AppError, AppState, ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED,
    ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING, OntologyReleaseWorkflowTrigger, WorkflowRun,
    ontology_release_workflow_trigger_status_allowed,
};

const DEFAULT_TRIGGER_RECLAIM_SECONDS: i64 = 300;

impl AppState {
    pub(crate) async fn claim_ontology_release_workflow_trigger(
        &self,
        ontology_release_id: Uuid,
        workflow_definition_id: Uuid,
    ) -> Result<Option<OntologyReleaseWorkflowTrigger>, AppError> {
        let now = Utc::now();
        let stale_before = ontology_release_workflow_trigger_stale_before(now);
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if let Some(trigger) =
                    store
                        .ontology_release_workflow_triggers
                        .values_mut()
                        .find(|trigger| {
                            trigger.ontology_release_id == ontology_release_id
                                && trigger.workflow_definition_id == workflow_definition_id
                        })
                {
                    let stale_pending = trigger.status
                        == ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING
                        && trigger.claimed_at.unwrap_or(trigger.updated_at) <= stale_before;
                    if trigger.status == ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED
                        || stale_pending
                    {
                        trigger.status =
                            ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING.to_string();
                        trigger.attempt_count += 1;
                        trigger.claimed_at = Some(now);
                        trigger.workflow_run_id = None;
                        trigger.error_message = None;
                        trigger.updated_at = now;
                        return Ok(Some(trigger.clone()));
                    }
                    return Ok(None);
                }
                let trigger = OntologyReleaseWorkflowTrigger {
                    id: Uuid::new_v4(),
                    ontology_release_id,
                    workflow_definition_id,
                    workflow_run_id: None,
                    status: ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING.to_string(),
                    attempt_count: 1,
                    claimed_at: Some(now),
                    error_message: None,
                    created_at: now,
                    updated_at: now,
                };
                store
                    .ontology_release_workflow_triggers
                    .insert(trigger.id, trigger.clone());
                Ok(Some(trigger))
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO ontology_release_workflow_triggers
                        (id, tenant_id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NULL, 'pending', 1, $5, NULL, $5, $5)
                     ON CONFLICT (tenant_id, ontology_release_id, workflow_definition_id)
                     DO UPDATE SET status = 'pending',
                                   attempt_count = ontology_release_workflow_triggers.attempt_count + 1,
                                   claimed_at = $5,
                                   workflow_run_id = NULL,
                                   error_message = NULL,
                                   updated_at = $5
                     WHERE ontology_release_workflow_triggers.status = 'failed'
                        OR (
                            ontology_release_workflow_triggers.status = 'pending'
                            AND COALESCE(ontology_release_workflow_triggers.claimed_at, ontology_release_workflow_triggers.updated_at) <= $6
                        )
                     RETURNING id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at",
                )
                .bind(Uuid::new_v4())
                .bind(self.current_tenant_id())
                .bind(ontology_release_id)
                .bind(workflow_definition_id)
                .bind(now)
                .bind(stale_before)
                .fetch_optional(pool)
                .await?;
                row.map(ontology_release_workflow_trigger_from_row)
                    .transpose()
            }
        }
    }

    pub(crate) async fn complete_ontology_release_workflow_trigger(
        &self,
        trigger_id: Uuid,
        status: &str,
        workflow_run_id: Option<Uuid>,
        error_message: Option<String>,
    ) -> Result<OntologyReleaseWorkflowTrigger, AppError> {
        if !ontology_release_workflow_trigger_status_allowed(status) {
            return Err(AppError::bad_request(format!(
                "unsupported ontology release workflow trigger status: {status}"
            )));
        }
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let trigger = store
                    .ontology_release_workflow_triggers
                    .get_mut(&trigger_id)
                    .ok_or_else(|| {
                        AppError::not_found("ontology release workflow trigger not found")
                    })?;
                trigger.status = status.to_string();
                trigger.workflow_run_id = workflow_run_id;
                trigger.error_message = error_message;
                trigger.updated_at = now;
                Ok(trigger.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE ontology_release_workflow_triggers
                     SET status = $3,
                         workflow_run_id = $4,
                         error_message = $5,
                         updated_at = $6
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(trigger_id)
                .bind(status)
                .bind(workflow_run_id)
                .bind(error_message)
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release workflow trigger not found"))?;
                ontology_release_workflow_trigger_from_row(row)
            }
        }
    }

    pub(crate) async fn retryable_ontology_release_workflow_triggers(
        &self,
        limit: usize,
    ) -> Result<Vec<OntologyReleaseWorkflowTrigger>, AppError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let now = Utc::now();
        let stale_before = ontology_release_workflow_trigger_stale_before(now);
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut triggers: Vec<_> = inner
                    .read()
                    .await
                    .ontology_release_workflow_triggers
                    .values()
                    .filter(|trigger| {
                        trigger.status == ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_FAILED
                            || (trigger.status == ONTOLOGY_RELEASE_WORKFLOW_TRIGGER_STATUS_PENDING
                                && trigger.claimed_at.unwrap_or(trigger.updated_at) <= stale_before)
                    })
                    .cloned()
                    .collect();
                triggers.sort_by_key(|trigger| trigger.updated_at);
                triggers.truncate(limit);
                Ok(triggers)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at
                     FROM ontology_release_workflow_triggers
                     WHERE tenant_id = $1
                       AND (
                         status = 'failed'
                         OR (
                           status = 'pending'
                           AND COALESCE(claimed_at, updated_at) <= $2
                         )
                       )
                     ORDER BY updated_at ASC
                     LIMIT $3",
                )
                .bind(self.current_tenant_id())
                .bind(stale_before)
                .bind(limit as i64)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(ontology_release_workflow_trigger_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_ontology_release_workflow_trigger(
        &self,
        trigger_id: Uuid,
    ) -> Result<OntologyReleaseWorkflowTrigger, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .ontology_release_workflow_triggers
                .get(&trigger_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("ontology release workflow trigger not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at
                     FROM ontology_release_workflow_triggers
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(trigger_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release workflow trigger not found"))?;
                ontology_release_workflow_trigger_from_row(row)
            }
        }
    }

    pub(crate) async fn ontology_release_workflow_run_for_trigger(
        &self,
        ontology_release_id: Uuid,
        workflow_definition_id: Uuid,
    ) -> Result<Option<WorkflowRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .workflow_runs
                .values()
                .find(|run| {
                    run.workflow_definition_id == workflow_definition_id
                        && run.input_payload["trigger"] == "ontology_release.promoted"
                        && run.input_payload["ontology_release_id"]
                            == serde_json::json!(ontology_release_id)
                })
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at
                     FROM workflow_runs
                     WHERE tenant_id = $1
                       AND workflow_definition_id = $2
                       AND input_payload->>'trigger' = 'ontology_release.promoted'
                       AND input_payload->>'ontology_release_id' = $3
                     ORDER BY created_at DESC
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(workflow_definition_id)
                .bind(ontology_release_id.to_string())
                .fetch_optional(pool)
                .await?;
                row.map(workflow_run_from_row).transpose()
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn list_ontology_release_workflow_triggers(
        &self,
    ) -> Result<Vec<OntologyReleaseWorkflowTrigger>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut triggers: Vec<_> = inner
                    .read()
                    .await
                    .ontology_release_workflow_triggers
                    .values()
                    .cloned()
                    .collect();
                triggers.sort_by_key(|trigger| trigger.created_at);
                triggers.reverse();
                Ok(triggers)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, ontology_release_id, workflow_definition_id, workflow_run_id, status, attempt_count, claimed_at, error_message, created_at, updated_at
                     FROM ontology_release_workflow_triggers
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(ontology_release_workflow_trigger_from_row)
                    .collect()
            }
        }
    }
}

fn ontology_release_workflow_trigger_from_row(
    row: PgRow,
) -> Result<OntologyReleaseWorkflowTrigger, AppError> {
    Ok(OntologyReleaseWorkflowTrigger {
        id: row.try_get("id")?,
        ontology_release_id: row.try_get("ontology_release_id")?,
        workflow_definition_id: row.try_get("workflow_definition_id")?,
        workflow_run_id: row.try_get("workflow_run_id")?,
        status: row.try_get("status")?,
        attempt_count: row.try_get("attempt_count")?,
        claimed_at: row.try_get("claimed_at")?,
        error_message: row.try_get("error_message")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn ontology_release_workflow_trigger_stale_before(
    now: chrono::DateTime<Utc>,
) -> chrono::DateTime<Utc> {
    let reclaim_seconds = std::env::var("MANDOFORGE_ONTOLOGY_RELEASE_TRIGGER_RECLAIM_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TRIGGER_RECLAIM_SECONDS);
    now - Duration::seconds(reclaim_seconds)
}
