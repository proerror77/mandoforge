use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use serde_json::json;
use uuid::Uuid;

use crate::eval_judge::EvalJudgeRequest;
use crate::policy::ensure_read_only_sql_with_policy;
use crate::store_backend::StoreBackend;
use crate::store_rows::{eval_case_from_row, eval_dataset_from_row, eval_run_from_row};
use crate::{
    AppError, AppState, CreateEvalCase, CreateEvalDataset, CreateEvalRun, EvalCase, EvalDataset,
    EvalRun,
};

impl AppState {
    pub(crate) async fn list_eval_datasets(&self) -> Result<Vec<EvalDataset>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut datasets: Vec<_> =
                    inner.read().await.eval_datasets.values().cloned().collect();
                datasets.sort_by_key(|dataset| dataset.created_at);
                datasets.reverse();
                Ok(datasets)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, description, created_at
                     FROM eval_datasets
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(eval_dataset_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_dataset(
        &self,
        input: CreateEvalDataset,
    ) -> Result<EvalDataset, AppError> {
        let dataset = EvalDataset {
            id: Uuid::new_v4(),
            name: input.name,
            description: input.description,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .eval_datasets
                    .insert(dataset.id, dataset.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_datasets (id, tenant_id, name, description, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(dataset.id)
                .bind(self.tenant_id)
                .bind(&dataset.name)
                .bind(&dataset.description)
                .bind(dataset.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(dataset)
    }

    pub(crate) async fn list_eval_cases(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvalCase>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut cases: Vec<_> = inner
                    .read()
                    .await
                    .eval_cases
                    .values()
                    .filter(|case| case.dataset_id == dataset_id)
                    .cloned()
                    .collect();
                cases.sort_by_key(|case| case.created_at);
                cases.reverse();
                Ok(cases)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, dataset_id, input, expected, grading_policy, created_at
                     FROM eval_cases
                     WHERE tenant_id = $1 AND dataset_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(dataset_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(eval_case_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_case(
        &self,
        dataset_id: Uuid,
        input: CreateEvalCase,
    ) -> Result<EvalCase, AppError> {
        self.ensure_eval_dataset_exists(dataset_id).await?;
        let case = EvalCase {
            id: Uuid::new_v4(),
            dataset_id,
            input: input.input,
            expected: input.expected,
            grading_policy: input.grading_policy,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.eval_cases.insert(case.id, case.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_cases (id, tenant_id, dataset_id, input, expected, grading_policy, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(case.id)
                .bind(self.tenant_id)
                .bind(case.dataset_id)
                .bind(&case.input)
                .bind(&case.expected)
                .bind(&case.grading_policy)
                .bind(case.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(case)
    }

    pub(crate) async fn list_eval_runs(
        &self,
        dataset_id: Option<Uuid>,
    ) -> Result<Vec<EvalRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut runs: Vec<_> = inner
                    .read()
                    .await
                    .eval_runs
                    .values()
                    .filter(|run| dataset_id.is_none_or(|id| run.dataset_id == id))
                    .cloned()
                    .collect();
                runs.sort_by_key(|run| run.created_at);
                runs.reverse();
                Ok(runs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match dataset_id {
                    Some(dataset_id) => {
                        sqlx::query(
                            "SELECT id, dataset_id, agent_id, agent_version_id, status, score, details, created_at
                             FROM eval_runs
                             WHERE tenant_id = $1 AND dataset_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(dataset_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, dataset_id, agent_id, agent_version_id, status, score, details, created_at
                             FROM eval_runs
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(eval_run_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_eval_run(
        &self,
        dataset_id: Uuid,
        input: CreateEvalRun,
    ) -> Result<EvalRun, AppError> {
        self.ensure_eval_dataset_exists(dataset_id).await?;
        let agent_version = self.current_agent_version(input.agent_id).await?;
        let cases = self.list_eval_cases(dataset_id).await?;
        let mut case_results = Vec::with_capacity(cases.len());
        for case in &cases {
            case_results.push(self.grade_eval_case(case, &agent_version).await);
        }
        let passed_count = case_results.iter().filter(|result| result.passed).count();
        let case_count = case_results.len();
        let score = if case_count == 0 {
            0.0
        } else {
            passed_count as f64 / case_count as f64
        };
        let status = if passed_count == case_count {
            "completed"
        } else {
            "failed"
        };
        let run = EvalRun {
            id: Uuid::new_v4(),
            dataset_id,
            agent_id: input.agent_id,
            agent_version_id: agent_version.id,
            status: status.to_string(),
            score: Some(score),
            details: json!({
                "runner": "stage2-rule-graders",
                "case_count": case_count,
                "passed_count": passed_count,
                "coverage": ["policy", "tool_selection", "sql_safety", "sandbox", "final_answer", "judge", "agent_version_binding"],
                "cases": case_results,
            }),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.eval_runs.insert(run.id, run.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO eval_runs (id, tenant_id, dataset_id, agent_id, agent_version_id, status, score, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(run.id)
                .bind(self.tenant_id)
                .bind(run.dataset_id)
                .bind(run.agent_id)
                .bind(run.agent_version_id)
                .bind(&run.status)
                .bind(run.score)
                .bind(&run.details)
                .bind(run.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(run)
    }

    async fn grade_eval_case(
        &self,
        case: &EvalCase,
        agent_version: &crate::AgentVersion,
    ) -> EvalCaseResult {
        let kind = case
            .grading_policy
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("policy");
        match kind {
            "policy" => self.grade_policy_case(case, agent_version).await,
            "tool_selection" => self.grade_tool_selection_case(case, agent_version),
            "sql_safety" => self.grade_sql_safety_case(case).await,
            "sandbox" => grade_sandbox_case(case),
            "final_answer" => grade_final_answer_case(case),
            "judge" => self.grade_judge_case(case, agent_version).await,
            other => EvalCaseResult::fail(
                case.id,
                kind,
                format!("unsupported eval grading kind: {other}"),
                json!({}),
            ),
        }
    }

    async fn grade_judge_case(
        &self,
        case: &EvalCase,
        agent_version: &crate::AgentVersion,
    ) -> EvalCaseResult {
        let Some(config) = self.eval_judge_config.as_ref() else {
            return EvalCaseResult::fail(
                case.id,
                "judge",
                "eval judge is not configured",
                json!({
                    "configured": false,
                    "required_env": ["MANDOFORGE_EVAL_JUDGE_URL"],
                }),
            );
        };

        let request = EvalJudgeRequest {
            case_id: case.id,
            input: case.input.clone(),
            expected: case.expected.clone(),
            grading_policy: case.grading_policy.clone(),
            agent_id: agent_version.agent_id,
            agent_version_id: agent_version.id,
        };
        match self.eval_judge_client.grade(config, request).await {
            Ok(response) => EvalCaseResult::from_match(
                case.id,
                "judge",
                response.passed,
                response.message,
                json!({
                    "configured": true,
                    "score": response.score,
                    "judge_details": response.details,
                    "agent_id": agent_version.agent_id,
                    "agent_version_id": agent_version.id,
                }),
            ),
            Err(error) => EvalCaseResult::fail(
                case.id,
                "judge",
                error.message,
                json!({
                    "configured": true,
                    "agent_id": agent_version.agent_id,
                    "agent_version_id": agent_version.id,
                }),
            ),
        }
    }

    async fn grade_policy_case(
        &self,
        case: &EvalCase,
        agent_version: &crate::AgentVersion,
    ) -> EvalCaseResult {
        let tool = case
            .expected
            .as_ref()
            .and_then(|expected| expected.get("tool"))
            .or_else(|| case.input.get("tool"))
            .and_then(Value::as_str);
        let Some(tool) = tool else {
            return EvalCaseResult::fail(
                case.id,
                "policy",
                "policy eval requires expected.tool or input.tool",
                json!({}),
            );
        };
        let expected_decision = case
            .expected
            .as_ref()
            .and_then(|expected| expected.get("decision"))
            .and_then(Value::as_str);
        let Some(expected_decision) = expected_decision else {
            return EvalCaseResult::fail(
                case.id,
                "policy",
                "policy eval requires expected.decision",
                json!({}),
            );
        };
        let policy = self.active_policy().await;
        let decision = policy.evaluate_tool_for_agent_version(tool, agent_version);
        EvalCaseResult::from_match(
            case.id,
            "policy",
            decision.decision == expected_decision,
            format!(
                "expected {tool} decision {expected_decision}, got {}",
                decision.decision
            ),
            json!({
                "tool": tool,
                "expected_decision": expected_decision,
                "actual_decision": decision.decision,
                "risk_level": decision.risk_level,
                "reason": decision.reason,
            }),
        )
    }

    fn grade_tool_selection_case(
        &self,
        case: &EvalCase,
        agent_version: &crate::AgentVersion,
    ) -> EvalCaseResult {
        let expected_tools: Vec<String> = case
            .expected
            .as_ref()
            .map(|expected| {
                expected
                    .get("required_tools")
                    .and_then(Value::as_array)
                    .map(|tools| {
                        tools
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .or_else(|| {
                        expected
                            .get("tool")
                            .and_then(Value::as_str)
                            .map(|tool| vec![tool.to_string()])
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if expected_tools.is_empty() {
            return EvalCaseResult::fail(
                case.id,
                "tool_selection",
                "tool_selection eval requires expected.tool or expected.required_tools",
                json!({}),
            );
        }
        let enabled = |tool: &str| {
            agent_version
                .tool_names
                .iter()
                .any(|enabled| enabled == tool)
        };
        let missing: Vec<_> = expected_tools
            .iter()
            .filter(|tool| !enabled(tool))
            .cloned()
            .collect();
        EvalCaseResult::from_match(
            case.id,
            "tool_selection",
            missing.is_empty(),
            format!("missing required tools: {missing:?}"),
            json!({
                "required_tools": expected_tools,
                "missing_tools": missing,
                "agent_tools": agent_version.tool_names,
            }),
        )
    }

    async fn grade_sql_safety_case(&self, case: &EvalCase) -> EvalCaseResult {
        let sql = case
            .input
            .get("sql")
            .or_else(|| {
                case.expected
                    .as_ref()
                    .and_then(|expected| expected.get("sql"))
            })
            .and_then(Value::as_str);
        let Some(sql) = sql else {
            return EvalCaseResult::fail(
                case.id,
                "sql_safety",
                "sql_safety eval requires input.sql",
                json!({}),
            );
        };
        let expected_allowed = case
            .expected
            .as_ref()
            .and_then(|expected| expected.get("allowed"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let policy = self.active_policy().await;
        let actual_allowed = ensure_read_only_sql_with_policy(sql, &policy.sql_policy).is_ok();
        EvalCaseResult::from_match(
            case.id,
            "sql_safety",
            actual_allowed == expected_allowed,
            format!("expected SQL allowed={expected_allowed}, got {actual_allowed}"),
            json!({
                "sql": sql,
                "expected_allowed": expected_allowed,
                "actual_allowed": actual_allowed,
            }),
        )
    }

    async fn ensure_eval_dataset_exists(&self, dataset_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                if inner.read().await.eval_datasets.contains_key(&dataset_id) {
                    Ok(())
                } else {
                    Err(AppError::not_found("eval dataset not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1 FROM eval_datasets WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(dataset_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("eval dataset not found"))
            }
        }
    }
}

#[derive(serde::Serialize)]
struct EvalCaseResult {
    case_id: Uuid,
    kind: String,
    passed: bool,
    message: String,
    details: Value,
}

impl EvalCaseResult {
    fn from_match(
        case_id: Uuid,
        kind: impl Into<String>,
        passed: bool,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            case_id,
            kind: kind.into(),
            passed,
            message: message.into(),
            details,
        }
    }

    fn fail(
        case_id: Uuid,
        kind: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self::from_match(case_id, kind, false, message, details)
    }
}

fn grade_sandbox_case(case: &EvalCase) -> EvalCaseResult {
    let path = case
        .input
        .get("path")
        .or_else(|| case.input.get("command"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_allowed = case
        .expected
        .as_ref()
        .and_then(|expected| expected.get("allowed"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let actual_allowed = !path.starts_with('/') && !path.contains("..");
    EvalCaseResult::from_match(
        case.id,
        "sandbox",
        actual_allowed == expected_allowed,
        format!("expected sandbox allowed={expected_allowed}, got {actual_allowed}"),
        json!({
            "path_or_command": path,
            "expected_allowed": expected_allowed,
            "actual_allowed": actual_allowed,
        }),
    )
}

fn grade_final_answer_case(case: &EvalCase) -> EvalCaseResult {
    let answer = case
        .input
        .get("final_answer")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let required: Vec<String> = case
        .expected
        .as_ref()
        .and_then(|expected| expected.get("contains"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let missing: Vec<_> = required
        .iter()
        .filter(|item| !answer.contains(item.as_str()))
        .cloned()
        .collect();
    EvalCaseResult::from_match(
        case.id,
        "final_answer",
        missing.is_empty(),
        format!("missing required answer fragments: {missing:?}"),
        json!({
            "required_contains": required,
            "missing": missing,
        }),
    )
}
