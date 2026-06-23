use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, CompileDynamicWorkflowPlan, CreateDynamicWorkflowPlan, CreateSession,
    DynamicWorkflowAdjudicationRequest, DynamicWorkflowAdjudicationResponse, DynamicWorkflowPlan,
    DynamicWorkflowPlanCompilationResponse, DynamicWorkflowPlanMaterializationResponse,
    DynamicWorkflowPressureTestRequest, DynamicWorkflowPressureTestResponse,
    MaterializeDynamicWorkflowPlan, Permission, ReviewDynamicWorkflowPlan, WorkflowDefinition,
    WorkflowRun, analyze_dynamic_workflow_plan, authorize_collection_request,
    authorize_dynamic_workflow_plan_read, authorize_dynamic_workflow_plan_run, authorize_request,
    compile_dynamic_workflow_phases, compile_native_dynamic_workflow_phases, dynamic_policy_u64,
    dynamic_workflow_plan_event_ingestion_policy, dynamic_workflow_plan_execution_strategy,
    dynamic_workflow_plan_handoff_rules, dynamic_workflow_plan_runtime_adapter,
    dynamic_workflow_plan_runtime_capability_contract, dynamic_workflow_plan_runtime_mode,
    dynamic_workflow_plan_step_graph, dynamic_workflow_plan_visible_to_principal,
    dynamic_workflow_step_vote, empty_json_object, ensure_primary_session_thread,
    issue_root_task_grant_for_workflow_run, materialize_workflow_graph_start_steps, new_audit_log,
    normalize_dynamic_workflow_plan_status, normalize_optional_runtime_adapter,
    normalize_optional_text, normalize_workflow_execution_strategy,
    record_dynamic_workflow_plan_audit, require_non_empty,
    validate_dynamic_workflow_agent_fleet_policy, validate_dynamic_workflow_governance,
    validate_dynamic_workflow_materialization, validate_dynamic_workflow_phases,
    validate_dynamic_workflow_validation, validate_workflow_execution_binding,
    validate_workflow_graph_definition, visible_session_ids_for_principal, workflow_input_digest,
    workflow_pack_materialization_default_agent, workflow_run_runtime_envelope, workflow_slug,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/dynamic-workflow-plans",
            get(list_dynamic_workflow_plans).post(create_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/compile",
            post(compile_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}",
            get(get_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/review",
            post(review_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/adjudicate",
            post(adjudicate_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/pressure-test",
            post(pressure_test_dynamic_workflow_plan),
        )
        .route(
            "/api/dynamic-workflow-plans/{id}/materialize",
            post(materialize_dynamic_workflow_plan),
        )
}

async fn list_dynamic_workflow_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DynamicWorkflowPlan>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "dynamic_workflow_plans",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    let mut visible_plans = Vec::new();
    for plan in state.list_dynamic_workflow_plans().await? {
        if dynamic_workflow_plan_visible_to_principal(
            &state,
            &principal,
            &visible_session_ids,
            &plan,
        )
        .await?
        {
            visible_plans.push(plan);
        }
    }
    Ok(Json(visible_plans))
}

async fn get_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    let plan = state.get_dynamic_workflow_plan(id).await?;
    authorize_dynamic_workflow_plan_read(&state, &headers, &plan).await?;
    Ok(Json(plan))
}

async fn create_dynamic_workflow_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "dynamic_workflow_plans",
        None,
    )
    .await?;
    if let Some(work_item_id) = input.source_work_item_id {
        authorize_request(
            &state,
            &headers,
            Permission::SessionsRead,
            "work_item",
            Some(work_item_id),
        )
        .await?;
    }
    if let Some(session_id) = input.source_session_id {
        authorize_request(
            &state,
            &headers,
            Permission::SessionsRead,
            "session",
            Some(session_id),
        )
        .await?;
        state.get_session(session_id).await?;
    }
    let objective = require_non_empty(input.objective, "dynamic workflow objective")?;
    let phases = validate_dynamic_workflow_phases(input.phases)?;
    let agent_fleet_policy =
        validate_dynamic_workflow_agent_fleet_policy(input.agent_fleet_policy)?;
    let governance = validate_dynamic_workflow_governance(input.governance)?;
    let validation = validate_dynamic_workflow_validation(input.validation)?;
    let materialization = validate_dynamic_workflow_materialization(input.materialization)?;
    let analysis = analyze_dynamic_workflow_plan(
        &phases,
        &agent_fleet_policy,
        &governance,
        &validation,
        &materialization,
    )?;
    let now = Utc::now();
    let plan = state
        .create_dynamic_workflow_plan(DynamicWorkflowPlan {
            id: Uuid::new_v4(),
            source_work_item_id: input.source_work_item_id,
            source_session_id: input.source_session_id,
            objective,
            status: "proposed".to_string(),
            phases,
            agent_fleet_policy,
            governance,
            validation,
            materialization,
            analysis,
            review: empty_json_object(),
            workflow_definition_id: None,
            workflow_run_id: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
            reviewed_at: None,
            materialized_at: None,
        })
        .await?;
    let audit = record_dynamic_workflow_plan_audit(
        &state,
        &plan,
        "dynamic_workflow_plan.created",
        json!({"status": plan.status, "analysis": plan.analysis}),
    )
    .await?;
    let plan = state
        .update_dynamic_workflow_plan_audit_trace(plan.id, Some(audit.id), audit.created_at)
        .await?;
    Ok(Json(plan))
}

async fn compile_dynamic_workflow_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CompileDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlanCompilationResponse>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "dynamic_workflow_plans",
        None,
    )
    .await?;
    if let Some(work_item_id) = input.source_work_item_id {
        authorize_request(
            &state,
            &headers,
            Permission::SessionsRead,
            "work_item",
            Some(work_item_id),
        )
        .await?;
    }
    if let Some(session_id) = input.source_session_id {
        authorize_request(
            &state,
            &headers,
            Permission::SessionsRead,
            "session",
            Some(session_id),
        )
        .await?;
        state.get_session(session_id).await?;
    }
    let objective = require_non_empty(input.objective, "dynamic workflow objective")?;
    let max_total_agents = input.max_total_agents.unwrap_or(8).clamp(1, 1000);
    let max_parallel_agents = input
        .max_parallel_agents
        .unwrap_or(4)
        .clamp(1, 16)
        .min(max_total_agents);
    let runtime_adapter = input
        .runtime_adapter
        .and_then(normalize_optional_text)
        .unwrap_or_else(|| "codex_app_server".to_string());
    let runtime_adapter = normalize_optional_runtime_adapter(Some(runtime_adapter))?
        .unwrap_or_else(|| "codex_app_server".to_string());
    let execution_strategy = input
        .execution_strategy
        .and_then(normalize_optional_text)
        .unwrap_or_else(|| "native_dynamic".to_string());
    let execution_strategy = normalize_workflow_execution_strategy(&execution_strategy)?;
    let phases = if execution_strategy == "native_dynamic" {
        compile_native_dynamic_workflow_phases(&objective, max_total_agents, max_parallel_agents)
    } else {
        compile_dynamic_workflow_phases(&objective, max_total_agents, max_parallel_agents)
    };
    let request = CreateDynamicWorkflowPlan {
        source_work_item_id: input.source_work_item_id,
        source_session_id: input.source_session_id,
        objective: objective.clone(),
        phases: phases.clone(),
        agent_fleet_policy: json!({
            "max_total_agents": max_total_agents,
            "max_parallel_agents": max_parallel_agents,
            "timeout_seconds": 3600,
            "retry_limit": 1
        }),
        governance: json!({
            "risk_level": "medium",
            "memory_scope": {"read": ["session"], "write": ["session"]},
            "tool_scope": {"allowed_tools": ["file.read", "artifact.create"]},
            "connector_scope": {"allowed_connectors": []},
            "approval_policy": {"required_for": ["external_commit"]},
            "external_effects": {"mode": "draft_only"}
        }),
        validation: json!({
            "cross_check_required": true,
            "vote_threshold": 0.67,
            "required_artifacts": ["final_report"],
            "adjudication": "majority_vote_with_synthesis"
        }),
        materialization: json!({
            "execution_strategy": execution_strategy,
            "runtime_adapter": runtime_adapter,
            "runtime_mode": "dynamic_workflow",
            "runtime_capability_contract": {
                "max_total_agents": max_total_agents,
                "max_parallel_agents": max_parallel_agents,
                "allowed_tools": ["file.read", "artifact.create"]
            },
            "event_ingestion_policy": "normalized"
        }),
    };
    let analysis = analyze_dynamic_workflow_plan(
        &request.phases,
        &request.agent_fleet_policy,
        &request.governance,
        &request.validation,
        &request.materialization,
    )?;
    Ok(Json(DynamicWorkflowPlanCompilationResponse {
        request,
        compiler: json!({
            "name": "mandoforge-rule-compiler-v1",
            "status": "compiled",
            "analysis": analysis,
            "notes": [
                "deterministic compiler generated reviewable dynamic workflow phases from objective",
                "materialization still requires plan creation and approval"
            ]
        }),
    }))
}

async fn review_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlan>, AppError> {
    let current = state.get_dynamic_workflow_plan(id).await?;
    authorize_dynamic_workflow_plan_run(&state, &headers, &current).await?;
    if !input.review.is_object() {
        return Err(AppError::bad_request(
            "dynamic workflow plan review must be a JSON object",
        ));
    }
    let status = match input.status {
        Some(status) => normalize_dynamic_workflow_plan_status(&status)?,
        None => "reviewed".to_string(),
    };
    if current.status == "materialized" {
        return Err(AppError::bad_request(
            "materialized dynamic workflow plan cannot be reviewed",
        ));
    }
    let reviewed_at = Utc::now();
    let reviewed = state
        .update_dynamic_workflow_plan_review(
            current.id,
            status,
            input.review,
            current.audit_trace_id,
            reviewed_at,
        )
        .await?;
    let audit = record_dynamic_workflow_plan_audit(
        &state,
        &reviewed,
        "dynamic_workflow_plan.reviewed",
        json!({"status": reviewed.status, "review": reviewed.review}),
    )
    .await?;
    let reviewed = state
        .update_dynamic_workflow_plan_review(
            reviewed.id,
            reviewed.status.clone(),
            reviewed.review.clone(),
            Some(audit.id),
            reviewed_at,
        )
        .await?;
    Ok(Json(reviewed))
}

async fn adjudicate_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DynamicWorkflowAdjudicationRequest>,
) -> Result<Json<DynamicWorkflowAdjudicationResponse>, AppError> {
    let plan = state.get_dynamic_workflow_plan(id).await?;
    authorize_dynamic_workflow_plan_run(&state, &headers, &plan).await?;
    let workflow_run_id = plan.workflow_run_id.ok_or_else(|| {
        AppError::bad_request("dynamic workflow plan must be materialized before adjudication")
    })?;
    let run = state.get_workflow_run(workflow_run_id).await?;
    let threshold = input
        .vote_threshold
        .or_else(|| {
            plan.validation
                .get("vote_threshold")
                .and_then(Value::as_f64)
        })
        .unwrap_or(0.67);
    if !(0.0..=1.0).contains(&threshold) {
        return Err(AppError::bad_request(
            "dynamic workflow adjudication threshold must be between 0 and 1",
        ));
    }
    let steps = state.list_workflow_step_runs(run.id).await?;
    let completed = steps
        .iter()
        .filter(|step| step.status == "completed")
        .collect::<Vec<_>>();
    let explicit_votes = completed
        .iter()
        .filter_map(|step| dynamic_workflow_step_vote(&step.output_payload))
        .collect::<Vec<_>>();
    let positive_votes = explicit_votes.iter().filter(|vote| **vote).count();
    let negative_votes = explicit_votes.len().saturating_sub(positive_votes);
    let missing_votes = completed.len().saturating_sub(explicit_votes.len());
    let score = if explicit_votes.is_empty() {
        0.0
    } else {
        positive_votes as f64 / explicit_votes.len() as f64
    };
    let decision = if explicit_votes.is_empty() || missing_votes > 0 {
        "insufficient_evidence"
    } else if score >= threshold {
        "accepted"
    } else {
        "needs_review"
    }
    .to_string();
    let response = DynamicWorkflowAdjudicationResponse {
        status: "completed".to_string(),
        plan_id: plan.id,
        workflow_run_id: run.id,
        threshold,
        completed_votes: completed.len(),
        positive_votes,
        negative_votes,
        score,
        decision,
        evidence: json!({
            "step_count": steps.len(),
            "completed_step_ids": completed.iter().map(|step| step.id).collect::<Vec<_>>(),
            "explicit_vote_count": explicit_votes.len(),
            "missing_vote_count": missing_votes,
            "validation": plan.validation,
            "cross_check_required": plan.validation.get("cross_check_required").and_then(Value::as_bool).unwrap_or(false)
        }),
    };
    state
        .append_event(
            "system",
            Some(run.id),
            run.primary_session_id,
            "dynamic_workflow.adjudicated",
            serde_json::to_value(&response)?,
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(run.id),
            "dynamic_workflow.adjudicated",
            "dynamic_workflow_plan",
            Some(plan.id),
            serde_json::to_value(&response)?,
        ))
        .await?;
    Ok(Json(response))
}

async fn pressure_test_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DynamicWorkflowPressureTestRequest>,
) -> Result<Json<DynamicWorkflowPressureTestResponse>, AppError> {
    let plan = state.get_dynamic_workflow_plan(id).await?;
    authorize_dynamic_workflow_plan_run(&state, &headers, &plan).await?;
    let max_total = dynamic_policy_u64(&plan.agent_fleet_policy, "max_total_agents", 64)?;
    let policy_parallel =
        dynamic_policy_u64(&plan.agent_fleet_policy, "max_parallel_agents", 16)?.clamp(1, 16);
    let target_agents = input.target_agents.unwrap_or(max_total).clamp(1, 1000);
    if target_agents > max_total {
        return Err(AppError::bad_request(format!(
            "dynamic workflow pressure target {target_agents} exceeds plan max_total_agents {max_total}"
        )));
    }
    let max_parallel_agents = input
        .max_parallel_agents
        .unwrap_or(policy_parallel)
        .clamp(1, 16)
        .min(target_agents);
    let simulated_batches = target_agents.div_ceil(max_parallel_agents);
    let response = DynamicWorkflowPressureTestResponse {
        status: "control_plane_passed".to_string(),
        plan_id: plan.id,
        target_agents,
        max_parallel_agents,
        simulated_batches,
        estimated_worker_claims: target_agents,
        evidence: json!({
            "type": "control_plane_pressure_simulation",
            "policy_max_total_agents": max_total,
            "policy_max_parallel_agents": policy_parallel,
            "claim_backpressure": "max_parallel_agents",
            "note": "This proves planning/backpressure math, not live LLM execution cost or provider capacity."
        }),
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            Some(plan.id),
            "dynamic_workflow.pressure_tested",
            "dynamic_workflow_plan",
            Some(plan.id),
            serde_json::to_value(&response)?,
        ))
        .await?;
    Ok(Json(response))
}

async fn materialize_dynamic_workflow_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<MaterializeDynamicWorkflowPlan>,
) -> Result<Json<DynamicWorkflowPlanMaterializationResponse>, AppError> {
    let plan = state.get_dynamic_workflow_plan(id).await?;
    authorize_dynamic_workflow_plan_run(&state, &headers, &plan).await?;
    if plan.status != "approved" {
        return Err(AppError::bad_request(
            "dynamic workflow plan must be approved before materialization",
        ));
    }
    let agent = workflow_pack_materialization_default_agent(&state).await?;
    let execution_strategy = dynamic_workflow_plan_execution_strategy(&plan)?;
    let runtime_adapter = dynamic_workflow_plan_runtime_adapter(&plan)?;
    let runtime_mode = dynamic_workflow_plan_runtime_mode(&plan)?;
    let runtime_capability_contract = dynamic_workflow_plan_runtime_capability_contract(&plan)?;
    let event_ingestion_policy = dynamic_workflow_plan_event_ingestion_policy(&plan)?;
    validate_workflow_execution_binding(
        &execution_strategy,
        runtime_adapter.as_deref(),
        &runtime_capability_contract,
    )?;
    let step_graph = dynamic_workflow_plan_step_graph(&plan, &execution_strategy)?;
    validate_workflow_graph_definition(&step_graph)?;
    let now = Utc::now();
    let workflow_definition = state
        .create_workflow_definition(WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: None,
            pack_id: None,
            pack_version: None,
            name: format!("Dynamic plan: {}", plan.objective),
            entrypoint: format!("dynamic-{}", workflow_slug(&plan.objective)),
            trigger_type: "manual".to_string(),
            default_agent_id: agent.id,
            default_environment_id: input.environment_id,
            input_schema_ref: None,
            output_schema_ref: None,
            step_graph,
            handoff_rules: dynamic_workflow_plan_handoff_rules(&plan),
            execution_strategy: execution_strategy.clone(),
            runtime_adapter: runtime_adapter.clone(),
            runtime_mode: runtime_mode.clone(),
            runtime_capability_contract: runtime_capability_contract.clone(),
            event_ingestion_policy: event_ingestion_policy.clone(),
            approval_policy_ref: None,
            eval_gate_refs: Vec::new(),
            release_state: "released".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .await?;
    let session = state
        .create_session(CreateSession {
            agent_id: workflow_definition.default_agent_id,
            environment_id: input.environment_id,
            title: input
                .title
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| format!("Dynamic workflow: {}", plan.objective)),
            message: Some(plan.objective.clone()),
        })
        .await?;
    ensure_primary_session_thread(&state, session.id).await?;
    let input_payload = if input.input_payload.is_object()
        && !input
            .input_payload
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
    {
        input.input_payload
    } else {
        json!({
            "objective": plan.objective,
            "dynamic_workflow_plan_id": plan.id,
            "phases": plan.phases
        })
    };
    let input_digest = workflow_input_digest(&input_payload);
    let runtime_envelope = workflow_run_runtime_envelope(
        &workflow_definition,
        &execution_strategy,
        runtime_adapter.as_deref(),
        runtime_mode.as_deref(),
        None,
        &json!({
            "dynamic_workflow_plan_id": plan.id,
            "objective": plan.objective,
            "phases": plan.phases,
            "agent_fleet_policy": plan.agent_fleet_policy,
            "validation": plan.validation,
            "analysis": plan.analysis
        }),
    );
    let workflow_run = state
        .create_workflow_run(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_definition_id: workflow_definition.id,
            pack_installation_id: None,
            source_event_id: None,
            source_work_item_id: plan.source_work_item_id,
            source_schedule_id: None,
            status: "queued".to_string(),
            primary_session_id: session.id,
            root_task_grant_id: None,
            input_payload,
            input_digest,
            execution_strategy,
            runtime_adapter,
            runtime_mode,
            delegation_status: (workflow_definition.execution_strategy == "delegated_runtime")
                .then_some("submitted".to_string()),
            external_run_ref: None,
            runtime_event_cursor: None,
            runtime_envelope,
            started_at: None,
            completed_at: None,
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let root_grant = issue_root_task_grant_for_workflow_run(
        &state,
        &workflow_run,
        &workflow_definition,
        &session,
    )
    .await?;
    let workflow_run = state
        .update_workflow_run_root_task_grant(workflow_run.id, root_grant.id)
        .await?;
    materialize_workflow_graph_start_steps(
        &state,
        &workflow_definition,
        &workflow_run,
        &session,
        &root_grant,
    )
    .await?;
    state
        .append_event(
            "system",
            Some(workflow_run.id),
            session.id,
            "dynamic_workflow_plan.materialized",
            json!({
                "dynamic_workflow_plan_id": plan.id,
                "workflow_definition_id": workflow_definition.id,
                "workflow_run_id": workflow_run.id,
                "execution_strategy": workflow_run.execution_strategy,
                "runtime_adapter": workflow_run.runtime_adapter,
                "runtime_mode": workflow_run.runtime_mode,
                "root_task_grant_id": workflow_run.root_task_grant_id
            }),
        )
        .await?;
    let audit = record_dynamic_workflow_plan_audit(
        &state,
        &plan,
        "dynamic_workflow_plan.materialized",
        json!({
            "workflow_definition_id": workflow_definition.id,
            "workflow_run_id": workflow_run.id,
            "root_task_grant_id": workflow_run.root_task_grant_id,
            "execution_strategy": workflow_run.execution_strategy,
            "runtime_adapter": workflow_run.runtime_adapter,
            "runtime_mode": workflow_run.runtime_mode
        }),
    )
    .await?;
    let plan = state
        .update_dynamic_workflow_plan_materialized(
            plan.id,
            workflow_definition.id,
            workflow_run.id,
            Some(audit.id),
            now,
        )
        .await?;
    Ok(Json(DynamicWorkflowPlanMaterializationResponse {
        plan,
        workflow_definition,
        workflow_run,
    }))
}
