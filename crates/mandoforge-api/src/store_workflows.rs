use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    task_grant_from_row, workflow_definition_from_row, workflow_run_from_row,
    workflow_step_run_from_row, workflow_transition_from_row,
};
use crate::{
    Agent, AgentVersion, AppError, AppState, TaskGrant, WorkflowDefinition, WorkflowRun,
    WorkflowStepRun, WorkflowTransition, WorkflowTransitionFilter,
};

const TASK_GRANT_COLUMNS: &str = "id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at";

async fn insert_workflow_step_run<'e, E>(
    executor: E,
    tenant_id: Uuid,
    step: WorkflowStepRun,
) -> Result<WorkflowStepRun, AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        "INSERT INTO workflow_step_runs
            (id, tenant_id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26)
         RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
    )
    .bind(step.id)
    .bind(tenant_id)
    .bind(step.workflow_run_id)
    .bind(&step.step_key)
    .bind(&step.step_type)
    .bind(step.agent_id)
    .bind(step.agent_version_id)
    .bind(step.session_id)
    .bind(step.thread_id)
    .bind(step.handoff_id)
    .bind(step.task_grant_id)
    .bind(step.environment_id)
    .bind(&step.status)
    .bind(&step.input_payload)
    .bind(&step.output_payload)
    .bind(serde_json::to_value(&step.artifact_ids)?)
    .bind(serde_json::to_value(&step.approval_ids)?)
    .bind(serde_json::to_value(&step.tool_call_ids)?)
    .bind(&step.claimed_by_worker)
    .bind(step.lease_expires_at)
    .bind(step.context_packet_id)
    .bind(step.started_at)
    .bind(step.completed_at)
    .bind(step.scheduled_at)
    .bind(step.created_at)
    .bind(step.updated_at)
    .fetch_one(executor)
    .await?;
    workflow_step_run_from_row(row)
}

#[derive(Clone, Copy)]
enum TaskGrantReservation {
    Turn,
    ToolCall,
}

pub(crate) fn task_grant_runtime_denial(
    grant: &TaskGrant,
    now: DateTime<Utc>,
) -> Option<(&'static str, bool)> {
    if grant.status != "active" {
        return Some(("task grant is not active", false));
    }
    if grant.expires_at.is_some_and(|expires_at| expires_at <= now) {
        return Some(("task grant is expired", true));
    }
    if grant.max_runtime_seconds.is_some_and(|seconds| {
        now.signed_duration_since(grant.created_at).num_seconds() >= i64::from(seconds)
    }) {
        return Some(("task grant runtime budget expired", true));
    }
    if grant
        .max_cost_usd_micros
        .is_some_and(|limit| grant.cost_usd_micros_used >= limit)
    {
        return Some(("task grant cost budget exhausted", false));
    }
    None
}

fn task_grant_reservation_denial(
    grant: &TaskGrant,
    reservation: TaskGrantReservation,
    now: DateTime<Utc>,
) -> Option<(&'static str, bool)> {
    if let Some(denial) = task_grant_runtime_denial(grant, now) {
        return Some(denial);
    }
    match reservation {
        TaskGrantReservation::Turn
            if grant
                .max_turns
                .is_some_and(|limit| grant.turns_used >= limit) =>
        {
            Some(("task grant turn budget exhausted", false))
        }
        TaskGrantReservation::ToolCall
            if grant
                .max_tool_calls
                .is_some_and(|limit| grant.tool_calls_used >= limit) =>
        {
            Some(("task grant tool call budget exhausted", false))
        }
        _ => None,
    }
}

impl AppState {
    pub(crate) async fn persist_prepared_agents_and_workflow_definitions(
        &self,
        agents: Vec<(Agent, AgentVersion)>,
        definitions: Vec<WorkflowDefinition>,
    ) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if agents
                    .iter()
                    .any(|(agent, _)| store.agents.contains_key(&agent.id))
                    || definitions
                        .iter()
                        .any(|definition| store.workflow_definitions.contains_key(&definition.id))
                {
                    return Err(AppError::bad_request(
                        "prepared workflow pack materialization conflicts with existing resources",
                    ));
                }
                for (agent, version) in agents {
                    store.agent_versions.insert(agent.id, vec![version]);
                    store.agents.insert(agent.id, agent);
                }
                for definition in definitions {
                    store.workflow_definitions.insert(definition.id, definition);
                }
                Ok(())
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                for (agent, version) in agents {
                    sqlx::query(
                        "INSERT INTO agents
                            (id, tenant_id, name, kind, team_id, project_id, runtime_profile_id, agent_role, provider, model, system_prompt, tools, tool_policy, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, release_state, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
                    )
                    .bind(agent.id)
                    .bind(self.current_tenant_id())
                    .bind(&agent.name)
                    .bind(&agent.kind)
                    .bind(agent.team_id)
                    .bind(agent.project_id)
                    .bind(agent.runtime_profile_id)
                    .bind(&agent.agent_role)
                    .bind(&agent.provider)
                    .bind(&agent.model)
                    .bind(&agent.system_prompt)
                    .bind(json!(&agent.tools))
                    .bind(&agent.tool_policy)
                    .bind(json!(&agent.mcp_server_ids))
                    .bind(json!(&agent.skill_ids))
                    .bind(json!(&agent.workflow_pack_ids))
                    .bind(&agent.remote_computer_profile)
                    .bind(&agent.semantic_scopes)
                    .bind(&agent.release_state)
                    .bind(agent.created_at)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO agent_versions (id, agent_id, version, provider, model, system_prompt, tools, tool_names, runtime_config, approval_policy, runtime_profile_id, runtime_profile_snapshot, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
                    )
                    .bind(version.id)
                    .bind(version.agent_id)
                    .bind(version.version)
                    .bind(&version.provider)
                    .bind(&version.model)
                    .bind(&version.system_prompt)
                    .bind(json!(&version.tools))
                    .bind(json!(&version.tool_names))
                    .bind(&version.runtime_config)
                    .bind(&version.approval_policy)
                    .bind(version.runtime_profile_id)
                    .bind(&version.runtime_profile_snapshot)
                    .bind(json!(&version.mcp_server_ids))
                    .bind(json!(&version.skill_ids))
                    .bind(json!(&version.workflow_pack_ids))
                    .bind(&version.remote_computer_profile)
                    .bind(&version.semantic_scopes)
                    .bind(version.created_at)
                    .execute(&mut *tx)
                    .await?;
                }
                for definition in definitions {
                    sqlx::query(
                        "INSERT INTO workflow_definitions
                            (id, tenant_id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)",
                    )
                    .bind(definition.id)
                    .bind(self.current_tenant_id())
                    .bind(definition.pack_installation_id)
                    .bind(&definition.pack_id)
                    .bind(&definition.pack_version)
                    .bind(&definition.name)
                    .bind(&definition.entrypoint)
                    .bind(&definition.trigger_type)
                    .bind(definition.default_agent_id)
                    .bind(definition.default_environment_id)
                    .bind(&definition.input_schema_ref)
                    .bind(&definition.output_schema_ref)
                    .bind(&definition.step_graph)
                    .bind(&definition.handoff_rules)
                    .bind(&definition.execution_strategy)
                    .bind(&definition.runtime_adapter)
                    .bind(&definition.runtime_mode)
                    .bind(&definition.runtime_capability_contract)
                    .bind(&definition.event_ingestion_policy)
                    .bind(&definition.approval_policy_ref)
                    .bind(json!(&definition.eval_gate_refs))
                    .bind(&definition.release_state)
                    .bind(definition.created_at)
                    .bind(definition.updated_at)
                    .bind(definition.archived_at)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok(())
            }
        }
    }

    pub(crate) async fn create_workflow_definition(
        &self,
        definition: WorkflowDefinition,
    ) -> Result<WorkflowDefinition, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .workflow_definitions
                    .insert(definition.id, definition.clone());
                Ok(definition)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO workflow_definitions
                        (id, tenant_id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)
                     RETURNING id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at",
                )
                .bind(definition.id)
                .bind(self.current_tenant_id())
                .bind(definition.pack_installation_id)
                .bind(&definition.pack_id)
                .bind(&definition.pack_version)
                .bind(&definition.name)
                .bind(&definition.entrypoint)
                .bind(&definition.trigger_type)
                .bind(definition.default_agent_id)
                .bind(definition.default_environment_id)
                .bind(&definition.input_schema_ref)
                .bind(&definition.output_schema_ref)
                .bind(&definition.step_graph)
                .bind(&definition.handoff_rules)
                .bind(&definition.execution_strategy)
                .bind(&definition.runtime_adapter)
                .bind(&definition.runtime_mode)
                .bind(&definition.runtime_capability_contract)
                .bind(&definition.event_ingestion_policy)
                .bind(&definition.approval_policy_ref)
                .bind(serde_json::to_value(&definition.eval_gate_refs)?)
                .bind(&definition.release_state)
                .bind(definition.created_at)
                .bind(definition.updated_at)
                .bind(definition.archived_at)
                .fetch_one(pool)
                .await?;
                workflow_definition_from_row(row)
            }
        }
    }

    pub(crate) async fn list_workflow_definitions(
        &self,
    ) -> Result<Vec<WorkflowDefinition>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut definitions: Vec<_> = inner
                    .read()
                    .await
                    .workflow_definitions
                    .values()
                    .filter(|definition| definition.archived_at.is_none())
                    .cloned()
                    .collect();
                definitions.sort_by_key(|definition| definition.created_at);
                definitions.reverse();
                Ok(definitions)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at
                     FROM workflow_definitions
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(workflow_definition_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_workflow_definition(
        &self,
        id: uuid::Uuid,
    ) -> Result<WorkflowDefinition, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .workflow_definitions
                .get(&id)
                .filter(|definition| definition.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("workflow definition not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at
                     FROM workflow_definitions
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow definition not found"))?;
                workflow_definition_from_row(row)
            }
        }
    }

    pub(crate) async fn update_workflow_definition(
        &self,
        definition: WorkflowDefinition,
    ) -> Result<WorkflowDefinition, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let existing = store
                    .workflow_definitions
                    .get_mut(&definition.id)
                    .filter(|existing| existing.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("workflow definition not found"))?;
                *existing = definition.clone();
                Ok(definition)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_definitions
                     SET name = $3,
                         entrypoint = $4,
                         trigger_type = $5,
                         default_agent_id = $6,
                         default_environment_id = $7,
                         input_schema_ref = $8,
                         output_schema_ref = $9,
                         step_graph = $10,
                         handoff_rules = $11,
                         execution_strategy = $12,
                         runtime_adapter = $13,
                         runtime_mode = $14,
                         runtime_capability_contract = $15,
                         event_ingestion_policy = $16,
                         approval_policy_ref = $17,
                         eval_gate_refs = $18,
                         release_state = $19,
                         updated_at = $20,
                         archived_at = $21
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL
                     RETURNING id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, execution_strategy, runtime_adapter, runtime_mode, runtime_capability_contract, event_ingestion_policy, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(definition.id)
                .bind(&definition.name)
                .bind(&definition.entrypoint)
                .bind(&definition.trigger_type)
                .bind(definition.default_agent_id)
                .bind(definition.default_environment_id)
                .bind(&definition.input_schema_ref)
                .bind(&definition.output_schema_ref)
                .bind(&definition.step_graph)
                .bind(&definition.handoff_rules)
                .bind(&definition.execution_strategy)
                .bind(&definition.runtime_adapter)
                .bind(&definition.runtime_mode)
                .bind(&definition.runtime_capability_contract)
                .bind(&definition.event_ingestion_policy)
                .bind(&definition.approval_policy_ref)
                .bind(serde_json::to_value(&definition.eval_gate_refs)?)
                .bind(&definition.release_state)
                .bind(definition.updated_at)
                .bind(definition.archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow definition not found"))?;
                workflow_definition_from_row(row)
            }
        }
    }

    pub(crate) async fn create_workflow_run(
        &self,
        run: WorkflowRun,
    ) -> Result<WorkflowRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .workflow_runs
                    .insert(run.id, run.clone());
                Ok(run)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO workflow_runs
                        (id, tenant_id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at",
                )
                .bind(run.id)
                .bind(self.current_tenant_id())
                .bind(run.workflow_definition_id)
                .bind(run.pack_installation_id)
                .bind(run.source_event_id)
                .bind(run.source_work_item_id)
                .bind(run.source_schedule_id)
                .bind(&run.status)
                .bind(run.primary_session_id)
                .bind(run.root_task_grant_id)
                .bind(&run.input_payload)
                .bind(&run.input_digest)
                .bind(&run.execution_strategy)
                .bind(&run.runtime_adapter)
                .bind(&run.runtime_mode)
                .bind(&run.delegation_status)
                .bind(&run.external_run_ref)
                .bind(&run.runtime_event_cursor)
                .bind(&run.runtime_envelope)
                .bind(run.started_at)
                .bind(run.completed_at)
                .bind(run.audit_trace_id)
                .bind(run.created_at)
                .bind(run.updated_at)
                .fetch_one(pool)
                .await?;
                workflow_run_from_row(row)
            }
        }
    }

    pub(crate) async fn list_workflow_runs(&self) -> Result<Vec<WorkflowRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut runs: Vec<_> = inner.read().await.workflow_runs.values().cloned().collect();
                runs.sort_by_key(|run| run.created_at);
                runs.reverse();
                Ok(runs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at
                     FROM workflow_runs
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(workflow_run_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_workflow_run(&self, id: uuid::Uuid) -> Result<WorkflowRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .workflow_runs
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("workflow run not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at
                     FROM workflow_runs
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                workflow_run_from_row(row)
            }
        }
    }

    pub(crate) async fn update_workflow_run_root_task_grant(
        &self,
        id: uuid::Uuid,
        root_task_grant_id: uuid::Uuid,
    ) -> Result<WorkflowRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                run.root_task_grant_id = Some(root_task_grant_id);
                run.updated_at = chrono::Utc::now();
                Ok(run.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_runs
                     SET root_task_grant_id = $3, updated_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(root_task_grant_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                workflow_run_from_row(row)
            }
        }
    }

    pub(crate) async fn update_workflow_run_status(
        &self,
        id: uuid::Uuid,
        status: String,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<WorkflowRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                run.status = status;
                run.started_at = started_at;
                run.completed_at = completed_at;
                run.updated_at = chrono::Utc::now();
                Ok(run.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_runs
                     SET status = $3, started_at = $4, completed_at = $5, updated_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, execution_strategy, runtime_adapter, runtime_mode, delegation_status, external_run_ref, runtime_event_cursor, runtime_envelope, started_at, completed_at, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(&status)
                .bind(started_at)
                .bind(completed_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                workflow_run_from_row(row)
            }
        }
    }

    pub(crate) async fn create_workflow_step_run(
        &self,
        step: WorkflowStepRun,
    ) -> Result<WorkflowStepRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get(&step.workflow_run_id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run.status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok(step)
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let mut tx = pool.begin().await?;
                let run_status: String = sqlx::query_scalar(
                    "SELECT status
                     FROM workflow_runs
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(step.workflow_run_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run_status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                let step = insert_workflow_step_run(&mut *tx, tenant_id, step).await?;
                tx.commit().await?;
                Ok(step)
            }
        }
    }

    pub(crate) async fn create_workflow_step_run_if_key_absent(
        &self,
        step: WorkflowStepRun,
    ) -> Result<Option<WorkflowStepRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get(&step.workflow_run_id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run.status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                if store.workflow_step_runs.values().any(|existing| {
                    existing.workflow_run_id == step.workflow_run_id
                        && existing.step_key == step.step_key
                }) {
                    return Ok(None);
                }
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok(Some(step))
            }
            StoreBackend::Postgres(pool) => {
                let tenant_id = self.current_tenant_id();
                let lock_key = format!("{tenant_id}:{}:{}", step.workflow_run_id, step.step_key);
                let mut tx = pool.begin().await?;
                let run_status: String = sqlx::query_scalar(
                    "SELECT status
                     FROM workflow_runs
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(step.workflow_run_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run_status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                    .bind(lock_key)
                    .execute(&mut *tx)
                    .await?;
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS (
                        SELECT 1
                        FROM workflow_step_runs
                        WHERE tenant_id = $1 AND workflow_run_id = $2 AND step_key = $3
                     )",
                )
                .bind(tenant_id)
                .bind(step.workflow_run_id)
                .bind(&step.step_key)
                .fetch_one(&mut *tx)
                .await?;
                if exists {
                    tx.commit().await?;
                    return Ok(None);
                }
                let step = insert_workflow_step_run(&mut *tx, tenant_id, step).await?;
                tx.commit().await?;
                Ok(Some(step))
            }
        }
    }

    pub(crate) async fn create_workflow_step_run_with_task_grant(
        &self,
        mut step: WorkflowStepRun,
        grant: TaskGrant,
    ) -> Result<(WorkflowStepRun, TaskGrant), AppError> {
        crate::validate_task_grant_scope_objects(&grant)?;
        crate::validate_task_grant_budgets(&grant)?;
        if grant.status != "active" {
            return Err(AppError::bad_request(
                "workflow step task grant must be active",
            ));
        }
        if grant.workflow_run_id != step.workflow_run_id
            || grant.workflow_step_run_id != Some(step.id)
        {
            return Err(AppError::bad_request(
                "workflow step and task grant must reference each other in the same workflow run",
            ));
        }
        if step
            .task_grant_id
            .is_some_and(|task_grant_id| task_grant_id != grant.id)
        {
            return Err(AppError::bad_request(
                "workflow step task grant id does not match task grant",
            ));
        }
        if let Some(session_id) = step.session_id
            && grant.session_id != Some(session_id)
            && grant.grantee_session_id != Some(session_id)
        {
            return Err(AppError::bad_request(
                "workflow step and task grant must reference the same session",
            ));
        }
        step.task_grant_id = Some(grant.id);
        let now = Utc::now();

        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get(&step.workflow_run_id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run.status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                if store.workflow_step_runs.contains_key(&step.id) {
                    return Err(AppError::bad_request("workflow step run already exists"));
                }
                if store.task_grants.contains_key(&grant.id) {
                    return Err(AppError::bad_request("task grant already exists"));
                }
                if let Some(parent_grant_id) = grant.parent_grant_id {
                    let parent = store
                        .task_grants
                        .get(&parent_grant_id)
                        .cloned()
                        .ok_or_else(|| AppError::not_found("task grant parent not found"))?;
                    if let Some((message, expire)) = task_grant_runtime_denial(&parent, now) {
                        if expire
                            && let Some(stored) = store.task_grants.get_mut(&parent_grant_id)
                            && stored.status == "active"
                        {
                            stored.status = "expired".to_string();
                            stored.updated_at = now;
                        }
                        return Err(AppError::forbidden(message));
                    }
                    crate::ensure_child_task_grant_within_parent(&parent, &grant)?;
                }
                store.task_grants.insert(grant.id, grant.clone());
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok((step, grant))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let run_status: String = sqlx::query_scalar(
                    "SELECT status
                     FROM workflow_runs
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(step.workflow_run_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if !crate::workflow_run_status_allows_step_creation(&run_status) {
                    return Err(AppError::forbidden("workflow run is not executable"));
                }
                if let Some(parent_grant_id) = grant.parent_grant_id {
                    let select_sql = format!(
                        "SELECT {TASK_GRANT_COLUMNS}
                         FROM task_grants
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE"
                    );
                    let row = sqlx::query(&select_sql)
                        .bind(self.current_tenant_id())
                        .bind(parent_grant_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| AppError::not_found("task grant parent not found"))?;
                    let parent = task_grant_from_row(row)?;
                    if let Some((message, expire)) = task_grant_runtime_denial(&parent, now) {
                        if expire {
                            sqlx::query(
                                "UPDATE task_grants
                                 SET status = 'expired', updated_at = $3
                                 WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
                            )
                            .bind(self.current_tenant_id())
                            .bind(parent_grant_id)
                            .bind(now)
                            .execute(&mut *tx)
                            .await?;
                        }
                        tx.commit().await?;
                        return Err(AppError::forbidden(message));
                    }
                    crate::ensure_child_task_grant_within_parent(&parent, &grant)?;
                }

                sqlx::query(
                    "INSERT INTO workflow_step_runs
                        (id, tenant_id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)",
                )
                .bind(step.id)
                .bind(self.current_tenant_id())
                .bind(step.workflow_run_id)
                .bind(&step.step_key)
                .bind(&step.step_type)
                .bind(step.agent_id)
                .bind(step.agent_version_id)
                .bind(step.session_id)
                .bind(step.thread_id)
                .bind(step.handoff_id)
                .bind(step.environment_id)
                .bind(&step.status)
                .bind(&step.input_payload)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(&step.claimed_by_worker)
                .bind(step.lease_expires_at)
                .bind(step.context_packet_id)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.created_at)
                .bind(step.updated_at)
                .execute(&mut *tx)
                .await?;

                let grant_row = sqlx::query(
                    "INSERT INTO task_grants
                        (id, tenant_id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35)
                     RETURNING id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at",
                )
                .bind(grant.id)
                .bind(self.current_tenant_id())
                .bind(grant.workflow_run_id)
                .bind(grant.workflow_step_run_id)
                .bind(grant.session_id)
                .bind(grant.parent_grant_id)
                .bind(grant.source_event_id)
                .bind(grant.source_handoff_id)
                .bind(&grant.issuer_subject)
                .bind(grant.grantee_agent_id)
                .bind(grant.grantee_session_id)
                .bind(&grant.agent_class)
                .bind(&grant.objective)
                .bind(&grant.risk_level)
                .bind(&grant.status)
                .bind(grant.expires_at)
                .bind(grant.max_turns)
                .bind(grant.max_tool_calls)
                .bind(grant.max_runtime_seconds)
                .bind(grant.max_cost_usd_micros)
                .bind(grant.turns_used)
                .bind(grant.tool_calls_used)
                .bind(grant.cost_usd_micros_used)
                .bind(&grant.semantic_scopes)
                .bind(&grant.memory_scope)
                .bind(&grant.tool_scope)
                .bind(&grant.connector_scope)
                .bind(&grant.approval_policy)
                .bind(&grant.external_effects)
                .bind(grant.context_packet_id)
                .bind(grant.policy_revision_id)
                .bind(&grant.immutable_args_hash)
                .bind(grant.audit_trace_id)
                .bind(grant.created_at)
                .bind(grant.updated_at)
                .fetch_one(&mut *tx)
                .await?;
                let grant = task_grant_from_row(grant_row)?;

                let step_row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET task_grant_id = $3, updated_at = $4
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step.id)
                .bind(grant.id)
                .bind(now)
                .fetch_one(&mut *tx)
                .await?;
                let step = workflow_step_run_from_row(step_row)?;
                tx.commit().await?;
                Ok((step, grant))
            }
        }
    }

    pub(crate) async fn get_workflow_step_run(
        &self,
        id: uuid::Uuid,
    ) -> Result<WorkflowStepRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .workflow_step_runs
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("workflow step run not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at
                     FROM workflow_step_runs
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
            }
        }
    }

    pub(crate) async fn update_workflow_step_run(
        &self,
        step: WorkflowStepRun,
    ) -> Result<WorkflowStepRun, AppError> {
        let updated = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store.workflow_step_runs.contains_key(&step.id) {
                    return Err(AppError::not_found("workflow step run not found"));
                }
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok(step)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET status = $3, output_payload = $4, artifact_ids = $5, approval_ids = $6, tool_call_ids = $7, claimed_by_worker = $8, lease_expires_at = $9, context_packet_id = $10, started_at = $11, completed_at = $12, scheduled_at = $13, updated_at = $14
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step.id)
                .bind(&step.status)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(&step.claimed_by_worker)
                .bind(step.lease_expires_at)
                .bind(step.context_packet_id)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
            }
        }?;
        self.finalize_workflow_step_task_grant(&updated).await?;
        Ok(updated)
    }

    pub(crate) async fn claim_workflow_step_run_if_available(
        &self,
        step: WorkflowStepRun,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<WorkflowStepRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let claimable = store
                    .workflow_step_runs
                    .get(&step.id)
                    .is_some_and(|current| {
                        current.status == "queued"
                            || (current.status == "running"
                                && current
                                    .lease_expires_at
                                    .is_some_and(|expires_at| expires_at <= now))
                    });
                if !claimable {
                    return Err(AppError::not_found("workflow step run not found"));
                }
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok(step)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET status = $3, output_payload = $4, artifact_ids = $5, approval_ids = $6, tool_call_ids = $7, claimed_by_worker = $8, lease_expires_at = $9, context_packet_id = $10, started_at = $11, completed_at = $12, scheduled_at = $13, updated_at = $14
                     WHERE tenant_id = $1 AND id = $2
                       AND (status = 'queued' OR (status = 'running' AND lease_expires_at <= $15))
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step.id)
                .bind(&step.status)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(&step.claimed_by_worker)
                .bind(step.lease_expires_at)
                .bind(step.context_packet_id)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.updated_at)
                .bind(now)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
            }
        }
    }

    pub(crate) async fn update_claimed_workflow_step_run(
        &self,
        step: WorkflowStepRun,
        worker_id: &str,
    ) -> Result<WorkflowStepRun, AppError> {
        let updated = match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                store
                    .workflow_step_runs
                    .get(&step.id)
                    .filter(|current| {
                        current.status == "running"
                            && current.claimed_by_worker.as_deref() == Some(worker_id)
                    })
                    .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                store.workflow_step_runs.insert(step.id, step.clone());
                Ok(step)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET status = $3, output_payload = $4, artifact_ids = $5, approval_ids = $6, tool_call_ids = $7, claimed_by_worker = $8, lease_expires_at = $9, context_packet_id = $10, started_at = $11, completed_at = $12, scheduled_at = $13, updated_at = $14
                     WHERE tenant_id = $1 AND id = $2 AND status = 'running' AND claimed_by_worker = $15
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step.id)
                .bind(&step.status)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(&step.claimed_by_worker)
                .bind(step.lease_expires_at)
                .bind(step.context_packet_id)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.updated_at)
                .bind(worker_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
            }
        }?;
        self.finalize_workflow_step_task_grant(&updated).await?;
        Ok(updated)
    }

    async fn finalize_workflow_step_task_grant(
        &self,
        updated: &WorkflowStepRun,
    ) -> Result<(), AppError> {
        if crate::workflow_step_status_terminal(&updated.status)
            && let Some(task_grant_id) = updated.task_grant_id
        {
            let run = self.get_workflow_run(updated.workflow_run_id).await?;
            if run.root_task_grant_id != Some(task_grant_id) {
                let grant = self.get_task_grant(task_grant_id).await?;
                if grant.status == "active" {
                    let next_status = if crate::workflow_step_status_successful(&updated.status) {
                        "completed"
                    } else {
                        "cancelled"
                    };
                    self.update_task_grant_status(task_grant_id, next_status)
                        .await?;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn renew_workflow_step_run_lease(
        &self,
        id: uuid::Uuid,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<WorkflowStepRun, AppError> {
        if !(1..=86_400).contains(&lease_seconds) {
            return Err(AppError::bad_request(
                "workflow step lease_seconds must be between 1 and 86400",
            ));
        }
        let now = chrono::Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let step = store
                    .workflow_step_runs
                    .get_mut(&id)
                    .filter(|step| {
                        step.status == "running"
                            && step.claimed_by_worker.as_deref() == Some(worker_id)
                    })
                    .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                step.lease_expires_at = Some(now + chrono::Duration::seconds(lease_seconds));
                step.updated_at = now;
                Ok(step.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET lease_expires_at = now() + $1 * interval '1 second', updated_at = $2
                     WHERE tenant_id = $3 AND id = $4 AND status = 'running' AND claimed_by_worker = $5
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(lease_seconds)
                .bind(now)
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(worker_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
            }
        }
    }

    pub(crate) async fn activate_scheduled_workflow_step_run(
        &self,
        step_id: uuid::Uuid,
        checked_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<WorkflowStepRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let Some(step) = store.workflow_step_runs.get_mut(&step_id) else {
                    return Err(AppError::not_found("workflow step run not found"));
                };
                if step.status != "scheduled"
                    || step
                        .scheduled_at
                        .is_none_or(|scheduled_at| scheduled_at > checked_at)
                {
                    return Ok(None);
                }
                step.status = "queued".to_string();
                step.updated_at = checked_at;
                Ok(Some(step.clone()))
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET status = 'queued', updated_at = $3
                     WHERE tenant_id = $1
                       AND id = $2
                       AND status = 'scheduled'
                       AND scheduled_at IS NOT NULL
                       AND scheduled_at <= $3
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step_id)
                .bind(checked_at)
                .fetch_optional(pool)
                .await?;
                row.map(workflow_step_run_from_row).transpose()
            }
        }
    }

    pub(crate) async fn list_workflow_step_runs(
        &self,
        workflow_run_id: uuid::Uuid,
    ) -> Result<Vec<WorkflowStepRun>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut steps: Vec<_> = inner
                    .read()
                    .await
                    .workflow_step_runs
                    .values()
                    .filter(|step| step.workflow_run_id == workflow_run_id)
                    .cloned()
                    .collect();
                steps.sort_by_key(|step| step.created_at);
                Ok(steps)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, claimed_by_worker, lease_expires_at, context_packet_id, started_at, completed_at, scheduled_at, created_at, updated_at
                     FROM workflow_step_runs
                     WHERE tenant_id = $1 AND workflow_run_id = $2
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(workflow_run_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(workflow_step_run_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_workflow_transition(
        &self,
        transition: WorkflowTransition,
    ) -> Result<WorkflowTransition, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .workflow_transitions
                    .insert(transition.id, transition.clone());
                Ok(transition)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO workflow_transitions
                        (id, tenant_id, workflow_run_id, from_step_run_id, from_step_key, to_step_run_id, to_step_key, transition_type, status, condition_payload, result_payload, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                     RETURNING id, workflow_run_id, from_step_run_id, from_step_key, to_step_run_id, to_step_key, transition_type, status, condition_payload, result_payload, created_at",
                )
                .bind(transition.id)
                .bind(self.current_tenant_id())
                .bind(transition.workflow_run_id)
                .bind(transition.from_step_run_id)
                .bind(&transition.from_step_key)
                .bind(transition.to_step_run_id)
                .bind(&transition.to_step_key)
                .bind(&transition.transition_type)
                .bind(&transition.status)
                .bind(&transition.condition_payload)
                .bind(&transition.result_payload)
                .bind(transition.created_at)
                .fetch_one(pool)
                .await?;
                workflow_transition_from_row(row)
            }
        }
    }

    pub(crate) async fn list_workflow_transitions(
        &self,
        workflow_run_id: uuid::Uuid,
    ) -> Result<Vec<WorkflowTransition>, AppError> {
        self.list_workflow_transitions_with_filter(
            workflow_run_id,
            &WorkflowTransitionFilter::default(),
        )
        .await
    }

    pub(crate) async fn list_workflow_transitions_with_filter(
        &self,
        workflow_run_id: uuid::Uuid,
        filter: &WorkflowTransitionFilter,
    ) -> Result<Vec<WorkflowTransition>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut transitions: Vec<_> = inner
                    .read()
                    .await
                    .workflow_transitions
                    .values()
                    .filter(|transition| {
                        transition.workflow_run_id == workflow_run_id
                            && filter
                                .transition_type
                                .as_ref()
                                .is_none_or(|value| transition.transition_type == *value)
                            && filter
                                .status
                                .as_ref()
                                .is_none_or(|value| transition.status == *value)
                            && filter.from_step_key.as_ref().is_none_or(|value| {
                                transition.from_step_key.as_ref() == Some(value)
                            })
                            && filter
                                .to_step_key
                                .as_ref()
                                .is_none_or(|value| transition.to_step_key.as_ref() == Some(value))
                    })
                    .cloned()
                    .collect();
                transitions.sort_by_key(|transition| transition.created_at);
                if let Some(limit) = filter.limit {
                    let start = transitions.len().saturating_sub(limit);
                    transitions = transitions.split_off(start);
                }
                Ok(transitions)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, workflow_run_id, from_step_run_id, from_step_key, to_step_run_id, to_step_key, transition_type, status, condition_payload, result_payload, created_at
                     FROM workflow_transitions
                     WHERE tenant_id = $1
                       AND workflow_run_id = $2
                       AND ($3::text IS NULL OR transition_type = $3)
                       AND ($4::text IS NULL OR status = $4)
                       AND ($5::text IS NULL OR from_step_key = $5)
                       AND ($6::text IS NULL OR to_step_key = $6)
                     ORDER BY created_at DESC
                     LIMIT $7",
                )
                .bind(self.current_tenant_id())
                .bind(workflow_run_id)
                .bind(&filter.transition_type)
                .bind(&filter.status)
                .bind(&filter.from_step_key)
                .bind(&filter.to_step_key)
                .bind(filter.limit.map_or(i64::MAX, |limit| limit as i64))
                .fetch_all(pool)
                .await?;
                let mut transitions = rows
                    .into_iter()
                    .map(workflow_transition_from_row)
                    .collect::<Result<Vec<_>, _>>()?;
                transitions.reverse();
                Ok(transitions)
            }
        }
    }

    pub(crate) async fn create_task_grant(&self, grant: TaskGrant) -> Result<TaskGrant, AppError> {
        crate::validate_task_grant_scope_objects(&grant)?;
        crate::validate_task_grant_budgets(&grant)?;
        if grant.status != "active" {
            return Err(AppError::bad_request("new task grant must be active"));
        }
        if grant.turns_used != 0 || grant.tool_calls_used != 0 || grant.cost_usd_micros_used != 0 {
            return Err(AppError::bad_request(
                "new task grant usage counters must be zero",
            ));
        }
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let run = store
                    .workflow_runs
                    .get(&grant.workflow_run_id)
                    .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if grant.parent_grant_id.is_some() {
                    if !crate::workflow_run_status_allows_execution(&run.status) {
                        return Err(AppError::forbidden("workflow run is not executable"));
                    }
                } else {
                    if (run.status != "initializing"
                        && !crate::workflow_run_status_allows_execution(&run.status))
                        || run.root_task_grant_id.is_some()
                    {
                        return Err(AppError::forbidden(
                            "workflow run does not allow root task grant issuance",
                        ));
                    }
                    if store.task_grants.values().any(|candidate| {
                        candidate.workflow_run_id == grant.workflow_run_id
                            && candidate.parent_grant_id.is_none()
                    }) {
                        return Err(AppError::bad_request(
                            "workflow run already has a root task grant",
                        ));
                    }
                }
                if store.task_grants.contains_key(&grant.id) {
                    return Err(AppError::bad_request("task grant already exists"));
                }
                if let Some(parent_grant_id) = grant.parent_grant_id {
                    let parent = store
                        .task_grants
                        .get(&parent_grant_id)
                        .cloned()
                        .ok_or_else(|| AppError::not_found("task grant parent not found"))?;
                    if let Some((message, expire)) = task_grant_runtime_denial(&parent, now) {
                        if expire
                            && let Some(stored) = store.task_grants.get_mut(&parent_grant_id)
                            && stored.status == "active"
                        {
                            stored.status = "expired".to_string();
                            stored.updated_at = now;
                        }
                        return Err(AppError::forbidden(message));
                    }
                    crate::ensure_child_task_grant_within_parent(&parent, &grant)?;
                }
                store.task_grants.insert(grant.id, grant.clone());
                Ok(grant)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let (run_status, root_task_grant_id): (String, Option<Uuid>) = sqlx::query_as(
                    "SELECT status, root_task_grant_id
                     FROM workflow_runs
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(grant.workflow_run_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("workflow run not found"))?;
                if grant.parent_grant_id.is_some() {
                    if !crate::workflow_run_status_allows_execution(&run_status) {
                        return Err(AppError::forbidden("workflow run is not executable"));
                    }
                } else {
                    if (run_status != "initializing"
                        && !crate::workflow_run_status_allows_execution(&run_status))
                        || root_task_grant_id.is_some()
                    {
                        return Err(AppError::forbidden(
                            "workflow run does not allow root task grant issuance",
                        ));
                    }
                    let root_exists: bool = sqlx::query_scalar(
                        "SELECT EXISTS (
                            SELECT 1
                            FROM task_grants
                            WHERE tenant_id = $1
                              AND workflow_run_id = $2
                              AND parent_grant_id IS NULL
                        )",
                    )
                    .bind(self.current_tenant_id())
                    .bind(grant.workflow_run_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if root_exists {
                        return Err(AppError::bad_request(
                            "workflow run already has a root task grant",
                        ));
                    }
                }
                if let Some(parent_grant_id) = grant.parent_grant_id {
                    let select_sql = format!(
                        "SELECT {TASK_GRANT_COLUMNS}
                         FROM task_grants
                         WHERE tenant_id = $1 AND id = $2
                         FOR UPDATE"
                    );
                    let row = sqlx::query(&select_sql)
                        .bind(self.current_tenant_id())
                        .bind(parent_grant_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| AppError::not_found("task grant parent not found"))?;
                    let parent = task_grant_from_row(row)?;
                    if let Some((message, expire)) = task_grant_runtime_denial(&parent, now) {
                        if expire {
                            sqlx::query(
                                "UPDATE task_grants
                                 SET status = 'expired', updated_at = $3
                                 WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
                            )
                            .bind(self.current_tenant_id())
                            .bind(parent_grant_id)
                            .bind(now)
                            .execute(&mut *tx)
                            .await?;
                        }
                        tx.commit().await?;
                        return Err(AppError::forbidden(message));
                    }
                    crate::ensure_child_task_grant_within_parent(&parent, &grant)?;
                }
                let insert_sql = format!(
                    "INSERT INTO task_grants
                        (id, tenant_id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35)
                     RETURNING {TASK_GRANT_COLUMNS}"
                );
                let row = sqlx::query(&insert_sql)
                    .bind(grant.id)
                    .bind(self.current_tenant_id())
                    .bind(grant.workflow_run_id)
                    .bind(grant.workflow_step_run_id)
                    .bind(grant.session_id)
                    .bind(grant.parent_grant_id)
                    .bind(grant.source_event_id)
                    .bind(grant.source_handoff_id)
                    .bind(&grant.issuer_subject)
                    .bind(grant.grantee_agent_id)
                    .bind(grant.grantee_session_id)
                    .bind(&grant.agent_class)
                    .bind(&grant.objective)
                    .bind(&grant.risk_level)
                    .bind(&grant.status)
                    .bind(grant.expires_at)
                    .bind(grant.max_turns)
                    .bind(grant.max_tool_calls)
                    .bind(grant.max_runtime_seconds)
                    .bind(grant.max_cost_usd_micros)
                    .bind(grant.turns_used)
                    .bind(grant.tool_calls_used)
                    .bind(grant.cost_usd_micros_used)
                    .bind(&grant.semantic_scopes)
                    .bind(&grant.memory_scope)
                    .bind(&grant.tool_scope)
                    .bind(&grant.connector_scope)
                    .bind(&grant.approval_policy)
                    .bind(&grant.external_effects)
                    .bind(grant.context_packet_id)
                    .bind(grant.policy_revision_id)
                    .bind(&grant.immutable_args_hash)
                    .bind(grant.audit_trace_id)
                    .bind(grant.created_at)
                    .bind(grant.updated_at)
                    .fetch_one(&mut *tx)
                    .await?;
                let grant = task_grant_from_row(row)?;
                tx.commit().await?;
                Ok(grant)
            }
        }
    }

    pub(crate) async fn get_task_grant(&self, id: uuid::Uuid) -> Result<TaskGrant, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .task_grants
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("task grant not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at
                     FROM task_grants
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("task grant not found"))?;
                task_grant_from_row(row)
            }
        }
    }

    pub(crate) async fn task_grant_lineage(
        &self,
        grant_id: Uuid,
    ) -> Result<Vec<TaskGrant>, AppError> {
        let mut lineage = Vec::new();
        let mut seen = HashSet::new();
        let mut current_id = Some(grant_id);
        while let Some(id) = current_id {
            if !seen.insert(id) {
                return Err(AppError::bad_request("task grant parent cycle detected"));
            }
            let grant = self.get_task_grant(id).await?;
            current_id = grant.parent_grant_id;
            lineage.push(grant);
        }
        Ok(lineage)
    }

    async fn reserve_task_grant_usage(
        &self,
        grant_id: Uuid,
        reservation: TaskGrantReservation,
    ) -> Result<TaskGrant, AppError> {
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut lineage = Vec::new();
                let mut seen = HashSet::new();
                let mut current_id = Some(grant_id);
                while let Some(id) = current_id {
                    if !seen.insert(id) {
                        return Err(AppError::bad_request("task grant parent cycle detected"));
                    }
                    let grant = store
                        .task_grants
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    current_id = grant.parent_grant_id;
                    lineage.push(grant);
                }
                for grant in &lineage {
                    if let Some((message, expire)) =
                        task_grant_reservation_denial(grant, reservation, now)
                    {
                        if expire
                            && let Some(stored) = store.task_grants.get_mut(&grant.id)
                            && stored.status == "active"
                        {
                            stored.status = "expired".to_string();
                            stored.updated_at = now;
                        }
                        return Err(AppError::forbidden(message));
                    }
                }
                for grant in &lineage {
                    let stored = store
                        .task_grants
                        .get_mut(&grant.id)
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    match reservation {
                        TaskGrantReservation::Turn => {
                            stored.turns_used =
                                stored.turns_used.checked_add(1).ok_or_else(|| {
                                    AppError::bad_request("task grant turn counter overflow")
                                })?;
                        }
                        TaskGrantReservation::ToolCall => {
                            stored.tool_calls_used =
                                stored.tool_calls_used.checked_add(1).ok_or_else(|| {
                                    AppError::bad_request("task grant tool call counter overflow")
                                })?;
                        }
                    }
                    stored.updated_at = now;
                }
                store
                    .task_grants
                    .get(&grant_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("task grant not found"))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let select_sql = format!(
                    "SELECT {TASK_GRANT_COLUMNS}
                     FROM task_grants
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE"
                );
                let mut lineage = Vec::new();
                let mut seen = HashSet::new();
                let mut current_id = Some(grant_id);
                while let Some(id) = current_id {
                    if !seen.insert(id) {
                        return Err(AppError::bad_request("task grant parent cycle detected"));
                    }
                    let row = sqlx::query(&select_sql)
                        .bind(self.current_tenant_id())
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    let grant = task_grant_from_row(row)?;
                    current_id = grant.parent_grant_id;
                    lineage.push(grant);
                }
                for grant in &lineage {
                    if let Some((message, expire)) =
                        task_grant_reservation_denial(grant, reservation, now)
                    {
                        if expire {
                            sqlx::query(
                                "UPDATE task_grants
                                 SET status = 'expired', updated_at = $3
                                 WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
                            )
                            .bind(self.current_tenant_id())
                            .bind(grant.id)
                            .bind(now)
                            .execute(&mut *tx)
                            .await?;
                        }
                        tx.commit().await?;
                        return Err(AppError::forbidden(message));
                    }
                }
                for grant in &lineage {
                    match reservation {
                        TaskGrantReservation::Turn => {
                            sqlx::query(
                                "UPDATE task_grants
                                 SET turns_used = turns_used + 1, updated_at = $3
                                 WHERE tenant_id = $1 AND id = $2",
                            )
                            .bind(self.current_tenant_id())
                            .bind(grant.id)
                            .bind(now)
                            .execute(&mut *tx)
                            .await?;
                        }
                        TaskGrantReservation::ToolCall => {
                            sqlx::query(
                                "UPDATE task_grants
                                 SET tool_calls_used = tool_calls_used + 1, updated_at = $3
                                 WHERE tenant_id = $1 AND id = $2",
                            )
                            .bind(self.current_tenant_id())
                            .bind(grant.id)
                            .bind(now)
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }
                let row = sqlx::query(&select_sql)
                    .bind(self.current_tenant_id())
                    .bind(grant_id)
                    .fetch_one(&mut *tx)
                    .await?;
                let grant = task_grant_from_row(row)?;
                tx.commit().await?;
                Ok(grant)
            }
        }
    }

    pub(crate) async fn reserve_task_grant_turn(
        &self,
        grant_id: Uuid,
    ) -> Result<TaskGrant, AppError> {
        self.reserve_task_grant_usage(grant_id, TaskGrantReservation::Turn)
            .await
    }

    pub(crate) async fn reserve_task_grant_tool_call(
        &self,
        grant_id: Uuid,
    ) -> Result<TaskGrant, AppError> {
        self.reserve_task_grant_usage(grant_id, TaskGrantReservation::ToolCall)
            .await
    }

    pub(crate) async fn add_task_grant_cost(
        &self,
        grant_id: Uuid,
        cost_usd_micros: i64,
    ) -> Result<TaskGrant, AppError> {
        if cost_usd_micros < 0 {
            return Err(AppError::bad_request(
                "task grant cost increment cannot be negative",
            ));
        }
        if cost_usd_micros == 0 {
            return self.get_task_grant(grant_id).await;
        }
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut lineage_ids = Vec::new();
                let mut seen = HashSet::new();
                let mut current_id = Some(grant_id);
                while let Some(id) = current_id {
                    if !seen.insert(id) {
                        return Err(AppError::bad_request("task grant parent cycle detected"));
                    }
                    let grant = store
                        .task_grants
                        .get(&id)
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    current_id = grant.parent_grant_id;
                    lineage_ids.push(id);
                }
                for id in lineage_ids {
                    let grant = store
                        .task_grants
                        .get_mut(&id)
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    grant.cost_usd_micros_used = grant
                        .cost_usd_micros_used
                        .checked_add(cost_usd_micros)
                        .ok_or_else(|| AppError::bad_request("task grant cost counter overflow"))?;
                    grant.updated_at = now;
                }
                store
                    .task_grants
                    .get(&grant_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("task grant not found"))
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let select_sql = format!(
                    "SELECT {TASK_GRANT_COLUMNS}
                     FROM task_grants
                     WHERE tenant_id = $1 AND id = $2
                     FOR UPDATE"
                );
                let mut lineage_ids = Vec::new();
                let mut seen = HashSet::new();
                let mut current_id = Some(grant_id);
                while let Some(id) = current_id {
                    if !seen.insert(id) {
                        return Err(AppError::bad_request("task grant parent cycle detected"));
                    }
                    let row = sqlx::query(&select_sql)
                        .bind(self.current_tenant_id())
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| AppError::not_found("task grant not found"))?;
                    let grant = task_grant_from_row(row)?;
                    current_id = grant.parent_grant_id;
                    lineage_ids.push(id);
                }
                for id in lineage_ids {
                    sqlx::query(
                        "UPDATE task_grants
                         SET cost_usd_micros_used = cost_usd_micros_used + $3,
                             updated_at = $4
                         WHERE tenant_id = $1 AND id = $2",
                    )
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .bind(cost_usd_micros)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
                }
                let row = sqlx::query(&select_sql)
                    .bind(self.current_tenant_id())
                    .bind(grant_id)
                    .fetch_one(&mut *tx)
                    .await?;
                let grant = task_grant_from_row(row)?;
                tx.commit().await?;
                Ok(grant)
            }
        }
    }

    pub(crate) async fn update_task_grant_status(
        &self,
        grant_id: Uuid,
        next_status: &str,
    ) -> Result<TaskGrant, AppError> {
        if !matches!(
            next_status,
            "revoked" | "expired" | "completed" | "cancelled"
        ) {
            return Err(AppError::bad_request(
                "unsupported task grant terminal status",
            ));
        }
        let now = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let grant = store
                    .task_grants
                    .get_mut(&grant_id)
                    .ok_or_else(|| AppError::not_found("task grant not found"))?;
                if grant.status == next_status {
                    return Ok(grant.clone());
                }
                if grant.status != "active" {
                    return Err(AppError::bad_request(
                        "terminal task grant status cannot be changed",
                    ));
                }
                grant.status = next_status.to_string();
                grant.updated_at = now;
                Ok(grant.clone())
            }
            StoreBackend::Postgres(pool) => {
                let sql = format!(
                    "UPDATE task_grants
                     SET status = $3, updated_at = $4
                     WHERE tenant_id = $1 AND id = $2
                       AND (status = 'active' OR status = $3)
                     RETURNING {TASK_GRANT_COLUMNS}"
                );
                let row = sqlx::query(&sql)
                    .bind(self.current_tenant_id())
                    .bind(grant_id)
                    .bind(next_status)
                    .bind(now)
                    .fetch_optional(pool)
                    .await?;
                match row {
                    Some(row) => task_grant_from_row(row),
                    None => {
                        self.get_task_grant(grant_id).await?;
                        Err(AppError::bad_request(
                            "terminal task grant status cannot be changed",
                        ))
                    }
                }
            }
        }
    }

    pub(crate) async fn close_active_task_grants_for_workflow_run(
        &self,
        workflow_run_id: Uuid,
        next_status: &str,
    ) -> Result<Vec<TaskGrant>, AppError> {
        let mut closed = Vec::new();
        for grant in self
            .list_task_grants_for_workflow_run(workflow_run_id)
            .await?
            .into_iter()
            .filter(|grant| grant.status == "active")
        {
            closed.push(self.update_task_grant_status(grant.id, next_status).await?);
        }
        Ok(closed)
    }

    pub(crate) async fn update_task_grant_context_packet(
        &self,
        grant_id: uuid::Uuid,
        context_packet_id: uuid::Uuid,
    ) -> Result<TaskGrant, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let grant = store
                    .task_grants
                    .get_mut(&grant_id)
                    .ok_or_else(|| AppError::not_found("task grant not found"))?;
                grant.context_packet_id = Some(context_packet_id);
                grant.updated_at = chrono::Utc::now();
                Ok(grant.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE task_grants
                     SET context_packet_id = $3, updated_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(grant_id)
                .bind(context_packet_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("task grant not found"))?;
                task_grant_from_row(row)
            }
        }
    }

    pub(crate) async fn list_task_grants_for_workflow_run(
        &self,
        workflow_run_id: uuid::Uuid,
    ) -> Result<Vec<TaskGrant>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut grants: Vec<_> = inner
                    .read()
                    .await
                    .task_grants
                    .values()
                    .filter(|grant| grant.workflow_run_id == workflow_run_id)
                    .cloned()
                    .collect();
                grants.sort_by_key(|grant| grant.created_at);
                Ok(grants)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, turns_used, tool_calls_used, cost_usd_micros_used, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at
                     FROM task_grants
                     WHERE tenant_id = $1 AND workflow_run_id = $2
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(workflow_run_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(task_grant_from_row).collect()
            }
        }
    }
}
