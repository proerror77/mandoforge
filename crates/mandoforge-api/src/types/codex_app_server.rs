use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerRun {
    pub(crate) id: Uuid,
    pub(crate) operation: String,
    pub(crate) thread_id: Option<String>,
    pub(crate) turn_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) status: String,
    pub(crate) request: Value,
    pub(crate) response: Value,
    pub(crate) error: Option<Value>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexAppServerPollRequest {
    #[serde(default = "default_codex_poll_attempts")]
    pub(crate) max_attempts: u32,
    #[serde(default)]
    pub(crate) retry_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerPollResponse {
    pub(crate) run: CodexAppServerRun,
    pub(crate) attempts: u32,
    pub(crate) terminal: bool,
    pub(crate) last_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexAppServerStalePollRequest {
    #[serde(default = "default_codex_stale_after_seconds")]
    pub(crate) stale_after_seconds: u64,
    #[serde(default = "default_codex_poll_attempts")]
    pub(crate) max_attempts: u32,
    #[serde(default)]
    pub(crate) retry_interval_ms: u64,
    #[serde(default = "default_codex_stale_poll_max_runs")]
    pub(crate) max_runs: usize,
}

impl Default for CodexAppServerStalePollRequest {
    fn default() -> Self {
        Self {
            stale_after_seconds: default_codex_stale_after_seconds(),
            max_attempts: default_codex_poll_attempts(),
            retry_interval_ms: 0,
            max_runs: default_codex_stale_poll_max_runs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerStalePollRun {
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) stale_after_seconds: u64,
    pub(crate) candidate_count: usize,
    pub(crate) polled_count: usize,
    pub(crate) terminal_count: usize,
    pub(crate) skipped_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) results: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerTraceSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) run_count: usize,
    pub(crate) turn_count: usize,
    pub(crate) active_turn_count: usize,
    pub(crate) failed_turn_count: usize,
    pub(crate) by_status: HashMap<String, usize>,
    pub(crate) by_operation: HashMap<String, usize>,
    pub(crate) by_failure_domain: HashMap<String, usize>,
    pub(crate) traces: Vec<CodexTurnTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerControlPlaneSummary {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) configured: bool,
    pub(crate) status: String,
    pub(crate) endpoint_configured: bool,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) run_count: usize,
    pub(crate) turn_count: usize,
    pub(crate) active_turn_count: usize,
    pub(crate) failed_turn_count: usize,
    pub(crate) stale_candidate_count: usize,
    pub(crate) pollable_turn_count: usize,
    pub(crate) latest_seen_at: Option<DateTime<Utc>>,
    pub(crate) by_status: HashMap<String, usize>,
    pub(crate) by_operation: HashMap<String, usize>,
    pub(crate) production_ops: CodexAppServerProductionOpsReadiness,
    pub(crate) deployment_readiness: CodexAppServerDeploymentReadiness,
    pub(crate) attention_items: Vec<CodexAppServerControlPlaneAttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerProductionOpsReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) configured: bool,
    pub(crate) run_count: usize,
    pub(crate) active_turn_count: usize,
    pub(crate) failed_turn_count: usize,
    pub(crate) stale_candidate_count: usize,
    pub(crate) latest_stale_poll_at: Option<DateTime<Utc>>,
    pub(crate) latest_stale_poll_age_hours: Option<i64>,
    pub(crate) latest_stale_poll_candidate_count: usize,
    pub(crate) latest_stale_poll_failed_count: usize,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerOpsValidationRun {
    pub(crate) status: String,
    pub(crate) configured: bool,
    pub(crate) production_ops_status: String,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) controller_execution: Value,
    pub(crate) checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerDeploymentReadiness {
    pub(crate) status: String,
    pub(crate) production_blocked: bool,
    pub(crate) configured: bool,
    pub(crate) endpoint_configured: bool,
    pub(crate) deployment_validated: bool,
    pub(crate) latest_validation_at: Option<DateTime<Utc>>,
    pub(crate) latest_validation_age_hours: Option<i64>,
    pub(crate) latest_validation_status: Option<String>,
    pub(crate) latest_validation_healthy: bool,
    pub(crate) controller_required: bool,
    pub(crate) controller_configured: bool,
    pub(crate) latest_controller_status: Option<String>,
    pub(crate) latest_controller_age_hours: Option<i64>,
    pub(crate) controller_evidence_fresh: bool,
    pub(crate) latest_controller_validated: bool,
    pub(crate) controller_execution_count: usize,
    pub(crate) controller_failed_count: usize,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerControlPlaneAttentionItem {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) trace_key: Option<String>,
    pub(crate) turn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexTurnTrace {
    pub(crate) trace_key: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) latest_run_id: Uuid,
    pub(crate) latest_status: String,
    pub(crate) terminal: bool,
    pub(crate) run_count: usize,
    pub(crate) command_count: usize,
    pub(crate) poll_count: usize,
    pub(crate) error_count: usize,
    pub(crate) duration_seconds: i64,
    pub(crate) command_ids: Vec<String>,
    pub(crate) operations: Vec<String>,
    pub(crate) next_action: String,
    pub(crate) latest_error: Option<Value>,
    pub(crate) dashboard: CodexTraceDashboard,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerTraceDetail {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) trace: CodexTurnTrace,
    pub(crate) runs: Vec<CodexAppServerRun>,
    pub(crate) status_timeline: Vec<CodexAppServerStatusPoint>,
    pub(crate) by_status: HashMap<String, usize>,
    pub(crate) by_operation: HashMap<String, usize>,
    pub(crate) terminal_count: usize,
    pub(crate) non_terminal_count: usize,
    pub(crate) command_ids: Vec<String>,
    pub(crate) errors: Vec<Value>,
    pub(crate) dashboard: CodexTraceDashboard,
    pub(crate) evidence: Vec<CodexTraceEvidence>,
    pub(crate) artifact_lineage: Vec<CodexTraceArtifactLineage>,
    pub(crate) latest_response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexAppServerStatusPoint {
    pub(crate) run_id: Uuid,
    pub(crate) operation: String,
    pub(crate) status: String,
    pub(crate) terminal: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) error: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CodexTraceDashboard {
    pub(crate) command_count: usize,
    pub(crate) poll_count: usize,
    pub(crate) interrupt_count: usize,
    pub(crate) worker_lease_count: usize,
    pub(crate) retry_count: usize,
    pub(crate) fallback_count: usize,
    pub(crate) artifact_sync_count: usize,
    pub(crate) failed_operation_count: usize,
    pub(crate) stuck: bool,
    pub(crate) failure_domain: String,
    pub(crate) operator_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexTraceEvidence {
    pub(crate) source: String,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) run_id: Option<Uuid>,
    pub(crate) event_id: Option<Uuid>,
    pub(crate) audit_log_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexTraceArtifactLineage {
    pub(crate) artifact_id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) turn_id: Option<String>,
    pub(crate) command_id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) artifact_type: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
}

pub(crate) fn default_codex_poll_attempts() -> u32 {
    120
}

pub(crate) fn default_codex_stale_after_seconds() -> u64 {
    300
}

pub(crate) fn default_codex_stale_poll_max_runs() -> usize {
    20
}
