use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{agent_from_row, agent_version_from_row, session_from_row};
use crate::{
    Agent, AgentVersion, AppError, AppState, CreateAgent, CreateSession, Session, SessionStatus,
};

impl AppState {
    pub(crate) async fn list_agents(&self) -> Result<Vec<Agent>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.agents.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, kind, provider, model, system_prompt, tools, created_at
                            , team_id, project_id
                     FROM agents
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(agent_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_agent(&self, input: CreateAgent) -> Result<Agent, AppError> {
        let agent = Agent {
            id: Uuid::new_v4(),
            name: input.name,
            kind: input.kind,
            team_id: input.team_id,
            project_id: input.project_id,
            provider: input.provider,
            model: input.model,
            system_prompt: input.system_prompt,
            tools: input.tools,
            created_at: Utc::now(),
        };
        if let Some(team_id) = agent.team_id {
            self.ensure_provider_model_allowed(team_id, &agent.provider, &agent.model)
                .await?;
            if let Some(project_id) = agent.project_id {
                self.ensure_project_belongs_to_team(project_id, team_id)
                    .await?;
            }
        } else if agent.project_id.is_some() {
            return Err(AppError::bad_request(
                "project_id requires a matching team_id on scoped agents",
            ));
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents (id, tenant_id, name, kind, team_id, project_id, provider, model, system_prompt, tools, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(agent.id)
                .bind(self.tenant_id)
                .bind(&agent.name)
                .bind(&agent.kind)
                .bind(agent.team_id)
                .bind(agent.project_id)
                .bind(&agent.provider)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(agent.created_at)
                .execute(pool)
                .await?;
            }
        }
        self.insert_agent_version(&agent, 1).await?;
        Ok(agent)
    }

    pub(crate) async fn insert_agent_version(
        &self,
        agent: &Agent,
        version: i32,
    ) -> Result<(), AppError> {
        let agent_version = AgentVersion {
            id: Uuid::new_v4(),
            agent_id: agent.id,
            version,
            model: agent.model.clone(),
            system_prompt: agent.system_prompt.clone(),
            tools: agent.tools.clone(),
            tool_names: agent.tools.clone(),
            runtime_config: json!({}),
            approval_policy: json!({}),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let versions = store.agent_versions.entry(agent.id).or_default();
                if !versions.iter().any(|existing| existing.version == version) {
                    versions.push(agent_version);
                    versions.sort_by_key(|version| version.version);
                }
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                        "INSERT INTO agent_versions (id, agent_id, version, model, system_prompt, tools, tool_names, runtime_config, approval_policy, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $6, $7, $8, $9)
                     ON CONFLICT (agent_id, version) DO NOTHING",
                )
                .bind(agent_version.id)
                .bind(agent.id)
                .bind(version)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(&agent_version.runtime_config)
                .bind(&agent_version.approval_policy)
                .bind(agent_version.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn list_agent_versions(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<AgentVersion>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                if !store.agents.contains_key(&agent_id) {
                    return Err(AppError::not_found("agent not found"));
                }
                Ok(store
                    .agent_versions
                    .get(&agent_id)
                    .cloned()
                    .unwrap_or_default())
            }
            StoreBackend::Postgres(pool) => {
                if !self.agent_exists(agent_id).await? {
                    return Err(AppError::not_found("agent not found"));
                }
                let rows = sqlx::query(
                    "SELECT av.id, av.agent_id, av.version, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.created_at
                     FROM agent_versions av
                     JOIN agents a ON a.id = av.agent_id
                     WHERE a.tenant_id = $1 AND av.agent_id = $2
                     ORDER BY av.version ASC",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(agent_version_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_agent_version(
        &self,
        agent_id: Uuid,
        version: i32,
    ) -> Result<AgentVersion, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agent_versions
                .get(&agent_id)
                .and_then(|versions| {
                    versions
                        .iter()
                        .find(|agent_version| agent_version.version == version)
                })
                .cloned()
                .ok_or_else(|| AppError::not_found("agent version not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT av.id, av.agent_id, av.version, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.created_at
                     FROM agent_versions av
                     JOIN agents a ON a.id = av.agent_id
                     WHERE a.tenant_id = $1 AND av.agent_id = $2 AND av.version = $3",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .bind(version)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent version not found"))?;
                agent_version_from_row(row)
            }
        }
    }

    pub(crate) async fn current_agent_version(
        &self,
        agent_id: Uuid,
    ) -> Result<AgentVersion, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                if !store.agents.contains_key(&agent_id) {
                    return Err(AppError::not_found("agent not found"));
                }
                store
                    .agent_versions
                    .get(&agent_id)
                    .and_then(|versions| versions.iter().max_by_key(|version| version.version))
                    .cloned()
                    .ok_or_else(|| AppError::not_found("agent version not found"))
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT av.id, av.agent_id, av.version, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.created_at
                     FROM agents a
                     JOIN agent_versions av ON av.agent_id = a.id AND av.version = a.current_version
                     WHERE a.tenant_id = $1 AND a.id = $2 AND a.archived_at IS NULL",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent version not found"))?;
                agent_version_from_row(row)
            }
        }
    }

    pub(crate) async fn agent_version_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<AgentVersion, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let store = inner.read().await;
                let session = store
                    .sessions
                    .get(&session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                if let Some(agent_version_id) = session.agent_version_id {
                    return store
                        .agent_versions
                        .values()
                        .flat_map(|versions| versions.iter())
                        .find(|version| version.id == agent_version_id)
                        .cloned()
                        .ok_or_else(|| AppError::not_found("agent version not found"));
                }
                store
                    .agent_versions
                    .get(&session.agent_id)
                    .and_then(|versions| versions.iter().max_by_key(|version| version.version))
                    .cloned()
                    .ok_or_else(|| AppError::not_found("agent version not found"))
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT av.id, av.agent_id, av.version, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.created_at
                     FROM sessions s
                     JOIN agents a ON a.id = s.agent_id
                     JOIN agent_versions av ON av.agent_id = s.agent_id
                     WHERE s.tenant_id = $1
                       AND s.id = $2
                       AND (av.id = s.agent_version_id OR (s.agent_version_id IS NULL AND av.version = a.current_version))
                     ORDER BY CASE WHEN av.id = s.agent_version_id THEN 0 ELSE 1 END
                     LIMIT 1",
                )
                .bind(self.tenant_id)
                .bind(session_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent version not found"))?;
                agent_version_from_row(row)
            }
        }
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<Session>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.sessions.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, agent_id, agent_version_id, title, status, created_at, updated_at
                     FROM sessions
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(session_from_row).collect()
            }
        }
    }

    pub(crate) async fn create_session(&self, input: CreateSession) -> Result<Session, AppError> {
        if !self.agent_exists(input.agent_id).await? {
            return Err(AppError::not_found("agent not found"));
        }
        let agent_version = self.current_agent_version(input.agent_id).await?;
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            agent_id: input.agent_id,
            agent_version_id: Some(agent_version.id),
            title: input.title,
            status: SessionStatus::Created,
            created_at: now,
            updated_at: now,
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .sessions
                    .insert(session.id, session.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO sessions (id, tenant_id, agent_id, agent_version_id, title, status, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(session.id)
                .bind(self.tenant_id)
                .bind(session.agent_id)
                .bind(session.agent_version_id)
                .bind(&session.title)
                .bind(session.status.as_str())
                .bind(session.created_at)
                .bind(session.updated_at)
                .execute(pool)
                .await?;
            }
        }
        if let Some(message) = input.message {
            self.append_event(
                "user",
                None,
                session.id,
                "user.message",
                json!({ "message": message }),
            )
            .await?;
        }
        Ok(session)
    }

    pub(crate) async fn agent_exists(&self, agent_id: Uuid) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner.read().await.agents.contains_key(&agent_id)),
            StoreBackend::Postgres(pool) => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM agents WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL)",
                )
                .bind(self.tenant_id)
                .bind(agent_id)
                .fetch_one(pool)
                .await?;
                Ok(exists)
            }
        }
    }

    pub(crate) async fn get_session(&self, id: Uuid) -> Result<Session, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .sessions
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("session not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, agent_id, agent_version_id, title, status, created_at, updated_at
                     FROM sessions
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                session_from_row(row)
            }
        }
    }

    pub(crate) async fn set_session_status(
        &self,
        session_id: Uuid,
        status: SessionStatus,
    ) -> Result<Session, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let session = store
                    .sessions
                    .get_mut(&session_id)
                    .ok_or_else(|| AppError::not_found("session not found"))?;
                session.status = status;
                session.updated_at = Utc::now();
                Ok(session.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE sessions
                     SET status = $1, updated_at = now()
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, agent_id, agent_version_id, title, status, created_at, updated_at",
                )
                .bind(status.as_str())
                .bind(self.tenant_id)
                .bind(session_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                session_from_row(row)
            }
        }
    }
}
