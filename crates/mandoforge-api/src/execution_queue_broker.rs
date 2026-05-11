use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    AppError,
    execution_queue::{ExecutionJob, ExecutionJobRequest, ExecutionQueueBackend},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum BrokerQueueKind {
    Redis,
    Nats,
}

#[allow(dead_code)]
pub(crate) struct BrokerExecutionQueue {
    kind: BrokerQueueKind,
}

#[allow(dead_code)]
impl BrokerExecutionQueue {
    pub(crate) fn new(kind: BrokerQueueKind) -> Self {
        Self { kind }
    }

    fn reserved_error(&self) -> AppError {
        AppError::bad_request(format!(
            "{:?} execution queue backend is reserved but not implemented",
            self.kind
        ))
    }
}

#[async_trait]
impl ExecutionQueueBackend for BrokerExecutionQueue {
    async fn enqueue(&self, _request: ExecutionJobRequest) -> Result<ExecutionJob, AppError> {
        Err(self.reserved_error())
    }

    async fn start(&self, _job_id: Uuid, _worker_id: &str) -> Result<ExecutionJob, AppError> {
        Err(self.reserved_error())
    }

    async fn complete(&self, _job_id: Uuid) -> Result<ExecutionJob, AppError> {
        Err(self.reserved_error())
    }

    async fn fail(&self, _job_id: Uuid) -> Result<ExecutionJob, AppError> {
        Err(self.reserved_error())
    }

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        Err(self.reserved_error())
    }

    async fn get(&self, _job_id: Uuid) -> Result<ExecutionJob, AppError> {
        Err(self.reserved_error())
    }
}
