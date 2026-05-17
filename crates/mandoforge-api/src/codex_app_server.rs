use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CodexAppServerConfig {
    pub(crate) endpoint: String,
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexThreadRequest {
    #[serde(default)]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexThreadResponse {
    pub(crate) thread_id: String,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexTurnRequest {
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexTurnResponse {
    pub(crate) turn_id: String,
    #[serde(default)]
    pub(crate) thread_id: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexCommandRequest {
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexCommandResponse {
    pub(crate) command_id: String,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct CodexInterruptResponse {
    pub(crate) turn_id: String,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[allow(dead_code)]
impl CodexAppServerConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = lookup("MANDOFORGE_CODEX_APP_SERVER_URL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::bad_request("MANDOFORGE_CODEX_APP_SERVER_URL is required"))?;
        let timeout_seconds = lookup("MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30);

        Ok(Self {
            endpoint,
            timeout_seconds,
        })
    }

    fn normalized_endpoint(&self) -> String {
        self.endpoint.trim_end_matches('/').to_string()
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait CodexAppServerClient: Send + Sync {
    async fn health_check(&self, config: &CodexAppServerConfig) -> Result<(), AppError>;

    async fn create_thread(
        &self,
        config: &CodexAppServerConfig,
        request: CodexThreadRequest,
    ) -> Result<CodexThreadResponse, AppError>;

    async fn create_turn(
        &self,
        config: &CodexAppServerConfig,
        thread_id: &str,
        request: CodexTurnRequest,
    ) -> Result<CodexTurnResponse, AppError>;

    async fn get_turn_status(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
    ) -> Result<CodexTurnResponse, AppError>;

    async fn interrupt_turn(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
    ) -> Result<CodexInterruptResponse, AppError>;

    async fn execute_command(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
        request: CodexCommandRequest,
    ) -> Result<CodexCommandResponse, AppError>;
}

#[allow(dead_code)]
pub(crate) struct ReservedCodexAppServerClient;

#[async_trait]
impl CodexAppServerClient for ReservedCodexAppServerClient {
    async fn health_check(&self, _config: &CodexAppServerConfig) -> Result<(), AppError> {
        Err(AppError::bad_request(
            "Codex App Server health check is reserved but not configured",
        ))
    }

    async fn create_thread(
        &self,
        _config: &CodexAppServerConfig,
        _request: CodexThreadRequest,
    ) -> Result<CodexThreadResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server thread creation is reserved but not configured",
        ))
    }

    async fn create_turn(
        &self,
        _config: &CodexAppServerConfig,
        _thread_id: &str,
        _request: CodexTurnRequest,
    ) -> Result<CodexTurnResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server turn creation is reserved but not configured",
        ))
    }

    async fn get_turn_status(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
    ) -> Result<CodexTurnResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server turn polling is reserved but not configured",
        ))
    }

    async fn interrupt_turn(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
    ) -> Result<CodexInterruptResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server interrupt is reserved but not configured",
        ))
    }

    async fn execute_command(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
        _request: CodexCommandRequest,
    ) -> Result<CodexCommandResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server command execution is reserved but not configured",
        ))
    }
}

#[allow(dead_code)]
pub(crate) struct HttpCodexAppServerClient {
    client: reqwest::Client,
}

#[allow(dead_code)]
pub(crate) struct WsCodexAppServerClient;

#[allow(dead_code)]
impl HttpCodexAppServerClient {
    pub(crate) fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
        })
    }

    fn health_url(config: &CodexAppServerConfig) -> String {
        format!("{}/healthz", config.normalized_endpoint())
    }

    fn threads_url(config: &CodexAppServerConfig) -> String {
        format!("{}/threads", config.normalized_endpoint())
    }

    fn turns_url(config: &CodexAppServerConfig, thread_id: &str) -> String {
        format!("{}/threads/{thread_id}/turns", config.normalized_endpoint())
    }

    fn turn_url(config: &CodexAppServerConfig, turn_id: &str) -> String {
        format!("{}/turns/{turn_id}", config.normalized_endpoint())
    }

    fn interrupt_url(config: &CodexAppServerConfig, turn_id: &str) -> String {
        format!("{}/turns/{turn_id}/interrupt", config.normalized_endpoint())
    }

    fn commands_url(config: &CodexAppServerConfig, turn_id: &str) -> String {
        format!("{}/turns/{turn_id}/commands", config.normalized_endpoint())
    }
}

#[async_trait]
impl CodexAppServerClient for WsCodexAppServerClient {
    async fn health_check(&self, config: &CodexAppServerConfig) -> Result<(), AppError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "mandoforge",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true,
                    "optOutNotificationMethods": []
                }
            }
        });
        let (mut socket, _) = tokio::time::timeout(
            Duration::from_secs(config.timeout_seconds),
            connect_async(config.normalized_endpoint()),
        )
        .await?
        .map_err(|error| {
            AppError::bad_request(format!("Codex App Server websocket connect failed: {error}"))
        })?;
        tokio::time::timeout(
            Duration::from_secs(config.timeout_seconds),
            socket.send(Message::Text(request.to_string())),
        )
        .await?
        .map_err(|error| {
            AppError::bad_request(format!("Codex App Server initialize send failed: {error}"))
        })?;

        loop {
            let message = tokio::time::timeout(
                Duration::from_secs(config.timeout_seconds),
                socket.next(),
            )
            .await?
            .ok_or_else(|| {
                AppError::bad_request("Codex App Server websocket closed before initialize response")
            })?
            .map_err(|error| {
                AppError::bad_request(format!(
                    "Codex App Server initialize receive failed: {error}"
                ))
            })?;
            let payload = match message {
                Message::Text(text) => text,
                Message::Binary(bytes) => String::from_utf8(bytes).map_err(|error| {
                    AppError::bad_request(format!(
                        "Codex App Server initialize response was not UTF-8: {error}"
                    ))
                })?,
                Message::Close(_) => {
                    return Err(AppError::bad_request(
                        "Codex App Server websocket closed before initialize response",
                    ));
                }
                _ => continue,
            };
            let response: Value = serde_json::from_str(&payload)?;
            if response.get("id").and_then(Value::as_i64) != Some(1) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(AppError::bad_request(format!(
                    "Codex App Server initialize failed: {error}"
                )));
            }
            if response.get("result").is_some() {
                return Ok(());
            }
            return Err(AppError::bad_request(
                "Codex App Server initialize response did not include result",
            ));
        }
    }

    async fn create_thread(
        &self,
        _config: &CodexAppServerConfig,
        _request: CodexThreadRequest,
    ) -> Result<CodexThreadResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server websocket steering is not implemented; use HTTP adapter for thread creation",
        ))
    }

    async fn create_turn(
        &self,
        _config: &CodexAppServerConfig,
        _thread_id: &str,
        _request: CodexTurnRequest,
    ) -> Result<CodexTurnResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server websocket steering is not implemented; use HTTP adapter for turn creation",
        ))
    }

    async fn get_turn_status(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
    ) -> Result<CodexTurnResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server websocket steering is not implemented; use HTTP adapter for turn polling",
        ))
    }

    async fn interrupt_turn(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
    ) -> Result<CodexInterruptResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server websocket steering is not implemented; use HTTP adapter for interrupts",
        ))
    }

    async fn execute_command(
        &self,
        _config: &CodexAppServerConfig,
        _turn_id: &str,
        _request: CodexCommandRequest,
    ) -> Result<CodexCommandResponse, AppError> {
        Err(AppError::bad_request(
            "Codex App Server websocket steering is not implemented; use HTTP adapter for command execution",
        ))
    }
}

#[async_trait]
impl CodexAppServerClient for HttpCodexAppServerClient {
    async fn health_check(&self, config: &CodexAppServerConfig) -> Result<(), AppError> {
        let response = tokio::time::timeout(
            Duration::from_secs(config.timeout_seconds),
            self.client.get(Self::health_url(config)).send(),
        )
        .await??;
        if response.status().is_success() {
            return Ok(());
        }
        Err(AppError::bad_request(format!(
            "Codex App Server health check failed with status {}",
            response.status()
        )))
    }

    async fn create_thread(
        &self,
        config: &CodexAppServerConfig,
        request: CodexThreadRequest,
    ) -> Result<CodexThreadResponse, AppError> {
        post_json(
            &self.client,
            config,
            Self::threads_url(config),
            &request,
            "Codex App Server thread creation",
        )
        .await
    }

    async fn create_turn(
        &self,
        config: &CodexAppServerConfig,
        thread_id: &str,
        request: CodexTurnRequest,
    ) -> Result<CodexTurnResponse, AppError> {
        post_json(
            &self.client,
            config,
            Self::turns_url(config, thread_id),
            &request,
            "Codex App Server turn creation",
        )
        .await
    }

    async fn get_turn_status(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
    ) -> Result<CodexTurnResponse, AppError> {
        get_json(
            &self.client,
            config,
            Self::turn_url(config, turn_id),
            "Codex App Server turn polling",
        )
        .await
    }

    async fn interrupt_turn(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
    ) -> Result<CodexInterruptResponse, AppError> {
        post_json(
            &self.client,
            config,
            Self::interrupt_url(config, turn_id),
            &json!({}),
            "Codex App Server interrupt",
        )
        .await
    }

    async fn execute_command(
        &self,
        config: &CodexAppServerConfig,
        turn_id: &str,
        request: CodexCommandRequest,
    ) -> Result<CodexCommandResponse, AppError> {
        post_json(
            &self.client,
            config,
            Self::commands_url(config, turn_id),
            &request,
            "Codex App Server command execution",
        )
        .await
    }
}

async fn post_json<T, R>(
    client: &reqwest::Client,
    config: &CodexAppServerConfig,
    url: String,
    request: &T,
    label: &str,
) -> Result<R, AppError>
where
    T: Serialize + ?Sized,
    R: for<'de> Deserialize<'de>,
{
    let response = tokio::time::timeout(
        Duration::from_secs(config.timeout_seconds),
        client.post(url).json(request).send(),
    )
    .await??;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "{label} failed with status {status}"
        )));
    }
    Ok(response.json::<R>().await?)
}

async fn get_json<R>(
    client: &reqwest::Client,
    config: &CodexAppServerConfig,
    url: String,
    label: &str,
) -> Result<R, AppError>
where
    R: for<'de> Deserialize<'de>,
{
    let response = tokio::time::timeout(
        Duration::from_secs(config.timeout_seconds),
        client.get(url).send(),
    )
    .await??;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "{label} failed with status {status}"
        )));
    }
    Ok(response.json::<R>().await?)
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, routing::get, routing::post};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio_tungstenite::tungstenite::Message;

    use super::{
        CodexAppServerClient, CodexAppServerConfig, CodexCommandRequest, CodexCommandResponse,
        CodexInterruptResponse, CodexThreadRequest, CodexThreadResponse, CodexTurnRequest,
        CodexTurnResponse, HttpCodexAppServerClient, ReservedCodexAppServerClient,
        WsCodexAppServerClient,
    };

    async fn mock_health() -> Json<serde_json::Value> {
        Json(json!({"status": "ok"}))
    }

    async fn mock_create_thread(
        Json(request): Json<CodexThreadRequest>,
    ) -> Json<CodexThreadResponse> {
        Json(CodexThreadResponse {
            thread_id: "thread-1".to_string(),
            status: Some("created".to_string()),
            metadata: request.metadata,
        })
    }

    async fn mock_create_turn(Json(request): Json<CodexTurnRequest>) -> Json<CodexTurnResponse> {
        Json(CodexTurnResponse {
            turn_id: "turn-1".to_string(),
            thread_id: Some("thread-1".to_string()),
            status: Some("running".to_string()),
            result: json!({"message": request.message}),
        })
    }

    async fn mock_get_turn() -> Json<CodexTurnResponse> {
        Json(CodexTurnResponse {
            turn_id: "turn-1".to_string(),
            thread_id: Some("thread-1".to_string()),
            status: Some("completed".to_string()),
            result: json!({"final": "done"}),
        })
    }

    async fn mock_interrupt() -> Json<CodexInterruptResponse> {
        Json(CodexInterruptResponse {
            turn_id: "turn-1".to_string(),
            status: Some("interrupted".to_string()),
        })
    }

    async fn mock_command(Json(request): Json<CodexCommandRequest>) -> Json<CodexCommandResponse> {
        Json(CodexCommandResponse {
            command_id: "command-1".to_string(),
            status: Some("completed".to_string()),
            result: json!({"command": request.command, "args": request.args}),
        })
    }

    #[test]
    fn codex_app_server_config_parses_endpoint_and_timeout() {
        let config = CodexAppServerConfig::from_lookup(|key| match key {
            "MANDOFORGE_CODEX_APP_SERVER_URL" => Some("http://127.0.0.1:9901/".to_string()),
            "MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS" => Some("9".to_string()),
            _ => None,
        })
        .expect("codex app server config");

        assert_eq!(config.endpoint, "http://127.0.0.1:9901/");
        assert_eq!(config.timeout_seconds, 9);
    }

    #[test]
    fn codex_app_server_config_requires_endpoint() {
        assert!(CodexAppServerConfig::from_lookup(|_| None).is_err());
    }

    #[tokio::test]
    async fn reserved_codex_app_server_client_fails_closed() {
        let config = CodexAppServerConfig::from_lookup(|key| match key {
            "MANDOFORGE_CODEX_APP_SERVER_URL" => Some("http://127.0.0.1:9901".to_string()),
            _ => None,
        })
        .expect("codex app server config");
        let client = ReservedCodexAppServerClient;

        assert!(client.health_check(&config).await.is_err());
        assert!(
            client
                .create_thread(
                    &config,
                    CodexThreadRequest {
                        metadata: json!({})
                    },
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn http_codex_app_server_client_calls_thread_turn_interrupt_and_command() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let app = Router::new()
            .route("/healthz", get(mock_health))
            .route("/threads", post(mock_create_thread))
            .route("/threads/thread-1/turns", post(mock_create_turn))
            .route("/turns/turn-1", get(mock_get_turn))
            .route("/turns/turn-1/interrupt", post(mock_interrupt))
            .route("/turns/turn-1/commands", post(mock_command));
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock codex app server");
        });
        let config = CodexAppServerConfig::from_lookup(|key| match key {
            "MANDOFORGE_CODEX_APP_SERVER_URL" => Some(format!("http://{addr}/")),
            "MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS" => Some("2".to_string()),
            _ => None,
        })
        .expect("codex app server config");
        let client = HttpCodexAppServerClient::new().expect("client");

        client.health_check(&config).await.expect("health");
        let thread = client
            .create_thread(
                &config,
                CodexThreadRequest {
                    metadata: json!({"session_id": "session-1"}),
                },
            )
            .await
            .expect("thread");
        assert_eq!(thread.thread_id, "thread-1");

        let turn = client
            .create_turn(
                &config,
                &thread.thread_id,
                CodexTurnRequest {
                    message: "Inspect workspace".to_string(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("turn");
        assert_eq!(turn.turn_id, "turn-1");
        assert_eq!(turn.result["message"], "Inspect workspace");

        let turn_status = client
            .get_turn_status(&config, &turn.turn_id)
            .await
            .expect("turn status");
        assert_eq!(turn_status.status.as_deref(), Some("completed"));
        assert_eq!(turn_status.result["final"], "done");

        let command = client
            .execute_command(
                &config,
                &turn.turn_id,
                CodexCommandRequest {
                    command: "ls".to_string(),
                    args: json!({"cwd": "/workspace"}),
                },
            )
            .await
            .expect("command");
        assert_eq!(command.command_id, "command-1");
        assert_eq!(command.result["command"], "ls");

        let interrupt = client
            .interrupt_turn(&config, &turn.turn_id)
            .await
            .expect("interrupt");
        assert_eq!(interrupt.status.as_deref(), Some("interrupted"));
        server.abort();
    }

    #[tokio::test]
    async fn ws_codex_app_server_client_initializes_for_health() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("websocket");
            let message = socket
                .next()
                .await
                .expect("message")
                .expect("websocket message");
            let request: serde_json::Value =
                serde_json::from_str(message.to_text().expect("text")).expect("json");
            assert_eq!(request["method"], "initialize");
            assert_eq!(request["params"]["clientInfo"]["name"], "mandoforge");
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": {
                            "codexHome": "/tmp/codex",
                            "platformFamily": "unix",
                            "platformOs": "linux",
                            "userAgent": "codex-test"
                        }
                    })
                    .to_string(),
                ))
                .await
                .expect("send response");
        });
        let config = CodexAppServerConfig::from_lookup(|key| match key {
            "MANDOFORGE_CODEX_APP_SERVER_URL" => Some(format!("ws://{addr}")),
            "MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS" => Some("2".to_string()),
            _ => None,
        })
        .expect("config");
        let client = WsCodexAppServerClient;

        client.health_check(&config).await.expect("health");
        server.await.expect("server task");
    }
}
