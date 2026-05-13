use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::RwLock,
    time::{Duration, timeout},
};
use uuid::Uuid;

use crate::{
    AppError,
    execution_queue::{
        ExecutionJob, ExecutionJobRequest, ExecutionJobStatus, ExecutionQueueBackend,
    },
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct RedisExecutionJobPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) job_id: Option<Uuid>,
    pub(crate) session_id: Uuid,
    pub(crate) approval_id: Uuid,
    pub(crate) tool_call_id: Uuid,
    pub(crate) tool_name: String,
    #[serde(default)]
    pub(crate) max_attempts: Option<i32>,
}

#[allow(dead_code)]
pub(crate) struct RedisStreamClient;

#[allow(dead_code)]
pub(crate) struct NatsCoreClient;

#[derive(Debug, Clone)]
struct BrokerPendingExecutionJob {
    message_id: String,
    job: ExecutionJob,
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

#[allow(dead_code)]
impl RedisExecutionJobPayload {
    pub(crate) fn from_request(request: &ExecutionJobRequest) -> Self {
        Self {
            job_id: None,
            session_id: request.session_id,
            approval_id: request.approval_id,
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name.clone(),
            max_attempts: request.max_attempts,
        }
    }

    fn from_job(job: &ExecutionJob) -> Self {
        Self {
            job_id: Some(job.id),
            session_id: job.session_id,
            approval_id: job.approval_id,
            tool_call_id: job.tool_call_id,
            tool_name: job.tool_name.clone(),
            max_attempts: Some(job.max_attempts),
        }
    }

    fn into_execution_job(self) -> ExecutionJob {
        let max_attempts = self.max_attempts.unwrap_or(3).clamp(1, 10);
        ExecutionJob {
            id: self.job_id.unwrap_or_else(Uuid::new_v4),
            session_id: self.session_id,
            approval_id: self.approval_id,
            tool_call_id: self.tool_call_id,
            tool_name: self.tool_name,
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

    fn to_json(&self) -> Value {
        json!({
            "job_id": self.job_id,
            "session_id": self.session_id,
            "approval_id": self.approval_id,
            "tool_call_id": self.tool_call_id,
            "tool_name": self.tool_name,
            "max_attempts": self.max_attempts,
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
    pub(crate) async fn execute_raw(
        config: &BrokerQueueConfig,
        command: &RedisStreamCommand,
    ) -> Result<String, AppError> {
        let addr = redis_tcp_addr(&config.endpoint)?;
        let mut stream = TcpStream::connect(addr).await?;
        let payload = encode_resp_array(&command.resp_args());
        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;
        let mut buffer = vec![0; 16 * 1024];
        let bytes = stream.read(&mut buffer).await?;
        if bytes == 0 {
            return Err(AppError::bad_request("Redis returned an empty response"));
        }
        Ok(String::from_utf8_lossy(&buffer[..bytes]).to_string())
    }

    pub(crate) async fn execute(
        config: &BrokerQueueConfig,
        command: &RedisStreamCommand,
    ) -> Result<String, AppError> {
        let response = Self::execute_raw(config, command).await?;
        parse_redis_response(&response)
    }
}

#[allow(dead_code)]
impl NatsCoreClient {
    pub(crate) async fn publish(
        config: &BrokerQueueConfig,
        payload: &RedisExecutionJobPayload,
    ) -> Result<(), AppError> {
        if config.kind != BrokerQueueKind::Nats {
            return Err(AppError::bad_request(
                "NATS publish requires NATS broker config",
            ));
        }
        let addr = nats_tcp_addr(&config.endpoint)?;
        let mut stream = TcpStream::connect(addr).await?;
        let mut buffer = vec![0; 4096];
        let _ = timeout(Duration::from_millis(250), stream.read(&mut buffer)).await;
        stream.write_all(b"CONNECT {\"verbose\":false}\r\n").await?;
        let payload = payload.to_json().to_string();
        let command = format!("PUB {} {}\r\n{}\r\n", config.stream, payload.len(), payload);
        stream.write_all(command.as_bytes()).await?;
        stream.write_all(b"PING\r\n").await?;
        stream.flush().await?;
        let _ = timeout(Duration::from_millis(250), stream.read(&mut buffer)).await;
        Ok(())
    }

    pub(crate) async fn drain_once(
        config: &BrokerQueueConfig,
    ) -> Result<Vec<(String, RedisExecutionJobPayload)>, AppError> {
        if config.kind != BrokerQueueKind::Nats {
            return Err(AppError::bad_request(
                "NATS drain requires NATS broker config",
            ));
        }
        let addr = nats_tcp_addr(&config.endpoint)?;
        let mut stream = TcpStream::connect(addr).await?;
        let mut buffer = vec![0; 64 * 1024];
        let _ = timeout(Duration::from_millis(250), stream.read(&mut buffer)).await;
        stream.write_all(b"CONNECT {\"verbose\":false}\r\n").await?;
        let sub = format!(
            "SUB {} {} 1\r\nPING\r\n",
            config.stream, config.consumer_group
        );
        stream.write_all(sub.as_bytes()).await?;
        stream.flush().await?;
        let bytes = timeout(Duration::from_millis(500), stream.read(&mut buffer))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(0);
        if bytes == 0 {
            return Ok(Vec::new());
        }
        parse_nats_messages(&String::from_utf8_lossy(&buffer[..bytes]))
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

fn nats_tcp_addr(endpoint: &str) -> Result<String, AppError> {
    let trimmed = endpoint.trim();
    let without_scheme = trimmed
        .strip_prefix("nats://")
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_NATS_URL must use nats://"))?;
    let authority = without_scheme
        .split('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::bad_request("MANDOFORGE_NATS_URL must include host:port"))?;
    if authority.contains('@') {
        return Err(AppError::bad_request(
            "authenticated NATS URLs are not supported by the current broker boundary",
        ));
    }
    if !authority.contains(':') {
        return Err(AppError::bad_request(
            "MANDOFORGE_NATS_URL must include host:port",
        ));
    }
    Ok(authority.to_string())
}

fn parse_nats_messages(
    response: &str,
) -> Result<Vec<(String, RedisExecutionJobPayload)>, AppError> {
    let mut jobs = Vec::new();
    let mut rest = response;
    while let Some(index) = rest.find("MSG ") {
        rest = &rest[index..];
        let Some((header, after_header)) = rest.split_once("\r\n") else {
            break;
        };
        let parts: Vec<_> = header.split_whitespace().collect();
        let Some(size_part) = parts.last() else {
            break;
        };
        let size = size_part
            .parse::<usize>()
            .map_err(|_| AppError::bad_request("invalid NATS message size"))?;
        if after_header.len() < size + 2 {
            break;
        }
        let payload = &after_header[..size];
        let job = serde_json::from_str::<RedisExecutionJobPayload>(payload)
            .map_err(|error| AppError::bad_request(format!("invalid NATS job payload: {error}")))?;
        let message_id = format!("nats:{}", job.job_id.unwrap_or_else(Uuid::new_v4));
        jobs.push((message_id, job));
        rest = &after_header[size + 2..];
    }
    Ok(jobs)
}

fn encode_resp_array(args: &[String]) -> String {
    let mut encoded = format!("*{}\r\n", args.len());
    for arg in args {
        encoded.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }
    encoded
}

fn parse_redis_response(response: &str) -> Result<String, AppError> {
    if response.starts_with("*-1") || response.starts_with("$-1") {
        return Ok(String::new());
    }
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

#[derive(Debug, Clone, PartialEq)]
enum RespValue {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Option<String>),
    Array(Option<Vec<RespValue>>),
}

fn parse_resp(input: &str) -> Result<RespValue, AppError> {
    let (value, offset) = parse_resp_at(input.as_bytes(), 0)?;
    if offset > input.len() {
        return Err(AppError::bad_request("invalid Redis response offset"));
    }
    Ok(value)
}

fn parse_resp_at(input: &[u8], offset: usize) -> Result<(RespValue, usize), AppError> {
    let kind = *input
        .get(offset)
        .ok_or_else(|| AppError::bad_request("empty Redis response"))?;
    match kind {
        b'+' => {
            let (line, next) = read_resp_line(input, offset + 1)?;
            Ok((RespValue::Simple(line), next))
        }
        b'-' => {
            let (line, next) = read_resp_line(input, offset + 1)?;
            Ok((RespValue::Error(line), next))
        }
        b':' => {
            let (line, next) = read_resp_line(input, offset + 1)?;
            let value = line
                .parse::<i64>()
                .map_err(|_| AppError::bad_request("invalid Redis integer response"))?;
            Ok((RespValue::Integer(value), next))
        }
        b'$' => {
            let (line, mut next) = read_resp_line(input, offset + 1)?;
            let len = line
                .parse::<isize>()
                .map_err(|_| AppError::bad_request("invalid Redis bulk length"))?;
            if len < 0 {
                return Ok((RespValue::Bulk(None), next));
            }
            let len = len as usize;
            let end = next + len;
            if input.len() < end + 2 {
                return Err(AppError::bad_request("truncated Redis bulk response"));
            }
            let value = String::from_utf8_lossy(&input[next..end]).to_string();
            next = end + 2;
            Ok((RespValue::Bulk(Some(value)), next))
        }
        b'*' => {
            let (line, mut next) = read_resp_line(input, offset + 1)?;
            let len = line
                .parse::<isize>()
                .map_err(|_| AppError::bad_request("invalid Redis array length"))?;
            if len < 0 {
                return Ok((RespValue::Array(None), next));
            }
            let mut values = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let (value, value_next) = parse_resp_at(input, next)?;
                values.push(value);
                next = value_next;
            }
            Ok((RespValue::Array(Some(values)), next))
        }
        _ => Err(AppError::bad_request("unsupported Redis response type")),
    }
}

fn read_resp_line(input: &[u8], offset: usize) -> Result<(String, usize), AppError> {
    let mut cursor = offset;
    while cursor + 1 < input.len() {
        if input[cursor] == b'\r' && input[cursor + 1] == b'\n' {
            return Ok((
                String::from_utf8_lossy(&input[offset..cursor]).to_string(),
                cursor + 2,
            ));
        }
        cursor += 1;
    }
    Err(AppError::bad_request("unterminated Redis response line"))
}

fn parse_xreadgroup_execution_jobs(
    response: &str,
) -> Result<Vec<(String, RedisExecutionJobPayload)>, AppError> {
    let value = parse_resp(response)?;
    let Some(streams) = resp_array(&value) else {
        return Ok(Vec::new());
    };
    let mut jobs = Vec::new();
    for stream in streams {
        let Some(stream_parts) = resp_array(stream) else {
            continue;
        };
        let Some(messages) = stream_parts.get(1).and_then(resp_array) else {
            continue;
        };
        for message in messages {
            let Some(message_parts) = resp_array(message) else {
                continue;
            };
            let Some(message_id) = message_parts.first().and_then(resp_string) else {
                continue;
            };
            let Some(fields) = message_parts.get(1).and_then(resp_array) else {
                continue;
            };
            let payload = redis_field_value(fields, "payload")
                .ok_or_else(|| AppError::bad_request("Redis stream message missing payload"))?;
            let payload =
                serde_json::from_str::<RedisExecutionJobPayload>(&payload).map_err(|error| {
                    AppError::bad_request(format!("invalid Redis job payload: {error}"))
                })?;
            jobs.push((message_id, payload));
        }
    }
    Ok(jobs)
}

fn resp_array(value: &RespValue) -> Option<&[RespValue]> {
    match value {
        RespValue::Array(Some(values)) => Some(values.as_slice()),
        _ => None,
    }
}

fn resp_string(value: &RespValue) -> Option<String> {
    match value {
        RespValue::Simple(value) | RespValue::Bulk(Some(value)) => Some(value.clone()),
        RespValue::Integer(value) => Some(value.to_string()),
        _ => None,
    }
}

fn redis_field_value(fields: &[RespValue], key: &str) -> Option<String> {
    fields.chunks(2).find_map(|chunk| {
        let field = chunk.first().and_then(resp_string)?;
        if field == key {
            chunk.get(1).and_then(resp_string)
        } else {
            None
        }
    })
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
    config: Option<BrokerQueueConfig>,
    pending: Arc<RwLock<HashMap<Uuid, BrokerPendingExecutionJob>>>,
}

#[allow(dead_code)]
impl BrokerExecutionQueue {
    pub(crate) fn new(kind: BrokerQueueKind) -> Self {
        Self {
            kind,
            config: None,
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn redis(config: BrokerQueueConfig) -> Self {
        Self {
            kind: BrokerQueueKind::Redis,
            config: Some(config),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn nats(config: BrokerQueueConfig) -> Self {
        Self {
            kind: BrokerQueueKind::Nats,
            config: Some(config),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn reserved_error(&self) -> AppError {
        AppError::bad_request(format!(
            "{:?} execution queue backend is reserved but not implemented",
            self.kind
        ))
    }

    async fn redis_config(&self) -> Result<&BrokerQueueConfig, AppError> {
        if self.kind != BrokerQueueKind::Redis {
            return Err(self.reserved_error());
        }
        self.config
            .as_ref()
            .ok_or_else(|| AppError::bad_request("Redis execution queue config is missing"))
    }

    async fn broker_config(&self) -> Result<&BrokerQueueConfig, AppError> {
        self.config
            .as_ref()
            .ok_or_else(|| AppError::bad_request("broker execution queue config is missing"))
    }

    async fn ensure_redis_group(&self, config: &BrokerQueueConfig) -> Result<(), AppError> {
        let command = RedisStreamCommand::xgroup_create(config)?;
        match RedisStreamClient::execute(config, &command).await {
            Ok(_) => Ok(()),
            Err(error) if error.message.contains("BUSYGROUP") => Ok(()),
            Err(error) => Err(error),
        }
    }

    async fn ack_redis_job(
        &self,
        config: &BrokerQueueConfig,
        job_id: Uuid,
    ) -> Result<(), AppError> {
        let message_id = {
            let pending = self.pending.read().await;
            pending
                .get(&job_id)
                .map(|pending| pending.message_id.clone())
                .ok_or_else(|| AppError::not_found("execution job not found"))?
        };
        let command = RedisStreamCommand::xack(config, message_id)?;
        RedisStreamClient::execute(config, &command).await?;
        Ok(())
    }
}

#[async_trait]
impl ExecutionQueueBackend for BrokerExecutionQueue {
    async fn enqueue(&self, request: ExecutionJobRequest) -> Result<ExecutionJob, AppError> {
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            session_id: request.session_id,
            approval_id: request.approval_id,
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name.clone(),
            status: ExecutionJobStatus::Queued,
            enqueued_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            lease_expires_at: None,
            attempt_count: 0,
            max_attempts: request.max_attempts.unwrap_or(3).clamp(1, 10),
            last_error: None,
        };
        let payload = RedisExecutionJobPayload::from_job(&job);
        match self.kind {
            BrokerQueueKind::Redis => {
                let config = self.redis_config().await?;
                let command = RedisStreamCommand::xadd_enqueue(config, &payload)?;
                RedisStreamClient::execute(config, &command).await?;
            }
            BrokerQueueKind::Nats => {
                let config = self.broker_config().await?;
                NatsCoreClient::publish(config, &payload).await?;
            }
        }
        Ok(job)
    }

    async fn start(&self, job_id: Uuid, worker_id: &str) -> Result<ExecutionJob, AppError> {
        self.broker_config().await?;
        let mut pending = self.pending.write().await;
        let pending_job = pending
            .get_mut(&job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        pending_job.job.status = ExecutionJobStatus::Running;
        pending_job.job.started_at = Some(Utc::now());
        pending_job.job.worker_id = Some(worker_id.to_string());
        pending_job.job.lease_expires_at = Some(Utc::now() + chrono::Duration::minutes(5));
        pending_job.job.attempt_count += 1;
        Ok(pending_job.job.clone())
    }

    async fn complete(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        if self.kind == BrokerQueueKind::Redis {
            let config = self.redis_config().await?;
            self.ack_redis_job(config, job_id).await?;
        } else {
            self.broker_config().await?;
        }
        let mut pending = self.pending.write().await;
        let pending_job = pending
            .get_mut(&job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        pending_job.job.status = ExecutionJobStatus::Completed;
        pending_job.job.completed_at = Some(Utc::now());
        pending_job.job.lease_expires_at = None;
        Ok(pending_job.job.clone())
    }

    async fn fail(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        if self.kind == BrokerQueueKind::Redis {
            let config = self.redis_config().await?;
            self.ack_redis_job(config, job_id).await?;
        } else {
            self.broker_config().await?;
        }
        let mut pending = self.pending.write().await;
        let pending_job = pending
            .get_mut(&job_id)
            .ok_or_else(|| AppError::not_found("execution job not found"))?;
        pending_job.job.status = ExecutionJobStatus::Failed;
        pending_job.job.completed_at = Some(Utc::now());
        pending_job.job.lease_expires_at = None;
        Ok(pending_job.job.clone())
    }

    async fn retry_or_fail(&self, job_id: Uuid, error: &str) -> Result<ExecutionJob, AppError> {
        let (job, message_id) = {
            let mut pending = self.pending.write().await;
            let pending_job = pending
                .get_mut(&job_id)
                .ok_or_else(|| AppError::not_found("execution job not found"))?;
            pending_job.job.last_error = Some(error.to_string());
            if pending_job.job.attempt_count < pending_job.job.max_attempts {
                pending_job.job.status = ExecutionJobStatus::Queued;
                pending_job.job.started_at = None;
                pending_job.job.completed_at = None;
                pending_job.job.worker_id = None;
                pending_job.job.lease_expires_at = None;
                (pending_job.job.clone(), None)
            } else {
                pending_job.job.status = ExecutionJobStatus::Failed;
                pending_job.job.completed_at = Some(Utc::now());
                pending_job.job.lease_expires_at = None;
                (
                    pending_job.job.clone(),
                    Some(pending_job.message_id.clone()),
                )
            }
        };
        if let Some(message_id) = message_id {
            if self.kind == BrokerQueueKind::Redis {
                let config = self.redis_config().await?;
                let command = RedisStreamCommand::xack(config, message_id)?;
                RedisStreamClient::execute(config, &command).await?;
            }
        }
        Ok(job)
    }

    async fn list(&self) -> Result<Vec<ExecutionJob>, AppError> {
        let messages = match self.kind {
            BrokerQueueKind::Redis => {
                let config = self.redis_config().await?;
                self.ensure_redis_group(config).await?;
                let command = RedisStreamCommand::xreadgroup_next(config, "api-worker", 10, 1)?;
                let response = RedisStreamClient::execute_raw(config, &command).await?;
                parse_xreadgroup_execution_jobs(&response)?
            }
            BrokerQueueKind::Nats => {
                let config = self.broker_config().await?;
                NatsCoreClient::drain_once(config).await?
            }
        };
        {
            let mut pending = self.pending.write().await;
            for (message_id, payload) in messages {
                let job = payload.into_execution_job();
                pending
                    .entry(job.id)
                    .or_insert(BrokerPendingExecutionJob { message_id, job });
            }
        }
        Ok(self
            .pending
            .read()
            .await
            .values()
            .map(|pending| pending.job.clone())
            .collect())
    }

    async fn get(&self, job_id: Uuid) -> Result<ExecutionJob, AppError> {
        self.broker_config().await?;
        self.pending
            .read()
            .await
            .get(&job_id)
            .map(|pending| pending.job.clone())
            .ok_or_else(|| AppError::not_found("execution job not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BrokerExecutionQueue, BrokerQueueConfig, BrokerQueueHealthCheck, BrokerQueueKind,
        RedisExecutionJobPayload, RedisStreamClient, RedisStreamCommand,
        ReservedBrokerQueueHealthCheck, encode_resp_array, nats_tcp_addr, parse_nats_messages,
        parse_redis_response, parse_xreadgroup_execution_jobs, redis_tcp_addr,
    };
    use crate::execution_queue::{ExecutionJobRequest, ExecutionJobStatus, ExecutionQueueBackend};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

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
            max_attempts: None,
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
    fn nats_core_client_parses_endpoint_and_messages() {
        assert_eq!(
            nats_tcp_addr("nats://127.0.0.1:4222").expect("addr"),
            "127.0.0.1:4222"
        );
        assert!(nats_tcp_addr("http://127.0.0.1:4222").is_err());
        assert!(nats_tcp_addr("nats://127.0.0.1").is_err());
        let payload = "{\"job_id\":\"00000000-0000-4000-8000-000000000004\",\"session_id\":\"00000000-0000-4000-8000-000000000001\",\"approval_id\":\"00000000-0000-4000-8000-000000000002\",\"tool_call_id\":\"00000000-0000-4000-8000-000000000003\",\"tool_name\":\"file.write\"}";
        let response = format!(
            "INFO {{}}\r\nMSG mandoforge.execution.jobs 1 {}\r\n{}\r\n",
            payload.len(),
            payload
        );

        let jobs = parse_nats_messages(&response).expect("nats message payload");

        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].0.starts_with("nats:"));
        assert_eq!(jobs[0].1.tool_name, "file.write");
    }

    #[test]
    fn redis_stream_client_parses_xreadgroup_payloads() {
        let payload = "{\"job_id\":\"00000000-0000-4000-8000-000000000004\",\"session_id\":\"00000000-0000-4000-8000-000000000001\",\"approval_id\":\"00000000-0000-4000-8000-000000000002\",\"tool_call_id\":\"00000000-0000-4000-8000-000000000003\",\"tool_name\":\"file.write\"}";
        let response = format!(
            "*1\r\n*2\r\n$25\r\nmandoforge:execution-jobs\r\n*1\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$7\r\npayload\r\n${}\r\n{}\r\n",
            payload.len(),
            payload
        );

        let jobs = parse_xreadgroup_execution_jobs(&response).expect("xreadgroup payload");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, "1-0");
        assert_eq!(jobs[0].1.tool_name, "file.write");
        assert_eq!(
            jobs[0].1.job_id.expect("job id").to_string(),
            "00000000-0000-4000-8000-000000000004"
        );
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
            max_attempts: None,
        };
        let payload = RedisExecutionJobPayload::from_request(&request);
        let command = RedisStreamCommand::xadd_enqueue(&config, &payload).expect("xadd command");

        let response = RedisStreamClient::execute(&config, &command)
            .await
            .expect("redis response");

        server.await.expect("server");
        assert_eq!(response, "1-0");
    }

    #[tokio::test]
    async fn broker_execution_queue_enqueues_to_redis_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0; 4096];
            let bytes = socket.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
            assert!(request.contains("file.write"));
            socket.write_all(b"$3\r\n1-0\r\n").await.expect("write");
        });
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some(format!("redis://{addr}/0")),
            _ => None,
        })
        .expect("redis config");
        let queue = BrokerExecutionQueue::redis(config);
        let job = queue
            .enqueue(ExecutionJobRequest {
                session_id: "00000000-0000-4000-8000-000000000001"
                    .parse()
                    .expect("session id"),
                approval_id: "00000000-0000-4000-8000-000000000002"
                    .parse()
                    .expect("approval id"),
                tool_call_id: "00000000-0000-4000-8000-000000000003"
                    .parse()
                    .expect("tool call id"),
                tool_name: "file.write".to_string(),
                max_attempts: None,
            })
            .await
            .expect("enqueue redis job");

        server.await.expect("server");
        assert_eq!(job.status, ExecutionJobStatus::Queued);
        assert_eq!(job.tool_name, "file.write");
    }

    #[tokio::test]
    async fn broker_execution_queue_enqueues_to_nats_core() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind nats");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept nats");
            socket
                .write_all(b"INFO {\"server_id\":\"test\"}\r\n")
                .await
                .expect("write info");
            let mut command = String::new();
            let mut buffer = vec![0; 4096];
            for _ in 0..8 {
                let bytes = socket.read(&mut buffer).await.expect("read command");
                if bytes == 0 {
                    break;
                }
                command.push_str(&String::from_utf8_lossy(&buffer[..bytes]));
                if command.contains("\"tool_name\":\"codex.exec\"") {
                    break;
                }
            }
            command
        });
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Nats, |key| match key {
            "MANDOFORGE_NATS_URL" => Some(format!("nats://{addr}")),
            _ => None,
        })
        .expect("nats config");
        let queue = BrokerExecutionQueue::nats(config);
        let request = ExecutionJobRequest {
            session_id: Uuid::new_v4(),
            approval_id: Uuid::new_v4(),
            tool_call_id: Uuid::new_v4(),
            tool_name: "codex.exec".to_string(),
            max_attempts: Some(2),
        };

        let job = queue.enqueue(request).await.expect("enqueue to nats");
        let command = server.await.expect("server command");

        assert_eq!(job.status, ExecutionJobStatus::Queued);
        assert!(command.contains("CONNECT"));
        assert!(command.contains("PUB mandoforge.execution.jobs"));
        assert!(command.contains("\"tool_name\":\"codex.exec\""));
    }

    #[tokio::test]
    async fn broker_execution_queue_drains_and_acks_redis_stream_jobs() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let payload = "{\"job_id\":\"00000000-0000-4000-8000-000000000004\",\"session_id\":\"00000000-0000-4000-8000-000000000001\",\"approval_id\":\"00000000-0000-4000-8000-000000000002\",\"tool_call_id\":\"00000000-0000-4000-8000-000000000003\",\"tool_name\":\"file.write\"}";
        let read_response = format!(
            "*1\r\n*2\r\n$25\r\nmandoforge:execution-jobs\r\n*1\r\n*2\r\n$3\r\n1-0\r\n*2\r\n$7\r\npayload\r\n${}\r\n{}\r\n",
            payload.len(),
            payload
        );
        let server = tokio::spawn(async move {
            for step in 0..3 {
                let (mut socket, _) = listener.accept().await.expect("accept");
                let mut buffer = vec![0; 4096];
                let bytes = socket.read(&mut buffer).await.expect("read");
                let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
                match step {
                    0 => {
                        assert!(request.contains("XGROUP"));
                        socket.write_all(b"+OK\r\n").await.expect("write");
                    }
                    1 => {
                        assert!(request.contains("XREADGROUP"));
                        socket
                            .write_all(read_response.as_bytes())
                            .await
                            .expect("write");
                    }
                    _ => {
                        assert!(request.contains("XACK"));
                        assert!(request.contains("1-0"));
                        socket.write_all(b":1\r\n").await.expect("write");
                    }
                }
            }
        });
        let config = BrokerQueueConfig::from_lookup(BrokerQueueKind::Redis, |key| match key {
            "MANDOFORGE_REDIS_URL" => Some(format!("redis://{addr}/0")),
            _ => None,
        })
        .expect("redis config");
        let queue = BrokerExecutionQueue::redis(config);

        let jobs = queue.list().await.expect("read redis jobs");
        let job_id = "00000000-0000-4000-8000-000000000004"
            .parse()
            .expect("job id");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, job_id);
        assert_eq!(jobs[0].status, ExecutionJobStatus::Queued);

        let running = queue.start(job_id, "worker-1").await.expect("start job");
        assert_eq!(running.status, ExecutionJobStatus::Running);
        assert_eq!(running.worker_id.as_deref(), Some("worker-1"));

        let completed = queue.complete(job_id).await.expect("ack job");
        assert_eq!(completed.status, ExecutionJobStatus::Completed);

        server.await.expect("server");
    }
}
