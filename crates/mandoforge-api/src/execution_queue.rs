use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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

    /// Returns the Postgres LISTEN channel name for this queue, if the backend
    /// supports push notifications. Workers subscribe to this channel to avoid
    /// polling — they are woken immediately when a job is enqueued.
    pub(crate) fn notify_channel(&self) -> Option<String> {
        self.backend.notify_channel()
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

    pub(crate) async fn begin_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .begin_finalizing_started(
                job_id,
                worker_id,
                claim_generation,
                error,
                finalization_details,
            )
            .await
    }

    pub(crate) async fn begin_executing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .begin_executing_started(job_id, worker_id, claim_generation)
            .await
    }

    pub(crate) async fn resume_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.resume_finalizing(job_id, worker_id).await
    }

    pub(crate) async fn finish_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        retryable_failure: bool,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .finish_finalizing_started(job_id, worker_id, claim_generation, retryable_failure)
            .await
    }

    pub(crate) async fn set_finalizing_failure(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .set_finalizing_failure(
                job_id,
                worker_id,
                claim_generation,
                error,
                finalization_details,
            )
            .await
    }

    pub(crate) async fn mark_outcome_unknown_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .mark_outcome_unknown_finalizing(job_id, worker_id, claim_generation, error)
            .await
    }

    pub(crate) async fn prepare_completion_tail(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .prepare_completion_tail(job_id, worker_id, claim_generation)
            .await
    }

    pub(crate) async fn mark_completion_published(
        &self,
        job_id: Uuid,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.mark_completion_published(job_id).await
    }

    pub(crate) async fn recover_expired_executing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .recover_expired_executing(job_id, worker_id, error, finalization_details)
            .await
    }

    pub(crate) async fn mark_outcome_unknown_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .mark_outcome_unknown_started(job_id, worker_id, claim_generation, error)
            .await
    }

    pub(crate) async fn renew_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        lease_seconds: i64,
    ) -> Result<ExecutionJob, AppError> {
        validate_execution_lease_seconds(lease_seconds)?;
        self.backend
            .renew_started(job_id, worker_id, claim_generation, lease_seconds)
            .await
    }

    #[allow(dead_code)]
    pub(crate) async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.fail(job_id).await
    }

    pub(crate) async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.cancel(job_id).await
    }

    pub(crate) async fn acknowledge_canceled(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .acknowledge_canceled(job_id, worker_id, claim_generation)
            .await
    }

    pub(crate) async fn begin_cancellation_cleanup(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        self.backend
            .begin_cancellation_cleanup(job_id, worker_id, claim_generation)
            .await
    }

    pub(crate) async fn claim_cancellation(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.claim_cancellation(job_id, worker_id).await
    }

    #[allow(dead_code)]
    pub(crate) async fn retry_or_fail(
        &self,
        job_id: Uuid,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        self.backend.retry_or_fail(job_id, error).await
    }

    pub(crate) async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        self.backend.list().await
    }

    #[allow(dead_code)]
    pub(crate) async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.backend.get(job_id).await
    }

    pub(crate) async fn lock_owned_claim(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        status: ExecutionJobStatus,
    ) -> Result<Box<dyn ExecutionClaimGuard>, AppError> {
        self.backend
            .lock_owned_claim(job_id, worker_id, claim_generation, status)
            .await
    }
}

pub(crate) trait ExecutionClaimGuard: Send + Sync {}

impl<T: Send + Sync> ExecutionClaimGuard for T {}

#[async_trait]
pub(crate) trait ExecutionQueueBackend: Send + Sync {
    fn backend_kind(&self) -> &'static str;

    fn notify_channel(&self) -> Option<String> {
        None
    }

    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError>;

    async fn start(&self, job_id: Uuid, worker_id: &str) -> Result<ExecutionJob, AppError>;

    async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn begin_executing_started(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support committed execution",
        ))
    }

    async fn begin_finalizing_started(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _error: Option<&str>,
        _finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support durable finalization",
        ))
    }

    async fn resume_finalizing(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support durable finalization",
        ))
    }

    async fn recover_expired_executing(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _error: Option<&str>,
        _finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support durable outcome recovery",
        ))
    }

    async fn finish_finalizing_started(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _retryable_failure: bool,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support durable finalization",
        ))
    }

    async fn set_finalizing_failure(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _error: &str,
        _finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support outcome reconciliation",
        ))
    }

    async fn mark_outcome_unknown_finalizing(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _error: &str,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support outcome reconciliation",
        ))
    }

    async fn prepare_completion_tail(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support completion publication",
        ))
    }

    async fn mark_completion_published(&self, _job_id: Uuid) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support completion publication",
        ))
    }

    async fn mark_outcome_unknown_started(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _error: &str,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support uncertain outcome recording",
        ))
    }

    async fn renew_started(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _lease_seconds: i64,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support lease renewal",
        ))
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn acknowledge_canceled(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support cancellation acknowledgement",
        ))
    }

    async fn begin_cancellation_cleanup(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support cancellation cleanup claims",
        ))
    }

    async fn claim_cancellation(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        Err(AppError::bad_request(
            "execution queue backend does not support cancellation claims",
        ))
    }

    async fn retry_or_fail(&self, job_id: Uuid, error: &str) -> Result<ExecutionJob, AppError>;

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError>;

    async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError>;

    async fn lock_owned_claim(
        &self,
        _job_id: Uuid,
        _worker_id: &str,
        _claim_generation: i64,
        _status: ExecutionJobStatus,
    ) -> Result<Box<dyn ExecutionClaimGuard>, AppError> {
        Err(AppError::internal(
            "execution queue backend does not support in-process claim fencing",
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExecutionJobRequest {
        ExecutionJobRequest {
            session_id: Uuid::new_v4(),
            environment_id: None,
            approval_id: Uuid::new_v4(),
            tool_call_id: Uuid::new_v4(),
            tool_name: "shell.exec".to_string(),
            max_attempts: None,
        }
    }

    #[test]
    fn execution_job_status_parsing_fails_closed() {
        for status in [
            ExecutionJobStatus::Queued,
            ExecutionJobStatus::Running,
            ExecutionJobStatus::Executing,
            ExecutionJobStatus::Finalizing,
            ExecutionJobStatus::CancelRequested,
            ExecutionJobStatus::Completed,
            ExecutionJobStatus::Failed,
            ExecutionJobStatus::OutcomeUnknown,
            ExecutionJobStatus::Canceled,
        ] {
            assert_eq!(status.as_str().parse(), Ok(status));
        }
        assert!("unexpected".parse::<ExecutionJobStatus>().is_err());
    }

    #[tokio::test]
    async fn memory_queue_fences_active_execution_lease() {
        let queue = ExecutionQueue::default();
        let job = queue.enqueue(request()).await.expect("enqueue job");
        queue
            .start(job.id, "worker-a")
            .await
            .expect("first worker claims job");

        let error = queue
            .start(job.id, "worker-b")
            .await
            .expect_err("active lease must fence a second worker");

        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn memory_queue_reclaims_only_expired_execution_lease() {
        let queue = MemoryExecutionQueue::default();
        let job = queue.enqueue(request()).await.expect("enqueue job");
        let first = queue
            .start(job.id, "worker-a")
            .await
            .expect("first worker claims job");
        queue.inner.write().await.jobs[0].lease_expires_at =
            Some(Utc::now() - chrono::Duration::seconds(1));
        let expired_owner = queue
            .begin_finalizing_started(job.id, "worker-a", first.claim_generation, None, json!({}))
            .await
            .expect_err("expired owner cannot begin finalization");
        assert_eq!(expired_owner.status, axum::http::StatusCode::NOT_FOUND);

        let reclaimed = queue
            .start(job.id, "worker-b")
            .await
            .expect("expired lease can be reclaimed");

        assert_eq!(reclaimed.status, ExecutionJobStatus::Running);
        assert_eq!(reclaimed.worker_id.as_deref(), Some("worker-b"));
        assert_eq!(reclaimed.attempt_count, 2);
        let stale_completion = queue
            .begin_finalizing_started(job.id, "worker-a", first.claim_generation, None, json!({}))
            .await
            .expect_err("old owner stays fenced after recovery");
        assert_eq!(stale_completion.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn memory_queue_recovers_finalization_without_restarting_attempt() {
        let queue = MemoryExecutionQueue::default();
        let job = queue.enqueue(request()).await.expect("enqueue job");
        let running = queue.start(job.id, "worker-a").await.expect("start job");
        queue
            .begin_executing_started(job.id, "worker-a", running.claim_generation)
            .await
            .expect("commit execution attempt");
        let finalizing = queue
            .begin_finalizing_started(
                job.id,
                "worker-a",
                running.claim_generation,
                None,
                json!({}),
            )
            .await
            .expect("begin finalization");
        assert_eq!(finalizing.status, ExecutionJobStatus::Finalizing);
        queue.inner.write().await.jobs[0].lease_expires_at =
            Some(Utc::now() - chrono::Duration::seconds(1));

        let resumed = queue
            .resume_finalizing(job.id, "worker-a")
            .await
            .expect("resume finalization");
        assert_eq!(resumed.status, ExecutionJobStatus::Finalizing);
        assert_eq!(resumed.worker_id.as_deref(), Some("worker-a"));
        assert_eq!(resumed.attempt_count, running.attempt_count);
        assert!(resumed.claim_generation > finalizing.claim_generation);

        let stale_completion = queue
            .finish_finalizing_started(job.id, "worker-a", finalizing.claim_generation, false)
            .await
            .expect_err("old future stays fenced when the worker id is reused");
        assert_eq!(stale_completion.status, axum::http::StatusCode::NOT_FOUND);

        let completed = queue
            .finish_finalizing_started(job.id, "worker-a", resumed.claim_generation, false)
            .await
            .expect("finish recovered finalization");
        assert_eq!(completed.status, ExecutionJobStatus::Completed);
        assert_eq!(completed.attempt_count, running.attempt_count);
    }

    #[tokio::test]
    async fn memory_queue_allows_only_one_cancellation_cleanup_owner() {
        let queue = MemoryExecutionQueue::default();
        let job = queue.enqueue(request()).await.expect("enqueue job");
        queue.cancel(job.id).await.expect("request cancellation");
        let claim = queue
            .claim_cancellation(job.id, "worker-a")
            .await
            .expect("claim cancellation");

        let cleanup = queue
            .begin_cancellation_cleanup(job.id, "worker-a", claim.claim_generation)
            .await
            .expect("begin cancellation cleanup");
        let fresh = queue
            .get(job.id)
            .await
            .expect("read current cancellation claim");
        let duplicate = queue
            .begin_cancellation_cleanup(job.id, "worker-a", fresh.claim_generation)
            .await
            .expect_err("cleanup stage cannot be claimed twice with a fresh generation");
        assert_eq!(duplicate.status, axum::http::StatusCode::NOT_FOUND);

        queue.inner.write().await.jobs[0].lease_expires_at =
            Some(Utc::now() - chrono::Duration::seconds(1));
        let recovered = queue
            .claim_cancellation(job.id, "worker-b")
            .await
            .expect("expired cleanup can be reclaimed");
        assert_eq!(
            recovered.finalization_details["stage"],
            "cancellation_pending"
        );
        let resumed = queue
            .begin_cancellation_cleanup(job.id, "worker-b", recovered.claim_generation)
            .await
            .expect("reclaimed cancellation can restart cleanup");
        assert!(resumed.claim_generation > cleanup.claim_generation);
    }

    #[tokio::test]
    async fn memory_queue_never_replays_an_expired_committed_execution() {
        let queue = MemoryExecutionQueue::default();
        let job = queue.enqueue(request()).await.expect("enqueue job");
        let running = queue.start(job.id, "worker-a").await.expect("start job");
        let executing = queue
            .begin_executing_started(job.id, "worker-a", running.claim_generation)
            .await
            .expect("commit execution attempt");

        let uncanceled = queue.cancel(job.id).await.expect("cancel is fenced");
        assert_eq!(uncanceled.status, ExecutionJobStatus::Executing);
        queue.inner.write().await.jobs[0].lease_expires_at =
            Some(Utc::now() - chrono::Duration::seconds(1));

        let replay = queue
            .start(job.id, "worker-b")
            .await
            .expect_err("committed execution cannot be reclaimed");
        assert_eq!(replay.status, axum::http::StatusCode::NOT_FOUND);
        let reconciling = queue
            .recover_expired_executing(
                job.id,
                "worker-b",
                None,
                json!({"stage": "outcome_reconciliation"}),
            )
            .await
            .expect("claim uncertain outcome reconciliation");
        let unknown = queue
            .mark_outcome_unknown_finalizing(
                job.id,
                "worker-b",
                reconciling.claim_generation,
                "execution outcome requires reconciliation",
            )
            .await
            .expect("record uncertain outcome");
        assert_eq!(unknown.status, ExecutionJobStatus::OutcomeUnknown);
        assert_eq!(unknown.attempt_count, executing.attempt_count);
        assert!(unknown.completed_at.is_some());
        assert!(unknown.lease_expires_at.is_none());
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
    pub(crate) claim_generation: i64,
    pub(crate) finalization_details: Value,
    pub(crate) attempt_count: i32,
    pub(crate) max_attempts: i32,
    pub(crate) last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionJobStatus {
    Queued,
    Running,
    Executing,
    Finalizing,
    CancelRequested,
    Completed,
    Failed,
    OutcomeUnknown,
    Canceled,
}

impl ExecutionJobStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Executing => "executing",
            Self::Finalizing => "finalizing",
            Self::CancelRequested => "cancel_requested",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::Canceled => "canceled",
        }
    }
}

impl FromStr for ExecutionJobStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "executing" => Ok(Self::Executing),
            "finalizing" => Ok(Self::Finalizing),
            "cancel_requested" => Ok(Self::CancelRequested),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "outcome_unknown" => Ok(Self::OutcomeUnknown),
            "canceled" | "cancelled" => Ok(Self::Canceled),
            _ => Err("unknown execution job status"),
        }
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
        claim_generation: 0,
        finalization_details: json!({}),
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

fn validate_execution_lease_seconds(lease_seconds: i64) -> Result<(), AppError> {
    if !(1..=86_400).contains(&lease_seconds) {
        return Err(AppError::bad_request(
            "execution job lease_seconds must be between 1 and 86400",
        ));
    }
    Ok(())
}

fn execution_job_from_row(row: PgRow) -> Result<ExecutionJob, AppError> {
    let status_value: String = row.try_get("status")?;
    let status = status_value.parse().map_err(|_| {
        AppError::internal(format!(
            "unknown execution job status in database: {status_value}"
        ))
    })?;
    Ok(ExecutionJob {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        environment_id: row.try_get("environment_id").unwrap_or(None),
        approval_id: row.try_get("approval_id")?,
        tool_call_id: row.try_get("tool_call_id")?,
        tool_name: row.try_get("tool_name")?,
        status,
        enqueued_at: row.try_get("enqueued_at")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        worker_id: row.try_get("worker_id")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        claim_generation: row.try_get("claim_generation")?,
        finalization_details: row.try_get("finalization_details")?,
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

    async fn begin_executing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Running
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::Executing;
        Ok(job.clone())
    }

    async fn mark_outcome_unknown_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Executing
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::OutcomeUnknown;
        job.completed_at = Some(now);
        job.lease_expires_at = None;
        job.last_error = Some(error.to_string());
        Ok(job.clone())
    }

    async fn begin_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && (job.status == ExecutionJobStatus::Executing
                        || (error.is_some() && job.status == ExecutionJobStatus::Running))
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::Finalizing;
        job.last_error = error.map(str::to_string);
        job.finalization_details = finalization_details;
        Ok(job.clone())
    }

    async fn resume_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Finalizing
                    && job
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.worker_id = Some(worker_id.to_string());
        job.lease_expires_at = Some(now + chrono::Duration::minutes(5));
        job.claim_generation += 1;
        Ok(job.clone())
    }

    async fn recover_expired_executing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Executing
                    && job
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::Finalizing;
        job.worker_id = Some(worker_id.to_string());
        job.lease_expires_at = Some(now + chrono::Duration::minutes(5));
        job.claim_generation += 1;
        job.last_error = error.map(str::to_string);
        job.finalization_details = finalization_details;
        Ok(job.clone())
    }

    async fn finish_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        retryable_failure: bool,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Finalizing
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        if job.last_error.is_none() {
            job.status = ExecutionJobStatus::Completed;
            job.completed_at = Some(now);
        } else if retryable_failure && job.attempt_count < job.max_attempts {
            job.status = ExecutionJobStatus::Queued;
            job.started_at = None;
            job.completed_at = None;
            job.worker_id = None;
        } else {
            job.status = ExecutionJobStatus::Failed;
            job.completed_at = Some(now);
        }
        job.lease_expires_at = None;
        Ok(job.clone())
    }

    async fn set_finalizing_failure(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Finalizing
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.last_error = Some(error.to_string());
        job.finalization_details = finalization_details;
        Ok(job.clone())
    }

    async fn mark_outcome_unknown_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Finalizing
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job.finalization_details["stage"] == "outcome_reconciliation"
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::OutcomeUnknown;
        job.completed_at = Some(now);
        job.lease_expires_at = None;
        job.last_error = Some(error.to_string());
        Ok(job.clone())
    }

    async fn prepare_completion_tail(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Finalizing
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job.last_error.is_none()
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.finalization_details = json!({"stage": "completion_pending"});
        Ok(job.clone())
    }

    async fn mark_completion_published(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::Completed
                    && matches!(
                        job.finalization_details["stage"].as_str(),
                        Some("completion_pending" | "completion_published")
                    )
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.finalization_details = json!({"stage": "completion_published"});
        Ok(job.clone())
    }

    async fn renew_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        lease_seconds: i64,
    ) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && matches!(
                        job.status,
                        ExecutionJobStatus::Running
                            | ExecutionJobStatus::Executing
                            | ExecutionJobStatus::Finalizing
                            | ExecutionJobStatus::CancelRequested
                    )
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > Utc::now())
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.lease_expires_at = Some(Utc::now() + chrono::Duration::seconds(lease_seconds));
        Ok(job.clone())
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed, None).await
    }

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| job.id == job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        match job.status {
            ExecutionJobStatus::Queued | ExecutionJobStatus::Running => {
                job.status = ExecutionJobStatus::CancelRequested;
                job.finalization_details = json!({"stage": "cancellation_pending"});
            }
            ExecutionJobStatus::Executing
            | ExecutionJobStatus::Finalizing
            | ExecutionJobStatus::CancelRequested
            | ExecutionJobStatus::Completed
            | ExecutionJobStatus::Failed
            | ExecutionJobStatus::OutcomeUnknown
            | ExecutionJobStatus::Canceled => {}
        }
        Ok(job.clone())
    }

    async fn acknowledge_canceled(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::CancelRequested
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job.finalization_details["stage"] == "cancellation_cleanup"
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.status = ExecutionJobStatus::Canceled;
        job.completed_at = Some(now);
        job.lease_expires_at = None;
        Ok(job.clone())
    }

    async fn begin_cancellation_cleanup(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::CancelRequested
                    && job.worker_id.as_deref() == Some(worker_id)
                    && job.claim_generation == claim_generation
                    && job.finalization_details["stage"] == "cancellation_pending"
                    && job
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.claim_generation += 1;
        job.finalization_details = json!({"stage": "cancellation_cleanup"});
        job.lease_expires_at = Some(now + chrono::Duration::minutes(5));
        Ok(job.clone())
    }

    async fn claim_cancellation(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        let now = Utc::now();
        let mut state = self.inner.write().await;
        let job = state
            .jobs
            .iter_mut()
            .find(|job| {
                job.id == job_id
                    && job.status == ExecutionJobStatus::CancelRequested
                    && job
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= now)
            })
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        job.worker_id = Some(worker_id.to_string());
        job.lease_expires_at = Some(now + chrono::Duration::minutes(5));
        job.claim_generation += 1;
        job.finalization_details = json!({"stage": "cancellation_pending"});
        Ok(job.clone())
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

    async fn lock_owned_claim(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        status: ExecutionJobStatus,
    ) -> Result<Box<dyn ExecutionClaimGuard>, AppError> {
        let guard = self.inner.clone().read_owned().await;
        let now = Utc::now();
        let owned = guard.jobs.iter().any(|job| {
            job.id == job_id
                && job.status == status
                && job.worker_id.as_deref() == Some(worker_id)
                && job.claim_generation == claim_generation
                && job
                    .lease_expires_at
                    .is_some_and(|lease_expires_at| lease_expires_at > now)
        });
        if !owned {
            return Err(AppError::not_found(
                "execution job claim is no longer owned",
            ));
        }
        Ok(Box::new(guard))
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
        if status == ExecutionJobStatus::Running
            && !(job.status == ExecutionJobStatus::Queued
                || (job.status == ExecutionJobStatus::Running
                    && job
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= Utc::now())))
        {
            return Err(AppError::not_found("execution job not found"));
        }
        job.status = status;
        match job.status {
            ExecutionJobStatus::Running => {
                job.started_at = Some(Utc::now());
                job.worker_id = worker_id.map(str::to_string);
                job.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
                job.claim_generation += 1;
                job.attempt_count += 1;
                job.last_error = None;
                job.finalization_details = json!({});
            }
            ExecutionJobStatus::Executing | ExecutionJobStatus::Finalizing => {
                return Err(AppError::bad_request(
                    "unsupported execution job transition",
                ));
            }
            ExecutionJobStatus::CancelRequested => {}
            ExecutionJobStatus::Completed
            | ExecutionJobStatus::Failed
            | ExecutionJobStatus::OutcomeUnknown
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
}

#[async_trait]
impl ExecutionQueueBackend for PostgresExecutionQueue {
    fn backend_kind(&self) -> &'static str {
        "postgres"
    }

    fn notify_channel(&self) -> Option<String> {
        Some(format!("mf_queue_{}", self.current_tenant_id().simple()))
    }

    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError> {
        let job = new_execution_job(request);
        sqlx::query(
            "INSERT INTO execution_jobs
                (id, tenant_id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
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
        .bind(job.claim_generation)
        .bind(&job.finalization_details)
        .bind(job.attempt_count)
        .bind(job.max_attempts)
        .bind(&job.last_error)
        .execute(&self.pool)
        .await?;
        // Wake any workers listening on this tenant's channel immediately.
        let channel = format!("mf_queue_{}", self.current_tenant_id().simple());
        let _ = sqlx::query("SELECT pg_notify($1, $2)")
            .bind(&channel)
            .bind(job.id.to_string())
            .execute(&self.pool)
            .await;
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

    async fn begin_executing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'executing'
             WHERE tenant_id = $1
               AND id = $2
               AND status = 'running'
               AND worker_id = $3
               AND claim_generation = $4
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn mark_outcome_unknown_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'outcome_unknown',
                 completed_at = COALESCE(completed_at, now()),
                 lease_expires_at = NULL,
                 last_error = $1
             WHERE tenant_id = $2
               AND id = $3
               AND status = 'executing'
               AND worker_id = $4
               AND claim_generation = $5
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn begin_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'finalizing', last_error = $1, finalization_details = $2
             WHERE tenant_id = $3
               AND id = $4
               AND (status = 'executing' OR ($1 IS NOT NULL AND status = 'running'))
               AND worker_id = $5
               AND claim_generation = $6
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(finalization_details)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn resume_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET worker_id = $1,
                 lease_expires_at = now() + interval '5 minutes',
                 claim_generation = claim_generation + 1
             WHERE tenant_id = $2
               AND id = $3
               AND status = 'finalizing'
               AND (lease_expires_at IS NULL OR lease_expires_at <= now())
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(worker_id)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn recover_expired_executing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: Option<&str>,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'finalizing',
                 worker_id = $1,
                 lease_expires_at = now() + interval '5 minutes',
                 claim_generation = claim_generation + 1,
                 last_error = $2,
                 finalization_details = $3
             WHERE tenant_id = $4
               AND id = $5
               AND status = 'executing'
               AND (lease_expires_at IS NULL OR lease_expires_at <= now())
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(worker_id)
        .bind(error)
        .bind(finalization_details)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn finish_finalizing_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        retryable_failure: bool,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = CASE
                     WHEN last_error IS NULL THEN 'completed'
                     WHEN $5 AND attempt_count < max_attempts THEN 'queued'
                     ELSE 'failed'
                 END,
                 started_at = CASE
                     WHEN last_error IS NOT NULL AND $5 AND attempt_count < max_attempts THEN NULL
                     ELSE started_at
                 END,
                 completed_at = CASE
                     WHEN last_error IS NULL OR NOT $5 OR attempt_count >= max_attempts
                         THEN COALESCE(completed_at, now())
                     ELSE NULL
                 END,
                 worker_id = CASE
                     WHEN last_error IS NOT NULL AND $5 AND attempt_count < max_attempts THEN NULL
                     ELSE worker_id
                 END,
                 lease_expires_at = NULL
             WHERE tenant_id = $1
               AND id = $2
               AND status = 'finalizing'
               AND worker_id = $3
               AND claim_generation = $4
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .bind(retryable_failure)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn renew_started(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        lease_seconds: i64,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET lease_expires_at = now() + $1 * interval '1 second'
             WHERE tenant_id = $2
               AND id = $3
               AND status IN ('running', 'executing', 'finalizing', 'cancel_requested')
               AND worker_id = $4
               AND claim_generation = $5
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(lease_seconds)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn set_finalizing_failure(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
        finalization_details: Value,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET last_error = $1, finalization_details = $2
             WHERE tenant_id = $3
               AND id = $4
               AND status = 'finalizing'
               AND worker_id = $5
               AND claim_generation = $6
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(finalization_details)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn mark_outcome_unknown_finalizing(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
        error: &str,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'outcome_unknown',
                 completed_at = COALESCE(completed_at, now()),
                 lease_expires_at = NULL,
                 last_error = $1
             WHERE tenant_id = $2
               AND id = $3
               AND status = 'finalizing'
               AND worker_id = $4
               AND claim_generation = $5
               AND finalization_details ->> 'stage' = 'outcome_reconciliation'
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed, None).await
    }

    async fn cancel(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'cancel_requested',
                 finalization_details = finalization_details || '{\"stage\":\"cancellation_pending\"}'::jsonb
             WHERE tenant_id = $1 AND id = $2 AND status IN ('queued', 'running')
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => execution_job_from_row(row),
            None => self.get(job_id).await,
        }
    }

    async fn acknowledge_canceled(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET status = 'canceled', completed_at = COALESCE(completed_at, now()), lease_expires_at = NULL
             WHERE tenant_id = $1
               AND id = $2
               AND status = 'cancel_requested'
               AND worker_id = $3
               AND claim_generation = $4
               AND finalization_details ->> 'stage' = 'cancellation_cleanup'
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn begin_cancellation_cleanup(
        &self,
        job_id: Uuid,
        worker_id: &str,
        claim_generation: i64,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET claim_generation = claim_generation + 1,
                 finalization_details = finalization_details || '{\"stage\":\"cancellation_cleanup\"}'::jsonb,
                 lease_expires_at = now() + interval '5 minutes'
             WHERE tenant_id = $1
               AND id = $2
               AND status = 'cancel_requested'
               AND worker_id = $3
               AND claim_generation = $4
               AND finalization_details ->> 'stage' = 'cancellation_pending'
               AND lease_expires_at > now()
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(self.current_tenant_id())
        .bind(job_id)
        .bind(worker_id)
        .bind(claim_generation)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn claim_cancellation(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<ExecutionJob, AppError> {
        let row = sqlx::query(
            "UPDATE execution_jobs
             SET worker_id = $1,
                 lease_expires_at = now() + interval '5 minutes',
                 claim_generation = claim_generation + 1,
                 finalization_details = finalization_details || '{\"stage\":\"cancellation_pending\"}'::jsonb
             WHERE tenant_id = $2
               AND id = $3
               AND status = 'cancel_requested'
               AND (lease_expires_at IS NULL OR lease_expires_at <= now())
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(worker_id)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
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
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
        )
        .bind(error)
        .bind(self.current_tenant_id())
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        let rows = sqlx::query(
            "SELECT id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error
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
            "SELECT id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error
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
                 SET status = 'running', started_at = COALESCE(started_at, now()), completed_at = NULL, worker_id = $1, lease_expires_at = now() + interval '5 minutes', claim_generation = claim_generation + 1, finalization_details = '{}'::jsonb, attempt_count = attempt_count + 1, last_error = NULL
                 WHERE tenant_id = $2
                   AND id = $3
                   AND (status = 'queued' OR (status = 'running' AND (lease_expires_at IS NULL OR lease_expires_at <= now())))
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
            )
            .bind(worker_id.unwrap_or("api"))
            .bind(self.current_tenant_id())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?,
            ExecutionJobStatus::Completed
            | ExecutionJobStatus::Failed
            | ExecutionJobStatus::OutcomeUnknown
            | ExecutionJobStatus::Canceled => sqlx::query(
                "UPDATE execution_jobs
                 SET status = $1, completed_at = COALESCE(completed_at, now()), lease_expires_at = NULL
                 WHERE tenant_id = $2 AND id = $3
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
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
             RETURNING id, session_id, environment_id, approval_id, tool_call_id, tool_name, status, enqueued_at, started_at, completed_at, worker_id, lease_expires_at, claim_generation, finalization_details, attempt_count, max_attempts, last_error",
            )
            .bind(self.current_tenant_id())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?,
            ExecutionJobStatus::Executing
            | ExecutionJobStatus::Finalizing
            | ExecutionJobStatus::CancelRequested => {
                return Err(AppError::bad_request("unsupported execution job transition"));
            }
        }
        .ok_or_else(|| AppError::not_found("execution job not found"))?;
        execution_job_from_row(row)
    }
}
