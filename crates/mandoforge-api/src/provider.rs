use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HarnessContext {
    pub(crate) session_id: Uuid,
    pub(crate) event_count: usize,
    pub(crate) last_user_message: Option<String>,
    pub(crate) approved_tool_result_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderToolCall {
    pub(crate) tool_name: String,
    pub(crate) args: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderResponse {
    pub(crate) plan: Vec<String>,
    pub(crate) tool_calls: Vec<ProviderToolCall>,
    pub(crate) final_message: Option<String>,
}

#[async_trait]
pub(crate) trait ProviderClient: Send + Sync {
    fn name(&self) -> &'static str;

    async fn complete(&self, context: HarnessContext) -> Result<ProviderResponse, AppError>;
}

pub(crate) struct MockProviderClient;

pub(crate) struct OpenAiCompatibleProviderClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

#[async_trait]
impl ProviderClient for MockProviderClient {
    fn name(&self) -> &'static str {
        "mock-openai-compatible"
    }

    async fn complete(&self, context: HarnessContext) -> Result<ProviderResponse, AppError> {
        if context.approved_tool_result_count > 0 {
            return Ok(ProviderResponse {
                plan: vec!["Review approved tool output and produce the final response".to_string()],
                tool_calls: Vec::new(),
                final_message: Some(
                    "Approved execution completed. The session timeline now contains the approved tool result and final provider response."
                        .to_string(),
                ),
            });
        }
        Ok(ProviderResponse {
            plan: vec![
                "Read README and Stage 1 policy/config from the workspace".to_string(),
                "Query generic_demo.platform_events for recent session health".to_string(),
                "Request approval before shell execution or writing diagnostics.md".to_string(),
                "Create diagnostics.md as an artifact and emit a final summary".to_string(),
            ],
            tool_calls: vec![
                ProviderToolCall {
                    tool_name: "file.read".to_string(),
                    args: json!({"paths": ["README.md", "config/policy.stage1.yaml"]}),
                },
                ProviderToolCall {
                    tool_name: "sql.get_schema".to_string(),
                    args: json!({"schema": "generic_demo"}),
                },
                ProviderToolCall {
                    tool_name: "sql.query".to_string(),
                    args: json!({"sql": "select event_type, status, count(*) from generic_demo.platform_events where created_at >= now() - interval '24 hours' group by event_type, status"}),
                },
                ProviderToolCall {
                    tool_name: "shell.exec".to_string(),
                    args: json!({"command": "pwd"}),
                },
            ],
            final_message: None,
        })
    }
}

impl OpenAiCompatibleProviderClient {
    pub(crate) fn from_env() -> Result<Option<Self>, AppError> {
        let Ok(base_url) = std::env::var("MANDOFORGE_PROVIDER_BASE_URL") else {
            return Ok(None);
        };
        let Ok(api_key) = std::env::var("MANDOFORGE_PROVIDER_API_KEY") else {
            return Ok(None);
        };
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() || api_key.trim().is_empty() {
            return Ok(None);
        }
        let model = std::env::var("MANDOFORGE_PROVIDER_MODEL")
            .unwrap_or_else(|_| "gpt-5.4-mini".to_string())
            .trim()
            .to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Some(Self {
            base_url,
            api_key,
            model,
            client,
        }))
    }
}

#[async_trait]
impl ProviderClient for OpenAiCompatibleProviderClient {
    fn name(&self) -> &'static str {
        "openai-compatible-http"
    }

    async fn complete(&self, context: HarnessContext) -> Result<ProviderResponse, AppError> {
        let endpoint = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are MandoForge's Stage 1 provider harness. Return tool calls only for the supplied generic runtime session. Use available tools through the runtime policy path."
                },
                {
                    "role": "user",
                    "content": serde_json::to_string(&context)?
                }
            ],
            "tools": provider_tool_schemas(),
            "tool_choice": "auto"
        });
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "provider request failed with status {status}: {}",
                redact_provider_error(&value)
            )));
        }
        parse_openai_compatible_provider_response(&value)
    }
}

fn provider_tool_schemas() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "file.read",
                "description": "Read summarized files from the session workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {"paths": {"type": "array", "items": {"type": "string"}}},
                    "required": ["paths"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "sql.get_schema",
                "description": "Return the generic demo database schema.",
                "parameters": {
                    "type": "object",
                    "properties": {"schema": {"type": "string"}},
                    "required": ["schema"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "sql.query",
                "description": "Execute read-only SQL against generic demo data.",
                "parameters": {
                    "type": "object",
                    "properties": {"sql": {"type": "string"}},
                    "required": ["sql"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "shell.exec",
                "description": "Request approval to run a shell command in the session workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {"command": {"type": "string"}},
                    "required": ["command"]
                }
            }
        }
    ])
}

pub(crate) fn parse_openai_compatible_provider_response(
    value: &Value,
) -> Result<ProviderResponse, AppError> {
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| AppError::bad_request("provider response missing choices[0].message"))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(parse_provider_tool_call)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let plan = provider_plan_from_content(content).unwrap_or_else(|| {
        vec![format!(
            "Provider returned {} runtime tool call(s)",
            tool_calls.len()
        )]
    });
    let final_message = (!content.trim().is_empty()).then(|| content.trim().to_string());
    Ok(ProviderResponse {
        plan,
        tool_calls,
        final_message,
    })
}

fn parse_provider_tool_call(value: &Value) -> Option<ProviderToolCall> {
    let function = value.get("function")?;
    let tool_name = function.get("name")?.as_str()?.to_string();
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let args =
        serde_json::from_str(arguments).unwrap_or_else(|_| json!({"raw_arguments": arguments}));
    Some(ProviderToolCall { tool_name, args })
}

fn provider_plan_from_content(content: &str) -> Option<Vec<String>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(plan) = value.get("plan").and_then(Value::as_array) {
            let steps: Vec<_> = plan
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            if !steps.is_empty() {
                return Some(steps);
            }
        }
    }
    Some(
        trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn redact_provider_error(value: &Value) -> String {
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("provider returned an error")
        .to_string()
}
