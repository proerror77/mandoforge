use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError,
    secrets::{SecretProvider, SecretProviderConfig, SecretRef, secret_provider_from_env},
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HarnessContext {
    pub(crate) session_id: Uuid,
    pub(crate) event_count: usize,
    pub(crate) pending_event_seq_start: Option<i64>,
    pub(crate) pending_event_seq_end: Option<i64>,
    pub(crate) pending_event_count: usize,
    pub(crate) last_user_message: Option<String>,
    pub(crate) latest_goal_event: Option<Value>,
    pub(crate) approved_tool_result_count: usize,
    pub(crate) rejected_tool_result_count: usize,
    pub(crate) manual_tool_result_count: usize,
    pub(crate) custom_tool_result_count: usize,
    pub(crate) execution_completed_count: usize,
    pub(crate) recent_custom_tool_results: Vec<Value>,
    pub(crate) recent_execution_completed: Vec<Value>,
    pub(crate) recent_goal_events: Vec<Value>,
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
    pub(crate) usage: Option<ProviderTokenUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderTokenUsage {
    pub(crate) prompt_tokens: i64,
    pub(crate) completion_tokens: i64,
    pub(crate) total_tokens: i64,
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
                usage: Some(ProviderTokenUsage {
                    prompt_tokens: 120,
                    completion_tokens: 45,
                    total_tokens: 165,
                }),
            });
        }
        if context.rejected_tool_result_count > 0 {
            return Ok(ProviderResponse {
                plan: vec!["Record rejected approval and stop the blocked tool path".to_string()],
                tool_calls: Vec::new(),
                final_message: Some(
                    "Approval rejected. The session timeline records the denied tool result and no blocked tool execution was run."
                        .to_string(),
                ),
                usage: Some(ProviderTokenUsage {
                    prompt_tokens: 96,
                    completion_tokens: 32,
                    total_tokens: 128,
                }),
            });
        }
        if context.manual_tool_result_count > 0 {
            return Ok(ProviderResponse {
                plan: vec!["Review manual tool result and continue the session timeline".to_string()],
                tool_calls: Vec::new(),
                final_message: Some(
                    "Manual tool result processed. The session loop consumed the durable tool result event and recorded a final provider response."
                        .to_string(),
                ),
                usage: Some(ProviderTokenUsage {
                    prompt_tokens: 88,
                    completion_tokens: 30,
                    total_tokens: 118,
                }),
            });
        }
        if context.execution_completed_count > 0 {
            return Ok(ProviderResponse {
                plan: vec!["Review completed worker execution and stop dispatching new runtime work".to_string()],
                tool_calls: Vec::new(),
                final_message: Some(
                    "Worker execution completed. The session timeline now contains the Codex App Server run, runtime events, and final tool result."
                        .to_string(),
                ),
                usage: Some(ProviderTokenUsage {
                    prompt_tokens: 104,
                    completion_tokens: 38,
                    total_tokens: 142,
                }),
            });
        }
        if context
            .last_user_message
            .as_deref()
            .is_some_and(looks_like_codex_app_server_delegation_request)
        {
            let task = context
                .last_user_message
                .clone()
                .unwrap_or_else(|| "Open the requested webpage and extract useful facts.".to_string());
            return Ok(ProviderResponse {
                plan: vec![
                    "Delegate the user request to Codex App Server through codex.exec".to_string(),
                    "Wait for the approved worker execution result before finalizing".to_string(),
                ],
                tool_calls: vec![ProviderToolCall {
                    tool_name: "codex.exec".to_string(),
                    args: json!({
                        "task": task,
                        "sandbox_mode": "workspace-write",
                        "execution_strategy": "app-server",
                        "poll_attempts": 6,
                        "poll_interval_ms": 500
                    }),
                }],
                final_message: None,
                usage: Some(ProviderTokenUsage {
                    prompt_tokens: 160,
                    completion_tokens: 42,
                    total_tokens: 202,
                }),
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
            usage: Some(ProviderTokenUsage {
                prompt_tokens: 180,
                completion_tokens: 60,
                total_tokens: 240,
            }),
        })
    }
}

fn looks_like_codex_app_server_delegation_request(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("codex")
        || normalized.contains("cdp")
        || normalized.contains("browser")
        || normalized.contains("webpage")
        || normalized.contains("open a page")
        || normalized.contains("open the page")
        || message.contains("网页")
        || message.contains("打开")
}

impl OpenAiCompatibleProviderClient {
    pub(crate) fn from_parts(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, AppError> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        let api_key = api_key.into();
        let model = model.into();
        if base_url.is_empty() || api_key.trim().is_empty() || model.trim().is_empty() {
            return Err(AppError::bad_request(
                "openai-compatible provider requires base_url, api key, and model",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            base_url,
            api_key,
            model: model.trim().to_string(),
            client,
        })
    }

    pub(crate) async fn from_env() -> Result<Option<Self>, AppError> {
        let secret_provider = secret_provider_from_env()?;
        Self::from_lookup_with_secret_provider(
            |key| std::env::var(key).ok(),
            secret_provider.as_ref(),
        )
        .await
    }

    async fn from_lookup_with_secret_provider<F>(
        lookup: F,
        secret_provider: &dyn SecretProvider,
    ) -> Result<Option<Self>, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let Some(base_url) = lookup("MANDOFORGE_PROVIDER_BASE_URL") else {
            return Ok(None);
        };
        let Some(api_key) = lookup("MANDOFORGE_PROVIDER_API_KEY") else {
            return Ok(None);
        };
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() || api_key.trim().is_empty() {
            return Ok(None);
        }
        let api_key =
            provider_api_key_from_env_value(api_key.trim(), &lookup, secret_provider).await?;
        let model = lookup("MANDOFORGE_PROVIDER_MODEL")
            .unwrap_or_else(|| "gpt-5.4-mini".to_string())
            .trim()
            .to_string();
        Ok(Some(Self::from_parts(base_url, api_key, model)?))
    }
}

async fn provider_api_key_from_env_value<F>(
    value: &str,
    lookup: &F,
    secret_provider: &dyn SecretProvider,
) -> Result<String, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(secret_ref) = provider_api_key_secret_ref(value)? else {
        return Ok(value.to_string());
    };
    let config = SecretProviderConfig::from_lookup(lookup)?;
    let secret = secret_provider.read_secret(&config, &secret_ref).await?;
    Ok(secret.expose_for_provider_use().to_string())
}

pub(crate) async fn provider_api_key_from_stored_value(
    value: &str,
    secret_provider: &dyn SecretProvider,
) -> Result<String, AppError> {
    provider_api_key_from_stored_value_with_lookup(
        value,
        &|key| std::env::var(key).ok(),
        secret_provider,
    )
    .await
}

pub(crate) async fn provider_api_key_from_stored_value_with_lookup<F>(
    value: &str,
    lookup: &F,
    secret_provider: &dyn SecretProvider,
) -> Result<String, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(secret_ref) = provider_api_key_secret_ref(value.trim())? else {
        return Err(AppError::bad_request(
            "stored provider API key must use a vault:path#key reference",
        ));
    };
    let config = SecretProviderConfig::from_lookup(lookup)?;
    let secret = secret_provider.read_secret(&config, &secret_ref).await?;
    Ok(secret.expose_for_provider_use().to_string())
}

fn provider_api_key_secret_ref(value: &str) -> Result<Option<SecretRef>, AppError> {
    let Some(reference) = value.strip_prefix("vault:") else {
        return Ok(None);
    };
    let Some((path, key)) = reference.split_once('#') else {
        return Err(AppError::bad_request(
            "vault provider API key reference must use vault:path#key",
        ));
    };
    Ok(Some(SecretRef::new(path, key)?))
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
        },
        {
            "type": "function",
            "function": {
                "name": "codex.exec",
                "description": "Delegate a task to the Codex App Server worker after policy approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task": {"type": "string"},
                        "sandbox_mode": {"type": "string"},
                        "execution_strategy": {"type": "string"},
                        "poll_attempts": {"type": "integer"},
                        "poll_interval_ms": {"type": "integer"}
                    },
                    "required": ["task"]
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
    let usage = parse_provider_token_usage(value.get("usage"));
    Ok(ProviderResponse {
        plan,
        tool_calls,
        final_message,
        usage,
    })
}

fn parse_provider_token_usage(value: Option<&Value>) -> Option<ProviderTokenUsage> {
    let value = value?;
    let prompt_tokens = value
        .get("prompt_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let completion_tokens = value
        .get("completion_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total_tokens = value
        .get("total_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(prompt_tokens + completion_tokens);
    Some(ProviderTokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
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

#[cfg(test)]
mod tests {
    use super::{
        OpenAiCompatibleProviderClient, provider_api_key_from_env_value,
        provider_api_key_secret_ref,
    };
    use crate::secrets::ReservedSecretProvider;

    #[test]
    fn parses_provider_api_key_vault_reference() {
        let secret_ref =
            provider_api_key_secret_ref("vault:providers/openai#api_key").expect("valid ref");
        let secret_ref = secret_ref.expect("secret ref");

        assert_eq!(secret_ref.path, "providers/openai");
        assert_eq!(secret_ref.key, "api_key");
        assert!(
            provider_api_key_secret_ref("plain-key")
                .expect("plain")
                .is_none()
        );
        assert!(provider_api_key_secret_ref("vault:providers/openai").is_err());
    }

    #[tokio::test]
    async fn openai_provider_uses_direct_env_api_key_without_secret_provider() {
        let provider = OpenAiCompatibleProviderClient::from_lookup_with_secret_provider(
            |key| match key {
                "MANDOFORGE_PROVIDER_BASE_URL" => Some("https://provider.example".to_string()),
                "MANDOFORGE_PROVIDER_API_KEY" => Some("direct-key".to_string()),
                "MANDOFORGE_PROVIDER_MODEL" => Some("model-a".to_string()),
                _ => None,
            },
            &ReservedSecretProvider,
        )
        .await
        .expect("provider")
        .expect("configured provider");

        assert_eq!(provider.base_url, "https://provider.example");
        assert_eq!(provider.api_key, "direct-key");
        assert_eq!(provider.model, "model-a");
    }

    #[tokio::test]
    async fn openai_provider_vault_key_fails_closed_with_reserved_secret_provider() {
        let result = OpenAiCompatibleProviderClient::from_lookup_with_secret_provider(
            |key| match key {
                "MANDOFORGE_PROVIDER_BASE_URL" => Some("https://provider.example".to_string()),
                "MANDOFORGE_PROVIDER_API_KEY" => Some("vault:providers/openai#api_key".to_string()),
                "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
                _ => None,
            },
            &ReservedSecretProvider,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn provider_api_key_value_keeps_plaintext_value() {
        let api_key =
            provider_api_key_from_env_value("direct-key", &|_| None, &ReservedSecretProvider)
                .await
                .expect("direct key");

        assert_eq!(api_key, "direct-key");
    }
}
