use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, patch, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::{AuthorizationRequest, Permission};
use crate::{
    AcceptTenantInvitation, AcceptedTenantInvitation, AppError, AppState,
    BootstrapTenantProvisioning, CreateMembership, CreateOrganization, CreateProject,
    CreateTeam, CreateTenantInvitation, Membership, Organization, Project, Team,
    TenantInvitation, TenantIsolationReadinessReport, TenantProvisioningResult,
    TransferOrganizationOwnership, authorize_request,
    accept_tenant_invitation as accept_tenant_invitation_impl,
    bootstrap_tenant_provisioning as bootstrap_tenant_provisioning_impl,
    create_membership as create_membership_impl,
    create_tenant_invitation as create_tenant_invitation_impl,
    delete_membership as delete_membership_impl,
    enforce_resource_scope,
    get_tenant_isolation_readiness as get_tenant_isolation_readiness_impl,
    list_memberships as list_memberships_impl,
    list_tenant_invitations as list_tenant_invitations_impl,
    new_audit_log, principal_from_request, revoke_tenant_invitation as revoke_tenant_invitation_impl,
    validate_tenant_production_routing as validate_tenant_production_routing_impl,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/organizations",
            get(list_organizations).post(create_organization),
        )
        .route(
            "/api/tenant-provisioning/bootstrap",
            post(bootstrap_tenant_provisioning),
        )
        .route(
            "/api/tenant-isolation/readiness",
            get(get_tenant_isolation_readiness),
        )
        .route(
            "/api/tenant-isolation/routing/validate",
            post(validate_tenant_production_routing),
        )
        .route(
            "/api/organizations/{id}",
            patch(update_organization).delete(delete_organization),
        )
        .route(
            "/api/organizations/{id}/teams",
            get(list_teams).post(create_team),
        )
        .route(
            "/api/organizations/{id}/memberships",
            get(list_memberships).post(create_membership),
        )
        .route("/api/memberships/{id}", delete(delete_membership))
        .route(
            "/api/organizations/{id}/invitations",
            get(list_tenant_invitations).post(create_tenant_invitation),
        )
        .route(
            "/api/invitations/{id}/revoke",
            post(revoke_tenant_invitation),
        )
        .route("/api/invitations/accept", post(accept_tenant_invitation))
        .route(
            "/api/organizations/{id}/archive",
            post(archive_organization),
        )
        .route(
            "/api/organizations/{id}/transfer-ownership",
            post(transfer_organization_ownership),
        )
        .route(
            "/api/teams/{id}/projects",
            get(list_projects).post(create_project),
        )
        .route("/api/teams/{id}", patch(update_team).delete(delete_team))
        .route("/api/teams/{id}/archive", post(archive_team))
        .route(
            "/api/projects/{id}",
            patch(update_project).delete(delete_project),
        )
        .route("/api/projects/{id}/archive", post(archive_project))
}

async fn list_organizations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Organization>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "organizations", None).await?;
    Ok(Json(state.list_organizations().await?))
}

async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganization>,
) -> Result<Json<Organization>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organizations".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let organization = state
        .create_organization(input, Some(principal.subject_id.clone()))
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "organization.created",
            "organization",
            Some(organization.id),
            json!({"subject": principal.subject_id, "owner_subject": organization.owner_subject}),
        ))
        .await?;
    Ok(Json(organization))
}

async fn bootstrap_tenant_provisioning(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<BootstrapTenantProvisioning>,
) -> Result<Json<TenantProvisioningResult>, AppError> {
    bootstrap_tenant_provisioning_impl(state, headers, input).await
}

async fn get_tenant_isolation_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TenantIsolationReadinessReport>, AppError> {
    get_tenant_isolation_readiness_impl(state, headers).await
}

async fn validate_tenant_production_routing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    validate_tenant_production_routing_impl(state, headers).await
}

async fn update_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganization>,
) -> Result<Json<Organization>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organization".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let organization = state.update_organization(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "organization.updated",
            "organization",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "name": organization.name,
                "slug": organization.slug
            }),
        ))
        .await?;
    Ok(Json(organization))
}

async fn archive_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organization".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let organization = state.archive_organization(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "organization.archived",
            "organization",
            Some(id),
            json!({"subject": principal.subject_id, "archived_at": organization.archived_at}),
        ))
        .await?;
    Ok(Json(organization))
}

async fn delete_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organization".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let organization = state.delete_organization(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "organization.deleted",
            "organization",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "slug": organization.slug,
                "owner_subject": organization.owner_subject
            }),
        ))
        .await?;
    Ok(Json(organization))
}

async fn transfer_organization_ownership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransferOrganizationOwnership>,
) -> Result<Json<Organization>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organization".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let previous = state
        .list_organizations()
        .await?
        .into_iter()
        .find(|organization| organization.id == id)
        .ok_or_else(|| AppError::not_found("organization not found"))?;
    let new_owner = input.owner_subject.trim();
    if new_owner.is_empty() {
        return Err(AppError::bad_request(
            "organization owner_subject is required",
        ));
    }
    let organization = state
        .transfer_organization_ownership(id, new_owner.to_string())
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "organization.ownership_transferred",
            "organization",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "previous_owner_subject": previous.owner_subject,
                "owner_subject": organization.owner_subject
            }),
        ))
        .await?;
    Ok(Json(organization))
}

async fn list_teams(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Team>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "organization",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_teams(id).await?))
}

async fn create_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTeam>,
) -> Result<Json<Team>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "organization",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_team(id, input).await?))
}

async fn update_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTeam>,
) -> Result<Json<Team>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let team = state.update_team(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "team.updated",
            "team",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "organization_id": team.organization_id,
                "name": team.name,
                "slug": team.slug
            }),
        ))
        .await?;
    Ok(Json(team))
}

async fn archive_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Team>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let team = state.archive_team(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "team.archived",
            "team",
            Some(id),
            json!({"subject": principal.subject_id, "archived_at": team.archived_at}),
        ))
        .await?;
    Ok(Json(team))
}

async fn delete_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Team>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "team".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let team = state.delete_team(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "team.deleted",
            "team",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "organization_id": team.organization_id,
                "slug": team.slug
            }),
        ))
        .await?;
    Ok(Json(team))
}

async fn list_projects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Project>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.list_projects(id).await?))
}

async fn create_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.create_project(id, input).await?))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "project".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let project = state.update_project(id, input).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "project.updated",
            "project",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "team_id": project.team_id,
                "name": project.name,
                "slug": project.slug
            }),
        ))
        .await?;
    Ok(Json(project))
}

async fn archive_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Project>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "project".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let project = state.archive_project(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "project.archived",
            "project",
            Some(id),
            json!({"subject": principal.subject_id, "archived_at": project.archived_at}),
        ))
        .await?;
    Ok(Json(project))
}

async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Project>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "project".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let project = state.delete_project(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "project.deleted",
            "project",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "team_id": project.team_id,
                "slug": project.slug
            }),
        ))
        .await?;
    Ok(Json(project))
}

async fn list_memberships(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Membership>>, AppError> {
    list_memberships_impl(state, id, headers).await
}

async fn create_membership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateMembership>,
) -> Result<Json<Membership>, AppError> {
    create_membership_impl(state, id, headers, input).await
}

async fn delete_membership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Membership>, AppError> {
    delete_membership_impl(state, id, headers).await
}

async fn list_tenant_invitations(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<TenantInvitation>>, AppError> {
    list_tenant_invitations_impl(state, id, headers).await
}

async fn create_tenant_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTenantInvitation>,
) -> Result<Json<TenantInvitation>, AppError> {
    create_tenant_invitation_impl(state, id, headers, input).await
}

async fn revoke_tenant_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TenantInvitation>, AppError> {
    revoke_tenant_invitation_impl(state, id, headers).await
}

async fn accept_tenant_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AcceptTenantInvitation>,
) -> Result<Json<AcceptedTenantInvitation>, AppError> {
    accept_tenant_invitation_impl(state, headers, input).await
}
