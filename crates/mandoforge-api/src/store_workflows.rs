use anyhow::Result;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    task_grant_from_row, workflow_definition_from_row, workflow_run_from_row,
    workflow_step_run_from_row, workflow_transition_from_row,
};
use crate::{
    AppError, AppState, TaskGrant, WorkflowDefinition, WorkflowRun, WorkflowStepRun,
    WorkflowTransition, WorkflowTransitionFilter,
};

impl AppState {
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
                        (id, tenant_id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                     RETURNING id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at",
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
                    "SELECT id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at
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
                    "SELECT id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at
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

    pub(crate) async fn update_workflow_definition_release_states_for_pack_installation(
        &self,
        installation_id: uuid::Uuid,
        release_state: &str,
    ) -> Result<Vec<WorkflowDefinition>, AppError> {
        let updated_at = chrono::Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let mut definitions = Vec::new();
                for definition in store.workflow_definitions.values_mut() {
                    if definition.pack_installation_id == Some(installation_id)
                        && definition.archived_at.is_none()
                    {
                        definition.release_state = release_state.to_string();
                        definition.updated_at = updated_at;
                        if release_state == "archived" {
                            definition.archived_at = Some(updated_at);
                        }
                        definitions.push(definition.clone());
                    }
                }
                definitions.sort_by_key(|definition| definition.created_at);
                Ok(definitions)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "UPDATE workflow_definitions
                     SET release_state = $3,
                         updated_at = $4,
                         archived_at = CASE WHEN $3 = 'archived' THEN $4 ELSE archived_at END
                     WHERE tenant_id = $1
                       AND pack_installation_id = $2
                       AND archived_at IS NULL
                     RETURNING id, pack_installation_id, pack_id, pack_version, name, entrypoint, trigger_type, default_agent_id, default_environment_id, input_schema_ref, output_schema_ref, step_graph, handoff_rules, approval_policy_ref, eval_gate_refs, release_state, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(installation_id)
                .bind(release_state)
                .bind(updated_at)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(workflow_definition_from_row).collect()
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
                        (id, tenant_id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at",
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
                    "SELECT id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at
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
                    "SELECT id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at
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
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at",
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
                     RETURNING id, workflow_definition_id, pack_installation_id, source_event_id, source_work_item_id, source_schedule_id, status, primary_session_id, root_task_grant_id, input_payload, input_digest, started_at, completed_at, audit_trace_id, created_at, updated_at",
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
                inner
                    .write()
                    .await
                    .workflow_step_runs
                    .insert(step.id, step.clone());
                Ok(step)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO workflow_step_runs
                        (id, tenant_id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at",
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
                .bind(step.task_grant_id)
                .bind(step.environment_id)
                .bind(&step.status)
                .bind(&step.input_payload)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.created_at)
                .bind(step.updated_at)
                .fetch_one(pool)
                .await?;
                workflow_step_run_from_row(row)
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
                    "SELECT id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at
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
        match &self.store {
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
                     SET status = $3, output_payload = $4, artifact_ids = $5, approval_ids = $6, tool_call_ids = $7, started_at = $8, completed_at = $9, scheduled_at = $10, updated_at = $11
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step.id)
                .bind(&step.status)
                .bind(&step.output_payload)
                .bind(serde_json::to_value(&step.artifact_ids)?)
                .bind(serde_json::to_value(&step.approval_ids)?)
                .bind(serde_json::to_value(&step.tool_call_ids)?)
                .bind(step.started_at)
                .bind(step.completed_at)
                .bind(step.scheduled_at)
                .bind(step.updated_at)
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
                    || !step
                        .scheduled_at
                        .is_some_and(|scheduled_at| scheduled_at <= checked_at)
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
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at",
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

    pub(crate) async fn update_workflow_step_run_task_grant(
        &self,
        step_id: uuid::Uuid,
        task_grant_id: uuid::Uuid,
    ) -> Result<WorkflowStepRun, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let step = store
                    .workflow_step_runs
                    .get_mut(&step_id)
                    .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                step.task_grant_id = Some(task_grant_id);
                step.updated_at = chrono::Utc::now();
                Ok(step.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE workflow_step_runs
                     SET task_grant_id = $3, updated_at = now()
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at",
                )
                .bind(self.current_tenant_id())
                .bind(step_id)
                .bind(task_grant_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("workflow step run not found"))?;
                workflow_step_run_from_row(row)
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
                    "SELECT id, workflow_run_id, step_key, step_type, agent_id, agent_version_id, session_id, thread_id, handoff_id, task_grant_id, environment_id, status, input_payload, output_payload, artifact_ids, approval_ids, tool_call_ids, started_at, completed_at, scheduled_at, created_at, updated_at
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
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .task_grants
                    .insert(grant.id, grant.clone());
                Ok(grant)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO task_grants
                        (id, tenant_id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32)
                     RETURNING id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at",
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
                .fetch_one(pool)
                .await?;
                task_grant_from_row(row)
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
                    "SELECT id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at
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
                    "SELECT id, workflow_run_id, workflow_step_run_id, session_id, parent_grant_id, source_event_id, source_handoff_id, issuer_subject, grantee_agent_id, grantee_session_id, agent_class, objective, risk_level, status, expires_at, max_turns, max_tool_calls, max_runtime_seconds, max_cost_usd_micros, semantic_scopes, memory_scope, tool_scope, connector_scope, approval_policy, external_effects, context_packet_id, policy_revision_id, immutable_args_hash, audit_trace_id, created_at, updated_at
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
