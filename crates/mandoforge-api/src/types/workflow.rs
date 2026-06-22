use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowDefinition {
    pub(crate) id: Uuid,
    pub(crate) pack_installation_id: Option<Uuid>,
    pub(crate) pack_id: Option<String>,
    pub(crate) pack_version: Option<String>,
    pub(crate) name: String,
    pub(crate) entrypoint: String,
    pub(crate) trigger_type: String,
    pub(crate) default_agent_id: Uuid,
    pub(crate) default_environment_id: Option<Uuid>,
    pub(crate) input_schema_ref: Option<String>,
    pub(crate) output_schema_ref: Option<String>,
    pub(crate) step_graph: Value,
    pub(crate) handoff_rules: Value,
    pub(crate) execution_strategy: String,
    pub(crate) runtime_adapter: Option<String>,
    pub(crate) runtime_mode: Option<String>,
    pub(crate) runtime_capability_contract: Value,
    pub(crate) event_ingestion_policy: String,
    pub(crate) approval_policy_ref: Option<String>,
    pub(crate) eval_gate_refs: Vec<String>,
    pub(crate) release_state: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkflowDefinition {
    #[serde(default)]
    pub(crate) pack_installation_id: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) entrypoint: String,
    #[serde(default = "crate::default_workflow_trigger_type")]
    pub(crate) trigger_type: String,
    pub(crate) default_agent_id: Uuid,
    #[serde(default)]
    pub(crate) default_environment_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) input_schema_ref: Option<String>,
    #[serde(default)]
    pub(crate) output_schema_ref: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) step_graph: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) handoff_rules: Value,
    #[serde(default = "crate::default_workflow_execution_strategy")]
    pub(crate) execution_strategy: String,
    #[serde(default)]
    pub(crate) runtime_adapter: Option<String>,
    #[serde(default)]
    pub(crate) runtime_mode: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) runtime_capability_contract: Value,
    #[serde(default = "crate::default_event_ingestion_policy")]
    pub(crate) event_ingestion_policy: String,
    #[serde(default)]
    pub(crate) approval_policy_ref: Option<String>,
    #[serde(default)]
    pub(crate) eval_gate_refs: Vec<String>,
    #[serde(default = "crate::default_workflow_release_state")]
    pub(crate) release_state: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateWorkflowDefinition {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) entrypoint: Option<String>,
    #[serde(default)]
    pub(crate) trigger_type: Option<String>,
    #[serde(default)]
    pub(crate) default_agent_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) default_environment_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub(crate) input_schema_ref: Option<Option<String>>,
    #[serde(default)]
    pub(crate) output_schema_ref: Option<Option<String>>,
    #[serde(default)]
    pub(crate) step_graph: Option<Value>,
    #[serde(default)]
    pub(crate) handoff_rules: Option<Value>,
    #[serde(default)]
    pub(crate) execution_strategy: Option<String>,
    #[serde(default)]
    pub(crate) runtime_adapter: Option<Option<String>>,
    #[serde(default)]
    pub(crate) runtime_mode: Option<Option<String>>,
    #[serde(default)]
    pub(crate) runtime_capability_contract: Option<Value>,
    #[serde(default)]
    pub(crate) event_ingestion_policy: Option<String>,
    #[serde(default)]
    pub(crate) approval_policy_ref: Option<Option<String>>,
    #[serde(default)]
    pub(crate) eval_gate_refs: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) release_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowRun {
    pub(crate) id: Uuid,
    pub(crate) workflow_definition_id: Uuid,
    pub(crate) pack_installation_id: Option<Uuid>,
    pub(crate) source_event_id: Option<Uuid>,
    pub(crate) source_work_item_id: Option<Uuid>,
    pub(crate) source_schedule_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) primary_session_id: Uuid,
    pub(crate) root_task_grant_id: Option<Uuid>,
    pub(crate) input_payload: Value,
    pub(crate) input_digest: String,
    pub(crate) execution_strategy: String,
    pub(crate) runtime_adapter: Option<String>,
    pub(crate) runtime_mode: Option<String>,
    pub(crate) delegation_status: Option<String>,
    pub(crate) external_run_ref: Option<String>,
    pub(crate) runtime_event_cursor: Option<String>,
    pub(crate) runtime_envelope: Value,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkflowRun {
    pub(crate) workflow_definition_id: Uuid,
    #[serde(default)]
    pub(crate) source_event_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) source_work_item_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) source_schedule_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) environment_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) execution_strategy: Option<String>,
    #[serde(default)]
    pub(crate) runtime_adapter: Option<String>,
    #[serde(default)]
    pub(crate) runtime_mode: Option<String>,
    #[serde(default)]
    pub(crate) external_run_ref: Option<String>,
    #[serde(default)]
    pub(crate) runtime_event_cursor: Option<String>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) runtime_envelope: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) input_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowStepRun {
    pub(crate) id: Uuid,
    pub(crate) workflow_run_id: Uuid,
    pub(crate) step_key: String,
    pub(crate) step_type: String,
    pub(crate) agent_id: Option<Uuid>,
    pub(crate) agent_version_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) thread_id: Option<Uuid>,
    pub(crate) handoff_id: Option<Uuid>,
    pub(crate) task_grant_id: Option<Uuid>,
    pub(crate) environment_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) input_payload: Value,
    pub(crate) output_payload: Value,
    pub(crate) artifact_ids: Vec<Uuid>,
    pub(crate) approval_ids: Vec<Uuid>,
    pub(crate) tool_call_ids: Vec<Uuid>,
    pub(crate) claimed_by_worker: Option<String>,
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) context_packet_id: Option<Uuid>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) scheduled_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkflowTransition {
    pub(crate) id: Uuid,
    pub(crate) workflow_run_id: Uuid,
    pub(crate) from_step_run_id: Option<Uuid>,
    pub(crate) from_step_key: Option<String>,
    pub(crate) to_step_run_id: Option<Uuid>,
    pub(crate) to_step_key: Option<String>,
    pub(crate) transition_type: String,
    pub(crate) status: String,
    pub(crate) condition_payload: Value,
    pub(crate) result_payload: Value,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkflowTransitionFilter {
    pub(crate) transition_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) from_step_key: Option<String>,
    pub(crate) to_step_key: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowTransitionQuery {
    #[serde(default)]
    pub(crate) transition_type: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) from_step_key: Option<String>,
    #[serde(default)]
    pub(crate) to_step_key: Option<String>,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkflowStepRun {
    pub(crate) step_key: String,
    pub(crate) step_type: String,
    #[serde(default)]
    pub(crate) agent_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) agent_version_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) session_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) thread_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) handoff_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) task_grant_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) environment_id: Option<Uuid>,
    #[serde(default = "crate::default_workflow_run_status")]
    pub(crate) status: String,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) input_payload: Value,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) output_payload: Value,
    #[serde(default)]
    pub(crate) artifact_ids: Vec<Uuid>,
    #[serde(default)]
    pub(crate) approval_ids: Vec<Uuid>,
    #[serde(default)]
    pub(crate) tool_call_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateWorkflowStepRun {
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) output_payload: Option<Value>,
    #[serde(default)]
    pub(crate) artifact_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub(crate) approval_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub(crate) tool_call_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskGrant {
    pub(crate) id: Uuid,
    pub(crate) workflow_run_id: Uuid,
    pub(crate) workflow_step_run_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) parent_grant_id: Option<Uuid>,
    pub(crate) source_event_id: Option<Uuid>,
    pub(crate) source_handoff_id: Option<Uuid>,
    pub(crate) issuer_subject: String,
    pub(crate) grantee_agent_id: Option<Uuid>,
    pub(crate) grantee_session_id: Option<Uuid>,
    pub(crate) agent_class: Option<String>,
    pub(crate) objective: String,
    pub(crate) risk_level: String,
    pub(crate) status: String,
    pub(crate) expires_at: Option<DateTime<Utc>>,
    pub(crate) max_turns: Option<i32>,
    pub(crate) max_tool_calls: Option<i32>,
    pub(crate) max_runtime_seconds: Option<i32>,
    pub(crate) max_cost_usd_micros: Option<i64>,
    pub(crate) semantic_scopes: Value,
    pub(crate) memory_scope: Value,
    pub(crate) tool_scope: Value,
    pub(crate) connector_scope: Value,
    pub(crate) approval_policy: Value,
    pub(crate) external_effects: Value,
    pub(crate) context_packet_id: Option<Uuid>,
    pub(crate) policy_revision_id: Option<Uuid>,
    pub(crate) immutable_args_hash: Option<String>,
    pub(crate) audit_trace_id: Option<Uuid>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTaskGrant {
    #[serde(default)]
    pub(crate) parent_grant_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) workflow_step_run_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) session_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) source_event_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) source_handoff_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) issuer_subject: Option<String>,
    #[serde(default)]
    pub(crate) grantee_agent_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) grantee_session_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) agent_class: Option<String>,
    #[serde(default)]
    pub(crate) objective: Option<String>,
    #[serde(default = "crate::default_task_grant_risk_level")]
    pub(crate) risk_level: String,
    #[serde(default)]
    pub(crate) expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) max_turns: Option<i32>,
    #[serde(default)]
    pub(crate) max_tool_calls: Option<i32>,
    #[serde(default)]
    pub(crate) max_runtime_seconds: Option<i32>,
    #[serde(default)]
    pub(crate) max_cost_usd_micros: Option<i64>,
    #[serde(default = "crate::empty_json_object")]
    pub(crate) semantic_scopes: Value,
    #[serde(default = "crate::default_task_grant_memory_scope")]
    pub(crate) memory_scope: Value,
    #[serde(default = "crate::default_task_grant_tool_scope")]
    pub(crate) tool_scope: Value,
    #[serde(default = "crate::default_task_grant_connector_scope")]
    pub(crate) connector_scope: Value,
    #[serde(default = "crate::default_task_grant_approval_policy")]
    pub(crate) approval_policy: Value,
    #[serde(default = "crate::default_task_grant_external_effects")]
    pub(crate) external_effects: Value,
    #[serde(default)]
    pub(crate) context_packet_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) policy_revision_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) immutable_args_hash: Option<String>,
}
