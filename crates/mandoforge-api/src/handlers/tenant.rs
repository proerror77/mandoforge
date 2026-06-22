use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, patch, post},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AcceptTenantInvitation, AcceptedTenantInvitation, AppError, AppState,
    BootstrapTenantProvisioning, CreateMembership, CreateOrganization, CreateProject,
    CreateTeam, CreateTenantInvitation, Membership, Organization, Project, Team,
    TenantInvitation, TenantIsolationReadinessReport, TenantProvisioningResult,
    TransferOrganizationOwnership,
    accept_tenant_invitation as accept_tenant_invitation_impl,
    archive_organization as archive_organization_impl,
    archive_project as archive_project_impl, archive_team as archive_team_impl,
    bootstrap_tenant_provisioning as bootstrap_tenant_provisioning_impl,
    create_membership as create_membership_impl,
    create_organization as create_organization_impl, create_project as create_project_impl,
    create_team as create_team_impl,
    create_tenant_invitation as create_tenant_invitation_impl,
    delete_membership as delete_membership_impl,
    delete_organization as delete_organization_impl, delete_project as delete_project_impl,
    delete_team as delete_team_impl,
    get_tenant_isolation_readiness as get_tenant_isolation_readiness_impl,
    list_memberships as list_memberships_impl, list_organizations as list_organizations_impl,
    list_projects as list_projects_impl, list_teams as list_teams_impl,
    list_tenant_invitations as list_tenant_invitations_impl,
    revoke_tenant_invitation as revoke_tenant_invitation_impl,
    transfer_organization_ownership as transfer_organization_ownership_impl,
    update_organization as update_organization_impl, update_project as update_project_impl,
    update_team as update_team_impl,
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
    list_organizations_impl(state, headers).await
}

async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateOrganization>,
) -> Result<Json<Organization>, AppError> {
    create_organization_impl(state, headers, input).await
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
    update_organization_impl(state, id, headers, input).await
}

async fn archive_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    archive_organization_impl(state, id, headers).await
}

async fn delete_organization(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Organization>, AppError> {
    delete_organization_impl(state, id, headers).await
}

async fn transfer_organization_ownership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<TransferOrganizationOwnership>,
) -> Result<Json<Organization>, AppError> {
    transfer_organization_ownership_impl(state, id, headers, input).await
}

async fn list_teams(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Team>>, AppError> {
    list_teams_impl(state, id, headers).await
}

async fn create_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTeam>,
) -> Result<Json<Team>, AppError> {
    create_team_impl(state, id, headers, input).await
}

async fn update_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTeam>,
) -> Result<Json<Team>, AppError> {
    update_team_impl(state, id, headers, input).await
}

async fn archive_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Team>, AppError> {
    archive_team_impl(state, id, headers).await
}

async fn delete_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Team>, AppError> {
    delete_team_impl(state, id, headers).await
}

async fn list_projects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Project>>, AppError> {
    list_projects_impl(state, id, headers).await
}

async fn create_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    create_project_impl(state, id, headers, input).await
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    update_project_impl(state, id, headers, input).await
}

async fn archive_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Project>, AppError> {
    archive_project_impl(state, id, headers).await
}

async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Project>, AppError> {
    delete_project_impl(state, id, headers).await
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
