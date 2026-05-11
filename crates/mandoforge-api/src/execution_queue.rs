use std::{str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgRow};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppError;

#[derive(Clone)]
pub(crate) enum ExecutionQueue {
    Memory {
        inner: Arc<RwLock<ExecutionQueueState>>,
    },
    Postgres {
        pool: PgPool,
        tenant_id: Uuid,
    },
}

impl Default for ExecutionQueue {
    fn default() -> Self {
        Self::Memory {
            inner: Arc::new(RwLock::new(ExecutionQueueState::default())),
        }
    }
}

impl ExecutionQueue {
    pub(crate) fn postgres(pool: PgPool, tenant_id: Uuid) -> Self {
        Self::Postgres { pool, tenant_id }
    }
}

#[derive(Default)]
pub(crate) struct ExecutionQueueState {
    jobs: Vec<ExecutionJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ExecutionJob {
    pub(crate) id: Uuid,
    pub(crate) session_id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
    pub(crate) status: ExecutionJobStatus,
    pub(crate) enqueued_at: DateTime<Utc>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl ExecutionJobStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for ExecutionJobStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Queued,
        })
    }
}

pub(crate) struct ExecutionJobRequest {
    pub(crate) session_id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
}

fn execution_job_from_row(row: PgRow) -> Result<ExecutionJob, AppError> {
    let status: String = row.try_get("status")?;
    Ok(ExecutionJob {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        approval_id: row.try_get("approval_id")?,
        tool_call_id: row.try_get("tool_call_id")?,
        tool_name: row.try_get("tool_name")?,
        status: status.parse().unwrap_or(ExecutionJobStatus::Queued),
        enqueued_at: row.try_get("enqueued_at")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        worker_id: row.try_get("worker_id")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
    })
}

impl ExecutionQueue {
    pub(crate) async fn enqueue(
        &self,
        request: ExecutionJobRequest,
    ) -> Result<ExecutionJob, AppError> {
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            session_id: request.session_id,
            approval_id: request.approval_id,
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name,
            status: ExecutionJobStatus::Queued,
            enqueued_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            lease_expires_at: None,
        };
        match self {
            Self::Memory { inner } => {
                inner.write().await.jobs.push(job.clone());
            }
            Self::Postgres { pool, tenant_id } => {
                sqlx::query(
                    "INSERT INTO execution_jobs
                        (id, tenant_id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(job.id)
                .bind(*tenant_id)
                .bind(job.session_id)
                .bind(job.approval_id)
                .bind(job.tool_call_id)
                .bind(&job.tool_name)
                .bind(job.status.as_str())
                .bind(job.enqueued_at)
                .bind(job.started_at)
                .bind(job.completed_at)
                .bind(&job.worker_id)
                .bind(job.lease_expires_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(job)
    }

    pub(crate) async fn start(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Running, Some(worker_id))
            .await
    }

    pub(crate) async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Completed, None)
            .await
    }

    pub(crate) async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed, None).await
    }

    pub(crate) async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        match self {
            Self::Memory { inner } => Ok(inner.read().await.jobs.clone()),
            Self::Postgres { pool, tenant_id } => {
                let rows = sqlx::query(
                    "SELECT id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at
                     FROM execution_jobs
                     WHERE tenant_id = $1
                     ORDER BY enqueued_at ASC",
                )
                .bind(*tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(execution_job_from_row).collect()
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        match self {
            Self::Memory { inner } => inner
                .read()
                .await
                .jobs
                .iter()
                .find(|job| job.id == job_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("execution job not found")),
            Self::Postgres { pool, tenant_id } => {
                let row = sqlx::query(
                    "SELECT id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at
                     FROM execution_jobs
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(*tenant_id)
                .bind(job_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("execution job not found"))?;
                execution_job_from_row(row)
            }
        }
    }

    async fn update(
        &self,
        job_id: Uuid,
        status: ExecutionJobStatus,
        worker_id: Option<&str>,
    ) -> Result<ExecutionJob, AppError> {
        match self {
            Self::Memory { inner } => {
                let mut state = inner.write().await;
                let job = state
                    .jobs
                    .iter_mut()
                    .find(|job| job.id == job_id)
                    .ok_or_else(|| AppError::not_found("execution job not found"))?;
                job.status = status;
                match job.status {
                    ExecutionJobStatus::Running => {
                        job.started_at = Some(Utc::now());
                        job.worker_id = worker_id.map(str::to_string);
                        job.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
                    }
                    ExecutionJobStatus::Completed | ExecutionJobStatus::Failed => {
                        job.completed_at = Some(Utc::now());
                        job.lease_expires_at = None;
                    }
                    ExecutionJobStatus::Queued => {
                        job.started_at = None;
                        job.completed_at = None;
                        job.worker_id = None;
                        job.lease_expires_at = None;
                    }
                }
                Ok(job.clone())
            }
            Self::Postgres { pool, tenant_id } => {
                let row = match status {
                    ExecutionJobStatus::Running => sqlx::query(
                        "UPDATE execution_jobs
                         SET status = 'running', started_at = COALESCE(started_at, now()), worker_id = $1, lease_expires_at = now() + interval '5 minutes'
                         WHERE tenant_id = $2
                           AND id = $3
                           AND (status = 'queued' OR (status = 'running' AND lease_expires_at < now()))
                         RETURNING id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at",
                    )
                    .bind(worker_id.unwrap_or("api"))
                    .bind(*tenant_id)
                    .bind(job_id)
                    .fetch_optional(pool)
                    .await?,
                    ExecutionJobStatus::Completed | ExecutionJobStatus::Failed => sqlx::query(
                        "UPDATE execution_jobs
                         SET status = $1, completed_at = COALESCE(completed_at, now()), lease_expires_at = NULL
                         WHERE tenant_id = $2 AND id = $3
                         RETURNING id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at",
                    )
                    .bind(status.as_str())
                    .bind(*tenant_id)
                    .bind(job_id)
                    .fetch_optional(pool)
                    .await?,
                    ExecutionJobStatus::Queued => sqlx::query(
                        "UPDATE execution_jobs
                         SET status = 'queued', started_at = NULL, completed_at = NULL, worker_id = NULL, lease_expires_at = NULL
                         WHERE tenant_id = $1 AND id = $2
                         RETURNING id, session_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at",
                    )
                    .bind(*tenant_id)
                    .bind(job_id)
                    .fetch_optional(pool)
                    .await?,
                }
                .ok_or_else(|| AppError::not_found("execution job not found"))?;
                execution_job_from_row(row)
            }
        }
    }
}
