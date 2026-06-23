use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::{AuthorizationRequest, Permission};
use crate::{
    AcceptTenantInvitation, AcceptedTenantInvitation, AppError, AppState,
    BootstrapTenantProvisioning, CreateMembership, CreateOrganization, CreateProject, CreateTeam,
    CreateTenantInvitation, Membership, Organization, Project, Team, TenantInvitation,
    TenantIsolationReadinessReport, TenantProvisioningResult, TransferOrganizationOwnership,
    authorize_request, build_tenant_isolation_readiness, enforce_resource_scope,
    execute_tenant_production_routing_controller, new_audit_log, optional_trimmed,
    principal_from_request, required_trimmed, subject_from_headers,
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
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "tenant_provisioning".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let owner_subject = required_trimmed(&input.owner_subject, "owner_subject")?;
    let organization_name = required_trimmed(&input.organization_name, "organization_name")?;
    let organization_slug = required_trimmed(&input.organization_slug, "organization_slug")?;
    let team_parts = match (
        optional_trimmed(input.team_name.as_deref()),
        optional_trimmed(input.team_slug.as_deref()),
    ) {
        (Some(name), Some(slug)) => Some((name, slug)),
        (None, None) => None,
        _ => {
            return Err(AppError::bad_request(
                "team_name and team_slug must be provided together",
            ));
        }
    };
    let project_parts = match (
        optional_trimmed(input.project_name.as_deref()),
        optional_trimmed(input.project_slug.as_deref()),
    ) {
        (Some(name), Some(slug)) => {
            if team_parts.is_none() {
                return Err(AppError::bad_request(
                    "project provisioning requires team_name and team_slug",
                ));
            }
            Some((name, slug))
        }
        (None, None) => None,
        _ => {
            return Err(AppError::bad_request(
                "project_name and project_slug must be provided together",
            ));
        }
    };
    let organization = state
        .create_organization(
            CreateOrganization {
                name: organization_name,
                slug: organization_slug,
            },
            Some(owner_subject.clone()),
        )
        .await?;
    let team = match team_parts {
        Some((name, slug)) => Some(
            state
                .create_team(organization.id, CreateTeam { name, slug })
                .await?,
        ),
        None => None,
    };
    let project = match project_parts {
        Some((name, slug)) => {
            let team = team.as_ref().expect("project parts require team parts");
            Some(
                state
                    .create_project(team.id, CreateProject { name, slug })
                    .await?,
            )
        }
        None => None,
    };
    let owner_membership = state
        .create_membership(
            organization.id,
            CreateMembership {
                user_id: owner_subject.clone(),
                team_id: team.as_ref().map(|team| team.id),
                project_id: project.as_ref().map(|project| project.id),
                role: input.owner_role.trim().to_string(),
            },
        )
        .await?;
    let result = TenantProvisioningResult {
        organization,
        team,
        project,
        owner_membership,
    };
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "tenant.provisioned",
            "tenant_provisioning",
            Some(result.organization.id),
            json!({
                "subject": principal.subject_id,
                "organization_id": result.organization.id,
                "team_id": result.team.as_ref().map(|team| team.id),
                "project_id": result.project.as_ref().map(|project| project.id),
                "owner_subject": owner_subject,
                "owner_membership_id": result.owner_membership.id
            }),
        ))
        .await?;
    Ok(Json(result))
}

async fn get_tenant_isolation_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TenantIsolationReadinessReport>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "tenant_isolation",
        None,
    )
    .await?;
    Ok(Json(build_tenant_isolation_readiness(&state).await?))
}

async fn validate_tenant_production_routing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "tenant_isolation".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let readiness = build_tenant_isolation_readiness(&state).await?;
    let checked_at = Utc::now();
    let execution = execute_tenant_production_routing_controller(
        &|key| std::env::var(key).ok(),
        Some(principal.subject_id.as_str()),
        checked_at,
        &readiness,
    )
    .await?;
    let status = execution
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed")
        .to_string();
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "tenant.production_routing_validation_run",
            "tenant_isolation",
            None,
            json!({
                "subject": principal.subject_id,
                "status": status,
                "controller_configured": true,
                "controller_execution": execution,
                "runtime_tenant_mode": readiness.runtime_tenant_mode,
                "rls_status": readiness.rls.status,
                "checked_at": checked_at,
            }),
        ))
        .await?;
    Ok(Json(json!({
        "status": status,
        "checked_at": checked_at,
        "controller_configured": true,
        "controller_execution": execution,
        "readiness_status": readiness.status,
        "production_routing_status": readiness.production_routing.status,
    })))
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
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "organization",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_memberships(id).await?))
}

async fn create_membership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateMembership>,
) -> Result<Json<Membership>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "organization",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_membership(id, input).await?))
}

async fn delete_membership(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Membership>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "membership".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let membership = state.delete_membership(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "membership.deleted",
            "membership",
            Some(id),
            json!({
                "subject": principal.subject_id,
                "user_id": membership.user_id,
                "organization_id": membership.organization_id,
                "team_id": membership.team_id,
                "project_id": membership.project_id,
                "role": membership.role
            }),
        ))
        .await?;
    Ok(Json(membership))
}

async fn list_tenant_invitations(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<TenantInvitation>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "organization",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_tenant_invitations(id).await?))
}

async fn create_tenant_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTenantInvitation>,
) -> Result<Json<TenantInvitation>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "organization".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let invitation = state
        .create_tenant_invitation(id, input, principal.subject_id.clone())
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "tenant.invitation_created",
            "tenant_invitation",
            Some(invitation.id),
            json!({
                "subject": principal.subject_id,
                "organization_id": invitation.organization_id,
                "team_id": invitation.team_id,
                "project_id": invitation.project_id,
                "email": invitation.email,
                "role": invitation.role,
                "expires_at": invitation.expires_at
            }),
        ))
        .await?;
    Ok(Json(invitation))
}

async fn revoke_tenant_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TenantInvitation>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.current_tenant_id(),
        permission: Permission::Admin,
        resource_type: "tenant_invitation".to_string(),
        resource_id: Some(id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(&state, &principal, &request).await?;
    let invitation = state.revoke_tenant_invitation(id).await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "tenant.invitation_revoked",
            "tenant_invitation",
            Some(invitation.id),
            json!({
                "subject": principal.subject_id,
                "organization_id": invitation.organization_id,
                "email": invitation.email,
                "decided_at": invitation.decided_at
            }),
        ))
        .await?;
    Ok(Json(invitation))
}

async fn accept_tenant_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AcceptTenantInvitation>,
) -> Result<Json<AcceptedTenantInvitation>, AppError> {
    let subject_id = subject_from_headers(&headers)?;
    let invitation = state.tenant_invitation_by_token(input.token.trim()).await?;
    if invitation.status != "pending" {
        return Err(AppError::bad_request("tenant invitation is not pending"));
    }
    if Utc::now() > invitation.expires_at {
        let expired = state.expire_tenant_invitation(invitation.id).await?;
        state
            .append_audit_log(new_audit_log(
                None,
                "system",
                None,
                "tenant.invitation_expired",
                "tenant_invitation",
                Some(expired.id),
                json!({
                    "organization_id": expired.organization_id,
                    "email": expired.email,
                    "expires_at": expired.expires_at
                }),
            ))
            .await?;
        return Err(AppError::bad_request("tenant invitation has expired"));
    }
    let membership = state
        .create_membership(
            invitation.organization_id,
            CreateMembership {
                user_id: subject_id.clone(),
                team_id: invitation.team_id,
                project_id: invitation.project_id,
                role: invitation.role.clone(),
            },
        )
        .await?;
    let invitation = state
        .mark_tenant_invitation_accepted(invitation.id, subject_id.clone())
        .await?;
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "tenant.invitation_accepted",
            "tenant_invitation",
            Some(invitation.id),
            json!({
                "subject": subject_id,
                "organization_id": invitation.organization_id,
                "team_id": invitation.team_id,
                "project_id": invitation.project_id,
                "membership_id": membership.id,
                "role": membership.role
            }),
        ))
        .await?;
    Ok(Json(AcceptedTenantInvitation {
        invitation,
        membership,
    }))
}
