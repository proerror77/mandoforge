use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    membership_from_row, organization_from_row, project_from_row, team_from_row,
};
use crate::{
    AppError, AppState, CreateMembership, CreateOrganization, CreateProject, CreateTeam,
    Membership, Organization, Project, Team,
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
                    "SELECT id, user_id, organization_id, team_id, role, created_at
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
        let membership = Membership {
            id: Uuid::new_v4(),
            user_id: input.user_id,
            organization_id: Some(organization_id),
            team_id: input.team_id,
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
                    "INSERT INTO memberships (id, tenant_id, user_id, organization_id, team_id, role, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(membership.id)
                .bind(self.tenant_id)
                .bind(&membership.user_id)
                .bind(membership.organization_id)
                .bind(membership.team_id)
                .bind(&membership.role)
                .bind(membership.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(membership)
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
}
