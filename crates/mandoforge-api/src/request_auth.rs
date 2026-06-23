use std::collections::HashSet;

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::*;

enum ResourceScope {
    Tenant,
    TeamProject {
        team_id: Option<Uuid>,
        project_id: Option<Uuid>,
    },
    Session(Uuid),
}

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

async fn session_scope(state: &AppState, session_id: Uuid) -> Result<ResourceScope, AppError> {
    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    Ok(ResourceScope::TeamProject {
        team_id: agent.team_id,
        project_id: agent.project_id,
    })
}

fn uuid_from_json_value(value: &serde_json::Value) -> Option<Uuid> {
    value.as_str().and_then(|value| Uuid::parse_str(value).ok())
}

fn uuid_from_semantic_scope(scopes: &serde_json::Value, key: &str) -> Option<Uuid> {
    scopes.get(key).and_then(uuid_from_json_value)
}

async fn semantic_scopes_scope(
    state: &AppState,
    scopes: &serde_json::Value,
) -> Result<ResourceScope, AppError> {
    if let Some(project_id) = uuid_from_semantic_scope(scopes, "project_id") {
        return Ok(ResourceScope::TeamProject {
            team_id: None,
            project_id: Some(project_id),
        });
    }
    if let Some(team_id) = uuid_from_semantic_scope(scopes, "team_id") {
        return Ok(ResourceScope::TeamProject {
            team_id: Some(team_id),
            project_id: None,
        });
    }
    if let Some(session_id) = uuid_from_semantic_scope(scopes, "session_id") {
        return session_scope(state, session_id).await;
    }
    if let Some(agent_id) = uuid_from_semantic_scope(scopes, "agent_id") {
        let agent = state.get_agent(agent_id).await?;
        return Ok(ResourceScope::TeamProject {
            team_id: agent.team_id,
            project_id: agent.project_id,
        });
    }
    if let Some(work_item_id) = uuid_from_semantic_scope(scopes, "work_item_id") {
        let work_item = state.get_work_item(work_item_id).await?;
        return Ok(ResourceScope::TeamProject {
            team_id: work_item.team_id,
            project_id: work_item.project_id,
        });
    }
    Ok(ResourceScope::Tenant)
}

async fn semantic_source_scope(
    state: &AppState,
    source: &SemanticSource,
) -> Result<ResourceScope, AppError> {
    let Some(owner_type) = source.owner_type.as_deref() else {
        return Ok(ResourceScope::Tenant);
    };
    let Some(owner_id) = source.owner_id else {
        return Ok(ResourceScope::Tenant);
    };
    match owner_type.trim().to_ascii_lowercase().as_str() {
        "project" => Ok(ResourceScope::TeamProject {
            team_id: None,
            project_id: Some(owner_id),
        }),
        "team" => Ok(ResourceScope::TeamProject {
            team_id: Some(owner_id),
            project_id: None,
        }),
        "agent" => {
            let agent = state.get_agent(owner_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: agent.team_id,
                project_id: agent.project_id,
            })
        }
        "session" => session_scope(state, owner_id).await,
        "work_item" => {
            let work_item = state.get_work_item(owner_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: work_item.team_id,
                project_id: work_item.project_id,
            })
        }
        _ => Ok(ResourceScope::Tenant),
    }
}

async fn semantic_object_scope(
    state: &AppState,
    object: &SemanticObject,
) -> Result<ResourceScope, AppError> {
    if let Some(source_id) = object.source_id {
        let source = state.get_semantic_source(source_id).await?;
        let scope = semantic_source_scope(state, &source).await?;
        if !matches!(scope, ResourceScope::Tenant) {
            return Ok(scope);
        }
    }
    semantic_scopes_scope(state, &object.semantic_scopes).await
}

async fn semantic_link_endpoint_scope(
    state: &AppState,
    entity_type: &str,
    entity_id: &str,
) -> Result<ResourceScope, AppError> {
    if entity_type != "semantic_object" {
        return Ok(ResourceScope::Tenant);
    }
    let object_id = Uuid::parse_str(entity_id)
        .map_err(|_| AppError::forbidden("semantic link endpoint is not a valid object id"))?;
    let object = state.get_semantic_object(object_id).await?;
    semantic_object_scope(state, &object).await
}

async fn scope_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    scope: ResourceScope,
) -> Result<bool, AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(true);
    }
    match scope {
        ResourceScope::Tenant => Ok(true),
        ResourceScope::Session(session_id) => {
            let visible = visible_session_ids_for_principal(state, principal).await?;
            Ok(visible.contains(&session_id))
        }
        ResourceScope::TeamProject {
            team_id,
            project_id,
        } => {
            if let Some(project_id) = project_id {
                return state
                    .subject_can_access_project(&principal.subject_id, project_id)
                    .await;
            }
            let Some(team_id) = team_id else {
                return Ok(true);
            };
            state
                .subject_can_access_team(&principal.subject_id, team_id)
                .await
        }
    }
}

pub(crate) async fn semantic_source_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    source: &SemanticSource,
) -> Result<bool, AppError> {
    let scope = semantic_source_scope(state, source).await?;
    scope_visible_to_principal(state, principal, scope).await
}

pub(crate) async fn semantic_object_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    object: &SemanticObject,
) -> Result<bool, AppError> {
    let scope = semantic_object_scope(state, object).await?;
    scope_visible_to_principal(state, principal, scope).await
}

pub(crate) async fn semantic_link_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    link: &SemanticLink,
) -> Result<bool, AppError> {
    let from_scope =
        semantic_link_endpoint_scope(state, &link.from_entity_type, &link.from_entity_id).await?;
    if !scope_visible_to_principal(state, principal, from_scope).await? {
        return Ok(false);
    }
    let to_scope =
        semantic_link_endpoint_scope(state, &link.to_entity_type, &link.to_entity_id).await?;
    scope_visible_to_principal(state, principal, to_scope).await
}

async fn resource_scope(
    state: &AppState,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<ResourceScope, AppError> {
    match resource_type {
        "agent" => {
            let agent = state.get_agent(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: agent.team_id,
                project_id: agent.project_id,
            })
        }
        "session" => session_scope(state, resource_id).await,
        "work_item" => {
            let work_item = state.get_work_item(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: work_item.team_id,
                project_id: work_item.project_id,
            })
        }
        "workflow_run" => {
            let run = state.get_workflow_run(resource_id).await?;
            Ok(ResourceScope::Session(run.primary_session_id))
        }
        "execution_job" => {
            let job = state.execution_queue.get(resource_id).await?;
            Ok(ResourceScope::Session(job.session_id))
        }
        "context_packet" => {
            let packet = state.get_context_packet(resource_id).await?;
            Ok(ResourceScope::Session(packet.session_id))
        }
        "semantic_source" => {
            let source = state.get_semantic_source(resource_id).await?;
            semantic_source_scope(state, &source).await
        }
        "semantic_object" => {
            let object = state.get_semantic_object(resource_id).await?;
            semantic_object_scope(state, &object).await
        }
        "semantic_link" => {
            let link = state.get_semantic_link(resource_id).await?;
            let from_scope =
                semantic_link_endpoint_scope(state, &link.from_entity_type, &link.from_entity_id)
                    .await?;
            let to_scope =
                semantic_link_endpoint_scope(state, &link.to_entity_type, &link.to_entity_id)
                    .await?;
            match (from_scope, to_scope) {
                (ResourceScope::Tenant, scope) | (scope, ResourceScope::Tenant) => Ok(scope),
                (ResourceScope::Session(session_id), _) => Ok(ResourceScope::Session(session_id)),
                (scope, ResourceScope::Session(_)) => Ok(scope),
                (
                    ResourceScope::TeamProject {
                        team_id,
                        project_id,
                    },
                    _,
                ) => Ok(ResourceScope::TeamProject {
                    team_id,
                    project_id,
                }),
            }
        }
        "dynamic_workflow_plan" => {
            let plan = state.get_dynamic_workflow_plan(resource_id).await?;
            if let Some(session_id) = plan.source_session_id {
                Ok(ResourceScope::Session(session_id))
            } else if let Some(workflow_run_id) = plan.workflow_run_id {
                let run = state.get_workflow_run(workflow_run_id).await?;
                Ok(ResourceScope::Session(run.primary_session_id))
            } else if let Some(work_item_id) = plan.source_work_item_id {
                let work_item = state.get_work_item(work_item_id).await?;
                Ok(ResourceScope::TeamProject {
                    team_id: work_item.team_id,
                    project_id: work_item.project_id,
                })
            } else {
                Err(AppError::forbidden(
                    "unscoped dynamic workflow plan requires admin access",
                ))
            }
        }
        _ => Ok(ResourceScope::Tenant),
    }
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
    if request.resource_type == "semantic_link" {
        let link = state.get_semantic_link(resource_id).await?;
        if semantic_link_visible_to_principal(state, principal, &link).await? {
            return Ok(());
        }
        return Err(AppError::forbidden(format!(
            "principal {} has no membership for scoped {}",
            principal.subject_id, request.resource_type
        )));
    }
    let scope = resource_scope(state, &request.resource_type, resource_id).await?;
    if scope_visible_to_principal(state, principal, scope).await? {
        return Ok(());
    }
    Err(AppError::forbidden(format!(
        "principal {} has no membership for scoped {}",
        principal.subject_id, request.resource_type
    )))
}
