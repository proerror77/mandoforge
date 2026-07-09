use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    AppError, AppState, CreateAgentHandoffAssignment, CreateAgentHandoffEvent,
    CreateManagerAgentPlan, CreateWorkflowRunFromDefinition, ManagerAgentPlan,
    MaterializeManagerAgentPlanHandoff, MaterializeManagerAgentPlanWorkflowRun,
    MaterializedManagerAgentPlanHandoff, MaterializedManagerAgentPlanWorkflowRun, Permission,
    ReviewManagerAgentPlan, WorkflowRun, assign_agent_handoff_event_for_runtime,
    authorize_collection_request, authorize_request, create_agent_handoff_event_for_session,
    create_workflow_run_from_definition_with_context, new_audit_log, normalize_handoff_risk_level,
    normalize_manager_plan_risk, normalize_manager_plan_status,
    record_agent_handoff_audit_and_event, record_manager_agent_plan_audit_and_event,
    record_manager_agent_plan_work_item_activity, visible_session_ids_for_principal,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/sessions/{id}/manager-plans",
            get(list_session_manager_agent_plans).post(create_manager_agent_plan),
        )
        .route(
            "/api/work-items/{id}/manager-plans",
            get(list_work_item_manager_agent_plans),
        )
        .route("/api/manager-plans", get(list_manager_agent_plans))
        .route("/api/manager-plans/{id}", get(get_manager_agent_plan))
        .route(
            "/api/manager-plans/{id}/review",
            post(review_manager_agent_plan),
        )
        .route(
            "/api/manager-plans/{id}/materialize-handoff",
            post(materialize_manager_agent_plan_handoff),
        )
        .route(
            "/api/manager-plans/{id}/materialize-workflow-run",
            post(materialize_manager_agent_plan_workflow_run),
        )
}

async fn list_manager_agent_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    let principal = authorize_collection_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "manager_agent_plans",
    )
    .await?;
    let visible_session_ids = visible_session_ids_for_principal(&state, &principal).await?;
    Ok(Json(
        state
            .list_manager_agent_plans(None)
            .await?
            .into_iter()
            .filter(|plan| visible_session_ids.contains(&plan.session_id))
            .collect(),
    ))
}

async fn list_session_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_manager_agent_plans(Some(id)).await?))
}

async fn list_work_item_manager_agent_plans(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagerAgentPlan>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_manager_agent_plans(id).await?))
}

async fn get_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    let plan = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(plan.session_id),
    )
    .await?;
    Ok(Json(plan))
}

async fn create_manager_agent_plan(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await?;
    let session = state.get_session(session_id).await?;
    let manager_agent = state.get_agent(session.agent_id).await?;
    if manager_agent.agent_role != "manager" {
        return Err(AppError::bad_request(
            "manager agent plans require a session bound to a manager agent",
        ));
    }
    if !input.task_intake.is_object()
        || !input.decomposition.is_object()
        || !input.specialist_selection.is_object()
        || !input.review.is_object()
    {
        return Err(AppError::bad_request(
            "manager agent plan sections must be JSON objects",
        ));
    }
    let risk_classification = normalize_manager_plan_risk(&input.risk_classification)?;
    if let Some(specialist_agent_id) = input.specialist_agent_id {
        let specialist = state.get_agent(specialist_agent_id).await?;
        if specialist.agent_role != "specialist" {
            return Err(AppError::bad_request(
                "specialist_agent_id must reference a specialist agent",
            ));
        }
    }
    if let Some(work_item_id) = input.work_item_id {
        state.ensure_work_item_exists(work_item_id).await?;
    }
    let now = Utc::now();
    let plan = state
        .create_manager_agent_plan(ManagerAgentPlan {
            id: Uuid::new_v4(),
            session_id,
            manager_agent_id: manager_agent.id,
            work_item_id: input.work_item_id,
            specialist_agent_id: input.specialist_agent_id,
            task_intake: input.task_intake,
            decomposition: input.decomposition,
            specialist_selection: input.specialist_selection,
            risk_classification,
            review: input.review,
            status: "planned".to_string(),
            audit_trace_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let audit =
        record_manager_agent_plan_audit_and_event(&state, &plan, "manager_plan.created").await?;
    record_manager_agent_plan_work_item_activity(&state, &plan, "manager_plan.created").await?;
    let plan = state
        .update_manager_agent_plan_review(
            plan.id,
            plan.review.clone(),
            plan.status.clone(),
            Some(audit.id),
        )
        .await?;
    Ok(Json(plan))
}

async fn review_manager_agent_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReviewManagerAgentPlan>,
) -> Result<Json<ManagerAgentPlan>, AppError> {
    let current = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(current.session_id),
    )
    .await?;
    if !input.review.is_object() {
        return Err(AppError::bad_request(
            "manager agent plan review must be a JSON object",
        ));
    }
    let status = match input.status {
        Some(status) => normalize_manager_plan_status(&status)?,
        None => "reviewed".to_string(),
    };
    let reviewed = state
        .update_manager_agent_plan_review(current.id, input.review, status, current.audit_trace_id)
        .await?;
    let audit =
        record_manager_agent_plan_audit_and_event(&state, &reviewed, "manager_plan.reviewed")
            .await?;
    record_manager_agent_plan_work_item_activity(&state, &reviewed, "manager_plan.reviewed")
        .await?;
    let reviewed = state
        .update_manager_agent_plan_review(
            reviewed.id,
            reviewed.review.clone(),
            reviewed.status.clone(),
            Some(audit.id),
        )
        .await?;
    Ok(Json(reviewed))
}

async fn materialize_manager_agent_plan_handoff(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<MaterializeManagerAgentPlanHandoff>,
) -> Result<Json<MaterializedManagerAgentPlanHandoff>, AppError> {
    let plan = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(plan.session_id),
    )
    .await?;
    if plan.status != "reviewed" && plan.status != "approved" {
        return Err(AppError::bad_request(
            "manager plan must be reviewed or approved before materialization",
        ));
    }
    let target_agent_id = input
        .target_agent_id
        .or(plan.specialist_agent_id)
        .ok_or_else(|| {
            AppError::bad_request(
                "manager plan materialization requires target_agent_id or specialist_agent_id",
            )
        })?;
    if let Some(specialist_agent_id) = plan.specialist_agent_id
        && specialist_agent_id != target_agent_id
    {
        return Err(AppError::bad_request(
            "target_agent_id must match the manager plan specialist_agent_id",
        ));
    }
    let risk_level = normalize_handoff_risk_level(&input.risk_level)?;
    if risk_level == "high" || input.approval_required {
        return Err(AppError::bad_request(
            "manager plan handoff materialization cannot auto-accept high-risk or approval-required handoffs",
        ));
    }
    let handoff = create_agent_handoff_event_for_session(
        &state,
        plan.session_id,
        CreateAgentHandoffEvent {
            target_agent_id,
            manager_plan_id: Some(plan.id),
            intent: input.intent,
            payload: input.payload,
            schema_version: input.schema_version,
            risk_level,
            approval_required: input.approval_required,
            semantic_scopes: input.semantic_scopes,
            runtime_profile_id: input.runtime_profile_id,
            remote_computer_required: input.remote_computer_required,
            review_status: None,
            human_escalation_status: None,
        },
    )
    .await?;
    let audit = record_agent_handoff_audit_and_event(
        &state,
        &handoff,
        "agent_handoff.accepted",
        Some("manager plan materialized handoff".to_string()),
    )
    .await?;
    let handoff = state
        .update_agent_handoff_event_status(handoff.id, "accepted", Some(audit.id))
        .await?;
    let assignment = assign_agent_handoff_event_for_runtime(
        &state,
        &handoff,
        CreateAgentHandoffAssignment {
            specialist_session_id: input.specialist_session_id,
            title: input.title,
            message: input.message,
            remote_computer_job_assignment_id: None,
            assigned_by: input.assigned_by,
            metadata: input.metadata,
        },
    )
    .await?;
    Ok(Json(MaterializedManagerAgentPlanHandoff {
        manager_plan: plan,
        handoff,
        assignment,
    }))
}

async fn materialize_manager_agent_plan_workflow_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<MaterializeManagerAgentPlanWorkflowRun>,
) -> Result<Json<MaterializedManagerAgentPlanWorkflowRun>, AppError> {
    let plan = state.get_manager_agent_plan(id).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRun,
        "session",
        Some(plan.session_id),
    )
    .await?;
    if plan.status != "reviewed" && plan.status != "approved" {
        return Err(AppError::bad_request(
            "manager plan must be reviewed or approved before materialization",
        ));
    }
    if plan.risk_classification == "high" {
        return Err(AppError::bad_request(
            "manager plan workflow materialization cannot auto-start high-risk plans",
        ));
    }
    let definition = state
        .get_workflow_definition(input.workflow_definition_id)
        .await?;
    if let Some(run) = find_existing_manager_plan_workflow_run(&state, definition.id, &plan).await?
    {
        authorize_request(
            &state,
            &headers,
            Permission::SessionsRead,
            "session",
            Some(run.primary_session_id),
        )
        .await?;
        return Ok(Json(MaterializedManagerAgentPlanWorkflowRun {
            manager_plan: plan,
            workflow_definition: definition,
            workflow_run: run,
        }));
    }
    let title = input
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| format!("Manager plan workflow: {}", definition.name));
    let input_payload = manager_plan_workflow_input_payload(&plan, input.input_payload);
    let runtime_envelope_request =
        manager_plan_workflow_runtime_envelope_request(&plan, input.runtime_envelope);
    let run = create_workflow_run_from_definition_with_context(
        &state,
        &definition,
        CreateWorkflowRunFromDefinition {
            title,
            input_payload,
            runtime_envelope_request,
            source_work_item_id: plan.work_item_id,
            environment_id: input.environment_id,
            session_message: Some(format!("ManagerPlan {}", plan.id)),
        },
    )
    .await?;
    state
        .append_event(
            "system",
            Some(run.id),
            run.primary_session_id,
            "manager_plan.workflow_run_materialized",
            json!({
                "manager_plan_id": plan.id,
                "source_manager_session_id": plan.session_id,
                "workflow_definition_id": definition.id,
                "workflow_run_id": run.id,
                "root_task_grant_id": run.root_task_grant_id,
                "source_work_item_id": run.source_work_item_id,
                "risk_classification": plan.risk_classification,
                "execution_strategy": run.execution_strategy,
                "runtime_adapter": run.runtime_adapter,
                "runtime_mode": run.runtime_mode
            }),
        )
        .await?;
    let audit = record_manager_agent_plan_audit_and_event(
        &state,
        &plan,
        "manager_plan.workflow_run_materialized",
    )
    .await?;
    state
        .append_audit_log(new_audit_log(
            Some(run.primary_session_id),
            "system",
            Some(run.id),
            "workflow_run.created",
            "workflow_run",
            Some(run.id),
            json!({
                "manager_plan_id": plan.id,
                "source_manager_session_id": plan.session_id,
                "workflow_definition_id": definition.id,
                "primary_session_id": run.primary_session_id,
                "root_task_grant_id": run.root_task_grant_id,
                "source_work_item_id": run.source_work_item_id,
                "input_digest": run.input_digest,
                "execution_strategy": run.execution_strategy,
                "runtime_adapter": run.runtime_adapter,
                "runtime_mode": run.runtime_mode
            }),
        ))
        .await?;
    record_manager_agent_plan_work_item_activity(
        &state,
        &ManagerAgentPlan {
            audit_trace_id: Some(audit.id),
            ..plan.clone()
        },
        "manager_plan.workflow_run_materialized",
    )
    .await?;
    Ok(Json(MaterializedManagerAgentPlanWorkflowRun {
        manager_plan: plan,
        workflow_definition: definition,
        workflow_run: run,
    }))
}

async fn find_existing_manager_plan_workflow_run(
    state: &AppState,
    workflow_definition_id: Uuid,
    plan: &ManagerAgentPlan,
) -> Result<Option<WorkflowRun>, AppError> {
    for run in state.list_workflow_runs().await? {
        if workflow_run_matches_manager_plan_materialization(&run, workflow_definition_id, plan)
            && workflow_run_has_manager_plan_materialization_event(state, &run, plan).await?
        {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

fn workflow_run_matches_manager_plan_materialization(
    run: &WorkflowRun,
    workflow_definition_id: Uuid,
    plan: &ManagerAgentPlan,
) -> bool {
    run.workflow_definition_id == workflow_definition_id
        && run.source_work_item_id == plan.work_item_id
        && run.input_payload.get("manager_plan_id") == Some(&json!(plan.id))
        && run
            .runtime_envelope
            .get("request_envelope")
            .and_then(|envelope| envelope.get("manager_plan_id"))
            == Some(&json!(plan.id))
}

async fn workflow_run_has_manager_plan_materialization_event(
    state: &AppState,
    run: &WorkflowRun,
    plan: &ManagerAgentPlan,
) -> Result<bool, AppError> {
    Ok(state
        .list_events(run.primary_session_id)
        .await?
        .into_iter()
        .any(|event| {
            event.actor_type == "system"
                && event.actor_id == Some(run.id)
                && event.event_type == "manager_plan.workflow_run_materialized"
                && event.payload.get("manager_plan_id") == Some(&json!(plan.id))
                && event.payload.get("workflow_definition_id")
                    == Some(&json!(run.workflow_definition_id))
                && event.payload.get("workflow_run_id") == Some(&json!(run.id))
        }))
}

fn manager_plan_workflow_input_payload(plan: &ManagerAgentPlan, input: Value) -> Value {
    let mut payload = match input {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    payload.insert("manager_plan_id".to_string(), json!(plan.id));
    payload.insert(
        "source_manager_session_id".to_string(),
        json!(plan.session_id),
    );
    payload.insert("manager_agent_id".to_string(), json!(plan.manager_agent_id));
    payload.insert(
        "risk_classification".to_string(),
        json!(plan.risk_classification),
    );
    payload.insert("task_intake".to_string(), plan.task_intake.clone());
    payload.insert("decomposition".to_string(), plan.decomposition.clone());
    payload.insert(
        "specialist_selection".to_string(),
        plan.specialist_selection.clone(),
    );
    if let Some(work_item_id) = plan.work_item_id {
        payload.insert("work_item_id".to_string(), json!(work_item_id));
    }
    if let Some(specialist_agent_id) = plan.specialist_agent_id {
        payload.insert(
            "specialist_agent_id".to_string(),
            json!(specialist_agent_id),
        );
    }
    Value::Object(payload)
}

fn manager_plan_workflow_runtime_envelope_request(
    plan: &ManagerAgentPlan,
    runtime_envelope: Value,
) -> Value {
    let mut envelope = match runtime_envelope {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    envelope.insert("manager_plan_id".to_string(), json!(plan.id));
    envelope.insert(
        "source_manager_session_id".to_string(),
        json!(plan.session_id),
    );
    envelope.insert("manager_agent_id".to_string(), json!(plan.manager_agent_id));
    envelope.insert(
        "risk_classification".to_string(),
        json!(plan.risk_classification),
    );
    Value::Object(envelope)
}
