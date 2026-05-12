use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct EvalJudgeConfig {
    pub(crate) endpoint: String,
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct EvalJudgeRequest {
    pub(crate) case_id: Uuid,
    pub(crate) input: Value,
    #[serde(default)]
    pub(crate) expected: Option<Value>,
    pub(crate) grading_policy: Value,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct EvalJudgeResponse {
    pub(crate) passed: bool,
    #[serde(default)]
    pub(crate) score: Option<f64>,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) details: Value,
}

#[allow(dead_code)]
impl EvalJudgeConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = lookup("MANDOFORGE_EVAL_JUDGE_URL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::bad_request("MANDOFORGE_EVAL_JUDGE_URL is required"))?;
        let timeout_seconds = lookup("MANDOFORGE_EVAL_JUDGE_TIMEOUT_SECONDS")
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
pub(crate) trait EvalJudgeClient: Send + Sync {
    async fn grade(
        &self,
        config: &EvalJudgeConfig,
        request: EvalJudgeRequest,
    ) -> Result<EvalJudgeResponse, AppError>;
}

#[allow(dead_code)]
pub(crate) struct ReservedEvalJudgeClient;

#[async_trait]
impl EvalJudgeClient for ReservedEvalJudgeClient {
    async fn grade(
        &self,
        _config: &EvalJudgeConfig,
        _request: EvalJudgeRequest,
    ) -> Result<EvalJudgeResponse, AppError> {
        Err(AppError::bad_request(
            "eval judge is reserved but not configured",
        ))
    }
}

#[allow(dead_code)]
pub(crate) struct HttpEvalJudgeClient {
    client: reqwest::Client,
}

#[allow(dead_code)]
impl HttpEvalJudgeClient {
    pub(crate) fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
        })
    }

    fn grade_url(config: &EvalJudgeConfig) -> String {
        format!("{}/grade", config.normalized_endpoint())
    }
}

#[async_trait]
impl EvalJudgeClient for HttpEvalJudgeClient {
    async fn grade(
        &self,
        config: &EvalJudgeConfig,
        request: EvalJudgeRequest,
    ) -> Result<EvalJudgeResponse, AppError> {
        let response = tokio::time::timeout(
            Duration::from_secs(config.timeout_seconds),
            self.client
                .post(Self::grade_url(config))
                .json(&request)
                .send(),
        )
        .await??;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "eval judge failed with status {status}"
            )));
        }
        Ok(response.json::<EvalJudgeResponse>().await?)
    }
}
