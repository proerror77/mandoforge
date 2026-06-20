use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::ontology_release_from_row;
use crate::{AppError, AppState, OntologyRelease};

impl AppState {
    pub(crate) async fn list_ontology_releases(&self) -> Result<Vec<OntologyRelease>, AppError> {
        self.list_ontology_releases_for_domain(None).await
    }

    pub(crate) async fn list_ontology_releases_for_domain(
        &self,
        domain_scope: Option<&str>,
    ) -> Result<Vec<OntologyRelease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .ontology_releases
                    .values()
                    .filter(|release| {
                        domain_scope.is_none_or(|scope| {
                            release.domain_scope.eq_ignore_ascii_case(scope.trim())
                        })
                    })
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.created_at);
                Ok(releases)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1
                       AND ($2::text IS NULL OR lower(domain_scope) = lower($2))
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(domain_scope.map(str::trim).filter(|scope| !scope.is_empty()))
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(ontology_release_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_ontology_release(&self, id: Uuid) -> Result<OntologyRelease, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .ontology_releases
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("ontology release not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                ontology_release_from_row(row)
            }
        }
    }

    pub(crate) async fn active_ontology_release_for_domain(
        &self,
        domain_scope: &str,
    ) -> Result<Option<OntologyRelease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .ontology_releases
                    .values()
                    .filter(|release| {
                        release.status == "active"
                            && release.domain_scope.eq_ignore_ascii_case(domain_scope)
                    })
                    .cloned()
                    .collect();
                releases.sort_by_key(|release| release.promoted_at.or(Some(release.created_at)));
                Ok(releases.pop())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND lower(domain_scope) = lower($2) AND status = 'active'
                     ORDER BY promoted_at DESC NULLS LAST, created_at DESC
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
                .bind(domain_scope)
                .fetch_optional(pool)
                .await?;
                row.map(ontology_release_from_row).transpose()
            }
        }
    }

    pub(crate) async fn create_ontology_release(
        &self,
        release: OntologyRelease,
    ) -> Result<OntologyRelease, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.ontology_releases.values().any(|existing| {
                    existing.version.eq_ignore_ascii_case(&release.version)
                        && existing
                            .domain_scope
                            .eq_ignore_ascii_case(&release.domain_scope)
                }) {
                    return Err(AppError::bad_request(format!(
                        "ontology release version already exists: {}",
                        release.version
                    )));
                }
                store.ontology_releases.insert(release.id, release.clone());
                Ok(release)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO ontology_releases
                        (id, tenant_id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
                     RETURNING id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at",
                )
                .bind(release.id)
                .bind(self.current_tenant_id())
                .bind(&release.version)
                .bind(&release.domain_scope)
                .bind(release.source_run_id)
                .bind(release.parent_release_id)
                .bind(release.rollback_target_release_id)
                .bind(&release.status)
                .bind(&release.release_class)
                .bind(release.object_count)
                .bind(release.relation_count)
                .bind(release.action_count)
                .bind(&release.migration_policy)
                .bind(&release.gate_result)
                .bind(&release.materialized_object_ids)
                .bind(&release.materialized_link_ids)
                .bind(&release.evidence_refs)
                .bind(&release.promoted_by)
                .bind(release.promoted_at)
                .bind(&release.rolled_back_by)
                .bind(release.rolled_back_at)
                .bind(release.archived_at)
                .bind(release.created_at)
                .bind(release.updated_at)
                .fetch_one(pool)
                .await?;
                ontology_release_from_row(row)
            }
        }
    }

    pub(crate) async fn update_ontology_release(
        &self,
        release: OntologyRelease,
        expected_status: Option<&str>,
    ) -> Result<OntologyRelease, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let existing = store
                    .ontology_releases
                    .get(&release.id)
                    .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                if let Some(expected) = expected_status {
                    if existing.status != expected {
                        return Err(AppError::bad_request(
                            "ontology release status conflict: concurrent update detected",
                        ));
                    }
                }
                store.ontology_releases.insert(release.id, release.clone());
                Ok(release)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE ontology_releases
                     SET version = $3,
                         domain_scope = $4,
                         source_run_id = $5,
                         parent_release_id = $6,
                         rollback_target_release_id = $7,
                         status = $8,
                         release_class = $9,
                         object_count = $10,
                         relation_count = $11,
                         action_count = $12,
                         migration_policy = $13,
                         gate_result = $14,
                         materialized_object_ids = $15,
                         materialized_link_ids = $16,
                         evidence_refs = $17,
                         promoted_by = $18,
                         promoted_at = $19,
                         rolled_back_by = $20,
                         rolled_back_at = $21,
                         archived_at = $22,
                         updated_at = $23
                     WHERE tenant_id = $1 AND id = $2
                       AND ($24 IS NULL OR status = $24)
                     RETURNING id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(release.id)
                .bind(&release.version)
                .bind(&release.domain_scope)
                .bind(release.source_run_id)
                .bind(release.parent_release_id)
                .bind(release.rollback_target_release_id)
                .bind(&release.status)
                .bind(&release.release_class)
                .bind(release.object_count)
                .bind(release.relation_count)
                .bind(release.action_count)
                .bind(&release.migration_policy)
                .bind(&release.gate_result)
                .bind(&release.materialized_object_ids)
                .bind(&release.materialized_link_ids)
                .bind(&release.evidence_refs)
                .bind(&release.promoted_by)
                .bind(release.promoted_at)
                .bind(&release.rolled_back_by)
                .bind(release.rolled_back_at)
                .bind(release.archived_at)
                .bind(release.updated_at)
                .bind(expected_status)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("ontology release status conflict: concurrent update detected"))?;
                ontology_release_from_row(row)
            }
        }
    }

    pub(crate) async fn promote_ontology_release_atomically(
        &self,
        release_id: Uuid,
        actor_subject: &str,
    ) -> Result<(OntologyRelease, Option<OntologyRelease>), AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .ontology_releases
                    .get(&release_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                validate_ontology_release_promotable(&release)?;
                let mut active_releases = store
                    .ontology_releases
                    .values()
                    .filter(|candidate| {
                        candidate.status == "active"
                            && candidate
                                .domain_scope
                                .eq_ignore_ascii_case(&release.domain_scope)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                active_releases
                    .sort_by_key(|candidate| candidate.promoted_at.or(Some(candidate.created_at)));
                let previous_active = active_releases.pop();
                for active in active_releases
                    .iter()
                    .chain(previous_active.iter())
                    .map(|active| active.id)
                    .collect::<Vec<_>>()
                {
                    if let Some(existing) = store.ontology_releases.get_mut(&active) {
                        existing.status = "superseded".to_string();
                        existing.updated_at = now;
                    }
                }
                let promoted = store
                    .ontology_releases
                    .get_mut(&release_id)
                    .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                promoted.rollback_target_release_id =
                    previous_active.as_ref().map(|active| active.id);
                promoted.parent_release_id = previous_active.as_ref().map(|active| active.id);
                promoted.status = "active".to_string();
                promoted.promoted_by = Some(actor_subject.to_string());
                promoted.promoted_at = Some(now);
                promoted.updated_at = now;
                Ok((promoted.clone(), previous_active))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let row = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(release_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                let release = ontology_release_from_row(row)?;
                validate_ontology_release_promotable(&release)?;
                let active_rows = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND lower(domain_scope) = lower($2) AND status = 'active'
                     ORDER BY promoted_at DESC NULLS LAST, created_at DESC
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(&release.domain_scope)
                .fetch_all(&mut *tx)
                .await?;
                let active_releases = active_rows
                    .into_iter()
                    .map(ontology_release_from_row)
                    .collect::<Result<Vec<_>, _>>()?;
                let previous_active = active_releases.first().cloned();
                sqlx::query(
                    "UPDATE ontology_releases
                     SET status = 'superseded', updated_at = $3
                     WHERE tenant_id = $1 AND lower(domain_scope) = lower($2) AND status = 'active'",
                )
                .bind(self.current_tenant_id())
                .bind(&release.domain_scope)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                let row = sqlx::query(
                    "UPDATE ontology_releases
                     SET status = 'active',
                         parent_release_id = $3,
                         rollback_target_release_id = $4,
                         promoted_by = $5,
                         promoted_at = $6,
                         updated_at = $7
                     WHERE tenant_id = $1 AND id = $2 AND status = 'candidate'
                     RETURNING id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(release_id)
                .bind(previous_active.as_ref().map(|active| active.id))
                .bind(previous_active.as_ref().map(|active| active.id))
                .bind(actor_subject)
                .bind(now)
                .bind(now)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::bad_request("ontology release status conflict: concurrent update detected"))?;
                let promoted = ontology_release_from_row(row)?;
                tx.commit().await?;
                Ok((promoted, previous_active))
            }
        }
    }

    pub(crate) async fn rollback_ontology_release_atomically(
        &self,
        release_id: Uuid,
        actor_subject: &str,
    ) -> Result<(OntologyRelease, OntologyRelease), AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let release = store
                    .ontology_releases
                    .get(&release_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                let target_id = validate_ontology_release_rollback_source(&release)?;
                let target = store
                    .ontology_releases
                    .get(&target_id)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::not_found("ontology release rollback target not found")
                    })?;
                validate_ontology_release_rollback_target(&release, &target)?;
                if let Some(current) = store.ontology_releases.get_mut(&release_id) {
                    current.status = "rolled_back".to_string();
                    current.rolled_back_by = Some(actor_subject.to_string());
                    current.rolled_back_at = Some(now);
                    current.updated_at = now;
                }
                let restored = store.ontology_releases.get_mut(&target_id).ok_or_else(|| {
                    AppError::not_found("ontology release rollback target not found")
                })?;
                restored.status = "active".to_string();
                restored.rolled_back_by = None;
                restored.rolled_back_at = None;
                restored.updated_at = now;
                Ok((release, restored.clone()))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let row = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(release_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                let release = ontology_release_from_row(row)?;
                let target_id = validate_ontology_release_rollback_source(&release)?;
                let row = sqlx::query(
                    "SELECT id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at
                     FROM ontology_releases
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(target_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release rollback target not found"))?;
                let target = ontology_release_from_row(row)?;
                validate_ontology_release_rollback_target(&release, &target)?;
                sqlx::query(
                    "UPDATE ontology_releases
                     SET status = 'rolled_back',
                         rolled_back_by = $3,
                         rolled_back_at = $4,
                         updated_at = $5
                     WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
                )
                .bind(self.current_tenant_id())
                .bind(release_id)
                .bind(actor_subject)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                let row = sqlx::query(
                    "UPDATE ontology_releases
                     SET status = 'active',
                         rolled_back_by = NULL,
                         rolled_back_at = NULL,
                         updated_at = $3
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, version, domain_scope, source_run_id, parent_release_id, rollback_target_release_id, status, release_class, object_count, relation_count, action_count, migration_policy, gate_result, materialized_object_ids, materialized_link_ids, evidence_refs, promoted_by, promoted_at, rolled_back_by, rolled_back_at, archived_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(target_id)
                .bind(now)
                .fetch_one(&mut *tx)
                .await?;
                let restored = ontology_release_from_row(row)?;
                tx.commit().await?;
                Ok((release, restored))
            }
        }
    }
}

fn validate_ontology_release_promotable(release: &OntologyRelease) -> Result<(), AppError> {
    if release.status != "candidate" {
        return Err(AppError::bad_request(
            "only candidate ontology releases can be promoted",
        ));
    }
    if release
        .gate_result
        .get("status")
        .and_then(serde_json::Value::as_str)
        != Some("passed")
    {
        return Err(AppError::bad_request(
            "ontology release promotion requires a passed gate",
        ));
    }
    Ok(())
}

fn validate_ontology_release_rollback_source(release: &OntologyRelease) -> Result<Uuid, AppError> {
    if release.status != "active" {
        return Err(AppError::bad_request(
            "only active ontology releases can be rolled back",
        ));
    }
    release
        .rollback_target_release_id
        .ok_or_else(|| AppError::bad_request("ontology release rollback target is missing"))
}

fn validate_ontology_release_rollback_target(
    release: &OntologyRelease,
    target: &OntologyRelease,
) -> Result<(), AppError> {
    if !target
        .domain_scope
        .eq_ignore_ascii_case(&release.domain_scope)
    {
        return Err(AppError::bad_request(
            "ontology release rollback target must share domain_scope",
        ));
    }
    if target.archived_at.is_some() || target.status == "archived" {
        return Err(AppError::bad_request(
            "ontology release rollback target is archived",
        ));
    }
    Ok(())
}
