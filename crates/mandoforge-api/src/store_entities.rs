use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::{agent_from_row, agent_version_from_row, session_from_row};
use crate::{
    Agent, AgentRuntimeProfile, AgentVersion, AppError, AppState, CreateAgent, CreateAgentVersion,
    CreateSession, Environment, Principal, Role, Session, SessionStatus,
};

fn validate_agent_version_json(value: &Value, field: &str) -> Result<(), AppError> {
    if !value.is_object() {
        return Err(AppError::bad_request(format!(
            "agent version {field} must be an object"
        )));
    }
    Ok(())
}

fn apply_agent_version_behavior(target: &mut Agent, source: &Agent) {
    target.provider = source.provider.clone();
    target.model = source.model.clone();
    target.runtime_profile_id = source.runtime_profile_id;
    target.system_prompt = source.system_prompt.clone();
    target.tools = source.tools.clone();
    target.tool_policy = source.tool_policy.clone();
    target.mcp_server_ids = source.mcp_server_ids.clone();
    target.skill_ids = source.skill_ids.clone();
    target.workflow_pack_ids = source.workflow_pack_ids.clone();
    target.remote_computer_profile = source.remote_computer_profile.clone();
    target.semantic_scopes = source.semantic_scopes.clone();
}

fn normalize_agent_role(role: &str) -> Result<String, AppError> {
    let normalized = role.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "manager" | "specialist" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "agent_role must be manager or specialist",
        )),
    }
}

fn normalize_agent_release_state(state: &str) -> Result<String, AppError> {
    let normalized = state.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "draft" | "staged" | "active" | "disabled" => Ok(normalized),
        _ => Err(AppError::bad_request(
            "release_state must be draft, staged, active, or disabled",
        )),
    }
}

fn apply_effective_agent_release_state(agent: &mut Agent, releases: &[crate::AgentRelease]) {
    if agent.release_state == "disabled" {
        return;
    }
    let target_environment = configured_agent_release_environment();
    let promoted = releases
        .iter()
        .filter(|release| release.agent_id == agent.id && release.status == "promoted")
        .collect::<Vec<_>>();
    let target_promoted = promoted.iter().any(|release| {
        target_environment
            .as_ref()
            .is_none_or(|environment| release.environment.eq_ignore_ascii_case(environment))
    });
    if target_promoted {
        agent.release_state = "active".to_string();
    } else if target_environment.is_some()
        && (!promoted.is_empty() || agent.release_state == "active")
    {
        agent.release_state = "staged".to_string();
    }
}

pub(crate) fn configured_agent_release_environment() -> Option<String> {
    std::env::var("MANDOFORGE_AGENT_RELEASE_ENVIRONMENT")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn production_agent_release_environment() -> Result<String, AppError> {
    configured_agent_release_environment().ok_or_else(|| {
        AppError::forbidden(
            "production agent runtime requires MANDOFORGE_AGENT_RELEASE_ENVIRONMENT",
        )
    })
}

fn environment_release_environment(environment: &crate::Environment) -> Result<String, AppError> {
    environment
        .worker_queue_binding
        .get("release_environment")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            AppError::forbidden(format!(
                "environment {} requires worker_queue_binding.release_environment",
                environment.name
            ))
        })
}

pub(crate) fn agent_release_enforcement_required() -> bool {
    crate::provider_runtime_production_mode()
        || std::env::var("MANDOFORGE_AGENT_RELEASE_ENFORCEMENT")
            .ok()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("required"))
}

impl AppState {
    pub(crate) async fn list_agents(&self) -> Result<Vec<Agent>, AppError> {
        let mut agents = match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.agents.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, kind, provider, model, system_prompt, tools, created_at,
                            team_id, project_id, runtime_profile_id, agent_role, tool_policy,
                            mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile,
                            semantic_scopes, release_state
                     FROM agents
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(agent_from_row).collect()
            }
        }?;
        let releases = self.list_all_agent_releases().await?;
        for agent in &mut agents {
            apply_effective_agent_release_state(agent, &releases);
        }
        Ok(agents)
    }

    pub(crate) async fn list_agents_visible_to(
        &self,
        principal: &Principal,
    ) -> Result<Vec<Agent>, AppError> {
        let agents = self.list_agents().await?;
        if principal.roles.contains(&Role::Admin) {
            return Ok(agents);
        }
        let mut visible = Vec::new();
        for agent in agents {
            if let Some(project_id) = agent.project_id {
                if self
                    .subject_can_access_project(&principal.subject_id, project_id)
                    .await?
                {
                    visible.push(agent);
                }
            } else if let Some(team_id) = agent.team_id {
                if self
                    .subject_can_access_team(&principal.subject_id, team_id)
                    .await?
                {
                    visible.push(agent);
                }
            } else {
                visible.push(agent);
            }
        }
        Ok(visible)
    }

    pub(crate) async fn get_agent(&self, agent_id: Uuid) -> Result<Agent, AppError> {
        let mut agent = match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agents
                .get(&agent_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("agent not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, kind, provider, model, system_prompt, tools, created_at,
                            team_id, project_id, runtime_profile_id, agent_role, tool_policy,
                            mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile,
                            semantic_scopes, release_state
                     FROM agents
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent not found"))?;
                agent_from_row(row)
            }
        }?;
        apply_effective_agent_release_state(&mut agent, &self.list_all_agent_releases().await?);
        Ok(agent)
    }

    pub(crate) async fn create_agent(&self, input: CreateAgent) -> Result<Agent, AppError> {
        let release_state = normalize_agent_release_state(&input.release_state)?;
        if release_state == "active" && agent_release_enforcement_required() {
            return Err(AppError::bad_request(
                "agents must become active through release promotion",
            ));
        }
        let agent = Agent {
            id: Uuid::new_v4(),
            name: input.name,
            kind: input.kind,
            team_id: input.team_id,
            project_id: input.project_id,
            runtime_profile_id: input.runtime_profile_id,
            agent_role: normalize_agent_role(&input.agent_role)?,
            provider: input.provider,
            model: input.model,
            system_prompt: input.system_prompt,
            tools: input.tools,
            tool_policy: input.tool_policy,
            mcp_server_ids: input.mcp_server_ids,
            skill_ids: input.skill_ids,
            workflow_pack_ids: input.workflow_pack_ids,
            remote_computer_profile: input.remote_computer_profile,
            semantic_scopes: input.semantic_scopes,
            release_state,
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
        if let Some(runtime_profile_id) = agent.runtime_profile_id {
            self.get_agent_runtime_profile(runtime_profile_id).await?;
        }
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents
                        (id, tenant_id, name, kind, team_id, project_id, runtime_profile_id, agent_role, provider, model, system_prompt, tools, tool_policy, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, release_state, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
                )
                .bind(agent.id)
                .bind(self.current_tenant_id())
                .bind(&agent.name)
                .bind(&agent.kind)
                .bind(agent.team_id)
                .bind(agent.project_id)
                .bind(agent.runtime_profile_id)
                .bind(&agent.agent_role)
                .bind(&agent.provider)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(&agent.tool_policy)
                .bind(json!(agent.mcp_server_ids))
                .bind(json!(agent.skill_ids))
                .bind(json!(agent.workflow_pack_ids))
                .bind(&agent.remote_computer_profile)
                .bind(&agent.semantic_scopes)
                .bind(&agent.release_state)
                .bind(agent.created_at)
                .execute(pool)
                .await?;
            }
        }
        self.insert_agent_version(&agent, 1, input.runtime_config)
            .await?;
        Ok(agent)
    }

    pub(crate) async fn insert_agent_version(
        &self,
        agent: &Agent,
        version: i32,
        runtime_config: serde_json::Value,
    ) -> Result<(), AppError> {
        let runtime_profile_snapshot = self
            .agent_runtime_profile_snapshot(agent.runtime_profile_id)
            .await?;
        let agent_version = AgentVersion {
            id: Uuid::new_v4(),
            agent_id: agent.id,
            version,
            provider: agent.provider.clone(),
            model: agent.model.clone(),
            system_prompt: agent.system_prompt.clone(),
            tools: agent.tools.clone(),
            tool_names: agent.tools.clone(),
            runtime_config,
            approval_policy: agent.tool_policy.clone(),
            runtime_profile_id: agent.runtime_profile_id,
            runtime_profile_snapshot,
            mcp_server_ids: agent.mcp_server_ids.clone(),
            skill_ids: agent.skill_ids.clone(),
            workflow_pack_ids: agent.workflow_pack_ids.clone(),
            remote_computer_profile: agent.remote_computer_profile.clone(),
            semantic_scopes: agent.semantic_scopes.clone(),
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let versions = store.agent_versions.entry(agent.id).or_default();
                if !versions.iter().any(|existing| existing.version == version) {
                    let advances_current = versions
                        .iter()
                        .all(|existing| existing.version < agent_version.version);
                    versions.push(agent_version);
                    versions.sort_by_key(|version| version.version);
                    if advances_current && let Some(stored_agent) = store.agents.get_mut(&agent.id)
                    {
                        apply_agent_version_behavior(stored_agent, agent);
                    }
                }
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query_scalar::<_, i32>(
                    "SELECT current_version
                     FROM agents
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(agent.id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("agent not found"))?;
                let inserted = sqlx::query(
                        "INSERT INTO agent_versions (id, agent_id, version, provider, model, system_prompt, tools, tool_names, runtime_config, approval_policy, runtime_profile_id, runtime_profile_snapshot, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                     ON CONFLICT (agent_id, version) DO NOTHING",
                )
                .bind(agent_version.id)
                .bind(agent.id)
                .bind(version)
                .bind(&agent_version.provider)
                .bind(&agent.model)
                .bind(&agent.system_prompt)
                .bind(json!(agent.tools))
                .bind(json!(&agent_version.tool_names))
                .bind(&agent_version.runtime_config)
                .bind(&agent_version.approval_policy)
                .bind(agent_version.runtime_profile_id)
                .bind(&agent_version.runtime_profile_snapshot)
                .bind(json!(&agent.mcp_server_ids))
                .bind(json!(&agent.skill_ids))
                .bind(json!(&agent.workflow_pack_ids))
                .bind(&agent_version.remote_computer_profile)
                .bind(&agent.semantic_scopes)
                .bind(agent_version.created_at)
                .execute(&mut *tx)
                .await?;
                if inserted.rows_affected() > 0 {
                    sqlx::query(
                        "UPDATE agents
                         SET current_version = $3,
                             provider = $4,
                             model = $5,
                             system_prompt = $6,
                             tools = $7,
                             runtime_profile_id = $8,
                             tool_policy = $9,
                             mcp_server_ids = $10,
                             skill_ids = $11,
                             workflow_pack_ids = $12,
                             remote_computer_profile = $13,
                             semantic_scopes = $14,
                             updated_at = $15
                         WHERE tenant_id = $1 AND id = $2 AND current_version < $3",
                    )
                    .bind(self.current_tenant_id())
                    .bind(agent.id)
                    .bind(version)
                    .bind(&agent_version.provider)
                    .bind(&agent_version.model)
                    .bind(&agent_version.system_prompt)
                    .bind(json!(&agent_version.tools))
                    .bind(agent_version.runtime_profile_id)
                    .bind(&agent_version.approval_policy)
                    .bind(json!(&agent_version.mcp_server_ids))
                    .bind(json!(&agent_version.skill_ids))
                    .bind(json!(&agent_version.workflow_pack_ids))
                    .bind(&agent_version.remote_computer_profile)
                    .bind(&agent_version.semantic_scopes)
                    .bind(agent_version.created_at)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    async fn agent_runtime_profile_snapshot(
        &self,
        runtime_profile_id: Option<Uuid>,
    ) -> Result<Value, AppError> {
        match runtime_profile_id {
            Some(runtime_profile_id) => {
                serde_json::to_value(self.get_agent_runtime_profile(runtime_profile_id).await?)
                    .map_err(|error| {
                        AppError::bad_request(format!(
                            "failed to snapshot agent runtime profile: {error}"
                        ))
                    })
            }
            None => Ok(json!({})),
        }
    }

    pub(crate) async fn create_agent_version(
        &self,
        agent_id: Uuid,
        input: CreateAgentVersion,
    ) -> Result<AgentVersion, AppError> {
        let mut agent = self.get_agent(agent_id).await?;
        let provider = input.provider.trim();
        let model = input.model.trim();
        if provider.is_empty() {
            return Err(AppError::bad_request(
                "agent version provider must be non-empty",
            ));
        }
        if model.is_empty() {
            return Err(AppError::bad_request(
                "agent version model must be non-empty",
            ));
        }
        validate_agent_version_json(&input.runtime_config, "runtime_config")?;
        validate_agent_version_json(&input.approval_policy, "approval_policy")?;
        validate_agent_version_json(&input.remote_computer_profile, "remote_computer_profile")?;
        validate_agent_version_json(&input.semantic_scopes, "semantic_scopes")?;
        if let Some(team_id) = agent.team_id {
            self.ensure_provider_model_allowed(team_id, provider, model)
                .await?;
        }
        let runtime_profile_snapshot = self
            .agent_runtime_profile_snapshot(input.runtime_profile_id)
            .await?;

        agent.provider = provider.to_string();
        agent.model = model.to_string();
        agent.runtime_profile_id = input.runtime_profile_id;
        agent.system_prompt = input.system_prompt;
        agent.tools = input.tools;
        agent.tool_policy = input.approval_policy;
        agent.mcp_server_ids = input.mcp_server_ids;
        agent.skill_ids = input.skill_ids;
        agent.workflow_pack_ids = input.workflow_pack_ids;
        agent.remote_computer_profile = input.remote_computer_profile;
        agent.semantic_scopes = input.semantic_scopes;

        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let versions = store.agent_versions.entry(agent_id).or_default();
                let next_version = versions
                    .iter()
                    .map(|version| version.version)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let agent_version = AgentVersion {
                    id: Uuid::new_v4(),
                    agent_id,
                    version: next_version,
                    provider: agent.provider.clone(),
                    model: agent.model.clone(),
                    system_prompt: agent.system_prompt.clone(),
                    tools: agent.tools.clone(),
                    tool_names: agent.tools.clone(),
                    runtime_config: input.runtime_config,
                    approval_policy: agent.tool_policy.clone(),
                    runtime_profile_id: agent.runtime_profile_id,
                    runtime_profile_snapshot,
                    mcp_server_ids: agent.mcp_server_ids.clone(),
                    skill_ids: agent.skill_ids.clone(),
                    workflow_pack_ids: agent.workflow_pack_ids.clone(),
                    remote_computer_profile: agent.remote_computer_profile.clone(),
                    semantic_scopes: agent.semantic_scopes.clone(),
                    created_at: Utc::now(),
                };
                versions.push(agent_version.clone());
                versions.sort_by_key(|version| version.version);
                let stored_agent = store
                    .agents
                    .get_mut(&agent_id)
                    .ok_or_else(|| AppError::not_found("agent not found"))?;
                apply_agent_version_behavior(stored_agent, &agent);
                Ok(agent_version)
            }
            StoreBackend::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let current_version: i32 = sqlx::query_scalar(
                    "SELECT current_version
                     FROM agents
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     FOR UPDATE",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::not_found("agent not found"))?;
                let max_version: Option<i32> = sqlx::query_scalar(
                    "SELECT MAX(version) FROM agent_versions WHERE agent_id = $1",
                )
                .bind(agent_id)
                .fetch_one(&mut *tx)
                .await?;
                let next_version = current_version.max(max_version.unwrap_or(0)) + 1;
                let agent_version = AgentVersion {
                    id: Uuid::new_v4(),
                    agent_id,
                    version: next_version,
                    provider: agent.provider.clone(),
                    model: agent.model.clone(),
                    system_prompt: agent.system_prompt.clone(),
                    tools: agent.tools.clone(),
                    tool_names: agent.tools.clone(),
                    runtime_config: input.runtime_config,
                    approval_policy: agent.tool_policy.clone(),
                    runtime_profile_id: agent.runtime_profile_id,
                    runtime_profile_snapshot,
                    mcp_server_ids: agent.mcp_server_ids.clone(),
                    skill_ids: agent.skill_ids.clone(),
                    workflow_pack_ids: agent.workflow_pack_ids.clone(),
                    remote_computer_profile: agent.remote_computer_profile.clone(),
                    semantic_scopes: agent.semantic_scopes.clone(),
                    created_at: Utc::now(),
                };
                sqlx::query(
                    "INSERT INTO agent_versions
                        (id, agent_id, version, provider, model, system_prompt, tools, tool_names, runtime_config, approval_policy, runtime_profile_id, runtime_profile_snapshot, mcp_server_ids, skill_ids, workflow_pack_ids, remote_computer_profile, semantic_scopes, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
                )
                .bind(agent_version.id)
                .bind(agent_version.agent_id)
                .bind(agent_version.version)
                .bind(&agent_version.provider)
                .bind(&agent_version.model)
                .bind(&agent_version.system_prompt)
                .bind(json!(&agent_version.tools))
                .bind(json!(&agent_version.tool_names))
                .bind(&agent_version.runtime_config)
                .bind(&agent_version.approval_policy)
                .bind(agent_version.runtime_profile_id)
                .bind(&agent_version.runtime_profile_snapshot)
                .bind(json!(&agent_version.mcp_server_ids))
                .bind(json!(&agent_version.skill_ids))
                .bind(json!(&agent_version.workflow_pack_ids))
                .bind(&agent_version.remote_computer_profile)
                .bind(&agent_version.semantic_scopes)
                .bind(agent_version.created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE agents
                     SET current_version = $3,
                         provider = $4,
                         model = $5,
                         system_prompt = $6,
                         tools = $7,
                         runtime_profile_id = $8,
                         tool_policy = $9,
                         mcp_server_ids = $10,
                         skill_ids = $11,
                         workflow_pack_ids = $12,
                         remote_computer_profile = $13,
                         semantic_scopes = $14,
                         updated_at = $15
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
                .bind(agent_id)
                .bind(agent_version.version)
                .bind(&agent_version.provider)
                .bind(&agent_version.model)
                .bind(&agent_version.system_prompt)
                .bind(json!(&agent_version.tools))
                .bind(agent_version.runtime_profile_id)
                .bind(&agent_version.approval_policy)
                .bind(json!(&agent_version.mcp_server_ids))
                .bind(json!(&agent_version.skill_ids))
                .bind(json!(&agent_version.workflow_pack_ids))
                .bind(&agent_version.remote_computer_profile)
                .bind(&agent_version.semantic_scopes)
                .bind(agent_version.created_at)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(agent_version)
            }
        }
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
                    "SELECT av.id, av.agent_id, av.version, av.provider, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.runtime_profile_id, av.runtime_profile_snapshot, av.mcp_server_ids, av.skill_ids, av.workflow_pack_ids, av.remote_computer_profile, av.semantic_scopes, av.created_at
                     FROM agent_versions av
                     JOIN agents a ON a.id = av.agent_id
                     WHERE a.tenant_id = $1 AND av.agent_id = $2
                     ORDER BY av.version ASC",
                )
                .bind(self.current_tenant_id())
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
                    "SELECT av.id, av.agent_id, av.version, av.provider, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.runtime_profile_id, av.runtime_profile_snapshot, av.mcp_server_ids, av.skill_ids, av.workflow_pack_ids, av.remote_computer_profile, av.semantic_scopes, av.created_at
                     FROM agent_versions av
                     JOIN agents a ON a.id = av.agent_id
                     WHERE a.tenant_id = $1 AND av.agent_id = $2 AND av.version = $3",
                )
                .bind(self.current_tenant_id())
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
                    "SELECT av.id, av.agent_id, av.version, av.provider, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.runtime_profile_id, av.runtime_profile_snapshot, av.mcp_server_ids, av.skill_ids, av.workflow_pack_ids, av.remote_computer_profile, av.semantic_scopes, av.created_at
                     FROM agents a
                     JOIN agent_versions av ON av.agent_id = a.id AND av.version = a.current_version
                     WHERE a.tenant_id = $1 AND a.id = $2 AND a.archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
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
                    "SELECT av.id, av.agent_id, av.version, av.provider, av.model, av.system_prompt, av.tools, av.tool_names, av.runtime_config, av.approval_policy, av.runtime_profile_id, av.runtime_profile_snapshot, av.mcp_server_ids, av.skill_ids, av.workflow_pack_ids, av.remote_computer_profile, av.semantic_scopes, av.created_at
                     FROM sessions s
                     JOIN agents a ON a.id = s.agent_id
                     JOIN agent_versions av ON av.agent_id = s.agent_id
                     WHERE s.tenant_id = $1
                       AND s.id = $2
                       AND (av.id = s.agent_version_id OR (s.agent_version_id IS NULL AND av.version = a.current_version))
                     ORDER BY CASE WHEN av.id = s.agent_version_id THEN 0 ELSE 1 END
                     LIMIT 1",
                )
                .bind(self.current_tenant_id())
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
                    "SELECT id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at
                     FROM sessions
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(session_from_row).collect()
            }
        }
    }

    pub(crate) async fn list_sessions_visible_to(
        &self,
        principal: &Principal,
    ) -> Result<Vec<Session>, AppError> {
        let sessions = self.list_sessions().await?;
        if principal.roles.contains(&Role::Admin) {
            return Ok(sessions);
        }
        let mut visible = Vec::new();
        for session in sessions {
            let agent = self.get_agent(session.agent_id).await?;
            if let Some(project_id) = agent.project_id {
                if self
                    .subject_can_access_project(&principal.subject_id, project_id)
                    .await?
                {
                    visible.push(session);
                }
            } else if let Some(team_id) = agent.team_id {
                if self
                    .subject_can_access_team(&principal.subject_id, team_id)
                    .await?
                {
                    visible.push(session);
                }
            } else {
                visible.push(session);
            }
        }
        Ok(visible)
    }

    pub(crate) async fn create_session(&self, input: CreateSession) -> Result<Session, AppError> {
        let agent_version = self
            .runnable_agent_version(input.agent_id, input.environment_id)
            .await?;
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            agent_id: input.agent_id,
            agent_version_id: Some(agent_version.id),
            environment_id: input.environment_id,
            title: input.title,
            status: SessionStatus::Idle,
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
                    "INSERT INTO sessions (id, tenant_id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(session.id)
                .bind(self.current_tenant_id())
                .bind(session.agent_id)
                .bind(session.agent_version_id)
                .bind(session.environment_id)
                .bind(&session.title)
                .bind(session.status.as_str())
                .bind(session.created_at)
                .bind(session.updated_at)
                .execute(pool)
                .await?;
            }
        }
        if let Some(environment_id) = session.environment_id {
            self.append_event(
                "system",
                None,
                session.id,
                "session.environment_bound",
                json!({ "environment_id": environment_id }),
            )
            .await?;
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

    async fn governed_runtime_environment(
        &self,
        environment_id: Option<Uuid>,
    ) -> Result<(Environment, String), AppError> {
        let environment = match environment_id {
            Some(environment_id) => self.get_environment(environment_id).await?,
            None => {
                return Err(AppError::forbidden(
                    "production agent runtime requires a bound environment",
                ));
            }
        };
        if environment.status != "enabled" || environment.release_state != "active" {
            return Err(AppError::forbidden(format!(
                "environment {} is not enabled and active",
                environment.name
            )));
        }
        let release_environment = environment_release_environment(&environment)?;
        let configured_release_environment = production_agent_release_environment()?;
        if release_environment != configured_release_environment {
            return Err(AppError::forbidden(format!(
                "environment {} release environment {} does not match runtime release environment {}",
                environment.name, release_environment, configured_release_environment
            )));
        }
        Ok((environment, release_environment))
    }

    async fn validate_agent_version_runtime_profile(
        &self,
        agent_version: &AgentVersion,
        environment: &Environment,
    ) -> Result<(), AppError> {
        let profile = if let Some(runtime_profile_id) = environment.runtime_profile_id {
            Some(self.get_agent_runtime_profile(runtime_profile_id).await?)
        } else if let Some(runtime_profile_id) = agent_version.runtime_profile_id {
            let snapshot_present = agent_version
                .runtime_profile_snapshot
                .as_object()
                .is_some_and(|snapshot| !snapshot.is_empty());
            if snapshot_present {
                Some(
                    serde_json::from_value::<AgentRuntimeProfile>(
                        agent_version.runtime_profile_snapshot.clone(),
                    )
                    .map_err(|error| {
                        AppError::forbidden(format!(
                            "agent version runtime profile snapshot is invalid: {error}"
                        ))
                    })?,
                )
            } else {
                Some(self.get_agent_runtime_profile(runtime_profile_id).await?)
            }
        } else {
            None
        };
        if let Some(profile) = profile {
            let gate = crate::evaluate_agent_runtime_profile_release_gate(&profile);
            if gate.fail_closed {
                return Err(AppError::forbidden(format!(
                    "agent runtime profile {} is blocked: {}",
                    profile.name,
                    gate.blocking_reasons.join(", ")
                )));
            }
        }
        Ok(())
    }

    async fn runnable_agent_version(
        &self,
        agent_id: Uuid,
        environment_id: Option<Uuid>,
    ) -> Result<AgentVersion, AppError> {
        let agent = self.get_agent(agent_id).await?;
        if agent.release_state == "disabled" {
            return Err(AppError::forbidden("agent is disabled"));
        }

        if !agent_release_enforcement_required() {
            return self.current_agent_version(agent_id).await;
        }
        let (environment, release_environment) =
            self.governed_runtime_environment(environment_id).await?;
        let agent_version = self
            .promoted_agent_version(agent_id, &release_environment)
            .await?;
        self.validate_agent_version_runtime_profile(&agent_version, &environment)
            .await?;
        Ok(agent_version)
    }

    pub(crate) async fn ensure_session_runnable(&self, session_id: Uuid) -> Result<(), AppError> {
        if !agent_release_enforcement_required() {
            return Ok(());
        }
        let session = self.get_session(session_id).await?;
        let agent = self.get_agent(session.agent_id).await?;
        if agent.release_state == "disabled" {
            return Err(AppError::forbidden("agent is disabled"));
        }
        let (environment, release_environment) = self
            .governed_runtime_environment(session.environment_id)
            .await?;
        let pinned_version_id = session
            .agent_version_id
            .ok_or_else(|| AppError::forbidden("production session must pin an agent version"))?;
        let agent_version = self.agent_version_for_session(session_id).await?;
        if agent_version.id != pinned_version_id || agent_version.agent_id != session.agent_id {
            return Err(AppError::forbidden(
                "production session agent version binding is invalid",
            ));
        }
        self.validate_agent_version_runtime_profile(&agent_version, &environment)
            .await?;
        if !self
            .agent_version_has_promoted_release(
                session.agent_id,
                pinned_version_id,
                &release_environment,
            )
            .await?
        {
            return Err(AppError::forbidden(
                "pinned agent version no longer has a promoted release",
            ));
        }
        Ok(())
    }

    pub(crate) async fn agent_exists(&self, agent_id: Uuid) -> Result<bool, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner.read().await.agents.contains_key(&agent_id)),
            StoreBackend::Postgres(pool) => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM agents WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL)",
                )
                .bind(self.current_tenant_id())
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
                    "SELECT id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at
                     FROM sessions
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.current_tenant_id())
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
                     RETURNING id, agent_id, agent_version_id, environment_id, title, status, created_at, updated_at",
                )
                .bind(status.as_str())
                .bind(self.current_tenant_id())
                .bind(session_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("session not found"))?;
                session_from_row(row)
            }
        }
    }
}
