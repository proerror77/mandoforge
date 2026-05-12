use anyhow::Result;

use crate::store_backend::StoreBackend;
use crate::{AppError, AppState, CodexAppServerRun};

impl AppState {
    pub(crate) async fn insert_codex_app_server_run(
        &self,
        run: CodexAppServerRun,
    ) -> Result<CodexAppServerRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .codex_app_server_runs
                    .insert(run.id, run.clone());
                Ok(run)
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO codex_app_server_runs
                        (id, tenant_id, operation, thread_id, turn_id, command_id, status, request, response, error, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(run.id)
                .bind(self.tenant_id)
                .bind(&run.operation)
                .bind(&run.thread_id)
                .bind(&run.turn_id)
                .bind(&run.command_id)
                .bind(&run.status)
                .bind(&run.request)
                .bind(&run.response)
                .bind(&run.error)
                .bind(run.created_at)
                .execute(pool)
                .await?;
                Ok(run)
            }
        }
    }

    pub(crate) async fn list_codex_app_server_runs(
        &self,
    ) -> Result<Vec<CodexAppServerRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut runs: Vec<_> = inner
                    .read()
                    .await
                    .codex_app_server_runs
                    .values()
                    .cloned()
                    .collect();
                runs.sort_by_key(|run| run.created_at);
                runs.reverse();
                Ok(runs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, operation, thread_id, turn_id, command_id, status, request, response, error, created_at
                     FROM codex_app_server_runs
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(|row| {
                        use sqlx::Row;
                        Ok(CodexAppServerRun {
                            id: row.try_get("id")?,
                            operation: row.try_get("operation")?,
                            thread_id: row.try_get("thread_id")?,
                            turn_id: row.try_get("turn_id")?,
                            command_id: row.try_get("command_id")?,
                            status: row.try_get("status")?,
                            request: row.try_get("request")?,
                            response: row.try_get("response")?,
                            error: row.try_get("error")?,
                            created_at: row.try_get("created_at")?,
                        })
                    })
                    .collect()
            }
        }
    }
}
