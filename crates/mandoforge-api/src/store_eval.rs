use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{eval_case_from_row, eval_dataset_from_row, eval_run_from_row};
use crate::{
    AppError, AppState, CreateEvalCase, CreateEvalDataset, CreateEvalRun, EvalCase, EvalDataset,
    EvalRun,
};

impl AppState {
    pub(crate) async fn list_eval_datasets(&self) -> Result<Vec<EvalDataset>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut datasets: Vec<_> =
                    inner.read().await.eval_datasets.values().cloned().collect();
                datasets.sort_by_key(|dataset| dataset.created_at);
                datasets.reverse();
                Ok(datasets)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, description, created_at
                     FROM eval_datasets
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(eval_dataset_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_dataset(
        &self,
        input: CreateEvalDataset,
    ) -> Result<EvalDataset, AppError> {
        let dataset = EvalDataset {
            id: Uuid::new_v4(),
            name: input.name,
            description: input.description,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .eval_datasets
                    .insert(dataset.id, dataset.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_datasets (id, tenant_id, name, description, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(dataset.id)
                .bind(self.tenant_id)
                .bind(&dataset.name)
                .bind(&dataset.description)
                .bind(dataset.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(dataset)
    }

    pub(crate) async fn list_eval_cases(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvalCase>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut cases: Vec<_> = inner
                    .read()
                    .await
                    .eval_cases
                    .values()
                    .filter(|case| case.dataset_id == dataset_id)
                    .cloned()
                    .collect();
                cases.sort_by_key(|case| case.created_at);
                cases.reverse();
                Ok(cases)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, dataset_id, input, expected, grading_policy, created_at
                     FROM eval_cases
                     WHERE tenant_id = $1 AND dataset_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(dataset_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(eval_case_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_case(
        &self,
        dataset_id: Uuid,
        input: CreateEvalCase,
    ) -> Result<EvalCase, AppError> {
        self.ensure_eval_dataset_exists(dataset_id).await?;
        let case = EvalCase {
            id: Uuid::new_v4(),
            dataset_id,
            input: input.input,
            expected: input.expected,
            grading_policy: input.grading_policy,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.eval_cases.insert(case.id, case.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_cases (id, tenant_id, dataset_id, input, expected, grading_policy, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(case.id)
                .bind(self.tenant_id)
                .bind(case.dataset_id)
                .bind(&case.input)
                .bind(&case.expected)
                .bind(&case.grading_policy)
                .bind(case.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(case)
    }

    pub(crate) async fn list_eval_runs(
        &self,
        dataset_id: Option<Uuid>,
    ) -> Result<Vec<EvalRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut runs: Vec<_> = inner
                    .read()
                    .await
                    .eval_runs
                    .values()
                    .filter(|run| dataset_id.is_none_or(|id| run.dataset_id == id))
                    .cloned()
                    .collect();
                runs.sort_by_key(|run| run.created_at);
                runs.reverse();
                Ok(runs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match dataset_id {
                    Some(dataset_id) => {
                        sqlx::query(
                            "SELECT id, dataset_id, agent_id, agent_version_id, status, score, details, created_at
                             FROM eval_runs
                             WHERE tenant_id = $1 AND dataset_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(dataset_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, dataset_id, agent_id, agent_version_id, status, score, details, created_at
                             FROM eval_runs
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(eval_run_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_run(
        &self,
        dataset_id: Uuid,
        input: CreateEvalRun,
    ) -> Result<EvalRun, AppError> {
        self.ensure_eval_dataset_exists(dataset_id).await?;
        let agent_version = self.current_agent_version(input.agent_id).await?;
        let case_count = self.list_eval_cases(dataset_id).await?.len();
        let score = if case_count == 0 {
            Some(0.0)
        } else {
            Some(1.0)
        };
        let run = EvalRun {
            id: Uuid::new_v4(),
            dataset_id,
            agent_id: input.agent_id,
            agent_version_id: agent_version.id,
            status: "completed".to_string(),
            score,
            details: json!({
                "runner": "stage2-skeleton",
                "case_count": case_count,
                "coverage": ["dataset_persistence", "agent_version_binding"],
                "note": "This first runner records a version-bound eval run; scenario grading is a later Stage 2 slice."
            }),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.eval_runs.insert(run.id, run.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_runs (id, tenant_id, dataset_id, agent_id, agent_version_id, status, score, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(run.id)
                .bind(self.tenant_id)
                .bind(run.dataset_id)
                .bind(run.agent_id)
                .bind(run.agent_version_id)
                .bind(&run.status)
                .bind(run.score)
                .bind(&run.details)
                .bind(run.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(run)
    }

    async fn ensure_eval_dataset_exists(&self, dataset_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                if inner.read().await.eval_datasets.contains_key(&dataset_id) {
                    Ok(())
                } else {
                    Err(AppError::not_found("eval dataset not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1 FROM eval_datasets WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(dataset_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("eval dataset not found"))
            }
        }
    }
}
