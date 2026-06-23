use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{
    agent_teammate_from_row, mcp_server_from_row, membership_from_row, organization_from_row,
    project_from_row, provider_access_from_row, provider_record_from_row, squad_from_row,
    squad_member_from_row, team_from_row, tenant_invitation_from_row,
    work_item_activity_entry_from_row, work_item_assignment_from_row, work_item_from_row,
    work_item_review_from_row,
};
use crate::{
    AgentTeammate, AppError, AppState, CreateAgentTeammate, CreateMcpServerRecord,
    CreateMembership, CreateOrganization, CreateProject, CreateProviderAccess,
    CreateProviderRecord, CreateSquad, CreateSquadMember, CreateTeam, CreateTenantInvitation,
    CreateWorkItem, CreateWorkItemAssignment, CreateWorkItemReview, McpServerRecord, Membership,
    Organization, Project, ProviderAccess, ProviderRecord, Role, Squad, SquadMember, Team,
    TenantInvitation, UpdateMcpServerRecord, UpdateProviderAccess, WorkItem, WorkItemActivityEntry,
    WorkItemAssignment, WorkItemReview,
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
                    "SELECT id, name, slug, owner_subject, created_at, archived_at
                     FROM organizations
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(organization_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_organization(
        &self,
        input: CreateOrganization,
        owner_subject: Option<String>,
    ) -> Result<Organization, AppError> {
        let organization = Organization {
            id: Uuid::new_v4(),
            name: input.name,
            slug: input.slug,
            owner_subject,
            created_at: Utc::now(),
            archived_at: None,
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
                    "INSERT INTO organizations (id, tenant_id, name, slug, owner_subject, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(organization.id)
                .bind(self.current_tenant_id())
                .bind(&organization.name)
                .bind(&organization.slug)
                .bind(&organization.owner_subject)
                .bind(organization.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(organization)
    }

    pub(crate) async fn update_organization(
        &self,
        organization_id: Uuid,
        input: CreateOrganization,
    ) -> Result<Organization, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let organization = store
                    .organizations
                    .get_mut(&organization_id)
                    .filter(|organization| organization.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("active organization not found"))?;
                organization.name = input.name;
                organization.slug = input.slug;
                Ok(organization.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE organizations
                     SET name = $1, slug = $2
                     WHERE tenant_id = $3 AND id = $4 AND archived_at IS NULL
                     RETURNING id, name, slug, owner_subject, created_at, archived_at",
                )
                .bind(input.name)
                .bind(input.slug)
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("active organization not found"))?;
                organization_from_row(row)
            }
        }
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
                    "SELECT id, organization_id, name, slug, created_at, archived_at
                     FROM teams
                     WHERE tenant_id = $1 AND organization_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
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
            archived_at: None,
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
                .bind(self.current_tenant_id())
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

    pub(crate) async fn update_team(
        &self,
        team_id: Uuid,
        input: CreateTeam,
    ) -> Result<Team, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let organization_id = store
                    .teams
                    .get(&team_id)
                    .map(|team| team.organization_id)
                    .ok_or_else(|| AppError::not_found("active team not found"))?;
                if store
                    .organizations
                    .get(&organization_id)
                    .is_none_or(|organization| organization.archived_at.is_some())
                {
                    return Err(AppError::not_found("active team not found"));
                }
                let team = store
                    .teams
                    .get_mut(&team_id)
                    .filter(|team| team.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("active team not found"))?;
                team.name = input.name;
                team.slug = input.slug;
                Ok(team.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE teams
                     SET name = $1, slug = $2
                     WHERE tenant_id = $3
                       AND id = $4
                       AND archived_at IS NULL
                       AND EXISTS (
                           SELECT 1 FROM organizations
                           WHERE organizations.tenant_id = $3
                             AND organizations.id = teams.organization_id
                             AND organizations.archived_at IS NULL
                       )
                     RETURNING id, organization_id, name, slug, created_at, archived_at",
                )
                .bind(input.name)
                .bind(input.slug)
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("active team not found"))?;
                team_from_row(row)
            }
        }
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
                    "SELECT id, team_id, name, slug, created_at, archived_at
                     FROM projects
                     WHERE tenant_id = $1 AND team_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
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
            archived_at: None,
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
                .bind(self.current_tenant_id())
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

    pub(crate) async fn update_project(
        &self,
        project_id: Uuid,
        input: CreateProject,
    ) -> Result<Project, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let team_id = store
                    .projects
                    .get(&project_id)
                    .map(|project| project.team_id)
                    .ok_or_else(|| AppError::not_found("project not found for team"))?;
                let active_team = store.teams.get(&team_id).is_some_and(|team| {
                    team.archived_at.is_none()
                        && store
                            .organizations
                            .get(&team.organization_id)
                            .is_some_and(|organization| organization.archived_at.is_none())
                });
                if !active_team {
                    return Err(AppError::not_found("project not found for team"));
                }
                let project = store
                    .projects
                    .get_mut(&project_id)
                    .filter(|project| project.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("project not found for team"))?;
                project.name = input.name;
                project.slug = input.slug;
                Ok(project.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE projects
                     SET name = $1, slug = $2
                     WHERE tenant_id = $3
                       AND id = $4
                       AND archived_at IS NULL
                       AND EXISTS (
                           SELECT 1 FROM teams
                           JOIN organizations ON organizations.id = teams.organization_id
                           WHERE teams.tenant_id = $3
                             AND organizations.tenant_id = $3
                             AND teams.id = projects.team_id
                             AND teams.archived_at IS NULL
                             AND organizations.archived_at IS NULL
                       )
                     RETURNING id, team_id, name, slug, created_at, archived_at",
                )
                .bind(input.name)
                .bind(input.slug)
                .bind(self.current_tenant_id())
                .bind(project_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("project not found for team"))?;
                project_from_row(row)
            }
        }
    }

    pub(crate) async fn list_agent_teammates(&self) -> Result<Vec<AgentTeammate>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut teammates: Vec<_> = inner
                    .read()
                    .await
                    .agent_teammates
                    .values()
                    .cloned()
                    .collect();
                teammates.sort_by_key(|teammate| teammate.created_at);
                teammates.reverse();
                Ok(teammates)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, agent_id, display_name, handle, role, status, metadata, created_at,
                            updated_at, archived_at
                     FROM agent_teammates
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(agent_teammate_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_agent_teammate(
        &self,
        input: CreateAgentTeammate,
    ) -> Result<AgentTeammate, AppError> {
        if let Some(agent_id) = input.agent_id {
            self.get_agent(agent_id).await?;
        }
        let display_name = required_text(input.display_name, "agent teammate display_name")?;
        let role = required_text(input.role, "agent teammate role")?;
        let status = normalize_collaboration_record_status(&input.status)?;
        let now = Utc::now();
        let teammate = AgentTeammate {
            id: Uuid::new_v4(),
            agent_id: input.agent_id,
            display_name,
            handle: input.handle.and_then(optional_text),
            role,
            status,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .agent_teammates
                    .insert(teammate.id, teammate.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agent_teammates
                         (id, tenant_id, agent_id, display_name, handle, role, status, metadata,
                          created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(teammate.id)
                .bind(self.current_tenant_id())
                .bind(teammate.agent_id)
                .bind(&teammate.display_name)
                .bind(&teammate.handle)
                .bind(&teammate.role)
                .bind(&teammate.status)
                .bind(&teammate.metadata)
                .bind(teammate.created_at)
                .bind(teammate.updated_at)
                .bind(teammate.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(teammate)
    }

    pub(crate) async fn list_squads(&self) -> Result<Vec<Squad>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut squads: Vec<_> = inner.read().await.squads.values().cloned().collect();
                squads.sort_by_key(|squad| squad.created_at);
                squads.reverse();
                Ok(squads)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, purpose, status, metadata, created_at, updated_at,
                            archived_at
                     FROM squads
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(squad_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_squad(&self, input: CreateSquad) -> Result<Squad, AppError> {
        let name = required_text(input.name, "squad name")?;
        let status = normalize_collaboration_record_status(&input.status)?;
        let now = Utc::now();
        let squad = Squad {
            id: Uuid::new_v4(),
            name,
            purpose: input.purpose.and_then(optional_text),
            status,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.squads.insert(squad.id, squad.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO squads
                         (id, tenant_id, name, purpose, status, metadata, created_at, updated_at,
                          archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(squad.id)
                .bind(self.current_tenant_id())
                .bind(&squad.name)
                .bind(&squad.purpose)
                .bind(&squad.status)
                .bind(&squad.metadata)
                .bind(squad.created_at)
                .bind(squad.updated_at)
                .bind(squad.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(squad)
    }

    pub(crate) async fn list_squad_members(
        &self,
        squad_id: Uuid,
    ) -> Result<Vec<SquadMember>, AppError> {
        self.ensure_squad_exists(squad_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut members: Vec<_> = inner
                    .read()
                    .await
                    .squad_members
                    .values()
                    .filter(|member| member.squad_id == squad_id)
                    .cloned()
                    .collect();
                members.sort_by_key(|member| member.created_at);
                Ok(members)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, squad_id, teammate_id, role, status, metadata, created_at,
                            updated_at, archived_at
                     FROM squad_members
                     WHERE tenant_id = $1 AND squad_id = $2
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(squad_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(squad_member_from_row).collect()
            }
        }
    }

    pub(crate) async fn add_squad_member(
        &self,
        squad_id: Uuid,
        input: CreateSquadMember,
    ) -> Result<SquadMember, AppError> {
        self.ensure_squad_exists(squad_id).await?;
        self.ensure_agent_teammate_exists(input.teammate_id).await?;
        let role = required_text(input.role, "squad member role")?;
        let status = normalize_collaboration_record_status(&input.status)?;
        let now = Utc::now();
        let member = SquadMember {
            id: Uuid::new_v4(),
            squad_id,
            teammate_id: input.teammate_id,
            role,
            status,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .squad_members
                    .insert(member.id, member.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO squad_members
                         (id, tenant_id, squad_id, teammate_id, role, status, metadata,
                          created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(member.id)
                .bind(self.current_tenant_id())
                .bind(member.squad_id)
                .bind(member.teammate_id)
                .bind(&member.role)
                .bind(&member.status)
                .bind(&member.metadata)
                .bind(member.created_at)
                .bind(member.updated_at)
                .bind(member.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(member)
    }

    pub(crate) async fn list_work_items(&self) -> Result<Vec<WorkItem>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut work_items: Vec<_> =
                    inner.read().await.work_items.values().cloned().collect();
                work_items.sort_by_key(|work_item| work_item.created_at);
                work_items.reverse();
                Ok(work_items)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, organization_id, team_id, project_id, title, description, source,
                            source_url, status, priority, assignee, metadata, created_at, updated_at,
                            archived_at
                     FROM work_items
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(work_item_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_work_item(&self, work_item_id: Uuid) -> Result<WorkItem, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .work_items
                .get(&work_item_id)
                .filter(|work_item| work_item.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("active work item not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, organization_id, team_id, project_id, title, description, source,
                            source_url, status, priority, assignee, metadata, created_at, updated_at,
                            archived_at
                     FROM work_items
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(work_item_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("active work item not found"))?;
                work_item_from_row(row)
            }
        }
    }

    pub(crate) async fn create_work_item(
        &self,
        input: CreateWorkItem,
    ) -> Result<WorkItem, AppError> {
        self.validate_work_item_scope(&input).await?;
        let title = required_text(input.title, "work item title")?;
        let source = required_text(input.source, "work item source")?;
        let status = normalize_work_item_status(&input.status)?;
        let priority = normalize_work_item_priority(&input.priority)?;
        let now = Utc::now();
        let work_item = WorkItem {
            id: Uuid::new_v4(),
            organization_id: input.organization_id,
            team_id: input.team_id,
            project_id: input.project_id,
            title,
            description: input.description.and_then(optional_text),
            source,
            source_url: input.source_url.and_then(optional_text),
            status,
            priority,
            assignee: input.assignee.and_then(optional_text),
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .work_items
                    .insert(work_item.id, work_item.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO work_items
                         (id, tenant_id, organization_id, team_id, project_id, title, description,
                          source, source_url, status, priority, assignee, metadata, created_at,
                          updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
                )
                .bind(work_item.id)
                .bind(self.current_tenant_id())
                .bind(work_item.organization_id)
                .bind(work_item.team_id)
                .bind(work_item.project_id)
                .bind(&work_item.title)
                .bind(&work_item.description)
                .bind(&work_item.source)
                .bind(&work_item.source_url)
                .bind(&work_item.status)
                .bind(&work_item.priority)
                .bind(&work_item.assignee)
                .bind(&work_item.metadata)
                .bind(work_item.created_at)
                .bind(work_item.updated_at)
                .bind(work_item.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(work_item)
    }

    pub(crate) async fn list_work_item_activity(
        &self,
        work_item_id: Uuid,
    ) -> Result<Vec<WorkItemActivityEntry>, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut entries: Vec<_> = inner
                    .read()
                    .await
                    .work_item_activity_entries
                    .values()
                    .filter(|entry| entry.work_item_id == work_item_id)
                    .cloned()
                    .collect();
                entries.sort_by_key(|entry| entry.created_at);
                Ok(entries)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, work_item_id, event_type, actor_subject, subject_type, subject_id,
                            summary, metadata, created_at
                     FROM work_item_activity_entries
                     WHERE tenant_id = $1
                       AND work_item_id = $2
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .bind(work_item_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(work_item_activity_entry_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn append_work_item_activity_entry(
        &self,
        work_item_id: Uuid,
        event_type: &str,
        actor_subject: Option<String>,
        subject_type: Option<&str>,
        subject_id: Option<Uuid>,
        summary: String,
        metadata: Value,
    ) -> Result<WorkItemActivityEntry, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        let event_type = required_text(event_type.to_string(), "activity event_type")?;
        let summary = required_text(summary, "activity summary")?;
        let entry = WorkItemActivityEntry {
            id: Uuid::new_v4(),
            work_item_id,
            event_type,
            actor_subject: actor_subject.and_then(optional_text),
            subject_type: subject_type.map(str::to_string).and_then(optional_text),
            subject_id,
            summary,
            metadata,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .work_item_activity_entries
                    .insert(entry.id, entry.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO work_item_activity_entries
                         (id, tenant_id, work_item_id, event_type, actor_subject, subject_type,
                          subject_id, summary, metadata, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(entry.id)
                .bind(self.current_tenant_id())
                .bind(entry.work_item_id)
                .bind(&entry.event_type)
                .bind(&entry.actor_subject)
                .bind(&entry.subject_type)
                .bind(entry.subject_id)
                .bind(&entry.summary)
                .bind(&entry.metadata)
                .bind(entry.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(entry)
    }

    pub(crate) async fn list_work_item_assignments(
        &self,
        work_item_id: Uuid,
    ) -> Result<Vec<WorkItemAssignment>, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut assignments: Vec<_> = inner
                    .read()
                    .await
                    .work_item_assignments
                    .values()
                    .filter(|assignment| {
                        assignment.work_item_id == work_item_id && assignment.archived_at.is_none()
                    })
                    .cloned()
                    .collect();
                assignments.sort_by_key(|assignment| assignment.created_at);
                assignments.reverse();
                Ok(assignments)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, work_item_id, assignee_kind, assignee_id, role, status, assigned_by,
                            metadata, created_at, updated_at, archived_at
                     FROM work_item_assignments
                     WHERE tenant_id = $1
                       AND work_item_id = $2
                       AND archived_at IS NULL
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .bind(work_item_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(work_item_assignment_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn create_work_item_assignment(
        &self,
        work_item_id: Uuid,
        input: CreateWorkItemAssignment,
        assigned_by: Option<String>,
    ) -> Result<WorkItemAssignment, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        let assignee_kind = normalize_work_item_assignment_assignee_kind(&input.assignee_kind)?;
        let assignee_id = required_text(input.assignee_id, "assignment assignee_id")?;
        let role = normalize_work_item_assignment_role(&input.role)?;
        let status = normalize_work_item_assignment_status(&input.status)?;
        let now = Utc::now();
        let assignment = WorkItemAssignment {
            id: Uuid::new_v4(),
            work_item_id,
            assignee_kind,
            assignee_id,
            role,
            status,
            assigned_by: assigned_by.and_then(optional_text),
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .work_item_assignments
                    .insert(assignment.id, assignment.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO work_item_assignments
                         (id, tenant_id, work_item_id, assignee_kind, assignee_id, role, status,
                          assigned_by, metadata, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(assignment.id)
                .bind(self.current_tenant_id())
                .bind(assignment.work_item_id)
                .bind(&assignment.assignee_kind)
                .bind(&assignment.assignee_id)
                .bind(&assignment.role)
                .bind(&assignment.status)
                .bind(&assignment.assigned_by)
                .bind(&assignment.metadata)
                .bind(assignment.created_at)
                .bind(assignment.updated_at)
                .bind(assignment.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(assignment)
    }

    pub(crate) async fn list_work_item_reviews(
        &self,
        work_item_id: Uuid,
    ) -> Result<Vec<WorkItemReview>, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut reviews: Vec<_> = inner
                    .read()
                    .await
                    .work_item_reviews
                    .values()
                    .filter(|review| {
                        review.work_item_id == work_item_id && review.archived_at.is_none()
                    })
                    .cloned()
                    .collect();
                reviews.sort_by_key(|review| review.created_at);
                reviews.reverse();
                Ok(reviews)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, work_item_id, reviewer_kind, reviewer_id, status, decision,
                            summary, metadata, created_at, updated_at, archived_at
                     FROM work_item_reviews
                     WHERE tenant_id = $1
                       AND work_item_id = $2
                       AND archived_at IS NULL
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .bind(work_item_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(work_item_review_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_work_item_review(
        &self,
        work_item_id: Uuid,
        input: CreateWorkItemReview,
    ) -> Result<WorkItemReview, AppError> {
        self.ensure_work_item_exists(work_item_id).await?;
        let reviewer_kind = normalize_work_item_review_reviewer_kind(&input.reviewer_kind)?;
        let reviewer_id = required_text(input.reviewer_id, "review reviewer_id")?;
        let status = normalize_work_item_review_status(&input.status)?;
        let decision = input
            .decision
            .map(|value| normalize_work_item_review_decision(&value))
            .transpose()?;
        let now = Utc::now();
        let review = WorkItemReview {
            id: Uuid::new_v4(),
            work_item_id,
            reviewer_kind,
            reviewer_id,
            status,
            decision,
            summary: input.summary.and_then(optional_text),
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .work_item_reviews
                    .insert(review.id, review.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO work_item_reviews
                         (id, tenant_id, work_item_id, reviewer_kind, reviewer_id, status,
                          decision, summary, metadata, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                )
                .bind(review.id)
                .bind(self.current_tenant_id())
                .bind(review.work_item_id)
                .bind(&review.reviewer_kind)
                .bind(&review.reviewer_id)
                .bind(&review.status)
                .bind(&review.decision)
                .bind(&review.summary)
                .bind(&review.metadata)
                .bind(review.created_at)
                .bind(review.updated_at)
                .bind(review.archived_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(review)
    }

    async fn validate_work_item_scope(&self, input: &CreateWorkItem) -> Result<(), AppError> {
        if let Some(organization_id) = input.organization_id {
            self.ensure_organization_exists(organization_id).await?;
        }
        if let Some(team_id) = input.team_id {
            match input.organization_id {
                Some(organization_id) => {
                    self.ensure_team_belongs_to_organization(team_id, organization_id)
                        .await?;
                }
                None => self.ensure_team_exists(team_id).await?,
            }
        }
        if let Some(project_id) = input.project_id {
            match input.team_id {
                Some(team_id) => {
                    self.ensure_project_belongs_to_team(project_id, team_id)
                        .await?;
                }
                None => match input.organization_id {
                    Some(organization_id) => {
                        self.ensure_project_belongs_to_organization(project_id, organization_id)
                            .await?;
                    }
                    None => self.ensure_project_exists(project_id).await?,
                },
            }
        }
        Ok(())
    }

    async fn ensure_agent_teammate_exists(&self, teammate_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let active = inner
                    .read()
                    .await
                    .agent_teammates
                    .get(&teammate_id)
                    .is_some_and(|teammate| teammate.archived_at.is_none());
                if active {
                    Ok(())
                } else {
                    Err(AppError::not_found("active agent teammate not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM agent_teammates
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(teammate_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active agent teammate not found"))
            }
        }
    }

    async fn ensure_squad_exists(&self, squad_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let active = inner
                    .read()
                    .await
                    .squads
                    .get(&squad_id)
                    .is_some_and(|squad| squad.archived_at.is_none());
                if active {
                    Ok(())
                } else {
                    Err(AppError::not_found("active squad not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM squads
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(squad_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active squad not found"))
            }
        }
    }

    pub(crate) async fn ensure_work_item_exists(&self, work_item_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let active = inner
                    .read()
                    .await
                    .work_items
                    .get(&work_item_id)
                    .is_some_and(|work_item| work_item.archived_at.is_none());
                if active {
                    Ok(())
                } else {
                    Err(AppError::not_found("active work item not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM work_items
                     WHERE tenant_id = $1
                       AND id = $2
                       AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(work_item_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active work item not found"))
            }
        }
    }

    pub(crate) async fn archive_organization(
        &self,
        organization_id: Uuid,
    ) -> Result<Organization, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let organization = store
                    .organizations
                    .get_mut(&organization_id)
                    .ok_or_else(|| AppError::not_found("organization not found"))?;
                organization.archived_at = Some(archived_at);
                Ok(organization.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE organizations
                     SET archived_at = COALESCE(archived_at, $1)
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, name, slug, owner_subject, created_at, archived_at",
                )
                .bind(archived_at)
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("organization not found"))?;
                organization_from_row(row)
            }
        }
    }

    pub(crate) async fn transfer_organization_ownership(
        &self,
        organization_id: Uuid,
        owner_subject: String,
    ) -> Result<Organization, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let organization = store
                    .organizations
                    .get_mut(&organization_id)
                    .filter(|organization| organization.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("active organization not found"))?;
                organization.owner_subject = Some(owner_subject);
                Ok(organization.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE organizations
                     SET owner_subject = $1
                     WHERE tenant_id = $2 AND id = $3 AND archived_at IS NULL
                     RETURNING id, name, slug, owner_subject, created_at, archived_at",
                )
                .bind(owner_subject)
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("active organization not found"))?;
                organization_from_row(row)
            }
        }
    }

    pub(crate) async fn delete_organization(
        &self,
        organization_id: Uuid,
    ) -> Result<Organization, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let organization = store
                    .organizations
                    .get(&organization_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("organization not found"))?;
                if organization.archived_at.is_none() {
                    return Err(AppError::bad_request(
                        "organization must be archived before delete",
                    ));
                }
                let has_children = store
                    .teams
                    .values()
                    .any(|team| team.organization_id == organization_id)
                    || store
                        .memberships
                        .values()
                        .any(|membership| membership.organization_id == Some(organization_id))
                    || store
                        .tenant_invitations
                        .values()
                        .any(|invitation| invitation.organization_id == organization_id);
                if has_children {
                    return Err(AppError::bad_request(
                        "organization has child teams, memberships, or invitations",
                    ));
                }
                store.organizations.remove(&organization_id);
                Ok(organization)
            }
            StoreBackend::Postgres(pool) => {
                let child_count: i64 = sqlx::query_scalar(
                    "SELECT
                        (SELECT count(*) FROM teams WHERE tenant_id = $1 AND organization_id = $2)
                      + (SELECT count(*) FROM memberships WHERE tenant_id = $1 AND organization_id = $2)
                      + (SELECT count(*) FROM tenant_invitations WHERE tenant_id = $1 AND organization_id = $2)",
                )
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_one(pool)
                .await?;
                if child_count > 0 {
                    return Err(AppError::bad_request(
                        "organization has child teams, memberships, or invitations",
                    ));
                }
                let row = sqlx::query(
                    "DELETE FROM organizations
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
                     RETURNING id, name, slug, owner_subject, created_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    AppError::bad_request("organization must be archived before delete")
                })?;
                organization_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_team(&self, team_id: Uuid) -> Result<Team, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let team = store
                    .teams
                    .get_mut(&team_id)
                    .ok_or_else(|| AppError::not_found("team not found"))?;
                team.archived_at = Some(archived_at);
                Ok(team.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE teams
                     SET archived_at = COALESCE(archived_at, $1)
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, organization_id, name, slug, created_at, archived_at",
                )
                .bind(archived_at)
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("team not found"))?;
                team_from_row(row)
            }
        }
    }

    pub(crate) async fn delete_team(&self, team_id: Uuid) -> Result<Team, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let team = store
                    .teams
                    .get(&team_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("team not found"))?;
                if team.archived_at.is_none() {
                    return Err(AppError::bad_request("team must be archived before delete"));
                }
                let has_children = store
                    .projects
                    .values()
                    .any(|project| project.team_id == team_id)
                    || store
                        .memberships
                        .values()
                        .any(|membership| membership.team_id == Some(team_id))
                    || store
                        .tenant_invitations
                        .values()
                        .any(|invitation| invitation.team_id == Some(team_id))
                    || store
                        .provider_access
                        .values()
                        .any(|access| access.team_id == team_id)
                    || store
                        .mcp_servers
                        .values()
                        .any(|server| server.team_id == team_id)
                    || store
                        .agents
                        .values()
                        .any(|agent| agent.team_id == Some(team_id));
                if has_children {
                    return Err(AppError::bad_request(
                        "team has child projects, memberships, invitations, provider access, MCP servers, or agents",
                    ));
                }
                store.teams.remove(&team_id);
                Ok(team)
            }
            StoreBackend::Postgres(pool) => {
                let child_count: i64 = sqlx::query_scalar(
                    "SELECT
                        (SELECT count(*) FROM projects WHERE tenant_id = $1 AND team_id = $2)
                      + (SELECT count(*) FROM memberships WHERE tenant_id = $1 AND team_id = $2)
                      + (SELECT count(*) FROM tenant_invitations WHERE tenant_id = $1 AND team_id = $2)
                      + (SELECT count(*) FROM provider_access WHERE tenant_id = $1 AND team_id = $2)
                      + (SELECT count(*) FROM mcp_servers WHERE tenant_id = $1 AND team_id = $2)
                      + (SELECT count(*) FROM agents WHERE tenant_id = $1 AND team_id = $2)",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_one(pool)
                .await?;
                if child_count > 0 {
                    return Err(AppError::bad_request(
                        "team has child projects, memberships, invitations, provider access, MCP servers, or agents",
                    ));
                }
                let row = sqlx::query(
                    "DELETE FROM teams
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
                     RETURNING id, organization_id, name, slug, created_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("team must be archived before delete"))?;
                team_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_project(&self, project_id: Uuid) -> Result<Project, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let project = store
                    .projects
                    .get_mut(&project_id)
                    .ok_or_else(|| AppError::not_found("project not found"))?;
                project.archived_at = Some(archived_at);
                Ok(project.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE projects
                     SET archived_at = COALESCE(archived_at, $1)
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, team_id, name, slug, created_at, archived_at",
                )
                .bind(archived_at)
                .bind(self.current_tenant_id())
                .bind(project_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("project not found"))?;
                project_from_row(row)
            }
        }
    }

    pub(crate) async fn delete_project(&self, project_id: Uuid) -> Result<Project, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let project = store
                    .projects
                    .get(&project_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("project not found"))?;
                if project.archived_at.is_none() {
                    return Err(AppError::bad_request(
                        "project must be archived before delete",
                    ));
                }
                let has_children = store
                    .memberships
                    .values()
                    .any(|membership| membership.project_id == Some(project_id))
                    || store
                        .tenant_invitations
                        .values()
                        .any(|invitation| invitation.project_id == Some(project_id))
                    || store
                        .agents
                        .values()
                        .any(|agent| agent.project_id == Some(project_id));
                if has_children {
                    return Err(AppError::bad_request(
                        "project has child memberships, invitations, or agents",
                    ));
                }
                store.projects.remove(&project_id);
                Ok(project)
            }
            StoreBackend::Postgres(pool) => {
                let child_count: i64 = sqlx::query_scalar(
                    "SELECT
                        (SELECT count(*) FROM memberships WHERE tenant_id = $1 AND project_id = $2)
                      + (SELECT count(*) FROM tenant_invitations WHERE tenant_id = $1 AND project_id = $2)
                      + (SELECT count(*) FROM agents WHERE tenant_id = $1 AND project_id = $2)",
                )
                .bind(self.current_tenant_id())
                .bind(project_id)
                .fetch_one(pool)
                .await?;
                if child_count > 0 {
                    return Err(AppError::bad_request(
                        "project has child memberships, invitations, or agents",
                    ));
                }
                let row = sqlx::query(
                    "DELETE FROM projects
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NOT NULL
                     RETURNING id, team_id, name, slug, created_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(project_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::bad_request("project must be archived before delete"))?;
                project_from_row(row)
            }
        }
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
                .bind(self.current_tenant_id())
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
                .bind(self.current_tenant_id())
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

    pub(crate) async fn delete_membership(
        &self,
        membership_id: Uuid,
    ) -> Result<Membership, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let membership = store
                    .memberships
                    .get(&membership_id)
                    .cloned()
                    .ok_or_else(|| AppError::not_found("membership not found"))?;
                if membership.role == "admin"
                    && let Some(organization_id) = membership.organization_id
                {
                    let remaining_admin_count = store
                        .memberships
                        .values()
                        .filter(|candidate| {
                            candidate.id != membership_id
                                && candidate.organization_id == Some(organization_id)
                                && candidate.role == "admin"
                        })
                        .count();
                    if remaining_admin_count == 0 {
                        return Err(AppError::bad_request(
                            "cannot delete the last admin membership",
                        ));
                    }
                }
                store.memberships.remove(&membership_id);
                Ok(membership)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, user_id, organization_id, team_id, project_id, role, created_at
                     FROM memberships
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(membership_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("membership not found"))?;
                let membership = membership_from_row(row)?;
                if membership.role == "admin"
                    && let Some(organization_id) = membership.organization_id
                {
                    let remaining_admin_count: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*)
                             FROM memberships
                             WHERE tenant_id = $1
                               AND organization_id = $2
                               AND role = 'admin'
                               AND id <> $3",
                    )
                    .bind(self.current_tenant_id())
                    .bind(organization_id)
                    .bind(membership_id)
                    .fetch_one(pool)
                    .await?;
                    if remaining_admin_count == 0 {
                        return Err(AppError::bad_request(
                            "cannot delete the last admin membership",
                        ));
                    }
                }
                sqlx::query("DELETE FROM memberships WHERE tenant_id = $1 AND id = $2")
                    .bind(self.current_tenant_id())
                    .bind(membership_id)
                    .execute(pool)
                    .await?;
                Ok(membership)
            }
        }
    }

    pub(crate) async fn list_tenant_invitations(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<TenantInvitation>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut invitations: Vec<_> = inner
                    .read()
                    .await
                    .tenant_invitations
                    .values()
                    .filter(|invitation| invitation.organization_id == organization_id)
                    .cloned()
                    .collect();
                invitations.sort_by_key(|invitation| invitation.created_at);
                invitations.reverse();
                Ok(invitations)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at
                     FROM tenant_invitations
                     WHERE tenant_id = $1 AND organization_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(tenant_invitation_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_tenant_invitation(
        &self,
        organization_id: Uuid,
        input: CreateTenantInvitation,
        invited_by: String,
    ) -> Result<TenantInvitation, AppError> {
        self.ensure_organization_exists(organization_id).await?;
        if let Some(team_id) = input.team_id {
            self.ensure_team_exists(team_id).await?;
        }
        if let Some(project_id) = input.project_id {
            let Some(team_id) = input.team_id else {
                return Err(AppError::bad_request(
                    "project_id requires a matching team_id on tenant invitations",
                ));
            };
            self.ensure_project_belongs_to_team(project_id, team_id)
                .await?;
        }
        let role = membership_role_name(membership_role_from_str(&input.role)?).to_string();
        let email = input.email.trim().to_ascii_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err(AppError::bad_request("tenant invitation email is invalid"));
        }
        let expires_in_hours = input.expires_in_hours.unwrap_or(168).clamp(1, 720);
        let invitation = TenantInvitation {
            id: Uuid::new_v4(),
            organization_id,
            team_id: input.team_id,
            project_id: input.project_id,
            email,
            role,
            status: "pending".to_string(),
            token: Uuid::new_v4().to_string(),
            invited_by: Some(invited_by),
            accepted_by: None,
            expires_at: Utc::now() + ChronoDuration::hours(expires_in_hours),
            created_at: Utc::now(),
            decided_at: None,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .tenant_invitations
                    .insert(invitation.id, invitation.clone());
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO tenant_invitations (id, tenant_id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     RETURNING id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at",
                )
                .bind(invitation.id)
                .bind(self.current_tenant_id())
                .bind(invitation.organization_id)
                .bind(invitation.team_id)
                .bind(invitation.project_id)
                .bind(&invitation.email)
                .bind(&invitation.role)
                .bind(&invitation.status)
                .bind(&invitation.token)
                .bind(&invitation.invited_by)
                .bind(&invitation.accepted_by)
                .bind(invitation.expires_at)
                .bind(invitation.created_at)
                .bind(invitation.decided_at)
                .fetch_one(pool)
                .await?;
                return tenant_invitation_from_row(row);
            }
        }
        Ok(invitation)
    }

    pub(crate) async fn revoke_tenant_invitation(
        &self,
        invitation_id: Uuid,
    ) -> Result<TenantInvitation, AppError> {
        let decided_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let invitation = store
                    .tenant_invitations
                    .get_mut(&invitation_id)
                    .ok_or_else(|| AppError::not_found("tenant invitation not found"))?;
                if invitation.status != "pending" {
                    return Err(AppError::bad_request("tenant invitation is not pending"));
                }
                invitation.status = "revoked".to_string();
                invitation.decided_at = Some(decided_at);
                Ok(invitation.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tenant_invitations
                     SET status = 'revoked', decided_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND status = 'pending'
                     RETURNING id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at",
                )
                .bind(self.current_tenant_id())
                .bind(invitation_id)
                .bind(decided_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("pending tenant invitation not found"))?;
                tenant_invitation_from_row(row)
            }
        }
    }

    pub(crate) async fn tenant_invitation_by_token(
        &self,
        token: &str,
    ) -> Result<TenantInvitation, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .tenant_invitations
                .values()
                .find(|invitation| invitation.token == token)
                .cloned()
                .ok_or_else(|| AppError::not_found("tenant invitation not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at
                     FROM tenant_invitations
                     WHERE tenant_id = $1 AND token = $2",
                )
                .bind(self.current_tenant_id())
                .bind(token)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tenant invitation not found"))?;
                tenant_invitation_from_row(row)
            }
        }
    }

    pub(crate) async fn expire_tenant_invitation(
        &self,
        invitation_id: Uuid,
    ) -> Result<TenantInvitation, AppError> {
        let decided_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let invitation = store
                    .tenant_invitations
                    .get_mut(&invitation_id)
                    .ok_or_else(|| AppError::not_found("tenant invitation not found"))?;
                invitation.status = "expired".to_string();
                invitation.decided_at = Some(decided_at);
                Ok(invitation.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tenant_invitations
                     SET status = 'expired', decided_at = $3
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at",
                )
                .bind(self.current_tenant_id())
                .bind(invitation_id)
                .bind(decided_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tenant invitation not found"))?;
                tenant_invitation_from_row(row)
            }
        }
    }

    pub(crate) async fn mark_tenant_invitation_accepted(
        &self,
        invitation_id: Uuid,
        accepted_by: String,
    ) -> Result<TenantInvitation, AppError> {
        let decided_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let invitation = store
                    .tenant_invitations
                    .get_mut(&invitation_id)
                    .ok_or_else(|| AppError::not_found("tenant invitation not found"))?;
                if invitation.status != "pending" {
                    return Err(AppError::bad_request("tenant invitation is not pending"));
                }
                invitation.status = "accepted".to_string();
                invitation.accepted_by = Some(accepted_by);
                invitation.decided_at = Some(decided_at);
                Ok(invitation.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tenant_invitations
                     SET status = 'accepted', accepted_by = $3, decided_at = $4
                     WHERE tenant_id = $1 AND id = $2 AND status = 'pending'
                     RETURNING id, organization_id, team_id, project_id, email, role, status, token, invited_by, accepted_by, expires_at, created_at, decided_at",
                )
                .bind(self.current_tenant_id())
                .bind(invitation_id)
                .bind(accepted_by)
                .bind(decided_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("pending tenant invitation not found"))?;
                tenant_invitation_from_row(row)
            }
        }
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
                .bind(self.current_tenant_id())
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
                if team.archived_at.is_some()
                    || store
                        .organizations
                        .get(&team.organization_id)
                        .is_none_or(|organization| organization.archived_at.is_some())
                {
                    return Ok(false);
                }
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
                        JOIN teams t ON t.id = $3 AND t.tenant_id = $1 AND t.archived_at IS NULL
                        JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1 AND o.archived_at IS NULL
                        WHERE m.tenant_id = $1
                          AND m.user_id = $2
                          AND (
                            (m.team_id = $3 AND m.project_id IS NULL)
                            OR (m.team_id IS NULL AND m.project_id IS NULL AND m.organization_id = t.organization_id)
                          )
                    )",
                )
                .bind(self.current_tenant_id())
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
                if project.archived_at.is_some() {
                    return Ok(false);
                }
                let Some(team) = store.teams.get(&project.team_id) else {
                    return Ok(false);
                };
                if team.archived_at.is_some()
                    || store
                        .organizations
                        .get(&team.organization_id)
                        .is_none_or(|organization| organization.archived_at.is_some())
                {
                    return Ok(false);
                }
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
                        JOIN projects p ON p.id = $3 AND p.tenant_id = $1 AND p.archived_at IS NULL
                        JOIN teams t ON t.id = p.team_id AND t.tenant_id = $1 AND t.archived_at IS NULL
                        JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1 AND o.archived_at IS NULL
                        WHERE m.tenant_id = $1
                          AND m.user_id = $2
                          AND (
                            m.project_id = $3
                            OR (m.project_id IS NULL AND m.team_id = p.team_id)
                            OR (m.project_id IS NULL AND m.team_id IS NULL AND m.organization_id = t.organization_id)
                          )
                    )",
                )
                .bind(self.current_tenant_id())
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
                    .get(&organization_id)
                    .is_some_and(|organization| organization.archived_at.is_none())
                {
                    Ok(())
                } else {
                    Err(AppError::not_found("active organization not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1 FROM organizations WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(organization_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active organization not found"))
            }
        }
    }

    async fn ensure_team_exists(&self, team_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let active = store.teams.get(&team_id).is_some_and(|team| {
                    team.archived_at.is_none()
                        && store
                            .organizations
                            .get(&team.organization_id)
                            .is_some_and(|organization| organization.archived_at.is_none())
                });
                if active {
                    Ok(())
                } else {
                    Err(AppError::not_found("active team not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                         FROM teams t
                         JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1
                         WHERE t.tenant_id = $1
                           AND t.id = $2
                           AND t.archived_at IS NULL
                           AND o.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active team not found"))
            }
        }
    }

    async fn ensure_team_belongs_to_organization(
        &self,
        team_id: Uuid,
        organization_id: Uuid,
    ) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let belongs = store.teams.get(&team_id).is_some_and(|team| {
                    team.organization_id == organization_id
                        && team.archived_at.is_none()
                        && store
                            .organizations
                            .get(&organization_id)
                            .is_some_and(|organization| organization.archived_at.is_none())
                });
                if belongs {
                    Ok(())
                } else {
                    Err(AppError::not_found("team not found for organization"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM teams t
                     JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1
                     WHERE t.tenant_id = $1
                       AND t.id = $2
                       AND t.organization_id = $3
                       AND t.archived_at IS NULL
                       AND o.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .bind(organization_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("team not found for organization"))
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
                let store = inner.read().await;
                let belongs = store.projects.get(&project_id).is_some_and(|project| {
                    project.team_id == team_id
                        && project.archived_at.is_none()
                        && store.teams.get(&project.team_id).is_some_and(|team| {
                            team.archived_at.is_none()
                                && store
                                    .organizations
                                    .get(&team.organization_id)
                                    .is_some_and(|organization| organization.archived_at.is_none())
                        })
                });
                if belongs {
                    Ok(())
                } else {
                    Err(AppError::not_found("project not found for team"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM projects p
                     JOIN teams t ON t.id = p.team_id AND t.tenant_id = $1
                     JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1
                     WHERE p.tenant_id = $1
                       AND p.id = $2
                       AND p.team_id = $3
                       AND p.archived_at IS NULL
                       AND t.archived_at IS NULL
                       AND o.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
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

    async fn ensure_project_belongs_to_organization(
        &self,
        project_id: Uuid,
        organization_id: Uuid,
    ) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let belongs = store.projects.get(&project_id).is_some_and(|project| {
                    project.archived_at.is_none()
                        && store.teams.get(&project.team_id).is_some_and(|team| {
                            team.organization_id == organization_id
                                && team.archived_at.is_none()
                                && store
                                    .organizations
                                    .get(&organization_id)
                                    .is_some_and(|organization| organization.archived_at.is_none())
                        })
                });
                if belongs {
                    Ok(())
                } else {
                    Err(AppError::not_found("project not found for organization"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM projects p
                     JOIN teams t ON t.id = p.team_id AND t.tenant_id = $1
                     JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1
                     WHERE p.tenant_id = $1
                       AND p.id = $2
                       AND t.organization_id = $3
                       AND p.archived_at IS NULL
                       AND t.archived_at IS NULL
                       AND o.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(project_id)
                .bind(organization_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("project not found for organization"))
            }
        }
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let active = store.projects.get(&project_id).is_some_and(|project| {
                    project.archived_at.is_none()
                        && store.teams.get(&project.team_id).is_some_and(|team| {
                            team.archived_at.is_none()
                                && store
                                    .organizations
                                    .get(&team.organization_id)
                                    .is_some_and(|organization| organization.archived_at.is_none())
                        })
                });
                if active {
                    Ok(())
                } else {
                    Err(AppError::not_found("active project not found"))
                }
            }
            StoreBackend::Postgres(pool) => {
                let exists: Option<i32> = sqlx::query_scalar(
                    "SELECT 1
                     FROM projects p
                     JOIN teams t ON t.id = p.team_id AND t.tenant_id = $1
                     JOIN organizations o ON o.id = t.organization_id AND o.tenant_id = $1
                     WHERE p.tenant_id = $1
                       AND p.id = $2
                       AND p.archived_at IS NULL
                       AND t.archived_at IS NULL
                       AND o.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(project_id)
                .fetch_optional(pool)
                .await?;
                exists
                    .map(|_| ())
                    .ok_or_else(|| AppError::not_found("active project not found"))
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
                .bind(self.current_tenant_id())
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
                .bind(self.current_tenant_id())
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

    pub(crate) async fn update_provider_access(
        &self,
        access_id: Uuid,
        input: UpdateProviderAccess,
    ) -> Result<ProviderAccess, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let access = store
                    .provider_access
                    .get_mut(&access_id)
                    .ok_or_else(|| AppError::not_found("provider access not found"))?;
                if access.status != "active" {
                    return Err(AppError::bad_request(
                        "archived provider access cannot be updated",
                    ));
                }
                access.provider_name = input.provider_name;
                access.model_allowlist = input.model_allowlist;
                Ok(access.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE provider_access
                     SET provider_name = $3, model_allowlist = $4
                     WHERE tenant_id = $1 AND id = $2 AND status = 'active'
                     RETURNING id, team_id, provider_name, model_allowlist, status, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(access_id)
                .bind(input.provider_name)
                .bind(serde_json::json!(input.model_allowlist))
                .fetch_optional(pool)
                .await?;
                let Some(row) = row else {
                    return Err(AppError::not_found("active provider access not found"));
                };
                provider_access_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_provider_access(
        &self,
        access_id: Uuid,
    ) -> Result<ProviderAccess, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let access = store
                    .provider_access
                    .get_mut(&access_id)
                    .ok_or_else(|| AppError::not_found("provider access not found"))?;
                access.status = "archived".to_string();
                Ok(access.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE provider_access
                     SET status = 'archived'
                     WHERE tenant_id = $1 AND id = $2
                     RETURNING id, team_id, provider_name, model_allowlist, status, created_at",
                )
                .bind(self.current_tenant_id())
                .bind(access_id)
                .fetch_optional(pool)
                .await?;
                let Some(row) = row else {
                    return Err(AppError::not_found("provider access not found"));
                };
                provider_access_from_row(row)
            }
        }
    }

    pub(crate) async fn ensure_provider_model_allowed(
        &self,
        team_id: Uuid,
        provider_name: &str,
        model: &str,
    ) -> Result<(), AppError> {
        if let Some(provider) = self.provider_by_name(provider_name).await?
            && provider.status != "active"
        {
            return Err(AppError::forbidden(format!(
                "provider {provider_name} is not active"
            )));
        }
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

    pub(crate) async fn list_providers(&self) -> Result<Vec<ProviderRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut providers: Vec<_> =
                    inner.read().await.providers.values().cloned().collect();
                providers.sort_by_key(|provider| provider.created_at);
                providers.reverse();
                Ok(providers)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, provider_type, name, base_url, default_model, config, status, created_at
                     FROM providers
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(provider_record_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_provider(
        &self,
        input: CreateProviderRecord,
    ) -> Result<ProviderRecord, AppError> {
        let mut provider = ProviderRecord {
            id: Uuid::new_v4(),
            provider_type: input.provider_type,
            name: input.name,
            base_url: input.base_url,
            default_model: input.default_model,
            config: input.config,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if let Some(existing_id) = store
                    .providers
                    .iter()
                    .find_map(|(id, existing)| (existing.name == provider.name).then_some(*id))
                {
                    provider.id = existing_id;
                    store.providers.insert(existing_id, provider.clone());
                } else {
                    store.providers.insert(provider.id, provider.clone());
                }
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO providers (id, tenant_id, provider_type, name, base_url, default_model, config, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     ON CONFLICT (tenant_id, name)
                     DO UPDATE SET provider_type = EXCLUDED.provider_type,
                                   base_url = EXCLUDED.base_url,
                                   default_model = EXCLUDED.default_model,
                                   config = EXCLUDED.config,
                                   status = EXCLUDED.status
                     RETURNING id, provider_type, name, base_url, default_model, config, status, created_at",
                )
                .bind(provider.id)
                .bind(self.current_tenant_id())
                .bind(&provider.provider_type)
                .bind(&provider.name)
                .bind(&provider.base_url)
                .bind(&provider.default_model)
                .bind(&provider.config)
                .bind(&provider.status)
                .bind(provider.created_at)
                .fetch_one(pool)
                .await?;
                return provider_record_from_row(row);
            }
        }
        Ok(provider)
    }

    pub(crate) async fn update_provider(
        &self,
        provider_id: Uuid,
        input: CreateProviderRecord,
    ) -> Result<ProviderRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let provider = store
                    .providers
                    .get_mut(&provider_id)
                    .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider.provider_type = input.provider_type;
                provider.name = input.name;
                provider.base_url = input.base_url;
                provider.default_model = input.default_model;
                provider.config = input.config;
                Ok(provider.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE providers
                     SET provider_type = $1,
                         name = $2,
                         base_url = $3,
                         default_model = $4,
                         config = $5
                     WHERE tenant_id = $6 AND id = $7
                     RETURNING id, provider_type, name, base_url, default_model, config, status, created_at",
                )
                .bind(input.provider_type)
                .bind(input.name)
                .bind(input.base_url)
                .bind(input.default_model)
                .bind(input.config)
                .bind(self.current_tenant_id())
                .bind(provider_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider_record_from_row(row)
            }
        }
    }

    pub(crate) async fn update_provider_status(
        &self,
        provider_id: Uuid,
        status: &str,
    ) -> Result<ProviderRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let provider = store
                    .providers
                    .get_mut(&provider_id)
                    .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider.status = status.to_string();
                Ok(provider.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE providers
                     SET status = $1
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, provider_type, name, base_url, default_model, config, status, created_at",
                )
                .bind(status)
                .bind(self.current_tenant_id())
                .bind(provider_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider_record_from_row(row)
            }
        }
    }

    pub(crate) async fn update_provider_config(
        &self,
        provider_id: Uuid,
        config: Value,
    ) -> Result<ProviderRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let provider = store
                    .providers
                    .get_mut(&provider_id)
                    .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider.config = config;
                Ok(provider.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE providers
                     SET config = $1
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, provider_type, name, base_url, default_model, config, status, created_at",
                )
                .bind(config)
                .bind(self.current_tenant_id())
                .bind(provider_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("provider not found"))?;
                provider_record_from_row(row)
            }
        }
    }

    pub(crate) async fn provider_by_name(
        &self,
        name: &str,
    ) -> Result<Option<ProviderRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .providers
                .values()
                .find(|provider| provider.name == name)
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, provider_type, name, base_url, default_model, config, status, created_at
                     FROM providers
                     WHERE tenant_id = $1 AND name = $2",
                )
                .bind(self.current_tenant_id())
                .bind(name)
                .fetch_optional(pool)
                .await?;
                row.map(provider_record_from_row).transpose()
            }
        }
    }

    pub(crate) async fn provider_request_count_since(
        &self,
        provider_name: &str,
        since: chrono::DateTime<Utc>,
    ) -> Result<i64, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let count = inner
                    .read()
                    .await
                    .events
                    .values()
                    .flatten()
                    .filter(|event| {
                        event.event_type == "llm.request"
                            && event.created_at >= since
                            && event
                                .payload
                                .get("provider")
                                .and_then(serde_json::Value::as_str)
                                == Some(provider_name)
                    })
                    .count();
                Ok(count as i64)
            }
            StoreBackend::Postgres(pool) => {
                let count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*)
                     FROM session_events
                     WHERE tenant_id = $1
                       AND event_type = 'llm.request'
                       AND payload->>'provider' = $2
                       AND created_at >= $3",
                )
                .bind(self.current_tenant_id())
                .bind(provider_name)
                .bind(since)
                .fetch_one(pool)
                .await?;
                Ok(count)
            }
        }
    }

    pub(crate) async fn list_mcp_servers(
        &self,
        team_id: Uuid,
    ) -> Result<Vec<McpServerRecord>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut servers: Vec<_> = inner
                    .read()
                    .await
                    .mcp_servers
                    .values()
                    .filter(|server| server.team_id == team_id)
                    .cloned()
                    .collect();
                servers.sort_by_key(|server| server.created_at);
                servers.reverse();
                Ok(servers)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, team_id, name, transport, config, tool_allowlist, status, created_at
                     FROM mcp_servers
                     WHERE tenant_id = $1 AND team_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(mcp_server_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_mcp_server(
        &self,
        team_id: Uuid,
        input: CreateMcpServerRecord,
    ) -> Result<McpServerRecord, AppError> {
        self.ensure_team_exists(team_id).await?;
        let server = McpServerRecord {
            id: Uuid::new_v4(),
            team_id,
            name: input.name,
            transport: input.transport,
            config: input.config,
            tool_allowlist: input.tool_allowlist,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if let Some(existing_id) = store.mcp_servers.iter().find_map(|(id, existing)| {
                    (existing.team_id == team_id && existing.name == server.name).then_some(*id)
                }) {
                    let mut updated = server.clone();
                    updated.id = existing_id;
                    store.mcp_servers.insert(existing_id, updated.clone());
                    return Ok(updated);
                } else {
                    store.mcp_servers.insert(server.id, server.clone());
                }
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO mcp_servers (id, tenant_id, team_id, name, transport, config, tool_allowlist, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     ON CONFLICT (team_id, name)
                     DO UPDATE SET transport = EXCLUDED.transport,
                                   config = EXCLUDED.config,
                                   tool_allowlist = EXCLUDED.tool_allowlist,
                                   status = EXCLUDED.status
                     RETURNING id, team_id, name, transport, config, tool_allowlist, status, created_at",
                )
                .bind(server.id)
                .bind(self.current_tenant_id())
                .bind(server.team_id)
                .bind(&server.name)
                .bind(&server.transport)
                .bind(&server.config)
                .bind(serde_json::json!(server.tool_allowlist))
                .bind(&server.status)
                .bind(server.created_at)
                .fetch_one(pool)
                .await?;
                return mcp_server_from_row(row);
            }
        }
        Ok(server)
    }

    pub(crate) async fn get_mcp_server(
        &self,
        team_id: Uuid,
        server_id: Uuid,
    ) -> Result<McpServerRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .mcp_servers
                .get(&server_id)
                .filter(|server| server.team_id == team_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("mcp server not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, team_id, name, transport, config, tool_allowlist, status, created_at
                     FROM mcp_servers
                     WHERE tenant_id = $1 AND team_id = $2 AND id = $3",
                )
                .bind(self.current_tenant_id())
                .bind(team_id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                mcp_server_from_row(row)
            }
        }
    }

    pub(crate) async fn update_mcp_server_tool_allowlist(
        &self,
        team_id: Uuid,
        server_id: Uuid,
        tool_allowlist: Vec<String>,
    ) -> Result<McpServerRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let server = store
                    .mcp_servers
                    .get_mut(&server_id)
                    .filter(|server| server.team_id == team_id)
                    .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                server.tool_allowlist = tool_allowlist;
                Ok(server.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE mcp_servers
                     SET tool_allowlist = $1
                     WHERE tenant_id = $2 AND team_id = $3 AND id = $4
                     RETURNING id, team_id, name, transport, config, tool_allowlist, status, created_at",
                )
                .bind(serde_json::json!(tool_allowlist))
                .bind(self.current_tenant_id())
                .bind(team_id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                mcp_server_from_row(row)
            }
        }
    }

    pub(crate) async fn update_mcp_server(
        &self,
        team_id: Uuid,
        server_id: Uuid,
        input: UpdateMcpServerRecord,
    ) -> Result<McpServerRecord, AppError> {
        let current = self.get_mcp_server(team_id, server_id).await?;
        let updated = McpServerRecord {
            transport: input.transport.unwrap_or(current.transport),
            config: input.config.unwrap_or(current.config),
            tool_allowlist: input.tool_allowlist.unwrap_or(current.tool_allowlist),
            ..current
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                store.mcp_servers.insert(server_id, updated.clone());
                Ok(updated)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE mcp_servers
                     SET transport = $1,
                         config = $2,
                         tool_allowlist = $3
                     WHERE tenant_id = $4 AND team_id = $5 AND id = $6
                     RETURNING id, team_id, name, transport, config, tool_allowlist, status, created_at",
                )
                .bind(&updated.transport)
                .bind(&updated.config)
                .bind(serde_json::json!(updated.tool_allowlist))
                .bind(self.current_tenant_id())
                .bind(team_id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                mcp_server_from_row(row)
            }
        }
    }

    pub(crate) async fn update_mcp_server_status(
        &self,
        team_id: Uuid,
        server_id: Uuid,
        status: &str,
    ) -> Result<McpServerRecord, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let server = store
                    .mcp_servers
                    .get_mut(&server_id)
                    .filter(|server| server.team_id == team_id)
                    .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                server.status = status.to_string();
                Ok(server.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE mcp_servers
                     SET status = $1
                     WHERE tenant_id = $2 AND team_id = $3 AND id = $4
                     RETURNING id, team_id, name, transport, config, tool_allowlist, status, created_at",
                )
                .bind(status)
                .bind(self.current_tenant_id())
                .bind(team_id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("mcp server not found"))?;
                mcp_server_from_row(row)
            }
        }
    }

    pub(crate) async fn mcp_server_for_session_tool(
        &self,
        session_id: Uuid,
        server_name: &str,
        tool_name: &str,
    ) -> Result<Option<McpServerRecord>, AppError> {
        let session = self.get_session(session_id).await?;
        let agent = self.get_agent(session.agent_id).await?;
        let Some(team_id) = agent.team_id else {
            return Ok(None);
        };
        let servers = self.list_mcp_servers(team_id).await?;
        let Some(server) = servers
            .into_iter()
            .find(|server| server.name == server_name && server.status == "active")
        else {
            return Err(AppError::forbidden(format!(
                "MCP server {server_name} is not registered for this session team"
            )));
        };
        if server.tool_allowlist.iter().any(|tool| tool == tool_name) {
            Ok(Some(server))
        } else {
            Err(AppError::forbidden(format!(
                "MCP tool {tool_name} is not allowed for server {server_name}"
            )))
        }
    }
}

fn required_text(value: String, field: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_work_item_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "open" | "in_progress" | "blocked" | "review" | "done" | "canceled" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported work item status value: {other}"
        ))),
    }
}

fn normalize_work_item_priority(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "normal" | "high" | "urgent" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported work item priority value: {other}"
        ))),
    }
}

fn normalize_collaboration_record_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "active" | "inactive" | "archived" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported collaboration status value: {other}"
        ))),
    }
}

fn normalize_work_item_assignment_assignee_kind(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "user" | "agent" | "squad" | "team" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported assignment assignee_kind value: {other}"
        ))),
    }
}

fn normalize_work_item_assignment_role(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "owner" | "contributor" | "reviewer" | "observer" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported assignment role value: {other}"
        ))),
    }
}

fn normalize_work_item_assignment_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "assigned" | "in_progress" | "blocked" | "review" | "done" | "canceled" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported assignment status value: {other}"
        ))),
    }
}

fn normalize_work_item_review_reviewer_kind(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "user" | "agent" | "squad" | "team" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported review reviewer_kind value: {other}"
        ))),
    }
}

fn normalize_work_item_review_status(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "requested" | "in_review" | "completed" | "canceled" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported review status value: {other}"
        ))),
    }
}

fn normalize_work_item_review_decision(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "approved" | "changes_requested" | "rejected" | "needs_info" => Ok(normalized),
        other => Err(AppError::bad_request(format!(
            "unsupported review decision value: {other}"
        ))),
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

fn membership_role_name(role: Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::Operator => "operator",
        Role::Worker => "worker",
        Role::Approver => "approver",
        Role::Viewer => "viewer",
    }
}
