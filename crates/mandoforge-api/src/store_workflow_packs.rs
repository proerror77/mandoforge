use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::workflow_pack_installation_from_row;
use crate::{AppError, AppState, WorkflowPackInstallation};

impl AppState {
    pub(crate) async fn list_workflow_pack_installations(
        &self,
    ) -> Result<Vec<WorkflowPackInstallation>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut installations: Vec<_> = inner
                    .read()
                    .await
                    .workflow_pack_installations
                    .values()
                    .filter(|installation| installation.archived_at.is_none())
                    .cloned()
                    .collect();
                installations.sort_by_key(|installation| installation.created_at);
                Ok(installations)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at
                     FROM workflow_pack_installations
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_installation_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_workflow_pack_installation(
        &self,
        id: Uuid,
    ) -> Result<WorkflowPackInstallation, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .workflow_pack_installations
                .get(&id)
                .filter(|installation| installation.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("workflow pack installation not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at
                     FROM workflow_pack_installations
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                workflow_pack_installation_from_row(row)
            }
        }
    }

    pub(crate) async fn create_workflow_pack_installation(
        &self,
        installation: WorkflowPackInstallation,
    ) -> Result<WorkflowPackInstallation, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .workflow_pack_installations
                    .insert(installation.id, installation.clone());
                Ok(installation)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO workflow_pack_installations
                        (id, tenant_id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                     RETURNING id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at",
                )
                .bind(installation.id)
                .bind(self.current_tenant_id())
                .bind(&installation.pack_id)
                .bind(&installation.kind)
                .bind(&installation.version)
                .bind(&installation.manifest_path)
                .bind(&installation.manifest)
                .bind(&installation.validation_report)
                .bind(&installation.status)
                .bind(&installation.eval_gate_status)
                .bind(&installation.release_gate_status)
                .bind(&installation.gate_evidence)
                .bind(installation.staged_at)
                .bind(installation.released_at)
                .bind(installation.archived_at)
                .bind(installation.created_at)
                .bind(installation.updated_at)
                .fetch_one(pool)
                .await?;
                workflow_pack_installation_from_row(row)
            }
        }
    }

    pub(crate) async fn update_workflow_pack_installation_state(
        &self,
        id: Uuid,
        status: &str,
        eval_gate_status: &str,
        release_gate_status: &str,
        gate_evidence: Value,
        staged_at: Option<chrono::DateTime<Utc>>,
        released_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<WorkflowPackInstallation, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let installation = store
                    .workflow_pack_installations
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                installation.status = status.to_string();
                installation.eval_gate_status = eval_gate_status.to_string();
                installation.release_gate_status = release_gate_status.to_string();
                installation.gate_evidence = gate_evidence;
                installation.staged_at = staged_at;
                installation.released_at = released_at;
                installation.updated_at = updated_at;
                Ok(installation.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_pack_installations
                     SET status = $3, eval_gate_status = $4, release_gate_status = $5, gate_evidence = $6, staged_at = $7, released_at = $8, updated_at = $9
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(status)
                .bind(eval_gate_status)
                .bind(release_gate_status)
                .bind(&gate_evidence)
                .bind(staged_at)
                .bind(released_at)
                .bind(updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                workflow_pack_installation_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_workflow_pack_installation(
        &self,
        id: Uuid,
    ) -> Result<WorkflowPackInstallation, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let installation = store
                    .workflow_pack_installations
                    .get_mut(&id)
                    .filter(|installation| installation.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                installation.status = "archived".to_string();
                installation.archived_at = Some(archived_at);
                installation.updated_at = archived_at;
                Ok(installation.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_pack_installations
                     SET status = 'archived', archived_at = $3, updated_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, pack_id, kind, version, manifest_path, manifest, validation_report, status, eval_gate_status, release_gate_status, gate_evidence, staged_at, released_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                workflow_pack_installation_from_row(row)
            }
        }
    }
}
