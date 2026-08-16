use uuid::Uuid;

use crate::{
    AppError, AppState, ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE, OntologySdkApplication,
    store_backend::StoreBackend, store_rows::ontology_sdk_application_from_row,
};

const APPLICATION_COLUMNS: &str = "id, tenant_id, subject, ontology_release_id, release_version, domain_scope, catalog_digest, subset_manifest, subset_digest, status, created_at";

impl AppState {
    pub(crate) async fn list_ontology_sdk_applications(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<OntologySdkApplication>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut applications = inner
                    .read()
                    .await
                    .ontology_sdk_applications
                    .values()
                    .filter(|application| {
                        subject.is_none_or(|subject| application.subject == subject)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                applications.sort_by_key(|application| application.created_at);
                Ok(applications)
            }
            StoreBackend::Postgres(pool) => {
                let query = format!(
                    "SELECT {APPLICATION_COLUMNS} FROM ontology_sdk_applications
                     WHERE tenant_id = $1
                       AND ($2::text IS NULL OR subject = $2)
                     ORDER BY created_at ASC"
                );
                let rows = sqlx::query(&query)
                    .bind(self.current_tenant_id())
                    .bind(subject.map(str::trim).filter(|subject| !subject.is_empty()))
                    .fetch_all(pool)
                    .await?;
                rows.into_iter()
                    .map(ontology_sdk_application_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_ontology_sdk_application(
        &self,
        id: Uuid,
    ) -> Result<OntologySdkApplication, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .ontology_sdk_applications
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("ontology SDK application not found")),
            StoreBackend::Postgres(pool) => {
                let query = format!(
                    "SELECT {APPLICATION_COLUMNS} FROM ontology_sdk_applications
                     WHERE tenant_id = $1 AND id = $2"
                );
                let row = sqlx::query(&query)
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("ontology SDK application not found"))?;
                ontology_sdk_application_from_row(row)
            }
        }
    }

    pub(crate) async fn create_ontology_sdk_application(
        &self,
        application: OntologySdkApplication,
    ) -> Result<OntologySdkApplication, AppError> {
        if application.tenant_id != self.current_tenant_id() {
            return Err(AppError::forbidden(
                "ontology SDK application tenant does not match the request",
            ));
        }
        if application.status != ONTOLOGY_SDK_APPLICATION_STATUS_ACTIVE {
            return Err(AppError::bad_request(
                "ontology SDK application status must be active",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.ontology_sdk_applications.values().any(|existing| {
                    existing.tenant_id == application.tenant_id
                        && existing.subject == application.subject
                        && existing.ontology_release_id == application.ontology_release_id
                        && existing.subset_digest == application.subset_digest
                }) {
                    return Err(AppError::conflict(
                        "ontology SDK application with the same immutable manifest already exists",
                    ));
                }
                store
                    .ontology_sdk_applications
                    .insert(application.id, application.clone());
                Ok(application)
            }
            StoreBackend::Postgres(pool) => {
                let query = format!(
                    "INSERT INTO ontology_sdk_applications
                        ({APPLICATION_COLUMNS})
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                     RETURNING {APPLICATION_COLUMNS}"
                );
                let subset_manifest = serde_json::to_value(&application.subset_manifest)?;
                let row = sqlx::query(&query)
                    .bind(application.id)
                    .bind(application.tenant_id)
                    .bind(&application.subject)
                    .bind(application.ontology_release_id)
                    .bind(&application.release_version)
                    .bind(&application.domain_scope)
                    .bind(&application.catalog_digest)
                    .bind(subset_manifest)
                    .bind(&application.subset_digest)
                    .bind(&application.status)
                    .bind(application.created_at)
                    .fetch_one(pool)
                    .await
                    .map_err(ontology_sdk_application_write_error)?;
                ontology_sdk_application_from_row(row)
            }
        }
    }
}

fn ontology_sdk_application_write_error(error: sqlx::Error) -> AppError {
    let sqlx::Error::Database(database_error) = &error else {
        return error.into();
    };
    if database_error.code().as_deref() == Some("23505") {
        return AppError::conflict(
            "ontology SDK application with the same immutable manifest already exists",
        );
    }
    error.into()
}
