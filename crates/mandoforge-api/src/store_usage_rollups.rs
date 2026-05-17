use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppError, AppState, UsageRollup, store_backend::StoreBackend, store_rows::usage_rollup_from_row,
};

impl AppState {
    pub(crate) async fn list_usage_rollups(&self) -> Result<Vec<UsageRollup>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut rollups: Vec<_> =
                    inner.read().await.usage_rollups.values().cloned().collect();
                rollups.sort_by_key(|rollup| rollup.created_at);
                rollups.reverse();
                Ok(rollups)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, period_start, period_end, summary, created_at
                     FROM usage_rollups
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(usage_rollup_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_usage_rollup(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        summary: Value,
    ) -> Result<UsageRollup, AppError> {
        let rollup = UsageRollup {
            id: Uuid::new_v4(),
            period_start,
            period_end,
            summary,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .usage_rollups
                    .insert(rollup.id, rollup.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO usage_rollups (id, tenant_id, period_start, period_end, summary, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(rollup.id)
                .bind(self.current_tenant_id())
                .bind(rollup.period_start)
                .bind(rollup.period_end)
                .bind(&rollup.summary)
                .bind(rollup.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(rollup)
    }
}
