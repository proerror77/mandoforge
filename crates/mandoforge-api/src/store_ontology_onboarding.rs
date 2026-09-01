use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::ontology_onboarding_run_record_from_row;
use crate::{AppError, AppState, OntologyOnboardingRunRecord};

const ONBOARDING_RUN_COLUMNS: &str = "id, industry, source_mode, domain_scope, status, dataset_count, profile_count, proposal_count, approved_count, materialized_count, actor_subject, created_at, updated_at";

impl AppState {
    pub(crate) async fn list_ontology_onboarding_run_records(
        &self,
    ) -> Result<Vec<OntologyOnboardingRunRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut records = inner
                    .read()
                    .await
                    .ontology_onboarding_runs
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                records.sort_by_key(|record| record.created_at);
                Ok(records)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(&format!(
                    "SELECT {ONBOARDING_RUN_COLUMNS}
                     FROM ontology_onboarding_runs
                     WHERE tenant_id = $1
                     ORDER BY created_at ASC"
                ))
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(ontology_onboarding_run_record_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn find_ontology_onboarding_run_record(
        &self,
        id: Uuid,
    ) -> Result<Option<OntologyOnboardingRunRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .ontology_onboarding_runs
                .get(&id)
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(&format!(
                    "SELECT {ONBOARDING_RUN_COLUMNS}
                     FROM ontology_onboarding_runs
                     WHERE tenant_id = $1 AND id = $2"
                ))
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?;
                row.map(ontology_onboarding_run_record_from_row).transpose()
            }
        }
    }

    pub(crate) async fn create_ontology_onboarding_run_record(
        &self,
        record: OntologyOnboardingRunRecord,
    ) -> Result<OntologyOnboardingRunRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.ontology_onboarding_runs.contains_key(&record.id) {
                    return Err(AppError::bad_request(
                        "ontology onboarding run already exists",
                    ));
                }
                store
                    .ontology_onboarding_runs
                    .insert(record.id, record.clone());
                Ok(record)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(&format!(
                    "INSERT INTO ontology_onboarding_runs
                        (id, tenant_id, industry, source_mode, domain_scope, status, dataset_count, profile_count, proposal_count, approved_count, materialized_count, actor_subject, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     RETURNING {ONBOARDING_RUN_COLUMNS}"
                ))
                .bind(record.id)
                .bind(self.current_tenant_id())
                .bind(&record.industry)
                .bind(&record.source_mode)
                .bind(&record.domain_scope)
                .bind(&record.status)
                .bind(record.dataset_count)
                .bind(record.profile_count)
                .bind(record.proposal_count)
                .bind(record.approved_count)
                .bind(record.materialized_count)
                .bind(&record.actor_subject)
                .bind(record.created_at)
                .bind(record.updated_at)
                .fetch_one(pool)
                .await?;
                ontology_onboarding_run_record_from_row(row)
            }
        }
    }

    pub(crate) async fn refresh_ontology_onboarding_run_record(
        &self,
        id: Uuid,
    ) -> Result<Option<OntologyOnboardingRunRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let objects = store.semantic_objects.values().filter(|object| {
                    object.object_type == "ontology_onboarding_proposal"
                        && object.status == "active"
                        && object
                            .content
                            .get("run_id")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|value| Uuid::parse_str(value).ok())
                            == Some(id)
                });
                let mut proposal_count = 0usize;
                let mut approved_count = 0usize;
                let mut materialized_count = 0usize;
                for object in objects {
                    proposal_count += 1;
                    approved_count += usize::from(
                        object
                            .content
                            .get("review_status")
                            .and_then(serde_json::Value::as_str)
                            == Some("approved"),
                    );
                    materialized_count += usize::from(
                        object
                            .content
                            .get("materialized")
                            .and_then(serde_json::Value::as_bool)
                            == Some(true),
                    );
                }
                let Some(record) = store.ontology_onboarding_runs.get_mut(&id) else {
                    return Ok(None);
                };
                record.status = onboarding_run_status(approved_count, materialized_count);
                record.proposal_count = onboarding_run_count(proposal_count)?;
                record.approved_count = onboarding_run_count(approved_count)?;
                record.materialized_count = onboarding_run_count(materialized_count)?;
                record.updated_at = Utc::now();
                Ok(Some(record.clone()))
            }
            StoreBackend::Postgres(pool) => {
                let mut transaction = pool.begin().await?;
                sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                    .bind(format!("{}:{id}", self.current_tenant_id()))
                    .execute(&mut *transaction)
                    .await?;
                let row = sqlx::query(
                    r#"WITH proposal_counts AS (
                         SELECT
                             COUNT(*)::integer AS proposal_count,
                             COUNT(*) FILTER (WHERE content->>'review_status' = 'approved')::integer AS approved_count,
                             COUNT(*) FILTER (WHERE content @> '{"materialized":true}'::jsonb)::integer AS materialized_count
                         FROM semantic_objects
                         WHERE tenant_id = $1
                           AND object_type = 'ontology_onboarding_proposal'
                           AND status = 'active'
                           AND content->>'run_id' = $2::text
                     )
                     UPDATE ontology_onboarding_runs AS run
                     SET status = CASE
                             WHEN counts.materialized_count > 0 THEN 'materialized'
                             WHEN counts.approved_count > 0 THEN 'reviewing'
                             ELSE 'pending_review'
                         END,
                         proposal_count = counts.proposal_count,
                         approved_count = counts.approved_count,
                         materialized_count = counts.materialized_count,
                         updated_at = NOW()
                     FROM proposal_counts AS counts
                     WHERE run.tenant_id = $1 AND run.id = $2
                     RETURNING run.id, run.industry, run.source_mode, run.domain_scope, run.status,
                               run.dataset_count, run.profile_count, run.proposal_count,
                               run.approved_count, run.materialized_count, run.actor_subject,
                               run.created_at, run.updated_at"#,
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(&mut *transaction)
                .await?;
                let record = row
                    .map(ontology_onboarding_run_record_from_row)
                    .transpose()?;
                transaction.commit().await?;
                Ok(record)
            }
        }
    }
}

fn onboarding_run_status(approved_count: usize, materialized_count: usize) -> String {
    if materialized_count > 0 {
        "materialized"
    } else if approved_count > 0 {
        "reviewing"
    } else {
        "pending_review"
    }
    .to_string()
}

fn onboarding_run_count(value: usize) -> Result<i32, AppError> {
    i32::try_from(value)
        .map_err(|_| AppError::bad_request("ontology onboarding count exceeds integer range"))
}
