use anyhow::Result;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::artifact_from_row;
use crate::{AppError, AppState, Artifact};

impl AppState {
    pub(crate) async fn insert_artifact(&self, artifact: Artifact) -> Result<Artifact, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .artifacts
                    .insert(artifact.id, artifact.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO artifacts (id, tenant_id, session_id, artifact_type, name, path, content, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(artifact.id)
                .bind(self.tenant_id)
                .bind(artifact.session_id)
                .bind(&artifact.artifact_type)
                .bind(&artifact.name)
                .bind(&artifact.path)
                .bind(&artifact.content)
                .bind(artifact.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(artifact)
    }

    pub(crate) async fn list_artifacts(&self, session_id: Uuid) -> Result<Vec<Artifact>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .artifacts
                .values()
                .filter(|artifact| artifact.session_id == session_id)
                .cloned()
                .collect()),
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, artifact_type, name, path, content, created_at
                     FROM artifacts
                     WHERE tenant_id = $1 AND session_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(session_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(artifact_from_row).collect()
            }
        }
    }
}
