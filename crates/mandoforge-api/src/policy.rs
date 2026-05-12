use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AgentVersion, AppError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct PolicyConfig {
    #[serde(default)]
    blocked_tools: Vec<String>,
    #[serde(default)]
    approval_required: Vec<ApprovalRequiredRule>,
    #[serde(default)]
    allowed_tools: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub(crate) sql_policy: SqlPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ApprovalRequiredRule {
    tool: String,
    risk: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SqlPolicy {
    #[serde(default = "default_sql_max_rows")]
    pub(crate) max_rows: i64,
    #[serde(default)]
    pub(crate) blocked_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolPolicyDecision {
    pub(crate) decision: &'static str,
    pub(crate) risk_level: String,
    pub(crate) reason: String,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            blocked_tools: vec![
                "secret.read".to_string(),
                "production_db.write".to_string(),
                "system.network.unrestricted".to_string(),
                "shell.exec.unrestricted".to_string(),
            ],
            approval_required: vec![
                ApprovalRequiredRule {
                    tool: "shell.exec".to_string(),
                    risk: "high".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "codex.exec".to_string(),
                    risk: "high".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "file.write".to_string(),
                    risk: "medium".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "http.request".to_string(),
                    risk: "high".to_string(),
                },
            ],
            allowed_tools: HashMap::from([(
                "generic-orchestrator-agent".to_string(),
                vec![
                    "file.read".to_string(),
                    "file.write".to_string(),
                    "sql.get_schema".to_string(),
                    "sql.query".to_string(),
                    "shell.exec".to_string(),
                    "codex.exec".to_string(),
                    "approval.request".to_string(),
                    "artifact.create".to_string(),
                    "mcp.call".to_string(),
                ],
            )]),
            sql_policy: SqlPolicy {
                max_rows: default_sql_max_rows(),
                blocked_keywords: vec![
                    "INSERT".to_string(),
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "DROP".to_string(),
                    "ALTER".to_string(),
                    "CREATE".to_string(),
                    "TRUNCATE".to_string(),
                    "GRANT".to_string(),
                    "REVOKE".to_string(),
                    "COPY".to_string(),
                    "CALL".to_string(),
                    "DO".to_string(),
                ],
            },
        }
    }
}

impl Default for SqlPolicy {
    fn default() -> Self {
        Self {
            max_rows: default_sql_max_rows(),
            blocked_keywords: PolicyConfig::default().sql_policy.blocked_keywords,
        }
    }
}

impl PolicyConfig {
    pub(crate) fn evaluate_tool(&self, name: &str) -> ToolPolicyDecision {
        if self.blocked_tools.iter().any(|tool| tool == name) {
            return ToolPolicyDecision {
                decision: "denied",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!("{name} is blocked by config/policy.stage1.yaml"),
            };
        }

        if let Some(rule) = self.approval_required.iter().find(|rule| rule.tool == name) {
            return ToolPolicyDecision {
                decision: "requires_approval",
                risk_level: rule.risk.clone(),
                reason: format!("{name} requires approval by config/policy.stage1.yaml"),
            };
        }

        if self
            .allowed_tools
            .values()
            .any(|tools| tools.iter().any(|tool| tool == name))
        {
            return ToolPolicyDecision {
                decision: "allowed",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!("{name} is allowed by config/policy.stage1.yaml"),
            };
        }

        ToolPolicyDecision {
            decision: "denied",
            risk_level: "unknown".to_string(),
            reason: format!("{name} is not allowed by config/policy.stage1.yaml"),
        }
    }

    pub(crate) fn evaluate_tool_for_agent_version(
        &self,
        name: &str,
        agent_version: &AgentVersion,
    ) -> ToolPolicyDecision {
        if self.blocked_tools.iter().any(|tool| tool == name) {
            return ToolPolicyDecision {
                decision: "denied",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!("{name} is blocked by config/policy.stage1.yaml"),
            };
        }

        if !agent_version_tool_enabled(agent_version, name) {
            return ToolPolicyDecision {
                decision: "denied",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!(
                    "{name} is not enabled for agent version {}",
                    agent_version.version
                ),
            };
        }

        if let Some(decision) = evaluate_agent_version_policy(name, agent_version) {
            return decision;
        }

        self.evaluate_tool(name)
    }
}

pub(crate) async fn load_policy_config(path: &str) -> Result<PolicyConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read policy config {path}"))?;
    serde_yml::from_str(&content).with_context(|| format!("failed to parse policy config {path}"))
}

pub(crate) fn ensure_read_only_sql_with_policy(
    sql: &str,
    policy: &SqlPolicy,
) -> Result<(), AppError> {
    let lowered = sql.trim().to_lowercase();
    if lowered.matches(';').count() > 1 {
        return Err(AppError::bad_request("only one SQL statement is allowed"));
    }
    if policy.blocked_keywords.iter().any(|keyword| {
        let keyword = keyword.to_lowercase();
        lowered.starts_with(&keyword) || lowered.contains(&format!(" {keyword} "))
    }) {
        return Err(AppError::bad_request(
            "sql.query only accepts read-only SQL",
        ));
    }
    if !lowered.starts_with("select")
        && !lowered.starts_with("with")
        && !lowered.starts_with("explain")
    {
        return Err(AppError::bad_request(
            "sql.query requires SELECT, WITH, or EXPLAIN",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn ensure_read_only_sql(sql: &str) -> Result<(), AppError> {
    ensure_read_only_sql_with_policy(sql, &PolicyConfig::default().sql_policy)
}

fn tool_risk_level(name: &str) -> &'static str {
    match name {
        "file.read" | "sql.get_schema" | "approval.request" | "artifact.create" => "low",
        "file.write" | "sql.query" => "medium",
        "shell.exec" | "codex.exec" | "http.request" | "mcp.call" => "high",
        _ => "unknown",
    }
}

fn agent_version_tool_enabled(agent_version: &AgentVersion, name: &str) -> bool {
    agent_version.tool_names.iter().any(|tool| tool == name)
        || agent_version.tools.iter().any(|tool| tool == name)
}

fn evaluate_agent_version_policy(
    name: &str,
    agent_version: &AgentVersion,
) -> Option<ToolPolicyDecision> {
    let policy = &agent_version.approval_policy;
    if json_string_array_contains(policy.get("blocked_tools"), name) {
        return Some(ToolPolicyDecision {
            decision: "denied",
            risk_level: tool_risk_level(name).to_string(),
            reason: format!(
                "{name} is blocked by agent version {} policy",
                agent_version.version
            ),
        });
    }

    if let Some(allowed_tools) = policy.get("allowed_tools") {
        if !json_string_array_contains(Some(allowed_tools), name) {
            return Some(ToolPolicyDecision {
                decision: "denied",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!(
                    "{name} is not allowed by agent version {} policy",
                    agent_version.version
                ),
            });
        }
    }

    if let Some(risk) = approval_required_risk(policy.get("approval_required"), name) {
        return Some(ToolPolicyDecision {
            decision: "requires_approval",
            risk_level: risk,
            reason: format!(
                "{name} requires approval by agent version {} policy",
                agent_version.version
            ),
        });
    }

    None
}

fn json_string_array_contains(value: Option<&Value>, needle: &str) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(needle)))
}

fn approval_required_risk(value: Option<&Value>, name: &str) -> Option<String> {
    value.and_then(Value::as_array).and_then(|items| {
        items.iter().find_map(|item| {
            if item.as_str() == Some(name) {
                return Some(tool_risk_level(name).to_string());
            }
            let tool = item.get("tool").and_then(Value::as_str)?;
            if tool == name {
                Some(
                    item.get("risk")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| tool_risk_level(name))
                        .to_string(),
                )
            } else {
                None
            }
        })
    })
}

fn default_sql_max_rows() -> i64 {
    500
}
