use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::secret_record_from_row;
use crate::{AppError, AppState, CreateSecretRecord, RotateSecretRecord, SecretRecord};

impl AppState {
    pub(crate) async fn list_secret_records(&self) -> Result<Vec<SecretRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut records: Vec<_> = inner
                    .read()
                    .await
                    .secret_records
                    .values()
                    .cloned()
                    .collect();
                records.sort_by_key(|record| record.created_at);
                records.reverse();
                Ok(records)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, path, key, scope_type, scope_id, status, version, created_at, updated_at
                     FROM secret_records
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(secret_record_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_secret_record(
        &self,
        input: CreateSecretRecord,
    ) -> Result<SecretRecord, AppError> {
        let now = Utc::now();
        let record = SecretRecord {
            id: Uuid::new_v4(),
            name: input.name,
            path: input.path,
            key: input.key,
            scope_type: input.scope_type,
            scope_id: input.scope_id,
            status: "active".to_string(),
            version: 1,
            created_at: now,
            updated_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store
                    .secret_records
                    .values()
                    .any(|existing| existing.name == record.name)
                {
                    return Err(AppError::bad_request("secret record name already exists"));
                }
                store.secret_records.insert(record.id, record.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO secret_records (id, tenant_id, name, path, key, scope_type, scope_id, status, version, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(record.id)
                .bind(self.tenant_id)
                .bind(&record.name)
                .bind(&record.path)
                .bind(&record.key)
                .bind(&record.scope_type)
                .bind(record.scope_id)
                .bind(&record.status)
                .bind(record.version)
                .bind(record.created_at)
                .bind(record.updated_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(record)
    }

    pub(crate) async fn rotate_secret_record(
        &self,
        id: Uuid,
        input: RotateSecretRecord,
    ) -> Result<SecretRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let record = store
                    .secret_records
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("secret record not found"))?;
                record.path = input.path;
                record.key = input.key;
                record.version += 1;
                record.updated_at = Utc::now();
                Ok(record.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE secret_records
                     SET path = $3, key = $4, version = version + 1, updated_at = $5
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, name, path, key, scope_type, scope_id, status, version, created_at, updated_at",
                )
                .bind(self.tenant_id)
                .bind(id)
                .bind(&input.path)
                .bind(&input.key)
                .bind(Utc::now())
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("secret record not found"))?;
                secret_record_from_row(row)
            }
        }
    }
}
