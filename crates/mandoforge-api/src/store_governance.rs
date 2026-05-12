use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    membership_from_row, organization_from_row, project_from_row, provider_access_from_row,
    team_from_row,
};
use crate::{
    AppError, AppState, CreateMembership, CreateOrganization, CreateProject, CreateProviderAccess,
    CreateTeam, Membership, Organization, Project, ProviderAccess, Role, Team,
};

impl AppState {
    pub(crate) async fn list_organizations(&self) -> Result<Vec<Organization>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut organizations: Vec<_> =
                    inner.read().await.organizations.values().cloned().collect();
                organizations.sort_by_key(|organization| organization.created_at);
                organizations.reverse();
                Ok(organizations)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, slug, created_at
                     FROM organizations
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(organization_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_organization(
        &self,
        input: CreateOrganization,
    ) -> Result<Organization, AppError> {
        let organization = Organization {
            id: Uuid::new_v4(),
            name: input.name,
            slug: input.slug,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .organizations
                    .insert(organization.id, organization.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO organizations (id, tenant_id, name, slug, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(organization.id)
                .bind(self.tenant_id)
                .bind(&organization.name)
                .bind(&organization.slug)
                .bind(organization.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(organization)
    }

    pub(crate) async fn list_teams(&self, organization_id: Uuid) -> Result<Vec<Team>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut teams: Vec<_> = inner
                    .read()
                    .await
                    .teams
                    .values()
                    .filter(|team| team.organization_id == organization_id)
                    .cloned()
                    .collect();
                teams.sort_by_key(|team| team.created_at);
                teams.reverse();
                Ok(teams)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, organization_id, name, slug, created_at
                     FROM teams
                     WHERE tenant_id = $1 AND organization_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(organization_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(team_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_team(
        &self,
        organization_id: Uuid,
        input: CreateTeam,
    ) -> Result<Team, AppError> {
        self.ensure_organization_exists(organization_id).await?;
        let team = Team {
            id: Uuid::new_v4(),
            organization_id,
            name: input.name,
            slug: input.slug,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.teams.insert(team.id, team.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO teams (id, tenant_id, organization_id, name, slug, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(team.id)
                .bind(self.tenant_id)
                .bind(team.organization_id)
                .bind(&team.name)
                .bind(&team.slug)
                .bind(team.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(team)
    }

    pub(crate) async fn list_projects(&self, team_id: Uuid) -> Result<Vec<Project>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut projects: Vec<_> = inner
                    .read()
                    .await
                    .projects
                    .values()
                    .filter(|project| project.team_id == team_id)
                    .cloned()
                    .collect();
                projects.sort_by_key(|project| project.created_at);
                projects.reverse();
                Ok(projects)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, team_id, name, slug, created_at
                     FROM projects
                     WHERE tenant_id = $1 AND team_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(team_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(project_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_project(
        &self,
        team_id: Uuid,
        input: CreateProject,
    ) -> Result<Project, AppError> {
        self.ensure_team_exists(team_id).await?;
        let project = Project {
            id: Uuid::new_v4(),
            team_id,
            name: input.name,
            slug: input.slug,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .projects
                    .insert(project.id, project.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO projects (id, tenant_id, team_id, name, slug, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(project.id)
                .bind(self.tenant_id)
                .bind(project.team_id)
                .bind(&project.name)
                .bind(&project.slug)
                .bind(project.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(project)
    }

    pub(crate) async fn list_memberships(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<Membership>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut memberships: Vec<_> = inner
                    .read()
                    .await
                    .memberships
                    .values()
                    .filter(|membership| membership.organization_id == Some(organization_id))
                    .cloned()
                    .collect();
                memberships.sort_by_key(|membership| membership.created_at);
                memberships.reverse();
                Ok(memberships)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, user_id, organization_id, team_id, project_id, role, created_at
                     FROM memberships
                     WHERE tenant_id = $1 AND organization_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(organization_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(membership_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_membership(
        &self,
        organization_id: Uuid,
        input: CreateMembership,
    ) -> Result<Membership, AppError> {
        self.ensure_organization_exists(organization_id).await?;
        if let Some(team_id) = input.team_id {
            self.ensure_team_exists(team_id).await?;
        }
        if let Some(project_id) = input.project_id {
            let Some(team_id) = input.team_id else {
                return Err(AppError::bad_request(
                    "project_id requires a matching team_id on memberships",
                ));
            };
            self.ensure_project_belongs_to_team(project_id, team_id)
                .await?;
        }
        let membership = Membership {
            id: Uuid::new_v4(),
            user_id: input.user_id,
            organization_id: Some(organization_id),
            team_id: input.team_id,
            project_id: input.project_id,
            role: input.role,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .memberships
                    .insert(membership.id, membership.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO memberships (id, tenant_id, user_id, organization_id, team_id, project_id, role, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(membership.id)
                .bind(self.tenant_id)
                .bind(&membership.user_id)
                .bind(membership.organization_id)
                .bind(membership.team_id)
                .bind(membership.project_id)
                .bind(&membership.role)
                .bind(membership.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(membership)
    }

    pub(crate) async fn membership_roles_for_subject(
        &self,
        subject_id: &str,
    ) -> Result<Vec<Role>, AppError> {
        let role_names: Vec<String> = match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .memberships
                .values()
                .filter(|membership| membership.user_id == subject_id)
                .map(|membership| membership.role.clone())
                .collect(),
            StoreBackend::Postgres(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT role
                     FROM memberships
                     WHERE tenant_id = $1 AND user_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(subject_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(|row| row.0).collect()
            }
        };
        let mut roles = Vec::new();
        for role_name in role_names {
            let role = membership_role_from_str(&role_name)?;
            if !roles.contains(&role) {
                roles.push(role);
            }
        }
        Ok(roles)
    }

    pub(crate) async fn subject_can_access_team(
        &self,
        subject_id: &str,
        team_id: Uuid,
    ) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let Some(team) = store.teams.get(&team_id) else {
                    return Ok(false);
                };
                Ok(store.memberships.values().any(|membership| {
                    membership.user_id == subject_id
                        && ((membership.team_id == Some(team_id)
                            && membership.project_id.is_none())
                            || (membership.team_id.is_none()
                                && membership.project_id.is_none()
                                && membership.organization_id == Some(team.organization_id)))
                }))
            }
            StoreBackend::Postgres(pool) => {
                let can_access: bool = sqlx::query_scalar(
                    "SELECT EXISTS(
                        SELECT 1
                        FROM memberships m
                        JOIN teams t ON t.id = $3 AND t.tenant_id = $1
                        WHERE m.tenant_id = $1
                          AND m.user_id = $2
                          AND (
                            (m.team_id = $3 AND m.project_id IS NULL)
                            OR (m.team_id IS NULL AND m.project_id IS NULL AND m.organization_id = t.organization_id)
                          )
                    )",
                )
                .bind(self.tenant_id)
                .bind(subject_id)
                .bind(team_id)
                .fetch_one(pool)
                .await?;
                Ok(can_access)
            }
        }
    }

    pub(crate) async fn subject_can_access_project(
        &self,
        subject_id: &str,
        project_id: Uuid,
    ) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let Some(project) = store.projects.get(&project_id) else {
                    return Ok(false);
                };
                let Some(team) = store.teams.get(&project.team_id) else {
                    return Ok(false);
                };
                Ok(store.memberships.values().any(|membership| {
                    membership.user_id == subject_id
                        && (membership.project_id == Some(project_id)
                            || (membership.project_id.is_none()
                                && membership.team_id == Some(project.team_id))
                            || (membership.project_id.is_none()
                                && membership.team_id.is_none()
                                && membership.organization_id == Some(team.organization_id)))
                }))
            }
            StoreBackend::Postgres(pool) => {
                let can_access: bool = sqlx::query_scalar(
                    "SELECT EXISTS(
                        SELECT 1
                        FROM memberships m
                        JOIN projects p ON p.id = $3 AND p.tenant_id = $1
                        JOIN teams t ON t.id = p.team_id AND t.tenant_id = $1
                        WHERE m.tenant_id = $1
                          AND m.user_id = $2
                          AND (
                            m.project_id = $3
                            OR (m.project_id IS NULL AND m.team_id = p.team_id)
                            OR (m.project_id IS NULL AND m.team_id IS NULL AND m.organization_id = t.organization_id)
                          )
                    )",
                )
                .bind(self.tenant_id)
                .bind(subject_id)
                .bind(project_id)
                .fetch_one(pool)
                .await?;
                Ok(can_access)
            }
        }
    }

    async fn ensure_organization_exists(&self, organization_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                if inner
                    .read()
                    .await
                    .organizations
                    .contains_key(&organization_id)
                {
                    Ok(())
                } else {
                    Err(AppError::not_found("organization not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1 FROM organizations WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(organization_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("organization not found"))
            }
        }
    }

    async fn ensure_team_exists(&self, team_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                if inner.read().await.teams.contains_key(&team_id) {
                    Ok(())
                } else {
                    Err(AppError::not_found("team not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> =
                    sqlx::query_scalar("SELECT 1 FROM teams WHERE tenant_id = $1 AND id = $2")
                        .bind(self.tenant_id)
                        .bind(team_id)
                        .fetch_optional(pool)
                        .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("team not found"))
            }
        }
    }

    pub(crate) async fn ensure_project_belongs_to_team(
        &self,
        project_id: Uuid,
        team_id: Uuid,
    ) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let belongs = inner
                    .read()
                    .await
                    .projects
                    .get(&project_id)
                    .is_some_and(|project| project.team_id == team_id);
                if belongs {
                    Ok(())
                } else {
                    Err(AppError::not_found("project not found for team"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1 FROM projects WHERE tenant_id = $1 AND id = $2 AND team_id = $3",
                )
                .bind(self.tenant_id)
                .bind(project_id)
                .bind(team_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("project not found for team"))
            }
        }
    }

    pub(crate) async fn list_provider_access(
        &self,
        team_id: Uuid,
    ) -> Result<Vec<ProviderAccess>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut access: Vec<_> = inner
                    .read()
                    .await
                    .provider_access
                    .values()
                    .filter(|access| access.team_id == team_id)
                    .cloned()
                    .collect();
                access.sort_by_key(|access| access.created_at);
                access.reverse();
                Ok(access)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, team_id, provider_name, model_allowlist, status, created_at
                     FROM provider_access
                     WHERE tenant_id = $1 AND team_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(team_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(provider_access_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_provider_access(
        &self,
        team_id: Uuid,
        input: CreateProviderAccess,
    ) -> Result<ProviderAccess, AppError> {
        self.ensure_team_exists(team_id).await?;
        let provider_access = ProviderAccess {
            id: Uuid::new_v4(),
            team_id,
            provider_name: input.provider_name,
            model_allowlist: input.model_allowlist,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .provider_access
                    .insert(provider_access.id, provider_access.clone());
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO provider_access (id, tenant_id, team_id, provider_name, model_allowlist, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT (team_id, provider_name)
                     DO UPDATE SET model_allowlist = EXCLUDED.model_allowlist, status = EXCLUDED.status
                     RETURNING id, team_id, provider_name, model_allowlist, status, created_at",
                )
                .bind(provider_access.id)
                .bind(self.tenant_id)
                .bind(provider_access.team_id)
                .bind(&provider_access.provider_name)
                .bind(serde_json::json!(provider_access.model_allowlist))
                .bind(&provider_access.status)
                .bind(provider_access.created_at)
                .fetch_one(pool)
                .await?;
                return provider_access_from_row(row);
            }
        }
        Ok(provider_access)
    }

    pub(crate) async fn ensure_provider_model_allowed(
        &self,
        team_id: Uuid,
        provider_name: &str,
        model: &str,
    ) -> Result<(), AppError> {
        let entries = self.list_provider_access(team_id).await?;
        let Some(access) = entries
            .iter()
            .find(|entry| entry.provider_name == provider_name && entry.status == "active")
        else {
            return Err(AppError::forbidden(format!(
                "team is not allowed to use provider {provider_name}"
            )));
        };
        if access
            .model_allowlist
            .iter()
            .any(|allowed| allowed == model)
        {
            Ok(())
        } else {
            Err(AppError::forbidden(format!(
                "team is not allowed to use model {model} for provider {provider_name}"
            )))
        }
    }
}

fn membership_role_from_str(role: &str) -> Result<Role, AppError> {
    match role.trim() {
        "admin" => Ok(Role::Admin),
        "operator" => Ok(Role::Operator),
        "approver" => Ok(Role::Approver),
        "viewer" => Ok(Role::Viewer),
        other => Err(AppError::bad_request(format!(
            "unsupported membership role value: {other}"
        ))),
    }
}
