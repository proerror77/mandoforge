use std::collections::HashSet;

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::*;

tokio::task_local! {
    static REQUEST_TENANT_ID: Uuid;
}

pub(crate) fn current_request_tenant_id(default_tenant_id: Uuid) -> Uuid {
    REQUEST_TENANT_ID
        .try_with(|tenant_id| *tenant_id)
        .unwrap_or(default_tenant_id)
}

pub(crate) async fn tenant_context_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let tenant_id = resolve_request_tenant_id(&state, &headers)?;
    Ok(REQUEST_TENANT_ID.scope(tenant_id, next.run(request)).await)
}

pub(crate) async fn authorize_request(
    state: &AppState,
    headers: &HeaderMap,
    permission: Permission,
    resource_type: impl Into<String>,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission,
        resource_type: resource_type.into(),
        resource_id,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(state, &principal, &request).await
}

pub(crate) async fn authorize_collection_request(
    state: &AppState,
    headers: &HeaderMap,
    permission: Permission,
    resource_type: impl Into<String>,
) -> Result<Principal, AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission,
        resource_type: resource_type.into(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(principal)
}

pub(crate) async fn visible_session_ids_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<HashSet<Uuid>, AppError> {
    Ok(state
        .list_sessions_visible_to(principal)
        .await?
        .into_iter()
        .map(|session| session.id)
        .collect())
}

async fn work_item_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    work_item_id: Uuid,
) -> Result<bool, AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(true);
    }
    let work_item = state.get_work_item(work_item_id).await?;
    if let Some(project_id) = work_item.project_id {
        return state
            .subject_can_access_project(&principal.subject_id, project_id)
            .await;
    }
    if let Some(team_id) = work_item.team_id {
        return state
            .subject_can_access_team(&principal.subject_id, team_id)
            .await;
    }
    Ok(true)
}

pub(crate) async fn dynamic_workflow_plan_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    visible_session_ids: &HashSet<Uuid>,
    plan: &DynamicWorkflowPlan,
) -> Result<bool, AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(true);
    }
    if let Some(session_id) = plan.source_session_id {
        return Ok(visible_session_ids.contains(&session_id));
    }
    if let Some(workflow_run_id) = plan.workflow_run_id {
        let run = state.get_workflow_run(workflow_run_id).await?;
        return Ok(visible_session_ids.contains(&run.primary_session_id));
    }
    if let Some(work_item_id) = plan.source_work_item_id {
        return work_item_visible_to_principal(state, principal, work_item_id).await;
    }
    Ok(false)
}

pub(crate) async fn enforce_resource_scope(
    state: &AppState,
    principal: &Principal,
    request: &AuthorizationRequest,
) -> Result<(), AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(());
    }
    let Some(resource_id) = request.resource_id else {
        return Ok(());
    };
    let (team_id, project_id) = match request.resource_type.as_str() {
        "agent" => {
            let agent = state.get_agent(resource_id).await?;
            (agent.team_id, agent.project_id)
        }
        "session" => {
            let session = state.get_session(resource_id).await?;
            let agent = state.get_agent(session.agent_id).await?;
            (agent.team_id, agent.project_id)
        }
        "work_item" => {
            let work_item = state.get_work_item(resource_id).await?;
            (work_item.team_id, work_item.project_id)
        }
        "dynamic_workflow_plan" => {
            let plan = state.get_dynamic_workflow_plan(resource_id).await?;
            if let Some(session_id) = plan.source_session_id {
                let session = state.get_session(session_id).await?;
                let agent = state.get_agent(session.agent_id).await?;
                (agent.team_id, agent.project_id)
            } else if let Some(workflow_run_id) = plan.workflow_run_id {
                let run = state.get_workflow_run(workflow_run_id).await?;
                let session = state.get_session(run.primary_session_id).await?;
                let agent = state.get_agent(session.agent_id).await?;
                (agent.team_id, agent.project_id)
            } else if let Some(work_item_id) = plan.source_work_item_id {
                let work_item = state.get_work_item(work_item_id).await?;
                (work_item.team_id, work_item.project_id)
            } else {
                return Err(AppError::forbidden(
                    "unscoped dynamic workflow plan requires admin access",
                ));
            }
        }
        _ => (None, None),
    };
    if let Some(project_id) = project_id {
        if state
            .subject_can_access_project(&principal.subject_id, project_id)
            .await?
        {
            return Ok(());
        }
        return Err(AppError::forbidden(format!(
            "principal {} has no membership for scoped {}",
            principal.subject_id, request.resource_type
        )));
    }
    let Some(team_id) = team_id else {
        return Ok(());
    };
    if state
        .subject_can_access_team(&principal.subject_id, team_id)
        .await?
    {
        Ok(())
    } else {
        Err(AppError::forbidden(format!(
            "principal {} has no membership for scoped {}",
            principal.subject_id, request.resource_type
        )))
    }
}
