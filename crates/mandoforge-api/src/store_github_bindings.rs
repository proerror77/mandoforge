use anyhow::Result;
use chrono::Utc;

use crate::store_backend::StoreBackend;
use crate::{AppError, AppState, ProjectGitHubBinding};

impl AppState {
    pub(crate) async fn upsert_project_github_binding(
        &self,
        binding: ProjectGitHubBinding,
    ) -> Result<ProjectGitHubBinding, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let existing_id = store
                    .project_github_bindings
                    .values()
                    .find(|b| {
                        b.repo_full_name
                            .eq_ignore_ascii_case(&binding.repo_full_name)
                    })
                    .map(|b| b.id);
                let id = existing_id.unwrap_or(binding.id);
                let mut upserted = binding.clone();
                upserted.id = id;
                upserted.updated_at = Utc::now();
                store.project_github_bindings.insert(id, upserted.clone());
                Ok(upserted)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO project_github_bindings
                        (id, tenant_id, repo_full_name, pack_installation_id, webhook_secret_ref, active, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     ON CONFLICT (tenant_id, repo_full_name)
                     DO UPDATE SET
                         pack_installation_id = EXCLUDED.pack_installation_id,
                         webhook_secret_ref = EXCLUDED.webhook_secret_ref,
                         active = EXCLUDED.active,
                         updated_at = EXCLUDED.updated_at
                     RETURNING id, repo_full_name, pack_installation_id, webhook_secret_ref, active, created_at, updated_at",
                )
                .bind(binding.id)
                .bind(self.current_tenant_id())
                .bind(&binding.repo_full_name)
                .bind(binding.pack_installation_id)
                .bind(&binding.webhook_secret_ref)
                .bind(binding.active)
                .bind(binding.created_at)
                .bind(binding.updated_at)
                .fetch_one(pool)
                .await?;
                project_github_binding_from_row(row)
            }
        }
    }

    pub(crate) async fn get_project_github_binding_by_repo(
        &self,
        repo_full_name: &str,
    ) -> Result<ProjectGitHubBinding, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .project_github_bindings
                .values()
                .find(|b| b.active && b.repo_full_name.eq_ignore_ascii_case(repo_full_name))
                .cloned()
                .ok_or_else(|| {
                    AppError::not_found(format!(
                        "no active github binding for repo: {repo_full_name}"
                    ))
                }),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, repo_full_name, pack_installation_id, webhook_secret_ref, active, created_at, updated_at
                     FROM project_github_bindings
                     WHERE tenant_id = $1 AND lower(repo_full_name) = lower($2) AND active = TRUE",
                )
                .bind(self.current_tenant_id())
                .bind(repo_full_name)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::not_found(format!(
                        "no active github binding for repo: {repo_full_name}"
                    ))
                })?;
                project_github_binding_from_row(row)
            }
        }
    }

    pub(crate) async fn list_project_github_bindings(
        &self,
    ) -> Result<Vec<ProjectGitHubBinding>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut bindings: Vec<_> = inner
                    .read()
                    .await
                    .project_github_bindings
                    .values()
                    .cloned()
                    .collect();
                bindings.sort_by_key(|b| b.created_at);
                Ok(bindings)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, repo_full_name, pack_installation_id, webhook_secret_ref, active, created_at, updated_at
                     FROM project_github_bindings
                     WHERE tenant_id = $1
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(project_github_binding_from_row)
                    .collect()
            }
        }
    }
}

fn project_github_binding_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<ProjectGitHubBinding, AppError> {
    use sqlx::Row;
    Ok(ProjectGitHubBinding {
        id: row.try_get("id")?,
        repo_full_name: row.try_get("repo_full_name")?,
        pack_installation_id: row.try_get("pack_installation_id")?,
        webhook_secret_ref: row.try_get("webhook_secret_ref")?,
        active: row.try_get("active")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
