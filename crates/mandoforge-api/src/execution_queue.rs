use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgRow};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppError;

#[derive(Clone)]
pub(crate) struct ExecutionQueue {
    backend: Arc<dyn ExecutionQueueBackend>,
}

impl Default for ExecutionQueue {
    fn default() -> Self {
        Self {
            backend: Arc::new(MemoryExecutionQueue::default()),
        }
    }
}

impl ExecutionQueue {
    pub(crate) fn backend_kind(&self) -> &'static str {
        self.backend.backend_kind()
    }

    pub(crate) fn postgres(pool: PgPool, tenant_id: Uuid) -> Self {
        Self {
            backend: Arc::new(PostgresExecutionQueue { pool, tenant_id }),
        }
    }

    pub(crate) fn broker(backend: Arc<dyn ExecutionQueueBackend>) -> Self {
        Self { backend }
    }

    pub(crate) async fn enqueue(
        &self,
        request: ExecutionJobRequest,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.enqueue(request).await
    }

    pub(crate) async fn start(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.start(job_id, worker_id).await
    }

    #[allow(dead_code)]
    pub(crate) async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.complete(job_id).await
    }

    pub(crate) async fn complete_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.complete_started(job_id, worker_id).await
    }

    #[allow(dead_code)]
    pub(crate) async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.fail(job_id).await
    }

    pub(crate) async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.cancel(job_id).await
    }

    #[allow(dead_code)]
    pub(crate) async fn retry_or_fail(
        &self,
        job_id: Uuid,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.retry_or_fail(job_id, error).await
    }

    pub(crate) async fn retry_or_fail_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .retry_or_fail_started(job_id, worker_id, error)
            .await
    }

    pub(crate) async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        self.backend.list().await
    }

    #[allow(dead_code)]
    pub(crate) async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.get(job_id).await
    }
}

#[async_trait]
pub(crate) trait ExecutionQueueBackend: Send + Sync {
    fn backend_kind(&self) -> &'static str;

    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError>;

    async fn start(&self, job_id: Uuid, worker_id: &str) -> Result<ExecutionJob, AppError>;

    async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn complete_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError>;

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn retry_or_fail(&self, job_id: Uuid, error: &str) -> Result<ExecutionJob, AppError>;

    async fn retry_or_fail_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<ExecutionJob, AppError>;

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError>;

    async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;
}

#[derive(Default)]
struct MemoryExecutionQueue {
    inner: Arc<RwLock<ExecutionQueueState>>,
}

struct PostgresExecutionQueue {
    pool: PgPool,
    tenant_id: Uuid,
}

impl PostgresExecutionQueue {
    fn current_tenant_id(&self) -> Uuid {
        crate::current_request_tenant_id(self.tenant_id)
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
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
    pub(crate) status: ExecutionJobStatus,
    pub(crate) enqueued_at: DateTime<Utc>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) worker_id: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) attempt_count: i32,
    pub(crate) max_attempts: i32,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

impl ExecutionJobStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
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
            "canceled" | "cancelled" => Self::Canceled,
            _ => Self::Queued,
        })
    }
}

pub(crate) struct ExecutionJobRequest {
    pub(crate) session_id: Uuid,
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
    pub(crate) max_attempts: Option<i32>,
}

fn new_execution_job(request: ExecutionJobRequest) -> ExecutionJob {
    let max_attempts = request
        .max_attempts
        .unwrap_or_else(default_execution_job_max_attempts)
        .clamp(1, 10);
    ExecutionJob {
        id: Uuid::new_v4(),
        session_id: request.session_id,
        environment_id: request.environment_id,
        approval_id: request.approval_id,
        tool_call_id: request.tool_call_id,
        tool_name: request.tool_name,
        status: ExecutionJobStatus::Queued,
        enqueued_at: Utc::now(),
        started_at: None,
        completed_at: None,
        worker_id: None,
        lease_expires_at: None,
        attempt_count: 0,
        max_attempts,
        last_error: None,
    }
}

fn default_execution_job_max_attempts() -> i32 {
    std::env::var("MANDOFORGE_EXECUTION_JOB_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(3)
}

fn execution_job_from_row(row: PgRow) -> Result<ExecutionJob, AppError> {
    let status: String = row.try_get("status")?;
    Ok(ExecutionJob {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        environment_id: row.try_get("environment_id").unwrap_or(None),
        approval_id: row.try_get("approval_id")?,
        tool_call_id: row.try_get("tool_call_id")?,
        tool_name: row.try_get("tool_name")?,
        status: status.parse().unwrap_or(ExecutionJobStatus::Queued),
        enqueued_at: row.try_get("enqueued_at")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        worker_id: row.try_get("worker_id")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        attempt_count: row.try_get("attempt_count")?,
        max_attempts: row.try_get("max_attempts")?,
        last_error: row.try_get("last_error")?,
    })
}

#[async_trait]
impl ExecutionQueueBackend for MemoryExecutionQueue {
    fn backend_kind(&self) -> &'static str {
        "memory"
    }

    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError> {
        let job = new_execution_job(request);
        self.inner.write().await.jobs.push(job.clone());
        Ok(job)
    }

    async fn start(&self, job_id: Uuid, worker_id: &str) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Running, Some(worker_id))
            .await
    }

    async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Completed, None)
            .await
    }

    async fn complete_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.update_started(job_id, worker_id, ExecutionJobStatus::Completed, None)
            .await
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed, None).await
    }

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Canceled, None)
            .await
    }

    async fn retry_or_fail(&self, job_id: Uuid, error: &str) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| job.id == job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.last_error = Some(error.to_string());
        if job.attempt_count < job.max_attempts {
            job.status = ExecutionJobStatus::Queued;
            job.started_at = None;
            job.completed_at = None;
            job.worker_id = None;
            job.lease_expires_at = None;
        } else {
            job.status = ExecutionJobStatus::Failed;
            job.completed_at = Some(Utc::now());
            job.lease_expires_at = None;
        }
        Ok(job.clone())
    }

    async fn retry_or_fail_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.update_started(
            job_id,
            worker_id,
            ExecutionJobStatus::Queued,
            Some(error.to_string()),
        )
        .await
    }

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        Ok(self.inner.read().await.jobs.clone())
    }

    async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.inner
            .read()
            .await
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("execution job not found"))
    }
}

impl MemoryExecutionQueue {
    async fn update(
        &self,
        job_id: Uuid,
        status: ExecutionJobStatus,
        worker_id: Option<&str>,
    ) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
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
                job.attempt_count += 1;
            }
            ExecutionJobStatus::Completed
            | ExecutionJobStatus::Failed
            | ExecutionJobStatus::Canceled => {
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

    async fn update_started(
        &self,
        job_id: Uuid,
        expected_worker_id: &str,
        status: ExecutionJobStatus,
        last_error: Option<String>,
    ) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| job.id == job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        if job.status != ExecutionJobStatus::Running
            || job.worker_id.as_deref() != Some(expected_worker_id)
        {
            return Err(AppError::not_found("execution job not found"));
        }
        if let Some(last_error) = last_error {
            job.last_error = Some(last_error);
        }
        match status {
            ExecutionJobStatus::Completed => {
                job.status = ExecutionJobStatus::Completed;
                job.completed_at = Some(Utc::now());
                job.lease_expires_at = None;
            }
            ExecutionJobStatus::Queued => {
                if job.attempt_count < job.max_attempts {
                    job.status = ExecutionJobStatus::Queued;
                    job.started_at = None;
                    job.completed_at = None;
                    job.worker_id = None;
                    job.lease_expires_at = None;
                } else {
                    job.status = ExecutionJobStatus::Failed;
                    job.completed_at = Some(Utc::now());
                    job.lease_expires_at = None;
                }
            }
            ExecutionJobStatus::Failed => {
                job.status = ExecutionJobStatus::Failed;
                job.completed_at = Some(Utc::now());
                job.lease_expires_at = None;
            }
            ExecutionJobStatus::Canceled | ExecutionJobStatus::Running => {
                return Err(AppError::bad_request("unsupported started job transition"));
            }
        }
        Ok(job.clone())
    }
}

#[async_trait]
impl ExecutionQueueBackend for PostgresExecutionQueue {
    fn backend_kind(&self) -> &'static str {
        "postgres"
    }

    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError> {
        let job = new_execution_job(request);
        sqlx::query(
            "INSERT INTO execution_jobs
                (id, tenant_id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
        )
        .bind(job.id)
        .bind(self.current_tenant_id())
        .bind(job.session_id)
        .bind(job.environment_id)
        .bind(job.approval_id)
        .bind(job.tool_call_id)
        .bind(&job.tool_name)
        .bind(job.status.as_str())
        .bind(job.enqueued_at)
        .bind(job.started_at)
        .bind(job.completed_at)
        .bind(&job.worker_id)
        .bind(job.lease_expires_at)
        .bind(job.attempt_count)
        .bind(job.max_attempts)
        .bind(&job.last_error)
        .execute(&self.pool)
        .await?;
        Ok(job)
    }

    async fn start(&self, job_id: Uuid, worker_id: &str) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Running, Some(worker_id))
            .await
    }

    async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Completed, None)
            .await
    }

    async fn complete_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.update_started(job_id, worker_id, ExecutionJobStatus::Completed, None)
            .await
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed, None).await
    }

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Canceled, None)
            .await
    }

    async fn retry_or_fail(&self, job_id: Uuid, error: &str) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = CASE WHEN attempt_count < max_attempts THEN 'queued' ELSE 'failed' END,
                 started_at = CASE WHEN attempt_count < max_attempts THEN NULL ELSE started_at END,
                 completed_at = CASE WHEN attempt_count < max_attempts THEN NULL ELSE COALESCE(completed_at, now()) END,
                 worker_id = CASE WHEN attempt_count < max_attempts THEN NULL ELSE worker_id END,
                 lease_expires_at = NULL,
                 last_error = $1
             WHERE tenant_id = $2 AND id = $3
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn retry_or_fail_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = CASE WHEN attempt_count < max_attempts THEN 'queued' ELSE 'failed' END,
                 started_at = CASE WHEN attempt_count < max_attempts THEN NULL ELSE started_at END,
                 completed_at = CASE WHEN attempt_count < max_attempts THEN NULL ELSE COALESCE(completed_at, now()) END,
                 worker_id = CASE WHEN attempt_count < max_attempts THEN NULL ELSE worker_id END,
                 lease_expires_at = NULL,
                 last_error = $1
             WHERE tenant_id = $2 AND id = $3 AND status = 'running' AND worker_id = $4
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        let rows = sqlx::query(
            "SELECT id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error
             FROM execution_jobs
             WHERE tenant_id = $1
             ORDER BY enqueued_at ASC",
        )
        .bind(self.current_tenant_id())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(execution_job_from_row).collect()
    }

    async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "SELECT id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error
             FROM execution_jobs
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }
}

impl PostgresExecutionQueue {
    async fn update(
        &self,
        job_id: Uuid,
        status: ExecutionJobStatus,
        worker_id: Option<&str>,
    ) -> Result<ExecutionJob, AppError> {
        let row = match status {
            ExecutionJobStatus::Running => sqlx::query(
                "UPDATE execution_jobs
                 SET status = 'running', started_at = COALESCE(started_at, now()), completed_at = NULL, worker_id = $1, lease_expires_at = now() + interval '5 minutes', attempt_count = attempt_count + 1
                 WHERE tenant_id = $2
                   AND id = $3
                   AND (status = 'queued' OR (status = 'running' AND lease_expires_at < now()))
                 RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
            )
            .bind(worker_id.unwrap_or("api"))
            .bind(self.current_tenant_id())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?,
            ExecutionJobStatus::Completed | ExecutionJobStatus::Failed | ExecutionJobStatus::Canceled => sqlx::query(
                "UPDATE execution_jobs
                 SET status = $1, completed_at = COALESCE(completed_at, now()), lease_expires_at = NULL
                 WHERE tenant_id = $2 AND id = $3
                 RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
            )
            .bind(status.as_str())
            .bind(self.current_tenant_id())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?,
            ExecutionJobStatus::Queued => sqlx::query(
                "UPDATE execution_jobs
                 SET status = 'queued', started_at = NULL, completed_at = NULL, worker_id = NULL, lease_expires_at = NULL
                 WHERE tenant_id = $1 AND id = $2
                 RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
            )
            .bind(self.current_tenant_id())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?,
        }
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn update_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        status: ExecutionJobStatus,
        last_error: Option<String>,
    ) -> Result<ExecutionJob, AppError> {
        let row = match status {
            ExecutionJobStatus::Completed | ExecutionJobStatus::Failed => sqlx::query(
                "UPDATE execution_jobs
                 SET status = $1, completed_at = COALESCE(completed_at, now()), lease_expires_at = NULL, last_error = COALESCE($2, last_error)
                 WHERE tenant_id = $3 AND id = $4 AND status = 'running' AND worker_id = $5
                 RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, attempt_count, max_attempts, last_error",
            )
            .bind(status.as_str())
            .bind(last_error)
            .bind(self.current_tenant_id())
            .bind(job_id)
            .bind(worker_id)
            .fetch_optional(&self.pool)
            .await?,
            ExecutionJobStatus::Queued | ExecutionJobStatus::Canceled | ExecutionJobStatus::Running => {
                return Err(AppError::bad_request("unsupported started job transition"));
            }
        }
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }
}
