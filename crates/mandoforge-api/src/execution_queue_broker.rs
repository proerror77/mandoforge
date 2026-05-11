use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RedisStreamCommand {
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct RedisExecutionJobPayload {
    pub(crate) session_id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
}

#[allow(dead_code)]
pub(crate) struct RedisStreamClient;

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

#[allow(dead_code)]
impl RedisExecutionJobPayload {
    pub(crate) fn from_request(request: &ExecutionJobRequest) -> Self {
        Self {
            session_id: request.session_id,
            approval_id: request.approval_id,
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name.clone(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "session_id": self.session_id,
            "approval_id": self.approval_id,
            "tool_call_id": self.tool_call_id,
            "tool_name": self.tool_name,
        })
    }
}

#[allow(dead_code)]
impl RedisStreamCommand {
    pub(crate) fn xadd_enqueue(
        config: &BrokerQueueConfig,
        payload: &RedisExecutionJobPayload,
    ) -> Result<Self, AppError> {
        if config.kind != BrokerQueueKind::Redis {
            return Err(AppError::bad_request(
                "Redis stream command requires Redis broker config",
            ));
        }
        Ok(Self {
            command: "XADD".to_string(),
            args: vec![
                config.stream.clone(),
                "*".to_string(),
                "payload".to_string(),
                payload.to_json().to_string(),
            ],
        })
    }

    pub(crate) fn xgroup_create(config: &BrokerQueueConfig) -> Result<Self, AppError> {
        if config.kind != BrokerQueueKind::Redis {
            return Err(AppError::bad_request(
                "Redis stream command requires Redis broker config",
            ));
        }
        Ok(Self {
            command: "XGROUP".to_string(),
            args: vec![
                "CREATE".to_string(),
                config.stream.clone(),
                config.consumer_group.clone(),
                "$".to_string(),
                "MKSTREAM".to_string(),
            ],
        })
    }

    pub(crate) fn xack(
        config: &BrokerQueueConfig,
        message_id: impl Into<String>,
    ) -> Result<Self, AppError> {
        if config.kind != BrokerQueueKind::Redis {
            return Err(AppError::bad_request(
                "Redis stream command requires Redis broker config",
            ));
        }
        Ok(Self {
            command: "XACK".to_string(),
            args: vec![
                config.stream.clone(),
                config.consumer_group.clone(),
                message_id.into(),
            ],
        })
    }

    pub(crate) fn xreadgroup_next(
        config: &BrokerQueueConfig,
        consumer_name: impl Into<String>,
        count: usize,
        block_ms: u64,
    ) -> Result<Self, AppError> {
        if config.kind != BrokerQueueKind::Redis {
            return Err(AppError::bad_request(
                "Redis stream command requires Redis broker config",
            ));
        }
        let count = count.max(1);
        Ok(Self {
            command: "XREADGROUP".to_string(),
            args: vec![
                "GROUP".to_string(),
                config.consumer_group.clone(),
                consumer_name.into(),
                "COUNT".to_string(),
                count.to_string(),
                "BLOCK".to_string(),
                block_ms.to_string(),
                "STREAMS".to_string(),
                config.stream.clone(),
                ">".to_string(),
            ],
        })
    }

    fn resp_args(&self) -> Vec<String> {
        std::iter::once(self.command.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

#[allow(dead_code)]
impl RedisStreamClient {
    pub(crate) async fn execute(
        config: &BrokerQueueConfig,
        command: &RedisStreamCommand,
    ) -> Result<String, AppError> {
        let addr = redis_tcp_addr(&config.endpoint)?;
        let mut stream = TcpStream::connect(addr).await?;
        let payload = encode_resp_array(&command.resp_args());
        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;
        let mut buffer = vec![0; 4096];
        let bytes = stream.read(&mut buffer).await?;
        if bytes == 0 {
            return Err(AppError::bad_request("Redis returned an empty response"));
        }
        parse_redis_response(&String::from_utf8_lossy(&buffer[..bytes]))
    }
}

fn redis_tcp_addr(endpoint: &str) -> Result<String, AppError> {
    let trimmed = endpoint.trim();
    let without_scheme = trimmed
        .strip_prefix("redis://")
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_REDIS_URL must use redis://"))?;
    let authority = without_scheme
        .split('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_REDIS_URL must include host:port"))?;
    if authority.contains('@') {
        return Err(AppError::bad_request(
            "authenticated Redis URLs are not supported by the current broker boundary",
        ));
    }
    if !authority.contains(':') {
        return Err(AppError::bad_request(
            "MANDOFORGE_REDIS_URL must include host:port",
        ));
    }
    Ok(authority.to_string())
}

fn encode_resp_array(args: &[String]) -> String {
    let mut encoded = format!("*{}\r\n", args.len());
    for arg in args {
        encoded.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }
    encoded
}

fn parse_redis_response(response: &str) -> Result<String, AppError> {
    if let Some(error) = response.strip_prefix('-') {
        let message = error.trim_end_matches("\r\n");
        return Err(AppError::bad_request(format!("Redis error: {message}")));
    }
    if let Some(value) = response.strip_prefix('+') {
        return Ok(value.trim_end_matches("\r\n").to_string());
    }
    if let Some(value) = response.strip_prefix('$') {
        let (_, rest) = value
            .split_once("\r\n")
            .ok_or_else(|| AppError::bad_request("invalid Redis bulk response"))?;
        let (bulk, _) = rest
            .split_once("\r\n")
            .ok_or_else(|| AppError::bad_request("invalid Redis bulk response"))?;
        return Ok(bulk.to_string());
    }
    if let Some(value) = response.strip_prefix(':') {
        return Ok(value.trim_end_matches("\r\n").to_string());
    }
    Err(AppError::bad_request("unsupported Redis response"))
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
        BrokerQueueConfig, BrokerQueueHealthCheck, BrokerQueueKind, RedisExecutionJobPayload,
        RedisStreamClient, RedisStreamCommand, ReservedBrokerQueueHealthCheck, encode_resp_array,
        parse_redis_response, redis_tcp_addr,
    };
    use crate::execution_queue::ExecutionJobRequest;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
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

    #[test]
    fn redis_stream_command_builds_enqueue_payload() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some("redis://127.0.0.1:6379/0".to_string()),
            _ => None,
        })
        .expect("redis config");
        let request = ExecutionJobRequest {
            session_id: "00000000-0000-4000-8000-000000000001"
                .parse()
                .expect("session id"),
            approval_id: "00000000-0000-4000-8000-000000000002"
                .parse()
                .expect("approval id"),
            tool_call_id: "00000000-0000-4000-8000-000000000003"
                .parse()
                .expect("tool call id"),
            tool_name: "codex.exec".to_string(),
        };
        let payload = RedisExecutionJobPayload::from_request(&request);
        let command = RedisStreamCommand::xadd_enqueue(&config, &payload).expect("xadd command");

        assert_eq!(command.command, "XADD");
        assert_eq!(command.args[0], "mandoforge:execution-jobs");
        assert_eq!(command.args[1], "*");
        assert_eq!(command.args[2], "payload");
        assert!(command.args[3].contains("\"tool_name\":\"codex.exec\""));
        assert!(command.args[3].contains("00000000-0000-4000-8000-000000000001"));
    }

    #[test]
    fn redis_stream_command_builds_group_and_ack_commands() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some("redis://127.0.0.1:6379/0".to_string()),
            "MANDOFORGE_REDIS_STREAM" => Some("custom-stream".to_string()),
            "MANDOFORGE_EXECUTION_QUEUE_CONSUMER_GROUP" => Some("custom-workers".to_string()),
            _ => None,
        })
        .expect("redis config");

        let group = RedisStreamCommand::xgroup_create(&config).expect("xgroup command");
        assert_eq!(group.command, "XGROUP");
        assert_eq!(
            group.args,
            vec!["CREATE", "custom-stream", "custom-workers", "$", "MKSTREAM"]
        );

        let ack = RedisStreamCommand::xack(&config, "1-0").expect("xack command");
        assert_eq!(ack.command, "XACK");
        assert_eq!(ack.args, vec!["custom-stream", "custom-workers", "1-0"]);
    }

    #[test]
    fn redis_stream_command_builds_readgroup_command() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some("redis://127.0.0.1:6379/0".to_string()),
            "MANDOFORGE_REDIS_STREAM" => Some("custom-stream".to_string()),
            "MANDOFORGE_EXECUTION_QUEUE_CONSUMER_GROUP" => Some("custom-workers".to_string()),
            _ => None,
        })
        .expect("redis config");

        let read = RedisStreamCommand::xreadgroup_next(&config, "worker-1", 0, 5000)
            .expect("xreadgroup command");

        assert_eq!(read.command, "XREADGROUP");
        assert_eq!(
            read.args,
            vec![
                "GROUP",
                "custom-workers",
                "worker-1",
                "COUNT",
                "1",
                "BLOCK",
                "5000",
                "STREAMS",
                "custom-stream",
                ">"
            ]
        );
        assert_eq!(
            encode_resp_array(&read.resp_args()),
            "*11\r\n$10\r\nXREADGROUP\r\n$5\r\nGROUP\r\n$14\r\ncustom-workers\r\n$8\r\nworker-1\r\n$5\r\nCOUNT\r\n$1\r\n1\r\n$5\r\nBLOCK\r\n$4\r\n5000\r\n$7\r\nSTREAMS\r\n$13\r\ncustom-stream\r\n$1\r\n>\r\n"
        );
    }

    #[test]
    fn redis_stream_commands_reject_non_redis_config() {
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Nats, |key| match key {
            "MANDOFORGE_NATS_URL" => Some("nats://127.0.0.1:4222".to_string()),
            _ => None,
        })
        .expect("nats config");

        assert!(RedisStreamCommand::xgroup_create(&config).is_err());
        assert!(RedisStreamCommand::xack(&config, "1-0").is_err());
        assert!(RedisStreamCommand::xreadgroup_next(&config, "worker-1", 1, 1000).is_err());
    }

    #[test]
    fn redis_stream_client_parses_endpoint_and_resp_responses() {
        assert_eq!(
            redis_tcp_addr("redis://127.0.0.1:6379/0").expect("addr"),
            "127.0.0.1:6379"
        );
        assert!(redis_tcp_addr("http://127.0.0.1:6379").is_err());
        assert!(redis_tcp_addr("redis://127.0.0.1").is_err());
        assert_eq!(parse_redis_response("+OK\r\n").expect("simple"), "OK");
        assert_eq!(parse_redis_response("$3\r\n1-0\r\n").expect("bulk"), "1-0");
        assert_eq!(parse_redis_response(":1\r\n").expect("int"), "1");
        assert!(parse_redis_response("-ERR no\r\n").is_err());
    }

    #[test]
    fn redis_stream_client_encodes_resp_arrays() {
        let encoded = encode_resp_array(&[
            "XACK".to_string(),
            "custom-stream".to_string(),
            "custom-workers".to_string(),
            "1-0".to_string(),
        ]);

        assert_eq!(
            encoded,
            "*4\r\n$4\r\nXACK\r\n$13\r\ncustom-stream\r\n$14\r\ncustom-workers\r\n$3\r\n1-0\r\n"
        );
    }

    #[tokio::test]
    async fn redis_stream_client_sends_resp_command_to_mock_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0; 4096];
            let bytes = socket.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
            assert!(request.starts_with("*5\r\n$4\r\nXADD\r\n"));
            assert!(request.contains("$25\r\nmandoforge:execution-jobs\r\n"));
            assert!(request.contains("$7\r\npayload\r\n"));
            assert!(request.contains("codex.exec"));
            socket.write_all(b"$3\r\n1-0\r\n").await.expect("write");
        });
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some(format!("redis://{addr}/0")),
            _ => None,
        })
        .expect("redis config");
        let request = ExecutionJobRequest {
            session_id: "00000000-0000-4000-8000-000000000001"
                .parse()
                .expect("session id"),
            approval_id: "00000000-0000-4000-8000-000000000002"
                .parse()
                .expect("approval id"),
            tool_call_id: "00000000-0000-4000-8000-000000000003"
                .parse()
                .expect("tool call id"),
            tool_name: "codex.exec".to_string(),
        };
        let payload = RedisExecutionJobPayload::from_request(&request);
        let command = RedisStreamCommand::xadd_enqueue(&config, &payload).expect("xadd command");

        let response = RedisStreamClient::execute(&config, &command)
            .await
            .expect("redis response");

        server.await.expect("server");
        assert_eq!(response, "1-0");
    }
}
