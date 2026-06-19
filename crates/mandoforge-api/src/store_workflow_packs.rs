use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    workflow_pack_binding_from_row, workflow_pack_installation_from_row,
    workflow_pack_profile_asset_from_row, workflow_pack_runtime_object_from_row,
};
use crate::{
    AppError, AppState, WorkflowPackBinding, WorkflowPackInstallation, WorkflowPackProfileAsset,
    WorkflowPackRuntimeObject,
};

impl AppState {
    pub(crate) async fn create_workflow_pack_installation_with_profile_assets(
        &self,
        installation: WorkflowPackInstallation,
        profile_assets: &[(String, String)],
    ) -> Result<(WorkflowPackInstallation, Vec<WorkflowPackProfileAsset>), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                store
                    .workflow_pack_installations
                    .insert(installation.id, installation.clone());
                let bootstrapped_assets = profile_assets
                    .iter()
                    .map(|(profile_id, content)| WorkflowPackProfileAsset {
                        id: Uuid::new_v4(),
                        installation_id: installation.id,
                        profile_id: profile_id.clone(),
                        content: content.clone(),
                        version: 1,
                        status: "active".to_string(),
                        created_at: installation.created_at,
                        archived_at: None,
                    })
                    .collect::<Vec<_>>();
                for asset in &bootstrapped_assets {
                    store
                        .workflow_pack_profile_assets
                        .insert(asset.id, asset.clone());
                }
                Ok((installation, bootstrapped_assets))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let installation_row = sqlx::query(
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
                .fetch_one(&mut *tx)
                .await?;
                let installation = workflow_pack_installation_from_row(installation_row)?;

                let mut bootstrapped_assets = Vec::with_capacity(profile_assets.len());
                for (profile_id, content) in profile_assets {
                    let row = sqlx::query(
                        "INSERT INTO workflow_pack_profile_assets
                            (id, tenant_id, installation_id, profile_id, content, version, status, created_at, archived_at)
                         VALUES ($1, $2, $3, $4, $5, 1, 'active', $6, NULL)
                         RETURNING id, installation_id, profile_id, content, version, status, created_at, archived_at",
                    )
                    .bind(Uuid::new_v4())
                    .bind(self.current_tenant_id())
                    .bind(installation.id)
                    .bind(profile_id)
                    .bind(content)
                    .bind(installation.created_at)
                    .fetch_one(&mut *tx)
                    .await?;
                    bootstrapped_assets.push(workflow_pack_profile_asset_from_row(row)?);
                }

                tx.commit().await?;
                Ok((installation, bootstrapped_assets))
            }
        }
    }

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

    pub(crate) async fn update_workflow_pack_installation_state(
        &self,
        id: Uuid,
        status: &str,
        eval_gate_status: &str,
        release_gate_status: &str,
        gate_evidence: Value,
        staged_at: Option<chrono::DateTime<Utc>>,
        released_at: Option<chrono::DateTime<Utc>>,
        expected_status: Option<&str>,
    ) -> Result<WorkflowPackInstallation, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let installation = store
                    .workflow_pack_installations
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("workflow pack installation not found"))?;
                if let Some(expected) = expected_status {
                    if installation.status != expected {
                        return Err(AppError::bad_request(
                            "workflow pack installation status conflict: concurrent update detected",
                        ));
                    }
                }
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
                       AND ($10 IS NULL OR status = $10)
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
                .bind(expected_status)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    if expected_status.is_some() {
                        AppError::bad_request("workflow pack installation status conflict: concurrent update detected")
                    } else {
                        AppError::not_found("workflow pack installation not found")
                    }
                })?;
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

    pub(crate) async fn list_workflow_pack_profile_assets(
        &self,
        installation_id: Uuid,
    ) -> Result<Vec<WorkflowPackProfileAsset>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut profiles: Vec<_> = inner
                    .read()
                    .await
                    .workflow_pack_profile_assets
                    .values()
                    .filter(|asset| {
                        asset.installation_id == installation_id && asset.archived_at.is_none()
                    })
                    .cloned()
                    .collect();
                profiles.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
                Ok(profiles)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, installation_id, profile_id, content, version, status, created_at, archived_at
                     FROM workflow_pack_profile_assets
                     WHERE tenant_id = $1 AND installation_id = $2 AND archived_at IS NULL
                     ORDER BY profile_id ASC",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_profile_asset_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn save_workflow_pack_profile_asset(
        &self,
        installation_id: Uuid,
        profile_id: &str,
        content: &str,
    ) -> Result<WorkflowPackProfileAsset, AppError> {
        let created_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let next_version = store
                    .workflow_pack_profile_assets
                    .values()
                    .filter(|asset| {
                        asset.installation_id == installation_id && asset.profile_id == profile_id
                    })
                    .map(|asset| asset.version)
                    .max()
                    .unwrap_or(0)
                    + 1;
                for asset in store.workflow_pack_profile_assets.values_mut() {
                    if asset.installation_id == installation_id
                        && asset.profile_id == profile_id
                        && asset.archived_at.is_none()
                    {
                        asset.status = "archived".to_string();
                        asset.archived_at = Some(created_at);
                    }
                }
                let asset = WorkflowPackProfileAsset {
                    id: Uuid::new_v4(),
                    installation_id,
                    profile_id: profile_id.to_string(),
                    content: content.to_string(),
                    version: next_version,
                    status: "active".to_string(),
                    created_at,
                    archived_at: None,
                };
                store
                    .workflow_pack_profile_assets
                    .insert(asset.id, asset.clone());
                Ok(asset)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let next_version: i32 = sqlx::query_scalar(
                    "SELECT COALESCE(MAX(version), 0) + 1
                     FROM workflow_pack_profile_assets
                     WHERE tenant_id = $1 AND installation_id = $2 AND profile_id = $3",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(profile_id)
                .fetch_one(&mut *tx)
                .await?;

                sqlx::query(
                    "UPDATE workflow_pack_profile_assets
                     SET status = 'archived', archived_at = $4
                     WHERE tenant_id = $1 AND installation_id = $2 AND profile_id = $3 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(profile_id)
                .bind(created_at)
                .execute(&mut *tx)
                .await?;

                let row = sqlx::query(
                    "INSERT INTO workflow_pack_profile_assets
                        (id, tenant_id, installation_id, profile_id, content, version, status, created_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, 'active', $7, NULL)
                     RETURNING id, installation_id, profile_id, content, version, status, created_at, archived_at",
                )
                .bind(Uuid::new_v4())
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(profile_id)
                .bind(content)
                .bind(next_version)
                .bind(created_at)
                .fetch_one(&mut *tx)
                .await?;

                tx.commit().await?;
                workflow_pack_profile_asset_from_row(row)
            }
        }
    }

    pub(crate) async fn create_workflow_pack_bindings(
        &self,
        bindings: Vec<WorkflowPackBinding>,
    ) -> Result<Vec<WorkflowPackBinding>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                for binding in store.workflow_pack_bindings.values_mut() {
                    if binding.status != "superseded"
                        && bindings.first().is_some_and(|new_binding| {
                            binding.installation_id == new_binding.installation_id
                        })
                    {
                        binding.status = "superseded".to_string();
                        binding.updated_at = Utc::now();
                    }
                }
                for binding in &bindings {
                    store
                        .workflow_pack_bindings
                        .insert(binding.id, binding.clone());
                }
                Ok(bindings)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                if let Some(first) = bindings.first() {
                    sqlx::query(
                        "UPDATE workflow_pack_bindings
                         SET status = 'superseded', updated_at = now()
                         WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'",
                    )
                    .bind(self.current_tenant_id())
                    .bind(first.installation_id)
                    .execute(&mut *tx)
                    .await?;
                }
                let mut created = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let row = sqlx::query(
                        "INSERT INTO workflow_pack_bindings
                            (id, tenant_id, installation_id, pack_id, pack_version, binding_type, binding_key, source_path, target_kind, target_id, status, materialized_payload, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                         RETURNING id, installation_id, pack_id, pack_version, binding_type, binding_key, source_path, target_kind, target_id, status, materialized_payload, created_at, updated_at",
                    )
                    .bind(binding.id)
                    .bind(self.current_tenant_id())
                    .bind(binding.installation_id)
                    .bind(&binding.pack_id)
                    .bind(&binding.pack_version)
                    .bind(&binding.binding_type)
                    .bind(&binding.binding_key)
                    .bind(&binding.source_path)
                    .bind(&binding.target_kind)
                    .bind(binding.target_id)
                    .bind(&binding.status)
                    .bind(&binding.materialized_payload)
                    .bind(binding.created_at)
                    .bind(binding.updated_at)
                    .fetch_one(&mut *tx)
                    .await?;
                    created.push(workflow_pack_binding_from_row(row)?);
                }
                tx.commit().await?;
                Ok(created)
            }
        }
    }

    pub(crate) async fn list_workflow_pack_bindings(
        &self,
        installation_id: Uuid,
    ) -> Result<Vec<WorkflowPackBinding>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut bindings: Vec<_> = inner
                    .read()
                    .await
                    .workflow_pack_bindings
                    .values()
                    .filter(|binding| {
                        binding.installation_id == installation_id && binding.status != "superseded"
                    })
                    .cloned()
                    .collect();
                bindings.sort_by(|left, right| {
                    left.binding_type
                        .cmp(&right.binding_type)
                        .then(left.binding_key.cmp(&right.binding_key))
                });
                Ok(bindings)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, installation_id, pack_id, pack_version, binding_type, binding_key, source_path, target_kind, target_id, status, materialized_payload, created_at, updated_at
                     FROM workflow_pack_bindings
                     WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'
                     ORDER BY binding_type ASC, binding_key ASC",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_binding_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn update_workflow_pack_binding_statuses(
        &self,
        installation_id: Uuid,
        status: &str,
    ) -> Result<Vec<WorkflowPackBinding>, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut bindings = Vec::new();
                for binding in store.workflow_pack_bindings.values_mut() {
                    if binding.installation_id == installation_id && binding.status != "superseded"
                    {
                        binding.status = status.to_string();
                        binding.updated_at = updated_at;
                        bindings.push(binding.clone());
                    }
                }
                bindings.sort_by(|left, right| {
                    left.binding_type
                        .cmp(&right.binding_type)
                        .then(left.binding_key.cmp(&right.binding_key))
                });
                Ok(bindings)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "UPDATE workflow_pack_bindings
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'
                     RETURNING id, installation_id, pack_id, pack_version, binding_type, binding_key, source_path, target_kind, target_id, status, materialized_payload, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(status)
                .bind(updated_at)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_binding_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_workflow_pack_runtime_objects(
        &self,
        objects: Vec<WorkflowPackRuntimeObject>,
    ) -> Result<Vec<WorkflowPackRuntimeObject>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                for object in store.workflow_pack_runtime_objects.values_mut() {
                    if object.status != "superseded"
                        && objects.first().is_some_and(|new_object| {
                            object.installation_id == new_object.installation_id
                        })
                    {
                        object.status = "superseded".to_string();
                        object.updated_at = Utc::now();
                    }
                }
                for object in &objects {
                    store
                        .workflow_pack_runtime_objects
                        .insert(object.id, object.clone());
                }
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                if let Some(first) = objects.first() {
                    sqlx::query(
                        "UPDATE workflow_pack_runtime_objects
                         SET status = 'superseded', updated_at = now()
                         WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'",
                    )
                    .bind(self.current_tenant_id())
                    .bind(first.installation_id)
                    .execute(&mut *tx)
                    .await?;
                }
                let mut created = Vec::with_capacity(objects.len());
                for object in objects {
                    let row = sqlx::query(
                        "INSERT INTO workflow_pack_runtime_objects
                            (id, tenant_id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                         RETURNING id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at",
                    )
                    .bind(object.id)
                    .bind(self.current_tenant_id())
                    .bind(object.installation_id)
                    .bind(object.binding_id)
                    .bind(&object.pack_id)
                    .bind(&object.pack_version)
                    .bind(&object.object_type)
                    .bind(&object.object_key)
                    .bind(&object.runtime_kind)
                    .bind(&object.status)
                    .bind(&object.spec)
                    .bind(object.created_at)
                    .bind(object.updated_at)
                    .fetch_one(&mut *tx)
                    .await?;
                    created.push(workflow_pack_runtime_object_from_row(row)?);
                }
                tx.commit().await?;
                Ok(created)
            }
        }
    }

    pub(crate) async fn list_workflow_pack_runtime_objects(
        &self,
        installation_id: Uuid,
    ) -> Result<Vec<WorkflowPackRuntimeObject>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut objects: Vec<_> = inner
                    .read()
                    .await
                    .workflow_pack_runtime_objects
                    .values()
                    .filter(|object| {
                        object.installation_id == installation_id && object.status != "superseded"
                    })
                    .cloned()
                    .collect();
                objects.sort_by(|left, right| {
                    left.object_type
                        .cmp(&right.object_type)
                        .then(left.object_key.cmp(&right.object_key))
                });
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at
                     FROM workflow_pack_runtime_objects
                     WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'
                     ORDER BY object_type ASC, object_key ASC",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_runtime_object_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn list_workflow_pack_runtime_objects_by_runtime_kind(
        &self,
        runtime_kind: &str,
    ) -> Result<Vec<WorkflowPackRuntimeObject>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut objects: Vec<_> = inner
                    .read()
                    .await
                    .workflow_pack_runtime_objects
                    .values()
                    .filter(|object| {
                        object.runtime_kind == runtime_kind && object.status != "superseded"
                    })
                    .cloned()
                    .collect();
                objects.sort_by(|left, right| {
                    left.object_type
                        .cmp(&right.object_type)
                        .then(left.object_key.cmp(&right.object_key))
                });
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at
                     FROM workflow_pack_runtime_objects
                     WHERE tenant_id = $1 AND runtime_kind = $2 AND status <> 'superseded'
                     ORDER BY object_type ASC, object_key ASC",
                )
                .bind(self.current_tenant_id())
                .bind(runtime_kind)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_runtime_object_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn update_workflow_pack_runtime_object_statuses(
        &self,
        installation_id: Uuid,
        status: &str,
    ) -> Result<Vec<WorkflowPackRuntimeObject>, AppError> {
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut objects = Vec::new();
                for object in store.workflow_pack_runtime_objects.values_mut() {
                    if object.installation_id == installation_id && object.status != "superseded" {
                        object.status = status.to_string();
                        object.updated_at = updated_at;
                        objects.push(object.clone());
                    }
                }
                objects.sort_by(|left, right| {
                    left.object_type
                        .cmp(&right.object_type)
                        .then(left.object_key.cmp(&right.object_key))
                });
                Ok(objects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "UPDATE workflow_pack_runtime_objects
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1 AND installation_id = $2 AND status <> 'superseded'
                     RETURNING id, installation_id, binding_id, pack_id, pack_version, object_type, object_key, runtime_kind, status, spec, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(status)
                .bind(updated_at)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(workflow_pack_runtime_object_from_row)
                    .collect()
            }
        }
    }
}
