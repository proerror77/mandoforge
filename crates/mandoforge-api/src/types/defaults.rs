use serde_json::{Value, json};

pub(crate) fn generic_file_read_summary() -> Value {
    json!({
        "files": [
            {
                "path": "README.md",
                "summary": "Rust-native Managed Agents runtime prototype with Postgres-backed event log and approval timeline."
            },
            {
                "path": "config/policy.stage1.yaml",
                "summary": "Generic Stage 1 policy requiring approval for shell.exec, codex.exec, file.write, and http.request."
            }
        ]
    })
}

pub(crate) fn generic_schema() -> Value {
    json!({
        "tables": {
            "generic_demo.platform_events": ["id", "session_id", "event_type", "status", "latency_ms", "payload", "created_at"],
            "generic_demo.sample_documents": ["id", "title", "body", "metadata", "created_at"],
            "generic_demo.sample_metrics": ["id", "metric_name", "metric_value", "dimensions", "observed_at"]
        },
        "metrics": {
            "sessions_started_24h": "count(*) where event_type = 'session.started'",
            "sessions_completed_24h": "count(*) where event_type = 'session.completed'",
            "approvals_requested_24h": "count(*) where event_type = 'policy.requires_approval'",
            "p95_latency_ms": "percentile_cont(0.95) within group (order by latency_ms)"
        }
    })
}

pub(crate) fn generic_diagnostics() -> Value {
    json!({
        "window": "24h",
        "sessions_started": 12,
        "sessions_completed": 9,
        "sessions_failed": 1,
        "approvals_requested": 3,
        "tool_success_rate": 0.91,
        "notable_events": [
            {"event_type": "policy.requires_approval", "status": "waiting_approval", "tool": "shell.exec"},
            {"event_type": "artifact.created", "status": "ok", "artifact": "diagnostics.md"},
            {"event_type": "session.failed", "status": "failed", "reason": "tool timeout"}
        ]
    })
}

pub(crate) fn default_agent_kind() -> String {
    "orchestrator".to_string()
}

pub(crate) fn default_release_environment() -> String {
    "staging".to_string()
}

pub(crate) fn default_secret_scope_type() -> String {
    "tenant".to_string()
}

pub(crate) fn default_cost_alert_severity_filter() -> String {
    "warning".to_string()
}

pub(crate) fn default_bootstrap_owner_role() -> String {
    "admin".to_string()
}

pub(crate) fn default_provider() -> String {
    "openai-compatible".to_string()
}

pub(crate) fn default_agent_runtime_type() -> String {
    "agent_cli".to_string()
}

pub(crate) fn default_environment_type() -> String {
    "local".to_string()
}

pub(crate) fn default_agent_role() -> String {
    "specialist".to_string()
}

pub(crate) fn default_agent_release_state() -> String {
    "draft".to_string()
}

pub(crate) fn default_enabled_status() -> String {
    "enabled".to_string()
}

pub(crate) fn default_semantic_source_status() -> String {
    "active".to_string()
}

pub(crate) fn default_semantic_record_status() -> String {
    "active".to_string()
}

pub(crate) fn default_memory_object_type() -> String {
    "memory".to_string()
}

pub(crate) fn default_semantic_trust_level() -> String {
    "unverified".to_string()
}

pub(crate) fn default_semantic_freshness() -> String {
    "unknown".to_string()
}

pub(crate) fn default_semantic_confidence() -> f64 {
    1.0
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_semantic_conflict_strategy() -> String {
    "flag".to_string()
}

pub(crate) fn default_mcp_transport() -> String {
    "http".to_string()
}

pub(crate) fn default_model() -> String {
    "gpt-5.5-mini".to_string()
}

pub(crate) fn default_session_title() -> String {
    "Untitled session".to_string()
}

pub(crate) fn default_workflow_trigger_type() -> String {
    "manual".to_string()
}

pub(crate) fn default_workflow_release_state() -> String {
    "draft".to_string()
}

pub(crate) fn default_workflow_run_status() -> String {
    "queued".to_string()
}

pub(crate) fn default_workflow_execution_strategy() -> String {
    "native_steps".to_string()
}

pub(crate) fn default_event_ingestion_policy() -> String {
    "normalized".to_string()
}

pub(crate) fn default_task_grant_risk_level() -> String {
    "low".to_string()
}

pub(crate) fn default_task_grant_memory_scope() -> Value {
    json!({
        "mode": "snapshot_only",
        "allowed_scope_keys": [],
        "allowed_object_types": [],
        "allowed_source_types": [],
        "allowed_object_ids": [],
        "minimum_trust_level": "verified",
        "max_objects": 0,
        "approval_memory_allowed": false,
        "handoff_memory_allowed": false,
        "writeback_allowed": false
    })
}

pub(crate) fn default_task_grant_tool_scope() -> Value {
    json!({
        "read": [],
        "write": [],
        "external_write": []
    })
}

pub(crate) fn default_task_grant_connector_scope() -> Value {
    json!({
        "mode": "read_only",
        "allowed_connector_ids": [],
        "allowed_tool_names": [],
        "tenant_scope": {},
        "side_effect_classes": []
    })
}

pub(crate) fn default_task_grant_approval_policy() -> Value {
    json!({
        "mode": "approval_required_for_mutation"
    })
}

pub(crate) fn default_task_grant_external_effects() -> Value {
    json!({
        "publish": false,
        "payment": false,
        "external_message": false,
        "account_mutation": false,
        "ad_spend_mutation": false
    })
}

pub(crate) fn empty_json_object() -> Value {
    json!({})
}

pub(crate) fn workflow_input_digest(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
