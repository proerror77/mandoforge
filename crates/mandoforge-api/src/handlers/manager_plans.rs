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
    AppError, AppState, Approval, CreateAgentHandoffAssignment, CreateAgentHandoffEvent,
    CreateManagerAgentPlan, CreateWorkflowRunFromDefinition, ManagerAgentPlan,
    MaterializeManagerAgentPlanHandoff, MaterializeManagerAgentPlanWorkflowRun,
    MaterializedManagerAgentPlanHandoff, MaterializedManagerAgentPlanWorkflowRun, Permission,
    ReviewManagerAgentPlan, SessionStatus, WorkflowDefinition, WorkflowRun, approval_is_expired,
    assign_agent_handoff_event_for_runtime, authorize_collection_request, authorize_request,
    create_agent_handoff_event_for_session, create_workflow_run_from_definition_with_context,
    new_audit_log, normalize_handoff_risk_level, normalize_manager_plan_risk,
    normalize_manager_plan_status, record_agent_handoff_audit_and_event,
    record_manager_agent_plan_audit_and_event, record_manager_agent_plan_work_item_activity,
    set_managed_session_status, visible_session_ids_for_principal,
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
    let selection = resolve_manager_plan_workflow_definition(&state, &plan, &input).await?;
    let definition = selection.definition.clone();
    let materialization_approval_id = manager_plan_workflow_materialization_approval_id(
        &state,
        &plan,
        &definition,
        input.approval_id,
        &selection.evidence,
    )
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
    let input_payload = manager_plan_workflow_input_payload(
        &plan,
        input.input_payload,
        materialization_approval_id,
        &selection.evidence,
    );
    let runtime_envelope_request = manager_plan_workflow_runtime_envelope_request(
        &plan,
        input.runtime_envelope,
        materialization_approval_id,
        &selection.evidence,
    );
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
                "approval_id": materialization_approval_id,
                "workflow_selection": selection.evidence.clone(),
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
                "approval_id": materialization_approval_id,
                "workflow_selection": selection.evidence.clone(),
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

#[derive(Debug, Clone)]
struct ManagerPlanWorkflowDefinitionSelection {
    definition: WorkflowDefinition,
    evidence: Value,
}

async fn resolve_manager_plan_workflow_definition(
    state: &AppState,
    plan: &ManagerAgentPlan,
    input: &MaterializeManagerAgentPlanWorkflowRun,
) -> Result<ManagerPlanWorkflowDefinitionSelection, AppError> {
    if let Some(id) = input.workflow_definition_id {
        let definition = get_released_manager_plan_workflow_definition(state, id).await?;
        return Ok(ManagerPlanWorkflowDefinitionSelection {
            evidence: json!({
                "mode": "workflow_definition_id",
                "source": "request",
                "workflow_definition_id": definition.id,
                "workflow_name": definition.name,
                "workflow_entrypoint": definition.entrypoint,
                "workflow_pack_id": definition.pack_id,
                "workflow_pack_installation_id": definition.pack_installation_id
            }),
            definition,
        });
    }

    if let Some(selector) = manager_plan_workflow_selector_from_request(input) {
        return resolve_manager_plan_workflow_definition_by_selector(state, selector).await;
    }

    if let Some(selector) = manager_plan_workflow_selector_from_specialist_selection(plan) {
        return resolve_manager_plan_workflow_definition_by_selector(state, selector).await;
    }

    Err(AppError::bad_request(
        "workflow_definition_id, workflow_entrypoint, or workflow_name is required for manager plan workflow materialization",
    ))
}

async fn get_released_manager_plan_workflow_definition(
    state: &AppState,
    workflow_definition_id: Uuid,
) -> Result<WorkflowDefinition, AppError> {
    let definition = state
        .get_workflow_definition(workflow_definition_id)
        .await?;
    if definition.release_state != "released" {
        return Err(AppError::not_found(
            "released workflow definition matching manager plan selection not found",
        ));
    }
    Ok(definition)
}

async fn resolve_manager_plan_workflow_definition_by_selector(
    state: &AppState,
    selector: ManagerPlanWorkflowSelector,
) -> Result<ManagerPlanWorkflowDefinitionSelection, AppError> {
    let definitions = state.list_workflow_definitions().await?;
    let mut candidates = definitions
        .into_iter()
        .filter(|definition| definition.release_state == "released")
        .filter(|definition| selector.matches(definition))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|definition| definition.created_at);
    candidates.reverse();
    if candidates.is_empty() {
        return Err(AppError::not_found(
            "released workflow definition matching manager plan selection not found",
        ));
    }
    if candidates.len() > 1 {
        return Err(AppError::bad_request(
            "manager plan workflow selection matched multiple released workflow definitions",
        ));
    }

    let definition = candidates.remove(0);
    Ok(ManagerPlanWorkflowDefinitionSelection {
        evidence: json!({
            "mode": selector.mode,
            "source": selector.source,
            "workflow_definition_id": definition.id,
            "workflow_name": definition.name,
            "workflow_entrypoint": definition.entrypoint,
            "workflow_pack_id": definition.pack_id,
            "workflow_pack_installation_id": definition.pack_installation_id
        }),
        definition,
    })
}

#[derive(Debug, Clone)]
struct ManagerPlanWorkflowSelector {
    source: &'static str,
    mode: &'static str,
    workflow_entrypoint: Option<String>,
    workflow_name: Option<String>,
    workflow_pack_id: Option<String>,
    workflow_pack_installation_id: Option<Uuid>,
}

impl ManagerPlanWorkflowSelector {
    fn matches(&self, definition: &WorkflowDefinition) -> bool {
        self.workflow_entrypoint
            .as_ref()
            .is_none_or(|entrypoint| definition.entrypoint == *entrypoint)
            && self
                .workflow_name
                .as_ref()
                .is_none_or(|name| definition.name == *name)
            && self
                .workflow_pack_id
                .as_ref()
                .is_none_or(|pack_id| definition.pack_id.as_ref() == Some(pack_id))
            && self
                .workflow_pack_installation_id
                .is_none_or(|installation_id| {
                    definition.pack_installation_id == Some(installation_id)
                })
    }
}

fn manager_plan_workflow_selector_from_request(
    input: &MaterializeManagerAgentPlanWorkflowRun,
) -> Option<ManagerPlanWorkflowSelector> {
    let workflow_entrypoint =
        normalize_manager_plan_workflow_selector_text(input.workflow_entrypoint.as_deref());
    let workflow_name =
        normalize_manager_plan_workflow_selector_text(input.workflow_name.as_deref());
    if workflow_entrypoint.is_none() && workflow_name.is_none() {
        return None;
    }
    Some(ManagerPlanWorkflowSelector {
        source: "request",
        mode: if workflow_entrypoint.is_some() {
            "workflow_entrypoint"
        } else {
            "workflow_name"
        },
        workflow_entrypoint,
        workflow_name,
        workflow_pack_id: normalize_manager_plan_workflow_selector_text(
            input.workflow_pack_id.as_deref(),
        ),
        workflow_pack_installation_id: input.workflow_pack_installation_id,
    })
}

fn manager_plan_workflow_selector_from_specialist_selection(
    plan: &ManagerAgentPlan,
) -> Option<ManagerPlanWorkflowSelector> {
    let selection = plan.specialist_selection.as_object()?;
    let workflow_entrypoint = manager_plan_selector_string(
        selection,
        &["workflow_entrypoint", "entrypoint", "workflow_key"],
    );
    let workflow_name = manager_plan_selector_string(selection, &["workflow_name", "name"]);
    if workflow_entrypoint.is_none() && workflow_name.is_none() {
        return None;
    }
    Some(ManagerPlanWorkflowSelector {
        source: "manager_plan.specialist_selection",
        mode: if workflow_entrypoint.is_some() {
            "workflow_entrypoint"
        } else {
            "workflow_name"
        },
        workflow_entrypoint,
        workflow_name,
        workflow_pack_id: manager_plan_selector_string(selection, &["workflow_pack_id", "pack_id"]),
        workflow_pack_installation_id: selection
            .get("workflow_pack_installation_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok()),
    })
}

fn manager_plan_selector_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        normalize_manager_plan_workflow_selector_text(map.get(*key).and_then(Value::as_str))
    })
}

fn normalize_manager_plan_workflow_selector_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

async fn manager_plan_workflow_materialization_approval_id(
    state: &AppState,
    plan: &ManagerAgentPlan,
    definition: &crate::WorkflowDefinition,
    approval_id: Option<Uuid>,
    workflow_selection: &Value,
) -> Result<Option<Uuid>, AppError> {
    if plan.risk_classification != "high" {
        return Ok(None);
    }
    let Some(approval_id) = approval_id else {
        ensure_pending_manager_plan_workflow_materialization_approval(
            state,
            plan,
            definition,
            workflow_selection,
        )
        .await?;
        return Err(AppError::bad_request(
            "manager plan workflow materialization cannot auto-start high-risk plans",
        ));
    };
    let approval = state.get_approval(approval_id).await?;
    validate_manager_plan_workflow_materialization_approval(plan, definition, &approval)?;
    Ok(Some(approval.id))
}

async fn ensure_pending_manager_plan_workflow_materialization_approval(
    state: &AppState,
    plan: &ManagerAgentPlan,
    definition: &crate::WorkflowDefinition,
    workflow_selection: &Value,
) -> Result<Approval, AppError> {
    let evidence = manager_plan_workflow_materialization_approval_evidence(plan, definition);
    if let Some(approval) = state.list_approvals().await?.into_iter().find(|approval| {
        approval.session_id == plan.session_id
            && approval.tool_call_id.is_none()
            && approval.action == "manager_plan.workflow_run_materialized"
            && approval.risk_level == "high"
            && approval.status == "pending"
            && approval.evidence == evidence
    }) {
        return Ok(approval);
    }
    let created_at = Utc::now();
    let approval = state
        .insert_approval(Approval {
            id: Uuid::new_v4(),
            session_id: plan.session_id,
            tool_call_id: None,
            action: "manager_plan.workflow_run_materialized".to_string(),
            risk_level: "high".to_string(),
            reason: "Approve high-risk manager plan workflow materialization".to_string(),
            evidence,
            decision_payload: json!({}),
            status: "pending".to_string(),
            expires_at: None,
            created_at,
            decided_at: None,
        })
        .await?;
    state
        .append_event(
            "system",
            Some(approval.id),
            plan.session_id,
            "approval.requested",
            json!({
                "approval_id": approval.id,
                "action": approval.action,
                "risk_level": approval.risk_level,
                "reason": approval.reason,
                "evidence": approval.evidence,
                "workflow_selection": workflow_selection,
                "expires_at": approval.expires_at
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(plan.session_id),
            "system",
            Some(plan.id),
            "approval.requested",
            "approval",
            Some(approval.id),
            json!({
                "manager_plan_id": plan.id,
                "workflow_definition_id": definition.id,
                "workflow_selection": workflow_selection,
                "action": approval.action,
                "risk_level": approval.risk_level,
                "expires_at": approval.expires_at
            }),
        ))
        .await?;
    set_managed_session_status(
        state,
        plan.session_id,
        SessionStatus::RequiresAction,
        "manager plan workflow materialization approval required",
    )
    .await?;
    Ok(approval)
}

fn validate_manager_plan_workflow_materialization_approval(
    plan: &ManagerAgentPlan,
    definition: &crate::WorkflowDefinition,
    approval: &Approval,
) -> Result<(), AppError> {
    if approval.session_id != plan.session_id
        || approval.tool_call_id.is_some()
        || approval.action != "manager_plan.workflow_run_materialized"
        || approval.risk_level != "high"
        || approval.status != "approved"
        || approval_is_expired(approval)
        || approval.evidence
            != manager_plan_workflow_materialization_approval_evidence(plan, definition)
    {
        return Err(AppError::forbidden(
            "approved manager plan workflow materialization approval is required",
        ));
    }
    Ok(())
}

fn manager_plan_workflow_materialization_approval_evidence(
    plan: &ManagerAgentPlan,
    definition: &crate::WorkflowDefinition,
) -> Value {
    json!({
        "manager_plan_id": plan.id,
        "workflow_definition_id": definition.id
    })
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

fn manager_plan_workflow_input_payload(
    plan: &ManagerAgentPlan,
    input: Value,
    approval_id: Option<Uuid>,
    workflow_selection: &Value,
) -> Value {
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
    if let Some(approval_id) = approval_id {
        payload.insert("approval_id".to_string(), json!(approval_id));
    }
    payload.insert("workflow_selection".to_string(), workflow_selection.clone());
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
    approval_id: Option<Uuid>,
    workflow_selection: &Value,
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
    if let Some(approval_id) = approval_id {
        envelope.insert("approval_id".to_string(), json!(approval_id));
    }
    envelope.insert("workflow_selection".to_string(), workflow_selection.clone());
    Value::Object(envelope)
}
