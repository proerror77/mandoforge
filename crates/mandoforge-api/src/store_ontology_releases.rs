use anyhow::Result;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::ontology_release_from_row;
use crate::{AppError, AppState, OntologyRelease};

impl AppState {
    pub(crate) async fn list_ontology_releases(&self) -> Result<Vec<OntologyRelease>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut releases: Vec<_> = inner
                    .read()
                    .await
                    .ontology_releases
                    .values()
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
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
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
                        "ontology release version already exists for domain {}: {}",
                        release.domain_scope, release.version
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
    ) -> Result<OntologyRelease, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store.ontology_releases.contains_key(&release.id) {
                    return Err(AppError::not_found("ontology release not found"));
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
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("ontology release not found"))?;
                ontology_release_from_row(row)
            }
        }
    }
}
