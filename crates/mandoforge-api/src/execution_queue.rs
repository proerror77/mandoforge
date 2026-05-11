use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppError;

#[derive(Clone, Default)]
pub(crate) struct ExecutionQueue {
    inner: Arc<RwLock<ExecutionQueueState>>,
}

#[derive(Default)]
struct ExecutionQueueState {
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

pub(crate) struct ExecutionJobRequest {
    pub(crate) session_id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
}

impl ExecutionQueue {
    pub(crate) async fn enqueue(&self, request: ExecutionJobRequest) -> ExecutionJob {
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
        };
        self.inner.write().await.jobs.push(job.clone());
        job
    }

    pub(crate) async fn start(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Running).await
    }

    pub(crate) async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Completed).await
    }

    pub(crate) async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.update(job_id, ExecutionJobStatus::Failed).await
    }

    pub(crate) async fn list(&self) -> Vec<ExecutionJob> {
        self.inner.read().await.jobs.clone()
    }

    pub(crate) async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.inner
            .read()
            .await
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("execution job not found"))
    }

    async fn update(
        &self,
        job_id: Uuid,
        status: ExecutionJobStatus,
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
            }
            ExecutionJobStatus::Completed | ExecutionJobStatus::Failed => {
                job.completed_at = Some(Utc::now());
            }
            ExecutionJobStatus::Queued => {}
        }
        Ok(job.clone())
    }
}
