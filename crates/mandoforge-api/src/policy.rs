use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::AppError;

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
struct ApprovalRequiredRule {
    tool: String,
    risk: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SqlPolicy {
    #[serde(default = "default_sql_max_rows")]
    pub(crate) max_rows: i64,
    #[serde(default)]
    pub(crate) blocked_keywords: Vec<String>,
}

#[derive(Debug, Clone)]
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
        "shell.exec" | "codex.exec" | "http.request" => "high",
        _ => "unknown",
    }
}

fn default_sql_max_rows() -> i64 {
    500
}
