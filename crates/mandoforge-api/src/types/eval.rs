use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalDataset {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEvalDataset {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalCase {
    pub(crate) id: Uuid,
    pub(crate) dataset_id: Uuid,
    pub(crate) input: Value,
    pub(crate) expected: Option<Value>,
    pub(crate) grading_policy: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEvalCase {
    pub(crate) input: Value,
    #[serde(default)]
    pub(crate) expected: Option<Value>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) grading_policy: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalRun {
    pub(crate) id: Uuid,
    pub(crate) dataset_id: Uuid,
    pub(crate) agent_id: Uuid,
    pub(crate) agent_version_id: Uuid,
    pub(crate) status: String,
    pub(crate) score: Option<f64>,
    pub(crate) details: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEvalRun {
    pub(crate) agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEvalJudgeProfile {
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) api_key_ref: Option<String>,
    #[serde(default)]
    pub(crate) timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BootstrapEvalSuite {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) judge_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalSuiteBootstrap {
    pub(crate) dataset: EvalDataset,
    pub(crate) cases: Vec<EvalCase>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EvalGateRequest {
    #[serde(default)]
    pub(crate) min_score: Option<f64>,
    #[serde(default)]
    pub(crate) require_completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EvalGateDecision {
    pub(crate) run_id: Uuid,
    pub(crate) status: String,
    pub(crate) score: Option<f64>,
    pub(crate) min_score: f64,
    pub(crate) failure_reasons: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EvalDriftDecision {
    pub(crate) run_id: Uuid,
    pub(crate) baseline_run_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) score_delta: Option<f64>,
    pub(crate) passed_count_delta: Option<i64>,
    pub(crate) case_count_delta: Option<i64>,
    pub(crate) messages: Vec<String>,
    pub(crate) checked_at: DateTime<Utc>,
}
