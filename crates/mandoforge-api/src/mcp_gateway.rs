use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct McpGatewayConfig {
    pub(crate) endpoint: String,
    pub(crate) timeout_seconds: u64,
    pub(crate) allowed_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct McpCallRequest {
    pub(crate) server: String,
    pub(crate) tool: String,
    #[serde(default)]
    pub(crate) args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct McpCallResponse {
    pub(crate) result: Value,
}

#[allow(dead_code)]
impl McpGatewayConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = lookup("MANDOFORGE_MCP_GATEWAY_URL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::bad_request("MANDOFORGE_MCP_GATEWAY_URL is required"))?;
        let timeout_seconds = lookup("MANDOFORGE_MCP_TIMEOUT_SECONDS")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(10);
        let allowed_servers = lookup("MANDOFORGE_MCP_ALLOWED_SERVERS")
            .map(|value| parse_csv(&value))
            .filter(|servers| !servers.is_empty())
            .ok_or_else(|| {
                AppError::bad_request(
                    "MANDOFORGE_MCP_ALLOWED_SERVERS must list at least one server",
                )
            })?;

        Ok(Self {
            endpoint,
            timeout_seconds,
            allowed_servers,
        })
    }

    pub(crate) fn allows_server(&self, server: &str) -> bool {
        self.allowed_servers.iter().any(|allowed| allowed == server)
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait McpGatewayClient: Send + Sync {
    async fn health_check(&self, config: &McpGatewayConfig) -> Result<(), AppError>;

    async fn call(
        &self,
        config: &McpGatewayConfig,
        request: McpCallRequest,
    ) -> Result<McpCallResponse, AppError>;
}

#[allow(dead_code)]
pub(crate) struct ReservedMcpGatewayClient;

#[async_trait]
impl McpGatewayClient for ReservedMcpGatewayClient {
    async fn health_check(&self, _config: &McpGatewayConfig) -> Result<(), AppError> {
        Err(AppError::bad_request(
            "MCP gateway health check is reserved but not implemented",
        ))
    }

    async fn call(
        &self,
        config: &McpGatewayConfig,
        request: McpCallRequest,
    ) -> Result<McpCallResponse, AppError> {
        if !config.allows_server(&request.server) {
            return Err(AppError::forbidden(format!(
                "MCP server {} is not allowed",
                request.server
            )));
        }

        Err(AppError::bad_request(
            "MCP gateway call is reserved but not implemented",
        ))
    }
}

fn parse_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        McpCallRequest, McpGatewayClient, McpGatewayConfig, ReservedMcpGatewayClient, parse_csv,
    };

    #[test]
    fn parses_mcp_gateway_config_with_required_allowlist() {
        let config = McpGatewayConfig::from_lookup(|key| match key {
            "MANDOFORGE_MCP_GATEWAY_URL" => Some("http://127.0.0.1:9900".to_string()),
            "MANDOFORGE_MCP_TIMEOUT_SECONDS" => Some("15".to_string()),
            "MANDOFORGE_MCP_ALLOWED_SERVERS" => Some("warehouse, docs ".to_string()),
            _ => None,
        })
        .expect("mcp config");

        assert_eq!(config.endpoint, "http://127.0.0.1:9900");
        assert_eq!(config.timeout_seconds, 15);
        assert!(config.allows_server("warehouse"));
        assert!(config.allows_server("docs"));
        assert!(!config.allows_server("browser"));
    }

    #[test]
    fn mcp_gateway_config_requires_endpoint_and_servers() {
        assert!(McpGatewayConfig::from_lookup(|_| None).is_err());
        assert!(
            McpGatewayConfig::from_lookup(|key| match key {
                "MANDOFORGE_MCP_GATEWAY_URL" => Some("http://127.0.0.1:9900".to_string()),
                _ => None,
            })
            .is_err()
        );
    }

    #[test]
    fn parses_csv_without_empty_items() {
        assert_eq!(parse_csv(" warehouse, ,docs "), vec!["warehouse", "docs"]);
    }

    #[tokio::test]
    async fn reserved_mcp_gateway_client_fails_closed() {
        let config = McpGatewayConfig::from_lookup(|key| match key {
            "MANDOFORGE_MCP_GATEWAY_URL" => Some("http://127.0.0.1:9900".to_string()),
            "MANDOFORGE_MCP_ALLOWED_SERVERS" => Some("warehouse".to_string()),
            _ => None,
        })
        .expect("mcp config");
        let client = ReservedMcpGatewayClient;

        assert!(client.health_check(&config).await.is_err());
        assert!(
            client
                .call(
                    &config,
                    McpCallRequest {
                        server: "browser".to_string(),
                        tool: "open".to_string(),
                        args: json!({}),
                    },
                )
                .await
                .is_err()
        );
        assert!(
            client
                .call(
                    &config,
                    McpCallRequest {
                        server: "warehouse".to_string(),
                        tool: "query".to_string(),
                        args: json!({}),
                    },
                )
                .await
                .is_err()
        );
    }
}
