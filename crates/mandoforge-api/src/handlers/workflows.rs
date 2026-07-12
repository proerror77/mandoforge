use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    AgentInboxSnapshot, AppError, AppState, ClaimWorkflowStepRun, ClaimWorkflowStepRunResponse,
    CreateSession, CreateTaskGrant, CreateWorkflowDefinition, CreateWorkflowRun,
    CreateWorkflowStepRun, Permission, RunDueWorkflowSteps, RunWorkflowStepRun,
    RunWorkflowStepRunResponse, SessionStatus, TaskBoardSnapshot, TaskGrant,
    UpdateWorkflowDefinition, UpdateWorkflowStepRun, WorkflowDefinition, WorkflowRun,
    WorkflowRunGraphConsole, WorkflowScheduledStepActivationRun, WorkflowStepRun,
    WorkflowTransition, WorkflowTransitionQuery, activate_due_workflow_steps_for_run,
    advance_workflow_graph_after_step_update, append_user_message_event,
    authorize_collection_request, authorize_request, build_agent_inbox_snapshot,
    build_task_board_snapshot, build_workflow_run_graph_console,
    claim_workflow_step_run as claim_workflow_step_run_inner, collect_session_runtime_refs,
    diff_session_runtime_refs, enforce_worker_environment_binding, enforce_worker_pool_binding,
    enqueue_session_loop, ensure_child_task_grant_within_parent, ensure_primary_session_thread,
    ensure_session_event_exists, issue_root_task_grant_for_workflow_run,
    materialize_workflow_graph_start_steps, new_audit_log, normalize_event_ingestion_policy,
    normalize_optional_runtime_adapter, normalize_optional_runtime_mode, normalize_optional_text,
    normalize_task_grant_risk_level, normalize_workflow_execution_strategy,
    normalize_workflow_release_state, normalize_workflow_run_status,
    normalize_workflow_trigger_type, principal_from_request, record_task_grant_issued,
    record_workflow_step_run_created, record_workflow_step_run_updated,
    record_workflow_step_worker_started, require_non_empty, run_session_loop,
    run_workflow_compensation_adapter_step, run_workflow_delegated_runtime_step,
    set_managed_session_status, task_grant_session_matches,
    update_workflow_step_after_worker_session, validate_dynamic_materialization_provenance_update,
    validate_task_grant_scope_objects, validate_workflow_execution_binding,
    validate_workflow_graph_definition, visible_session_ids_for_principal,
    workflow_definition_agent_version_id, workflow_definition_step_graph_for_execution,
    workflow_handoff_rules_is_dynamic_materialization, workflow_input_digest,
    workflow_run_execution_denial, workflow_run_owns_session,
    workflow_run_runtime_envelope_with_pinned_ontology_release,
    workflow_step_is_adapter_owned_compensation, workflow_step_status_terminal,
    workflow_step_worker_message, workflow_transition_filter_from_query,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workflow-definitions",
            get(list_workflow_definitions).post(create_workflow_definition),
        )
        .route(
            "/api/workflow-definitions/{id}",
            get(get_workflow_definition).patch(update_workflow_definition),
        )
        .route(
            "/api/workflow-runs",
            get(list_workflow_runs).post(create_workflow_run),
        )
        .route("/api/workflow-runs/{id}", get(get_workflow_run))
        .route(
            "/api/workflow-runs/{id}/transitions",
            get(list_workflow_transitions),
        )
        .route(
            "/api/workflow-runs/{id}/graph",
            get(get_workflow_run_graph_console),
        )
        .route(
            "/api/workflow-runs/{id}/scheduled-steps/run-due",
            post(run_due_workflow_steps),
        )
        .route(
            "/api/workflow-runs/{id}/steps",
            get(list_workflow_step_runs).post(create_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}",
            patch(update_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}/claim",
            post(claim_workflow_step_run),
        )
        .route(
            "/api/workflow-step-runs/{id}/run",
            post(run_workflow_step_run),
        )
        .route(
            "/api/workflow-runs/{id}/task-grants",
            get(list_workflow_task_grants).post(create_workflow_task_grant),
        )
        .route("/api/task-grants/{id}", get(get_task_grant))
        .route("/api/task-board", get(get_task_board))
        .route("/api/agents/{id}/inbox", get(get_agent_inbox))
}

async fn list_workflow_definitions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowDefinition>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definitions",
        None,
    )
    .await?;
    Ok(Json(state.list_workflow_definitions().await?))
}

async fn get_workflow_definition(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowDefinition>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definition",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_workflow_definition(id).await?))
}

async fn create_workflow_definition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowDefinition>,
) -> Result<Json<WorkflowDefinition>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definitions",
        None,
    )
    .await?;
    let name = require_non_empty(input.name, "workflow definition name")?;
    let entrypoint = require_non_empty(input.entrypoint, "workflow definition entrypoint")?;
    let trigger_type = normalize_workflow_trigger_type(&input.trigger_type)?;
    let release_state = normalize_workflow_release_state(&input.release_state)?;
    state.get_agent(input.default_agent_id).await?;
    if let Some(environment_id) = input.default_environment_id {
        state.get_environment(environment_id).await?;
    }
    if !input.step_graph.is_object() {
        return Err(AppError::bad_request(
            "workflow definition step_graph must be a JSON object",
        ));
    }
    validate_workflow_graph_definition(&input.step_graph)?;
    if !input.handoff_rules.is_object() {
        return Err(AppError::bad_request(
            "workflow definition handoff_rules must be a JSON object",
        ));
    }
    if workflow_handoff_rules_is_dynamic_materialization(&input.handoff_rules) {
        return Err(AppError::bad_request(
            "dynamic workflow materialization definitions can only be created from an approved dynamic workflow plan",
        ));
    }
    let execution_strategy = normalize_workflow_execution_strategy(&input.execution_strategy)?;
    let runtime_adapter = normalize_optional_runtime_adapter(input.runtime_adapter)?;
    let runtime_mode = normalize_optional_runtime_mode(input.runtime_mode)?;
    let event_ingestion_policy = normalize_event_ingestion_policy(&input.event_ingestion_policy)?;
    validate_workflow_execution_binding(
        &execution_strategy,
        runtime_adapter.as_deref(),
        &input.runtime_capability_contract,
    )?;
    let step_graph =
        workflow_definition_step_graph_for_execution(&execution_strategy, &input.step_graph);
    let (pack_id, pack_version) = if let Some(installation_id) = input.pack_installation_id {
        let installation = state
            .get_workflow_pack_installation(installation_id)
            .await?;
        (Some(installation.pack_id), Some(installation.version))
    } else {
        (None, None)
    };
    let now = Utc::now();
    let definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: input.pack_installation_id,
            pack_id,
            pack_version,
            name,
            entrypoint,
            trigger_type,
            default_agent_id: input.default_agent_id,
            default_environment_id: input.default_environment_id,
            input_schema_ref: input.input_schema_ref,
            output_schema_ref: input.output_schema_ref,
            step_graph,
            handoff_rules: input.handoff_rules,
            execution_strategy,
            runtime_adapter,
            runtime_mode,
            runtime_capability_contract: input.runtime_capability_contract,
            event_ingestion_policy,
            approval_policy_ref: input.approval_policy_ref,
            eval_gate_refs: input.eval_gate_refs,
            release_state,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_definition.created",
            "workflow_definition",
            Some(definition.id),
            json!({
                "name": definition.name,
                "entrypoint": definition.entrypoint,
                "default_agent_id": definition.default_agent_id,
                "pack_installation_id": definition.pack_installation_id,
                "execution_strategy": definition.execution_strategy,
                "runtime_adapter": definition.runtime_adapter,
                "runtime_mode": definition.runtime_mode,
                "event_ingestion_policy": definition.event_ingestion_policy,
                "release_state": definition.release_state
            }),
        ))
        .await?;
    Ok(Json(definition))
}

async fn update_workflow_definition(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateWorkflowDefinition>,
) -> Result<Json<WorkflowDefinition>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "workflow_definition",
        Some(id),
    )
    .await?;
    let mut definition = state.get_workflow_definition(id).await?;
    let dynamic_materialization =
        workflow_handoff_rules_is_dynamic_materialization(&definition.handoff_rules);
    let mut changed_fields = Vec::new();

    if let Some(name) = input.name {
        definition.name = require_non_empty(name, "workflow definition name")?;
        changed_fields.push("name");
    }
    if let Some(entrypoint) = input.entrypoint {
        definition.entrypoint = require_non_empty(entrypoint, "workflow definition entrypoint")?;
        changed_fields.push("entrypoint");
    }
    if let Some(trigger_type) = input.trigger_type {
        definition.trigger_type = normalize_workflow_trigger_type(&trigger_type)?;
        changed_fields.push("trigger_type");
    }
    if let Some(default_agent_id) = input.default_agent_id {
        state.get_agent(default_agent_id).await?;
        definition.default_agent_id = default_agent_id;
        changed_fields.push("default_agent_id");
    }
    if let Some(default_environment_id) = input.default_environment_id {
        if let Some(environment_id) = default_environment_id {
            state.get_environment(environment_id).await?;
        }
        definition.default_environment_id = default_environment_id;
        changed_fields.push("default_environment_id");
    }
    if let Some(input_schema_ref) = input.input_schema_ref {
        definition.input_schema_ref = input_schema_ref.and_then(normalize_optional_text);
        changed_fields.push("input_schema_ref");
    }
    if let Some(output_schema_ref) = input.output_schema_ref {
        definition.output_schema_ref = output_schema_ref.and_then(normalize_optional_text);
        changed_fields.push("output_schema_ref");
    }
    if let Some(step_graph) = input.step_graph {
        if !step_graph.is_object() {
            return Err(AppError::bad_request(
                "workflow definition step_graph must be a JSON object",
            ));
        }
        validate_workflow_graph_definition(&step_graph)?;
        definition.step_graph = step_graph;
        changed_fields.push("step_graph");
    }
    if let Some(handoff_rules) = input.handoff_rules {
        if !handoff_rules.is_object() {
            return Err(AppError::bad_request(
                "workflow definition handoff_rules must be a JSON object",
            ));
        }
        validate_dynamic_materialization_provenance_update(
            &definition.handoff_rules,
            &handoff_rules,
        )?;
        definition.handoff_rules = handoff_rules;
        changed_fields.push("handoff_rules");
    }
    if let Some(execution_strategy) = input.execution_strategy {
        definition.execution_strategy = normalize_workflow_execution_strategy(&execution_strategy)?;
        changed_fields.push("execution_strategy");
    }
    if let Some(runtime_adapter) = input.runtime_adapter {
        definition.runtime_adapter = normalize_optional_runtime_adapter(runtime_adapter)?;
        changed_fields.push("runtime_adapter");
    }
    if let Some(runtime_mode) = input.runtime_mode {
        definition.runtime_mode = normalize_optional_runtime_mode(runtime_mode)?;
        changed_fields.push("runtime_mode");
    }
    if let Some(runtime_capability_contract) = input.runtime_capability_contract {
        if !runtime_capability_contract.is_object() {
            return Err(AppError::bad_request(
                "runtime_capability_contract must be a JSON object",
            ));
        }
        definition.runtime_capability_contract = runtime_capability_contract;
        changed_fields.push("runtime_capability_contract");
    }
    if let Some(event_ingestion_policy) = input.event_ingestion_policy {
        definition.event_ingestion_policy =
            normalize_event_ingestion_policy(&event_ingestion_policy)?;
        changed_fields.push("event_ingestion_policy");
    }
    validate_workflow_execution_binding(
        &definition.execution_strategy,
        definition.runtime_adapter.as_deref(),
        &definition.runtime_capability_contract,
    )?;
    definition.step_graph = workflow_definition_step_graph_for_execution(
        &definition.execution_strategy,
        &definition.step_graph,
    );
    if let Some(approval_policy_ref) = input.approval_policy_ref {
        definition.approval_policy_ref = approval_policy_ref.and_then(normalize_optional_text);
        changed_fields.push("approval_policy_ref");
    }
    if let Some(eval_gate_refs) = input.eval_gate_refs {
        definition.eval_gate_refs = eval_gate_refs
            .into_iter()
            .filter_map(normalize_optional_text)
            .collect();
        changed_fields.push("eval_gate_refs");
    }
    if let Some(release_state) = input.release_state {
        let release_state = normalize_workflow_release_state(&release_state)?;
        if dynamic_materialization && release_state == "released" {
            return Err(AppError::forbidden(
                "dynamic workflow materializations cannot be released through generic workflow definition updates; publish a Workflow Pack with release gates",
            ));
        }
        definition.release_state = release_state;
        changed_fields.push("release_state");
    }

    if changed_fields.is_empty() {
        return Ok(Json(definition));
    }

    let now = Utc::now();
    definition.updated_at = now;
    if definition.release_state == "archived" {
        definition.archived_at = Some(now);
    }
    let updated = state.update_workflow_definition(definition).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_definition.updated",
            "workflow_definition",
            Some(updated.id),
            json!({
                "changed_fields": changed_fields,
                "name": updated.name,
                "entrypoint": updated.entrypoint,
                "default_agent_id": updated.default_agent_id,
                "pack_installation_id": updated.pack_installation_id,
                "execution_strategy": updated.execution_strategy,
                "runtime_adapter": updated.runtime_adapter,
                "runtime_mode": updated.runtime_mode,
                "event_ingestion_policy": updated.event_ingestion_policy,
                "release_state": updated.release_state
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn list_workflow_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowRun>>, AppError> {
    let principal =
        authorize_collection_request(&state, &headers, Permission::SessionsRead, "workflow_runs")
            .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_workflow_runs()
            .await?
            .into_iter()
            .filter(|run| visible_session_ids.contains(&run.primary_session_id))
            .collect(),
    ))
}

async fn get_workflow_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRun>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(run))
}

async fn create_workflow_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowRun>,
) -> Result<Json<WorkflowRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "workflow_runs",
        None,
    )
    .await?;
    let definition = state
        .get_workflow_definition(input.workflow_definition_id)
        .await?;
    if definition.release_state != "released" {
        return Err(AppError::bad_request(
            "workflow run requires a released workflow definition",
        ));
    }
    if let Some(work_item_id) = input.source_work_item_id {
        state.ensure_work_item_exists(work_item_id).await?;
    }
    if let Some(source_event_id) = input.source_event_id {
        ensure_session_event_exists(&state, source_event_id).await?;
    }
    let environment_id = input.environment_id.or(definition.default_environment_id);
    if let Some(environment_id) = environment_id {
        state.get_environment(environment_id).await?;
    }
    let title = input
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| format!("Workflow: {}", definition.name));
    let input_digest = workflow_input_digest(&input.input_payload);
    let execution_strategy = input
        .execution_strategy
        .as_deref()
        .map(normalize_workflow_execution_strategy)
        .transpose()?
        .unwrap_or_else(|| definition.execution_strategy.clone());
    let runtime_adapter = match input.runtime_adapter {
        Some(runtime_adapter) => normalize_optional_runtime_adapter(Some(runtime_adapter))?,
        None => definition.runtime_adapter.clone(),
    };
    let runtime_mode = match input.runtime_mode {
        Some(runtime_mode) => normalize_optional_runtime_mode(Some(runtime_mode))?,
        None => definition.runtime_mode.clone(),
    };
    validate_workflow_execution_binding(
        &execution_strategy,
        runtime_adapter.as_deref(),
        &definition.runtime_capability_contract,
    )?;
    let external_run_ref = input.external_run_ref.and_then(normalize_optional_text);
    let runtime_event_cursor = input.runtime_event_cursor.and_then(normalize_optional_text);
    let delegation_status =
        (execution_strategy == "delegated_runtime").then_some("submitted".to_string());
    let runtime_envelope = workflow_run_runtime_envelope_with_pinned_ontology_release(
        &state,
        &definition,
        &execution_strategy,
        runtime_adapter.as_deref(),
        runtime_mode.as_deref(),
        external_run_ref.as_deref(),
        &input.runtime_envelope,
    )
    .await?;
    let session_input = CreateSession {
        agent_id: definition.default_agent_id,
        environment_id,
        title,
        message: None,
    };
    let session =
        match workflow_definition_agent_version_id(&definition, definition.default_agent_id)? {
            Some(agent_version_id) => {
                state
                    .create_session_for_agent_version(session_input, agent_version_id)
                    .await?
            }
            None => state.create_session(session_input).await?,
        };
    if let Err(error) = ensure_primary_session_thread(&state, session.id).await {
        let _ = set_managed_session_status(
            &state,
            session.id,
            SessionStatus::Failed,
            "workflow run initialization failed before primary thread creation",
        )
        .await;
        return Err(error);
    }
    let now = Utc::now();
    let run = match state
        .create_workflow_run(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_definition_id: definition.id,
            pack_installation_id: definition.pack_installation_id,
            source_event_id: input.source_event_id,
            source_work_item_id: input.source_work_item_id,
            source_schedule_id: input.source_schedule_id,
            status: "initializing".to_string(),
            primary_session_id: session.id,
            root_task_grant_id: None,
            input_payload: input.input_payload,
            input_digest,
            execution_strategy,
            runtime_adapter,
            runtime_mode,
            delegation_status,
            external_run_ref,
            runtime_event_cursor,
            runtime_envelope,
            started_at: None,
            completed_at: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await
    {
        Ok(run) => run,
        Err(error) => {
            let _ = set_managed_session_status(
                &state,
                session.id,
                SessionStatus::Failed,
                "workflow run persistence failed",
            )
            .await;
            return Err(error);
        }
    };
    let initialized = async {
        let root_grant =
            issue_root_task_grant_for_workflow_run(&state, &run, &definition, &session).await?;
        let linked_run = state
            .update_workflow_run_root_task_grant(run.id, root_grant.id)
            .await?;
        materialize_workflow_graph_start_steps(
            &state,
            &definition,
            &linked_run,
            &session,
            &root_grant,
        )
        .await?;
        state
            .update_workflow_run_status(run.id, "queued".to_string(), None, None)
            .await
    }
    .await;
    let run = match initialized {
        Ok(run) => run,
        Err(error) => {
            let failed_at = Utc::now();
            let _ = state
                .update_workflow_run_status(run.id, "failed".to_string(), None, Some(failed_at))
                .await;
            let _ = state
                .close_active_task_grants_for_workflow_run(run.id, "cancelled")
                .await;
            let _ = set_managed_session_status(
                &state,
                session.id,
                SessionStatus::Failed,
                "workflow run initialization failed",
            )
            .await;
            let _ = state
                .append_audit_log(new_audit_log(
                    Some(session.id),
                    "system",
                    Some(run.id),
                    "workflow_run.initialization_failed",
                    "workflow_run",
                    Some(run.id),
                    json!({
                        "workflow_definition_id": definition.id,
                        "primary_session_id": session.id,
                        "error": error.message
                    }),
                ))
                .await;
            return Err(error);
        }
    };
    state
        .append_event(
            "system",
            Some(run.id),
            session.id,
            "workflow.run.created",
            json!({
                "workflow_run_id": run.id,
                "workflow_definition_id": run.workflow_definition_id,
                "pack_installation_id": run.pack_installation_id,
                "root_task_grant_id": run.root_task_grant_id,
                "input_digest": run.input_digest,
                "execution_strategy": run.execution_strategy,
                "runtime_adapter": run.runtime_adapter,
                "runtime_mode": run.runtime_mode,
                "delegation_status": run.delegation_status,
                "external_run_ref": run.external_run_ref,
                "status": run.status
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session.id),
            "system",
            Some(run.id),
            "workflow_run.created",
            "workflow_run",
            Some(run.id),
            json!({
                "workflow_definition_id": run.workflow_definition_id,
                "pack_installation_id": run.pack_installation_id,
                "primary_session_id": run.primary_session_id,
                "root_task_grant_id": run.root_task_grant_id,
                "input_digest": run.input_digest,
                "execution_strategy": run.execution_strategy,
                "runtime_adapter": run.runtime_adapter,
                "runtime_mode": run.runtime_mode,
                "delegation_status": run.delegation_status,
                "external_run_ref": run.external_run_ref
            }),
        ))
        .await?;
    Ok(Json(run))
}

async fn list_workflow_transitions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkflowTransitionQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowTransition>>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let filter = workflow_transition_filter_from_query(query)?;
    Ok(Json(
        state
            .list_workflow_transitions_with_filter(id, &filter)
            .await?,
    ))
}

async fn get_workflow_run_graph_console(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<WorkflowRunGraphConsole>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(build_workflow_run_graph_console(&state, &run).await?))
}

async fn run_due_workflow_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunDueWorkflowSteps>,
) -> Result<Json<WorkflowScheduledStepActivationRun>, AppError> {
    let _ = input;
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let checked_at = Utc::now();
    Ok(Json(
        activate_due_workflow_steps_for_run(&state, &run, checked_at).await?,
    ))
}

async fn list_workflow_step_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkflowStepRun>>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(state.list_workflow_step_runs(id).await?))
}

async fn create_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkflowStepRun>,
) -> Result<Json<WorkflowStepRun>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    let step_key = require_non_empty(input.step_key, "workflow step key")?;
    let step_type = require_non_empty(input.step_type, "workflow step type")?;
    let status = normalize_workflow_run_status(&input.status)?;
    let session_id = input.session_id.unwrap_or(run.primary_session_id);
    if !workflow_run_owns_session(&state, &run, session_id).await? {
        return Err(AppError::forbidden(
            "session is not part of this workflow run",
        ));
    }
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    let session = state.get_session(session_id).await?;
    if input
        .environment_id
        .is_some_and(|environment_id| Some(environment_id) != session.environment_id)
    {
        return Err(AppError::bad_request(
            "workflow step environment must match its session environment",
        ));
    }
    if input
        .agent_id
        .is_some_and(|agent_id| agent_id != session.agent_id)
    {
        return Err(AppError::bad_request(
            "workflow step agent must match its session agent",
        ));
    }
    if let Some(thread_id) = input.thread_id {
        state.get_session_thread(thread_id).await?;
    }
    if let Some(handoff_id) = input.handoff_id {
        state.get_agent_handoff_event(handoff_id).await?;
    }
    let agent_version_id = match (input.agent_version_id, input.agent_id) {
        (Some(agent_version_id), Some(_)) => {
            if Some(agent_version_id) != session.agent_version_id {
                return Err(AppError::bad_request(
                    "workflow step agent_version_id must match its session agent version",
                ));
            }
            Some(agent_version_id)
        }
        (Some(_), None) => {
            return Err(AppError::bad_request(
                "workflow step agent_id is required when agent_version_id is provided",
            ));
        }
        (None, Some(_)) => session.agent_version_id,
        (None, None) => None,
    };
    if let Some(task_grant_id) = input.task_grant_id {
        let grant = state.get_task_grant(task_grant_id).await?;
        if grant.workflow_run_id != run.id
            || grant.status != "active"
            || !task_grant_session_matches(&grant, &run, session_id)
        {
            return Err(AppError::bad_request(
                "workflow step task_grant_id must reference an active grant for the workflow session",
            ));
        }
    }
    let now = Utc::now();
    let step = state
        .create_workflow_step_run(WorkflowStepRun {
            id: Uuid::new_v4(),
            workflow_run_id: run.id,
            step_key,
            step_type,
            agent_id: input.agent_id,
            agent_version_id,
            session_id: Some(session_id),
            thread_id: input.thread_id,
            handoff_id: input.handoff_id,
            task_grant_id: input.task_grant_id,
            environment_id: session.environment_id,
            status,
            input_payload: input.input_payload,
            output_payload: input.output_payload,
            artifact_ids: input.artifact_ids,
            approval_ids: input.approval_ids,
            tool_call_ids: input.tool_call_ids,
            claimed_by_worker: None,
            lease_expires_at: None,
            context_packet_id: None,
            started_at: None,
            completed_at: None,
            scheduled_at: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    record_workflow_step_run_created(&state, &run, &step).await?;
    Ok(Json(step))
}

async fn update_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateWorkflowStepRun>,
) -> Result<Json<WorkflowStepRun>, AppError> {
    let current = state.get_workflow_step_run(id).await?;
    let run = state.get_workflow_run(current.workflow_run_id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;

    let previous_status = current.status.clone();
    let next_status = input
        .status
        .as_deref()
        .map(normalize_workflow_run_status)
        .transpose()?
        .unwrap_or_else(|| current.status.clone());
    if workflow_step_status_terminal(&previous_status) && previous_status != next_status {
        return Err(AppError::bad_request(
            "terminal workflow step runs cannot be transitioned",
        ));
    }

    let now = Utc::now();
    let mut next = current;
    next.status = next_status;
    if let Some(output_payload) = input.output_payload {
        if !output_payload.is_object() {
            return Err(AppError::bad_request(
                "workflow step output_payload must be a JSON object",
            ));
        }
        next.output_payload = output_payload;
    }
    if let Some(artifact_ids) = input.artifact_ids {
        next.artifact_ids = artifact_ids;
    }
    if let Some(approval_ids) = input.approval_ids {
        next.approval_ids = approval_ids;
    }
    if let Some(tool_call_ids) = input.tool_call_ids {
        next.tool_call_ids = tool_call_ids;
    }
    if next.status == "running" && next.started_at.is_none() {
        next.started_at = Some(now);
    }
    if workflow_step_status_terminal(&next.status) && next.completed_at.is_none() {
        next.completed_at = Some(now);
        if next.started_at.is_none() {
            next.started_at = Some(now);
        }
    }
    next.updated_at = now;

    let updated = state.update_workflow_step_run(next).await?;
    record_workflow_step_run_updated(&state, &run, &updated, &previous_status).await?;
    if previous_status != updated.status {
        advance_workflow_graph_after_step_update(&state, &run, &updated).await?;
    }
    Ok(Json(updated))
}

async fn claim_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ClaimWorkflowStepRun>,
) -> Result<Json<ClaimWorkflowStepRunResponse>, AppError> {
    let current = state.get_workflow_step_run(id).await?;
    let run = state.get_workflow_run(current.workflow_run_id).await?;
    Ok(Json(
        claim_workflow_step_run_inner(&state, &headers, current, run, input).await?,
    ))
}

async fn run_workflow_step_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunWorkflowStepRun>,
) -> Result<Json<RunWorkflowStepRunResponse>, AppError> {
    let current = state.get_workflow_step_run(id).await?;
    let run = state.get_workflow_run(current.workflow_run_id).await?;
    if let Some(reason) = workflow_run_execution_denial(&run.status) {
        return Err(AppError::forbidden(reason));
    }
    let session_id = current.session_id.unwrap_or(run.primary_session_id);
    enforce_worker_environment_binding(&state, &headers, session_id, current.environment_id)
        .await?;
    enforce_worker_pool_binding(&state, &headers, session_id, current.environment_id).await?;
    if workflow_step_is_adapter_owned_compensation(&current) {
        return run_workflow_compensation_adapter_step(&state, &headers, current, run, input).await;
    }
    if current.step_type == "delegated_runtime" || run.execution_strategy == "delegated_runtime" {
        return run_workflow_delegated_runtime_step(&state, &headers, current, run, input).await;
    }
    let agent_id = input
        .agent_id
        .or(current.agent_id)
        .ok_or_else(|| AppError::bad_request("workflow step run requires agent_id"))?;
    let worker_id = input
        .worker_id
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            headers
                .get("x-mandoforge-worker-id")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("agent:{agent_id}"))
        });
    let claim = claim_workflow_step_run_inner(
        &state,
        &headers,
        current,
        run.clone(),
        ClaimWorkflowStepRun {
            agent_id,
            worker_id: Some(worker_id.clone()),
            lease_seconds: input.lease_seconds,
        },
    )
    .await?;
    let session_id = claim.step.session_id.unwrap_or(run.primary_session_id);
    let before_refs = collect_session_runtime_refs(&state, session_id).await?;
    record_workflow_step_worker_started(
        &state,
        &run,
        &claim.step,
        &worker_id,
        claim.context_packet.id,
    )
    .await?;
    let trigger_event = append_user_message_event(
        &state,
        session_id,
        workflow_step_worker_message(&claim.step, &claim.task_grant),
    )
    .await?;
    let queued = enqueue_session_loop(
        &state,
        session_id,
        Some(trigger_event.id),
        "workflow.step.run",
    )
    .await?;
    let running = state.start_session_loop_job(queued.id, &worker_id).await?;
    state
        .append_event(
            "worker",
            Some(running.id),
            running.session_id,
            "session.loop.started",
            json!({
                "session_loop_job_id": running.id,
                "environment_id": running.environment_id,
                "worker_id": worker_id,
                "attempt_count": running.attempt_count,
                "workflow_step_run_id": claim.step.id
            }),
        )
        .await?;

    match run_session_loop(&state, &running).await {
        Ok(session) => {
            let completed = state
                .complete_session_loop_job(running.id, &worker_id)
                .await?;
            state
                .append_event(
                    "worker",
                    Some(completed.id),
                    completed.session_id,
                    "session.loop.completed",
                    json!({
                        "session_loop_job_id": completed.id,
                        "status": completed.status,
                        "session_status": session.status,
                        "worker_id": worker_id,
                        "workflow_step_run_id": claim.step.id
                    }),
                )
                .await?;
            let refs = diff_session_runtime_refs(
                &before_refs,
                &collect_session_runtime_refs(&state, session_id).await?,
            );
            let step = update_workflow_step_after_worker_session(
                &state,
                &run,
                &claim.step,
                &session,
                &completed,
                &worker_id,
                refs,
                None,
                false,
            )
            .await?;
            Ok(Json(RunWorkflowStepRunResponse {
                step,
                task_grant: claim.task_grant,
                context_packet: claim.context_packet,
                session,
                session_loop_job: completed,
            }))
        }
        Err(error) => {
            let error_message = error.message.clone();
            let failed = state
                .fail_session_loop_job(running.id, &worker_id, &error_message)
                .await?;
            set_managed_session_status(
                &state,
                failed.session_id,
                SessionStatus::Failed,
                "workflow step session loop failed",
            )
            .await?;
            state
                .append_event(
                    "worker",
                    Some(failed.id),
                    failed.session_id,
                    "session.loop.failed",
                    json!({
                        "session_loop_job_id": failed.id,
                        "status": failed.status,
                        "error": error_message,
                        "worker_id": worker_id,
                        "workflow_step_run_id": claim.step.id
                    }),
                )
                .await?;
            let session = state.get_session(session_id).await?;
            let refs = diff_session_runtime_refs(
                &before_refs,
                &collect_session_runtime_refs(&state, session_id).await?,
            );
            let step = update_workflow_step_after_worker_session(
                &state,
                &run,
                &claim.step,
                &session,
                &failed,
                &worker_id,
                refs,
                Some(error_message),
                false,
            )
            .await?;
            Ok(Json(RunWorkflowStepRunResponse {
                step,
                task_grant: claim.task_grant,
                context_packet: claim.context_packet,
                session,
                session_loop_job: failed,
            }))
        }
    }
}

async fn list_workflow_task_grants(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<TaskGrant>>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(state.list_task_grants_for_workflow_run(id).await?))
}

async fn get_task_grant(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TaskGrant>, AppError> {
    let grant = state.get_task_grant(id).await?;
    let run = state.get_workflow_run(grant.workflow_run_id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    Ok(Json(grant))
}

async fn create_workflow_task_grant(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTaskGrant>,
) -> Result<Json<TaskGrant>, AppError> {
    let run = state.get_workflow_run(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(run.primary_session_id),
    )
    .await?;
    if let Some(reason) = workflow_run_execution_denial(&run.status) {
        return Err(AppError::forbidden(reason));
    }
    let principal = principal_from_request(&state, &headers).await?;
    let parent_grant_id = input
        .parent_grant_id
        .ok_or_else(|| AppError::bad_request("task grant parent_grant_id is required"))?;
    let parent = state.get_task_grant(parent_grant_id).await?;
    if parent.workflow_run_id != run.id {
        return Err(AppError::bad_request(
            "task grant parent_grant_id must belong to workflow run",
        ));
    }
    if parent.status != "active" {
        return Err(AppError::bad_request(
            "task grant parent_grant_id must reference an active grant",
        ));
    }
    if let Some(step_run_id) = input.workflow_step_run_id {
        let step_exists = state
            .list_workflow_step_runs(run.id)
            .await?
            .into_iter()
            .any(|step| step.id == step_run_id);
        if !step_exists {
            return Err(AppError::bad_request(
                "task grant workflow_step_run_id must belong to workflow run",
            ));
        }
    }
    let session_id = match (input.session_id, input.grantee_session_id) {
        (Some(session_id), Some(grantee_session_id)) if session_id != grantee_session_id => {
            return Err(AppError::bad_request(
                "task grant session_id and grantee_session_id must match",
            ));
        }
        (Some(session_id), _) | (_, Some(session_id)) => session_id,
        (None, None) => run.primary_session_id,
    };
    if !workflow_run_owns_session(&state, &run, session_id).await? {
        return Err(AppError::forbidden(
            "session is not part of this workflow run",
        ));
    }
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    let session = state.get_session(session_id).await?;
    if let Some(source_event_id) = input.source_event_id {
        ensure_session_event_exists(&state, source_event_id).await?;
    }
    if let Some(handoff_id) = input.source_handoff_id {
        state.get_agent_handoff_event(handoff_id).await?;
    }
    if let Some(agent_id) = input.grantee_agent_id {
        state.get_agent(agent_id).await?;
        if agent_id != session.agent_id {
            return Err(AppError::bad_request(
                "task grant grantee agent must match its session agent",
            ));
        }
    }
    if let Some(context_packet_id) = input.context_packet_id {
        let context_packet = state.get_context_packet(context_packet_id).await?;
        if context_packet.session_id != session_id {
            return Err(AppError::bad_request(
                "task grant context packet must belong to its session",
            ));
        }
    }
    if let Some(policy_revision_id) = input.policy_revision_id {
        state.get_policy_revision(policy_revision_id).await?;
    }
    let now = Utc::now();
    let issuer_subject = input
        .issuer_subject
        .filter(|subject| !subject.trim().is_empty())
        .unwrap_or(principal.subject_id);
    let objective = input
        .objective
        .filter(|objective| !objective.trim().is_empty())
        .unwrap_or_else(|| parent.objective.clone());
    let grant = TaskGrant {
        id: Uuid::new_v4(),
        workflow_run_id: run.id,
        workflow_step_run_id: input.workflow_step_run_id,
        session_id: Some(session_id),
        parent_grant_id: Some(parent.id),
        source_event_id: input.source_event_id,
        source_handoff_id: input.source_handoff_id,
        issuer_subject,
        grantee_agent_id: input.grantee_agent_id,
        grantee_session_id: input.grantee_session_id.or(Some(session_id)),
        agent_class: input.agent_class.filter(|value| !value.trim().is_empty()),
        objective,
        risk_level: normalize_task_grant_risk_level(&input.risk_level)?,
        status: "active".to_string(),
        expires_at: input.expires_at,
        max_turns: input.max_turns,
        max_tool_calls: input.max_tool_calls,
        max_runtime_seconds: input.max_runtime_seconds,
        max_cost_usd_micros: input.max_cost_usd_micros,
        turns_used: 0,
        tool_calls_used: 0,
        cost_usd_micros_used: 0,
        semantic_scopes: input.semantic_scopes,
        memory_scope: input.memory_scope,
        tool_scope: input.tool_scope,
        connector_scope: input.connector_scope,
        approval_policy: input.approval_policy,
        external_effects: input.external_effects,
        context_packet_id: input.context_packet_id,
        policy_revision_id: input.policy_revision_id,
        immutable_args_hash: input
            .immutable_args_hash
            .filter(|value| !value.trim().is_empty()),
        audit_trace_id: None,
        created_at: now,
        updated_at: now,
    };
    validate_task_grant_scope_objects(&grant)?;
    ensure_child_task_grant_within_parent(&parent, &grant)?;
    let grant = state.create_task_grant(grant).await?;
    record_task_grant_issued(&state, &grant, run.primary_session_id).await?;
    Ok(Json(grant))
}

async fn get_task_board(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TaskBoardSnapshot>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "task_board",
        None,
    )
    .await?;
    Ok(Json(build_task_board_snapshot(&state).await?))
}

async fn get_agent_inbox(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentInboxSnapshot>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_inbox",
        Some(id),
    )
    .await?;
    state.get_agent(id).await?;
    Ok(Json(build_agent_inbox_snapshot(&state, id).await?))
}
