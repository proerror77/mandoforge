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

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct BrokerQueueConfig {
    pub(crate) kind: BrokerQueueKind,
    pub(crate) endpoint: String,
    pub(crate) stream: String,
    pub(crate) consumer_group: String,
}

#[allow(dead_code)]
impl BrokerQueueKind {
    fn endpoint_env_key(self) -> &'static str {
        match self {
            Self::Redis => "MANDOFORGE_REDIS_URL",
            Self::Nats => "MANDOFORGE_NATS_URL",
        }
    }

    fn default_stream(self) -> &'static str {
        match self {
            Self::Redis => "mandoforge:execution-jobs",
            Self::Nats => "mandoforge.execution.jobs",
        }
    }

    fn stream_env_key(self) -> &'static str {
        match self {
            Self::Redis => "MANDOFORGE_REDIS_STREAM",
            Self::Nats => "MANDOFORGE_NATS_SUBJECT",
        }
    }
}

#[allow(dead_code)]
impl BrokerQueueConfig {
    pub(crate) fn from_env(kind: BrokerQueueKind) -> Result<Self, AppError> {
        Self::from_lookup(kind, |key| std::env::var(key).ok())
    }

    fn from_lookup<F>(kind: BrokerQueueKind, lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint_key = kind.endpoint_env_key();
        let endpoint = lookup(endpoint_key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "{endpoint_key} is required for {:?} execution queue backend",
                    kind
                ))
            })?;
        let stream = lookup(kind.stream_env_key())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| kind.default_stream().to_string());
        let consumer_group = lookup("MANDOFORGE_EXECUTION_QUEUE_CONSUMER_GROUP")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "mandoforge-workers".to_string());

        Ok(Self {
            kind,
            endpoint,
            stream,
            consumer_group,
        })
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait BrokerQueueHealthCheck: Send + Sync {
    async fn check(&self, config: &BrokerQueueConfig) -> Result<(), AppError>;
}

#[allow(dead_code)]
pub(crate) struct ReservedBrokerQueueHealthCheck;

#[async_trait]
impl BrokerQueueHealthCheck for ReservedBrokerQueueHealthCheck {
    async fn check(&self, config: &BrokerQueueConfig) -> Result<(), AppError> {
        Err(AppError::bad_request(format!(
            "{:?} execution queue health check is reserved but not implemented",
            config.kind
        )))
    }
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

#[cfg(test)]
mod tests {
    use super::{
        BrokerQueueConfig, BrokerQueueHealthCheck, BrokerQueueKind, ReservedBrokerQueueHealthCheck,
    };

    #[test]
    fn broker_queue_config_requires_kind_endpoint() {
        let error = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |_| None)
            .expect_err("missing redis endpoint should fail");

        assert!(
            error.message.contains("MANDOFORGE_REDIS_URL"),
            "{:?}",
            error
        );
    }

    #[test]
    fn broker_queue_config_defaults_stream_and_consumer_group() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Nats, |key| match key {
            "MANDOFORGE_NATS_URL" => Some("nats://127.0.0.1:4222".to_string()),
            _ => None,
        })
        .expect("nats config");

        assert_eq!(config.endpoint, "nats://127.0.0.1:4222");
        assert_eq!(config.stream, "mandoforge.execution.jobs");
        assert_eq!(config.consumer_group, "mandoforge-workers");
    }

    #[test]
    fn broker_queue_config_allows_stream_and_group_overrides() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some("redis://127.0.0.1:6379/0".to_string()),
            "MANDOFORGE_REDIS_STREAM" => Some("custom-stream".to_string()),
            "MANDOFORGE_EXECUTION_QUEUE_CONSUMER_GROUP" => Some("custom-workers".to_string()),
            _ => None,
        })
        .expect("redis config");

        assert_eq!(config.endpoint, "redis://127.0.0.1:6379/0");
        assert_eq!(config.stream, "custom-stream");
        assert_eq!(config.consumer_group, "custom-workers");
    }

    #[tokio::test]
    async fn broker_queue_health_check_is_reserved_until_implemented() {
        let health_check = ReservedBrokerQueueHealthCheck;
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some("redis://127.0.0.1:6379/0".to_string()),
            _ => None,
        })
        .expect("redis config");

        assert!(health_check.check(&config).await.is_err());
    }
}
