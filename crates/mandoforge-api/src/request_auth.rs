use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::*;

#[derive(Clone)]
enum ResourceScope {
    Tenant,
    ScopedUnknown,
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

pub(crate) async fn visible_work_items_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<WorkItem>, AppError> {
    let work_items = state.list_work_items().await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok(work_items);
    }
    let mut visible = Vec::new();
    for work_item in work_items {
        if scope_visible_to_principal(
            state,
            principal,
            ResourceScope::TeamProject {
                team_id: work_item.team_id,
                project_id: work_item.project_id,
            },
        )
        .await?
        {
            visible.push(work_item);
        }
    }
    Ok(visible)
}

async fn session_scope(state: &AppState, session_id: Uuid) -> Result<ResourceScope, AppError> {
    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    Ok(ResourceScope::TeamProject {
        team_id: agent.team_id,
        project_id: agent.project_id,
    })
}

async fn team_scope(state: &AppState, team_id: Uuid) -> Result<ResourceScope, AppError> {
    state.get_team(team_id).await?;
    Ok(ResourceScope::TeamProject {
        team_id: Some(team_id),
        project_id: None,
    })
}

async fn project_scope(state: &AppState, project_id: Uuid) -> Result<ResourceScope, AppError> {
    let project = state.get_project(project_id).await?;
    Ok(ResourceScope::TeamProject {
        team_id: Some(project.team_id),
        project_id: Some(project.id),
    })
}

async fn organization_exists(state: &AppState, organization_id: Uuid) -> Result<(), AppError> {
    state.get_organization(organization_id).await.map(|_| ())
}

async fn find_project_by_scope_label(
    state: &AppState,
    label: &str,
) -> Result<Option<Project>, AppError> {
    let normalized = label.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    let mut matches = Vec::new();
    for organization in state.list_organizations().await? {
        for team in state.list_teams(organization.id).await? {
            matches.extend(
                state
                    .list_projects(team.id)
                    .await?
                    .into_iter()
                    .filter(|project| {
                        project.slug.eq_ignore_ascii_case(normalized)
                            || project.name.eq_ignore_ascii_case(normalized)
                    }),
            );
        }
    }
    Ok((matches.len() == 1).then(|| matches.remove(0)))
}

async fn find_team_by_scope_label(state: &AppState, label: &str) -> Result<Option<Team>, AppError> {
    let normalized = label.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    let mut matches = Vec::new();
    for organization in state.list_organizations().await? {
        matches.extend(
            state
                .list_teams(organization.id)
                .await?
                .into_iter()
                .filter(|team| {
                    team.slug.eq_ignore_ascii_case(normalized)
                        || team.name.eq_ignore_ascii_case(normalized)
                }),
        );
    }
    Ok((matches.len() == 1).then(|| matches.remove(0)))
}

fn uuid_from_json_value(value: &serde_json::Value) -> Option<Uuid> {
    value.as_str().and_then(|value| Uuid::parse_str(value).ok())
}

fn uuid_from_semantic_scope(scopes: &serde_json::Value, key: &str) -> Option<Uuid> {
    scopes.get(key).and_then(uuid_from_json_value)
}

fn text_from_semantic_scope<'a>(scopes: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    scopes
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

async fn semantic_scopes_scope(
    state: &AppState,
    scopes: &serde_json::Value,
) -> Result<ResourceScope, AppError> {
    if let Some(project_id) = uuid_from_semantic_scope(scopes, "project_id") {
        return project_scope(state, project_id).await;
    }
    if let Some(team_id) = uuid_from_semantic_scope(scopes, "team_id") {
        return team_scope(state, team_id).await;
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
    for key in ["project_scope", "project_slug"] {
        if let Some(label) = text_from_semantic_scope(scopes, key) {
            return Ok(match find_project_by_scope_label(state, label).await? {
                Some(project) => ResourceScope::TeamProject {
                    team_id: Some(project.team_id),
                    project_id: Some(project.id),
                },
                None => ResourceScope::ScopedUnknown,
            });
        }
    }
    for key in ["team_scope", "team_slug"] {
        if let Some(label) = text_from_semantic_scope(scopes, key) {
            return Ok(match find_team_by_scope_label(state, label).await? {
                Some(team) => ResourceScope::TeamProject {
                    team_id: Some(team.id),
                    project_id: None,
                },
                None => ResourceScope::ScopedUnknown,
            });
        }
    }
    if ["session_scope", "agent_scope", "work_item_scope"]
        .iter()
        .any(|key| text_from_semantic_scope(scopes, key).is_some())
    {
        return Ok(ResourceScope::ScopedUnknown);
    }
    Ok(ResourceScope::Tenant)
}

async fn semantic_source_scope(
    state: &AppState,
    source: &SemanticSource,
    active_workflow_pack_ids: Option<&HashSet<Uuid>>,
) -> Result<ResourceScope, AppError> {
    match (source.owner_type.as_deref(), source.owner_id) {
        (None, None) => return Ok(ResourceScope::Tenant),
        (None, Some(_)) | (Some(_), None) => return Ok(ResourceScope::ScopedUnknown),
        (Some(_), Some(_)) => {}
    };
    let owner_type = source.owner_type.as_deref().unwrap_or_default();
    let owner_id = source.owner_id.expect("owner_id checked above");
    match owner_type.trim().to_ascii_lowercase().as_str() {
        "project" => project_scope(state, owner_id).await,
        "team" => team_scope(state, owner_id).await,
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
        "workflow_pack_installation" => {
            if let Some(active_workflow_pack_ids) = active_workflow_pack_ids {
                return Ok(if active_workflow_pack_ids.contains(&owner_id) {
                    ResourceScope::Tenant
                } else {
                    ResourceScope::ScopedUnknown
                });
            }
            match state.get_workflow_pack_installation(owner_id).await {
                Ok(_) => Ok(ResourceScope::Tenant),
                Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {
                    Ok(ResourceScope::ScopedUnknown)
                }
                Err(error) => Err(error),
            }
        }
        _ => Ok(ResourceScope::ScopedUnknown),
    }
}

async fn semantic_object_scope(
    state: &AppState,
    object: &SemanticObject,
) -> Result<ResourceScope, AppError> {
    if let Some(source_id) = object.source_id {
        let source = state.get_semantic_source(source_id).await?;
        let scope = semantic_source_scope(state, &source, None).await?;
        if !matches!(scope, ResourceScope::Tenant) {
            return Ok(scope);
        }
    }
    semantic_scopes_scope(state, &object.semantic_scopes).await
}

async fn semantic_object_scope_with_source_scopes(
    state: &AppState,
    object: &SemanticObject,
    source_scopes: &HashMap<Uuid, ResourceScope>,
) -> Result<ResourceScope, AppError> {
    if let Some(source_id) = object.source_id {
        let scope = match source_scopes.get(&source_id) {
            Some(scope) => scope.clone(),
            None => {
                let source = state.get_semantic_source(source_id).await?;
                semantic_source_scope(state, &source, None).await?
            }
        };
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
    if entity_type == "semantic_object" {
        let object_id = Uuid::parse_str(entity_id)
            .map_err(|_| AppError::forbidden("semantic link endpoint is not a valid object id"))?;
        let object = state.get_semantic_object(object_id).await?;
        return semantic_object_scope(state, &object).await;
    }
    let Some(resource_type) = semantic_link_endpoint_resource_type(entity_type) else {
        return Ok(ResourceScope::ScopedUnknown);
    };
    let resource_id = Uuid::parse_str(entity_id)
        .map_err(|_| AppError::forbidden("semantic link endpoint is not a valid resource id"))?;
    semantic_link_endpoint_resource_scope(state, resource_type, resource_id).await
}

async fn semantic_link_endpoint_scope_with_object_scopes(
    state: &AppState,
    entity_type: &str,
    entity_id: &str,
    object_scopes: &HashMap<Uuid, ResourceScope>,
) -> Result<ResourceScope, AppError> {
    if entity_type != "semantic_object" {
        return semantic_link_endpoint_scope(state, entity_type, entity_id).await;
    }
    let object_id = Uuid::parse_str(entity_id)
        .map_err(|_| AppError::forbidden("semantic link endpoint is not a valid object id"))?;
    match object_scopes.get(&object_id) {
        Some(scope) => Ok(scope.clone()),
        None => {
            let object = state.get_semantic_object(object_id).await?;
            semantic_object_scope(state, &object).await
        }
    }
}

fn semantic_link_endpoint_resource_type(entity_type: &str) -> Option<&'static str> {
    match entity_type.trim().to_ascii_lowercase().as_str() {
        "agent" => Some("agent"),
        "project" => Some("project"),
        "session" => Some("session"),
        "work_item" => Some("work_item"),
        "workflow_run" => Some("workflow_run"),
        _ => None,
    }
}

async fn semantic_link_endpoint_resource_scope(
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
        "project" => project_scope(state, resource_id).await,
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
        _ => Ok(ResourceScope::ScopedUnknown),
    }
}

async fn scope_visible_to_principal(
    state: &AppState,
    principal: &Principal,
    scope: ResourceScope,
) -> Result<bool, AppError> {
    scope_visible_to_principal_with_sessions(state, principal, scope, None).await
}

async fn scope_visible_to_principal_with_sessions(
    state: &AppState,
    principal: &Principal,
    scope: ResourceScope,
    visible_session_ids: Option<&HashSet<Uuid>>,
) -> Result<bool, AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(true);
    }
    match scope {
        ResourceScope::Tenant => Ok(true),
        ResourceScope::ScopedUnknown => Ok(false),
        ResourceScope::Session(session_id) => {
            if let Some(visible_session_ids) = visible_session_ids {
                Ok(visible_session_ids.contains(&session_id))
            } else {
                let visible = visible_session_ids_for_principal(state, principal).await?;
                Ok(visible.contains(&session_id))
            }
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

pub(crate) async fn visible_semantic_sources_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<SemanticSource>, AppError> {
    let sources = state.list_semantic_sources().await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok(sources);
    }
    let visible_session_ids = visible_session_ids_for_principal(state, principal).await?;
    let source_scopes = semantic_source_scope_cache_for_sources(state, &sources).await?;
    let mut visible = Vec::new();
    for source in sources {
        let scope = source_scopes
            .get(&source.id)
            .expect("scope cached for every semantic source")
            .clone();
        if scope_visible_to_principal_with_sessions(
            state,
            principal,
            scope,
            Some(&visible_session_ids),
        )
        .await?
        {
            visible.push(source);
        }
    }
    Ok(visible)
}

pub(crate) async fn visible_semantic_objects_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<SemanticObject>, AppError> {
    let objects = state.list_semantic_objects().await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok(objects);
    }
    let visible_session_ids = visible_session_ids_for_principal(state, principal).await?;
    let source_scopes = semantic_source_scope_cache(state).await?;
    let mut visible = Vec::new();
    for object in objects {
        let scope =
            semantic_object_scope_with_source_scopes(state, &object, &source_scopes).await?;
        if scope_visible_to_principal_with_sessions(
            state,
            principal,
            scope,
            Some(&visible_session_ids),
        )
        .await?
        {
            visible.push(object);
        }
    }
    Ok(visible)
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

pub(crate) async fn visible_semantic_links_for_principal(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<SemanticLink>, AppError> {
    let links = state.list_semantic_links().await?;
    if principal.roles.contains(&Role::Admin) {
        return Ok(links);
    }
    let visible_session_ids = visible_session_ids_for_principal(state, principal).await?;
    let source_scopes = semantic_source_scope_cache(state).await?;
    let object_scopes = semantic_object_scope_cache(state, &source_scopes).await?;
    let mut visible = Vec::new();
    for link in links {
        let from_scope = semantic_link_endpoint_scope_with_object_scopes(
            state,
            &link.from_entity_type,
            &link.from_entity_id,
            &object_scopes,
        )
        .await?;
        if !scope_visible_to_principal_with_sessions(
            state,
            principal,
            from_scope,
            Some(&visible_session_ids),
        )
        .await?
        {
            continue;
        }
        let to_scope = semantic_link_endpoint_scope_with_object_scopes(
            state,
            &link.to_entity_type,
            &link.to_entity_id,
            &object_scopes,
        )
        .await?;
        if scope_visible_to_principal_with_sessions(
            state,
            principal,
            to_scope,
            Some(&visible_session_ids),
        )
        .await?
        {
            visible.push(link);
        }
    }
    Ok(visible)
}

async fn semantic_source_scope_cache(
    state: &AppState,
) -> Result<HashMap<Uuid, ResourceScope>, AppError> {
    let sources = state.list_semantic_sources().await?;
    semantic_source_scope_cache_for_sources(state, &sources).await
}

async fn semantic_source_scope_cache_for_sources(
    state: &AppState,
    sources: &[SemanticSource],
) -> Result<HashMap<Uuid, ResourceScope>, AppError> {
    let active_workflow_pack_ids = if sources.iter().any(|source| {
        source.owner_type.as_deref().is_some_and(|owner_type| {
            owner_type
                .trim()
                .eq_ignore_ascii_case("workflow_pack_installation")
        })
    }) {
        Some(state.active_workflow_pack_installation_ids().await?)
    } else {
        None
    };
    let mut scopes = HashMap::new();
    for source in sources {
        scopes.insert(
            source.id,
            semantic_source_scope(state, source, active_workflow_pack_ids.as_ref()).await?,
        );
    }
    Ok(scopes)
}

async fn semantic_object_scope_cache(
    state: &AppState,
    source_scopes: &HashMap<Uuid, ResourceScope>,
) -> Result<HashMap<Uuid, ResourceScope>, AppError> {
    let mut scopes = HashMap::new();
    for object in state.list_semantic_objects().await? {
        scopes.insert(
            object.id,
            semantic_object_scope_with_source_scopes(state, &object, source_scopes).await?,
        );
    }
    Ok(scopes)
}

async fn resource_scope(
    state: &AppState,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<ResourceScope, AppError> {
    match resource_type {
        "agent" | "agent_inbox" => {
            let agent = state.get_agent(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: agent.team_id,
                project_id: agent.project_id,
            })
        }
        "organization" => {
            organization_exists(state, resource_id).await?;
            Ok(ResourceScope::Tenant)
        }
        "team" => team_scope(state, resource_id).await,
        "project" => project_scope(state, resource_id).await,
        "membership" => {
            let membership = state.get_membership(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: membership.team_id,
                project_id: membership.project_id,
            })
        }
        "tenant_invitation" => {
            let invitation = state.get_tenant_invitation(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: invitation.team_id,
                project_id: invitation.project_id,
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
        "tool_call" => {
            let tool_call = state.get_tool_call(resource_id).await?;
            Ok(ResourceScope::Session(tool_call.session_id))
        }
        "approval" => {
            let approval = state.get_approval(resource_id).await?;
            Ok(ResourceScope::Session(approval.session_id))
        }
        "manager_plan" | "manager_agent_plan" => {
            let plan = state.get_manager_agent_plan(resource_id).await?;
            Ok(ResourceScope::Session(plan.session_id))
        }
        "agent_handoff" | "agent_handoff_event" => {
            let handoff = state.get_agent_handoff_event(resource_id).await?;
            Ok(ResourceScope::Session(handoff.source_session_id))
        }
        "agent_handoff_assignment" => {
            let assignment = state.get_agent_handoff_assignment(resource_id).await?;
            Ok(ResourceScope::Session(assignment.source_session_id))
        }
        "memory_writeback_candidate" => {
            let candidate = state.get_memory_writeback_candidate(resource_id).await?;
            Ok(ResourceScope::Session(candidate.session_id))
        }
        "session_thread" => {
            let thread = state.get_session_thread(resource_id).await?;
            Ok(ResourceScope::Session(thread.session_id))
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
            semantic_source_scope(state, &source, None).await
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
                (ResourceScope::ScopedUnknown, _) | (_, ResourceScope::ScopedUnknown) => {
                    Ok(ResourceScope::ScopedUnknown)
                }
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
        "agent_release" => {
            let release = state
                .list_all_agent_releases()
                .await?
                .into_iter()
                .find(|release| release.id == resource_id)
                .ok_or_else(|| AppError::not_found("agent release not found"))?;
            let agent = state.get_agent(release.agent_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: agent.team_id,
                project_id: agent.project_id,
            })
        }
        "provider" | "providers" => state
            .list_providers()
            .await?
            .into_iter()
            .find(|provider| provider.id == resource_id)
            .map(|_| ResourceScope::Tenant)
            .ok_or_else(|| AppError::not_found("provider not found")),
        "mcp_server" => {
            let server = state.get_mcp_server_by_id(resource_id).await?;
            Ok(ResourceScope::TeamProject {
                team_id: Some(server.team_id),
                project_id: None,
            })
        }
        "policy_revision" => {
            state.get_policy_revision(resource_id).await?;
            Ok(ResourceScope::Tenant)
        }
        "codex_app_server" => {
            state.get_codex_app_server_run(resource_id).await?;
            Ok(ResourceScope::Tenant)
        }
        "remote_computer" => state
            .list_remote_computers()
            .await?
            .into_iter()
            .find(|computer| computer.id == resource_id)
            .map(|_| ResourceScope::Tenant)
            .ok_or_else(|| AppError::not_found("remote computer not found")),
        _ => Err(AppError::forbidden(format!(
            "resource type {} is not scoped for non-admin access",
            resource_type
        ))),
    }
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
