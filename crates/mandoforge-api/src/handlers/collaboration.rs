use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AgentTeammate, AppError, AppState, AuthorizationRequest, CreateAgentTeammate, CreateSquad,
    CreateSquadMember, CreateWorkItem, CreateWorkItemAssignment, CreateWorkItemReview,
    Permission, Squad, SquadMember, WorkItem, WorkItemActivityEntry, WorkItemAssignment,
    WorkItemReview, authorize_request, capability_failure_modes, capability_primary_action,
    capability_sample_tasks, new_audit_log, principal_from_request,
    project_work_item_semantic_object, validate_work_item_semantic_scopes,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/work-items",
            get(list_work_items).post(create_work_item),
        )
        .route(
            "/api/agent-teammates",
            get(list_agent_teammates).post(create_agent_teammate),
        )
        .route("/api/squads", get(list_squads).post(create_squad))
        .route(
            "/api/squads/{id}/members",
            get(list_squad_members).post(add_squad_member),
        )
        .route(
            "/api/work-items/{id}/assignments",
            get(list_work_item_assignments).post(create_work_item_assignment),
        )
        .route(
            "/api/work-items/{id}/reviews",
            get(list_work_item_reviews).post(create_work_item_review),
        )
        .route(
            "/api/work-items/{id}/activity",
            get(list_work_item_activity),
        )
        .route("/api/capability-discovery", get(get_capability_discovery))
}

async fn list_work_items(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItem>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_items",
        None,
    )
    .await?;
    Ok(Json(state.list_work_items().await?))
}

async fn create_work_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItem>,
) -> Result<Json<WorkItem>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    validate_work_item_semantic_scopes(&input.metadata)?;
    let work_item = state.create_work_item(input).await?;
    state
        .append_work_item_activity_entry(
            work_item.id,
            "work_item.created",
            Some(principal.subject_id.clone()),
            Some("work_item"),
            Some(work_item.id),
            format!("Created WorkItem: {}", work_item.title),
            json!({
                "title": work_item.title.clone(),
                "source": work_item.source.clone(),
                "status": work_item.status.clone(),
                "priority": work_item.priority.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.created",
            "work_item",
            Some(work_item.id),
            json!({
                "subject": principal.subject_id,
                "organization_id": work_item.organization_id,
                "team_id": work_item.team_id,
                "project_id": work_item.project_id,
                "title": work_item.title,
                "source": work_item.source,
                "status": work_item.status,
                "priority": work_item.priority
            }),
        ))
        .await?;
    project_work_item_semantic_object(&state, &work_item).await?;
    Ok(Json(work_item))
}

async fn list_agent_teammates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentTeammate>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "agent_teammates",
        None,
    )
    .await?;
    Ok(Json(state.list_agent_teammates().await?))
}

async fn create_agent_teammate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgentTeammate>,
) -> Result<Json<AgentTeammate>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "agent_teammate".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    let teammate = state.create_agent_teammate(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "agent_teammate.created",
            "agent_teammate",
            Some(teammate.id),
            json!({
                "subject": principal.subject_id,
                "agent_id": teammate.agent_id,
                "display_name": teammate.display_name,
                "handle": teammate.handle,
                "role": teammate.role,
                "status": teammate.status
            }),
        ))
        .await?;
    Ok(Json(teammate))
}

async fn list_squads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Squad>>, AppError> {
    authorize_request(&state, &headers, Permission::SessionsRead, "squads", None).await?;
    Ok(Json(state.list_squads().await?))
}

async fn create_squad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSquad>,
) -> Result<Json<Squad>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "squad".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    let squad = state.create_squad(input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "squad.created",
            "squad",
            Some(squad.id),
            json!({
                "subject": principal.subject_id,
                "name": squad.name,
                "status": squad.status
            }),
        ))
        .await?;
    Ok(Json(squad))
}

async fn list_squad_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SquadMember>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "squad",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_squad_members(id).await?))
}

async fn add_squad_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateSquadMember>,
) -> Result<Json<SquadMember>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "squad".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let member = state.add_squad_member(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "squad.member_added",
            "squad_member",
            Some(member.id),
            json!({
                "subject": principal.subject_id,
                "squad_id": member.squad_id,
                "teammate_id": member.teammate_id,
                "role": member.role,
                "status": member.status
            }),
        ))
        .await?;
    Ok(Json(member))
}

async fn list_work_item_assignments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemAssignment>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_assignments(id).await?))
}

async fn create_work_item_assignment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemAssignment>,
) -> Result<Json<WorkItemAssignment>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let assignment = state
        .create_work_item_assignment(id, input, Some(principal.subject_id.clone()))
        .await?;
    state
        .append_work_item_activity_entry(
            assignment.work_item_id,
            "work_item.assignment_created",
            Some(principal.subject_id.clone()),
            Some("work_item_assignment"),
            Some(assignment.id),
            format!(
                "Assigned {} {} as {}",
                assignment.assignee_kind, assignment.assignee_id, assignment.role
            ),
            json!({
                "assignee_kind": assignment.assignee_kind.clone(),
                "assignee_id": assignment.assignee_id.clone(),
                "role": assignment.role.clone(),
                "status": assignment.status.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.assignment_created",
            "work_item_assignment",
            Some(assignment.id),
            json!({
                "subject": principal.subject_id,
                "work_item_id": assignment.work_item_id,
                "assignee_kind": assignment.assignee_kind,
                "assignee_id": assignment.assignee_id,
                "role": assignment.role,
                "status": assignment.status,
                "assigned_by": assignment.assigned_by
            }),
        ))
        .await?;
    Ok(Json(assignment))
}

async fn list_work_item_reviews(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemReview>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_reviews(id).await?))
}

async fn create_work_item_review(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateWorkItemReview>,
) -> Result<Json<WorkItemReview>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::SessionsWrite,
        resource_type: "work_item".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let review = state.create_work_item_review(id, input).await?;
    state
        .append_work_item_activity_entry(
            review.work_item_id,
            "work_item.review_created",
            Some(principal.subject_id.clone()),
            Some("work_item_review"),
            Some(review.id),
            match &review.decision {
                Some(decision) => format!("Review completed with decision: {decision}"),
                None => "Review requested".to_string(),
            },
            json!({
                "reviewer_kind": review.reviewer_kind.clone(),
                "reviewer_id": review.reviewer_id.clone(),
                "status": review.status.clone(),
                "decision": review.decision.clone(),
                "summary": review.summary.clone()
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "work_item.review_created",
            "work_item_review",
            Some(review.id),
            json!({
                "subject": principal.subject_id,
                "work_item_id": review.work_item_id,
                "reviewer_kind": review.reviewer_kind,
                "reviewer_id": review.reviewer_id,
                "status": review.status,
                "decision": review.decision
            }),
        ))
        .await?;
    Ok(Json(review))
}

async fn list_work_item_activity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkItemActivityEntry>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "work_item",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_work_item_activity(id).await?))
}

async fn get_capability_discovery(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::AgentsRead,
        "capability_discovery",
        None,
    )
    .await?;
    let agents = state.list_agents().await?;
    let work_items = state.list_work_items().await?;
    let pending_memory = state
        .list_memory_writeback_candidates(None)
        .await?
        .into_iter()
        .filter(|candidate| candidate.status == "pending")
        .count();
    let agent_cards = agents
        .iter()
        .map(|agent| {
            json!({
                "agent_id": agent.id,
                "name": agent.name,
                "kind": agent.kind,
                "agent_role": agent.agent_role,
                "provider": agent.provider,
                "model": agent.model,
                "release_state": agent.release_state,
                "tools": agent.tools,
                "skill_ids": agent.skill_ids,
                "workflow_pack_ids": agent.workflow_pack_ids,
                "semantic_scopes": agent.semantic_scopes,
                "primary_action": capability_primary_action(agent),
                "failure_modes": capability_failure_modes(agent),
                "sample_tasks": capability_sample_tasks(agent),
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "status": "ready",
        "generated_at": Utc::now(),
        "summary": {
            "agent_count": agents.len(),
            "open_work_item_count": work_items.iter().filter(|item| !matches!(item.status.as_str(), "done" | "canceled")).count(),
            "pending_memory_review_count": pending_memory,
        },
        "agent_cards": agent_cards,
        "suggested_prompts": [
            {
                "target_view": "manager",
                "title": "拆解并派发一个工作项",
                "prompt": "请把这个目标拆解成 WorkItem、选择合适 Agent、设置 SLA，并建立复审点。",
                "action": "create_work_item_then_start_pack_workflow"
            },
            {
                "target_view": "semantic",
                "title": "整理领域记忆",
                "prompt": "请扫描这个 domain 的冲突、过期记忆与 ontology 缺口，建立需要复审的队列。",
                "action": "open_semantic_workbench"
            },
            {
                "target_view": "board",
                "title": "查看队列风险",
                "prompt": "请找出 blocked、overdue、无人领取的任务，并给出下一步处理建议。",
                "action": "inspect_task_board"
            }
        ],
        "onboarding_steps": [
            {
                "key": "create_work_item",
                "title": "建立 WorkItem",
                "description": "先把业务目标变成可审计、可派工、可复审的任务。"
            },
            {
                "key": "start_pack_manager_workflow",
                "title": "启动 Pack Manager Workflow",
                "description": "通过 Workflow Pack 定义的 manager workflow 做 intake、拆解、派工、SLA 检查和复审。"
            },
            {
                "key": "review_memory",
                "title": "复审 Memory Queue",
                "description": "批准或拒绝 reflection / dreaming 产生的记忆候选。"
            },
            {
                "key": "install_pack",
                "title": "安装 Workflow Pack",
                "description": "把领域流程、Agent 技能和运行对象绑定成可复用模板。"
            }
        ],
        "empty_states": [
            {
                "view": "manager",
                "title": "还没有 Pack Manager Workflow 结果",
                "action": "start_pack_manager_workflow"
            },
            {
                "view": "semantic",
                "title": "还没有可治理的语义对象",
                "action": "ingest_semantic_source"
            },
            {
                "view": "board",
                "title": "还没有 WorkItem",
                "action": "create_work_item"
            }
        ],
    })))
}
