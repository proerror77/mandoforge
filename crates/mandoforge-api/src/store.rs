use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{PgPool, Row, postgres::PgRow};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    Agent, AgentVersion, AppError, AppState, Approval, Artifact, AuditLog, CreateAgent,
    CreateSession, Session, SessionEvent, SessionStatus, ToolCall,
};

#[derive(Default)]
pub(crate) struct MemoryStore {
    agents: HashMap<Uuid, Agent>,
    agent_versions: HashMap<Uuid, Vec<AgentVersion>>,
    sessions: HashMap<Uuid, Session>,
    events: HashMap<Uuid, Vec<SessionEvent>>,
    approvals: HashMap<Uuid, Approval>,
    artifacts: HashMap<Uuid, Artifact>,
    tool_calls: HashMap<Uuid, ToolCall>,
    audit_logs: HashMap<Uuid, AuditLog>,
}

#[derive(Clone)]
pub(crate) enum StoreBackend {
    Memory(Arc<RwLock<MemoryStore>>),
    Postgres(PgPool),
}

fn agent_from_row(row: PgRow) -> Result<Agent, AppError> {
    let tools: Value = row.try_get("tools")?;
    Ok(Agent {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        kind: row.try_get("kind")?,
        provider: row.try_get("provider")?,
        model: row.try_get("model")?,
        system_prompt: row.try_get("system_prompt")?,
        tools: serde_json::from_value(tools).unwrap_or_default(),
        created_at: row.try_get("created_at")?,
    })
}

fn session_from_row(row: PgRow) -> Result<Session, AppError> {
    let status: String = row.try_get("status")?;
    Ok(Session {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        title: row.try_get("title")?,
        status: status.into(),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn agent_version_from_row(row: PgRow) -> Result<AgentVersion, AppError> {
    let tools: Value = row.try_get("tools")?;
    let tool_names: Value = row.try_get("tool_names")?;
    Ok(AgentVersion {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        version: row.try_get("version")?,
        model: row.try_get("model")?,
        system_prompt: row.try_get("system_prompt")?,
        tools: serde_json::from_value(tools).unwrap_or_default(),
        tool_names: serde_json::from_value(tool_names).unwrap_or_default(),
        runtime_config: row.try_get("runtime_config")?,
        approval_policy: row.try_get("approval_policy")?,
        created_at: row.try_get("created_at")?,
    })
}

fn event_from_row(row: PgRow) -> Result<SessionEvent, AppError> {
    Ok(SessionEvent {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        seq: row.try_get("seq")?,
        parent_event_id: row.try_get("parent_event_id")?,
        actor_type: row
            .try_get::<Option<String>, _>("actor_type")?
            .unwrap_or_else(|| "system".to_string()),
        actor_id: row.try_get("actor_id")?,
        event_type: row.try_get("event_type")?,
        payload: row.try_get("payload")?,
        created_at: row.try_get("created_at")?,
    })
}

fn artifact_from_row(row: PgRow) -> Result<Artifact, AppError> {
    Ok(Artifact {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        artifact_type: row.try_get("artifact_type")?,
        name: row.try_get("name")?,
        path: row.try_get("path")?,
        content: row
            .try_get::<Option<Value>, _>("content")?
            .unwrap_or(json!({})),
        created_at: row.try_get("created_at")?,
    })
}

fn approval_from_row(row: PgRow) -> Result<Approval, AppError> {
    Ok(Approval {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        tool_call_id: row.try_get("tool_call_id")?,
        action: row.try_get("action")?,
        risk_level: row.try_get("risk_level")?,
        reason: row.try_get("reason")?,
        evidence: row.try_get("evidence")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
        decided_at: row.try_get("decided_at")?,
    })
}

fn tool_call_from_row(row: PgRow) -> Result<ToolCall, AppError> {
    Ok(ToolCall {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        event_id: row.try_get("event_id")?,
        tool_name: row.try_get("tool_name")?,
        args: row.try_get("args")?,
        status: row.try_get("status")?,
        risk_level: row.try_get("risk_level")?,
        policy_decision: row.try_get("policy_decision")?,
        result: row.try_get("result")?,
        error: row.try_get("error")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        created_at: row.try_get("created_at")?,
    })
}

fn audit_log_from_row(row: PgRow) -> Result<AuditLog, AppError> {
    Ok(AuditLog {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        action: row.try_get("action")?,
        resource_type: row.try_get("resource_type")?,
        resource_id: row.try_get("resource_id")?,
        details: row.try_get("details")?,
        created_at: row.try_get("created_at")?,
    })
}

impl AppState {
    pub(crate) async fn list_agents(&self) -> Result<Vec<Agent>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.agents.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, kind, provider, model, system_prompt, tools, created_at
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
            provider: input.provider,
            model: input.model,
            system_prompt: input.system_prompt,
            tools: input.tools,
            created_at: Utc::now(),
        };
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents (id, tenant_id, name, kind, provider, model, system_prompt, tools, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(agent.id)
                .bind(self.tenant_id)
                .bind(&agent.name)
                .bind(&agent.kind)
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

    pub(crate) async fn list_sessions(&self) -> Result<Vec<Session>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.sessions.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, agent_id, title, status, created_at, updated_at
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
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            agent_id: input.agent_id,
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
                    "INSERT INTO sessions (id, tenant_id, agent_id, title, status, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(session.id)
                .bind(self.tenant_id)
                .bind(session.agent_id)
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
                    "SELECT id, agent_id, title, status, created_at, updated_at
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
                     RETURNING id, agent_id, title, status, created_at, updated_at",
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

    pub(crate) async fn list_events(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<SessionEvent>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .events
                .get(&session_id)
                .cloned()
                .unwrap_or_default()),
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at
                     FROM session_events
                     WHERE tenant_id = $1 AND session_id = $2
                     ORDER BY seq ASC",
                )
                .bind(self.tenant_id)
                .bind(session_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(event_from_row).collect()
            }
        }
    }

    pub(crate) async fn append_event(
        &self,
        actor_type: &str,
        actor_id: Option<Uuid>,
        session_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<SessionEvent, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if !store.sessions.contains_key(&session_id) {
                    return Err(AppError::not_found("session not found"));
                }
                let seq = store
                    .events
                    .get(&session_id)
                    .map_or(1, |events| events.len() as i64 + 1);
                let event = SessionEvent {
                    id: Uuid::new_v4(),
                    session_id,
                    seq,
                    parent_event_id: None,
                    actor_type: actor_type.to_string(),
                    actor_id,
                    event_type: event_type.to_string(),
                    payload,
                    created_at: Utc::now(),
                };
                store
                    .events
                    .entry(session_id)
                    .or_default()
                    .push(event.clone());
                Ok(event)
            }
            StoreBackend::Postgres(pool) => {
                if self.get_session(session_id).await.is_err() {
                    return Err(AppError::not_found("session not found"));
                }
                let row = sqlx::query(
                    "WITH next_seq AS (
                        SELECT COALESCE(MAX(seq), 0) + 1 AS seq
                        FROM session_events
                        WHERE tenant_id = $1 AND session_id = $2
                     )
                     INSERT INTO session_events
                        (id, tenant_id, session_id, seq, actor_type, actor_id, event_type, payload, created_at)
                     SELECT $3, $1, $2, next_seq.seq, $4, $5, $6, $7, $8
                     FROM next_seq
                     RETURNING id, session_id, seq, parent_event_id, actor_type, actor_id, event_type, payload, created_at",
                )
                .bind(self.tenant_id)
                .bind(session_id)
                .bind(Uuid::new_v4())
                .bind(actor_type)
                .bind(actor_id)
                .bind(event_type)
                .bind(payload)
                .bind(Utc::now())
                .fetch_one(pool)
                .await?;
                event_from_row(row)
            }
        }
    }

    pub(crate) async fn insert_tool_call(&self, tool_call: ToolCall) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .tool_calls
                    .insert(tool_call.id, tool_call.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO tool_calls
                        (id, tenant_id, session_id, event_id, tool_name, args, result, status, risk_level, policy_decision, started_at, completed_at, error, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
                )
                .bind(tool_call.id)
                .bind(self.tenant_id)
                .bind(tool_call.session_id)
                .bind(tool_call.event_id)
                .bind(&tool_call.tool_name)
                .bind(&tool_call.args)
                .bind(&tool_call.result)
                .bind(&tool_call.status)
                .bind(&tool_call.risk_level)
                .bind(&tool_call.policy_decision)
                .bind(tool_call.started_at)
                .bind(tool_call.completed_at)
                .bind(&tool_call.error)
                .bind(tool_call.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(tool_call)
    }

    pub(crate) async fn update_tool_call_status(
        &self,
        id: Uuid,
        status: &str,
        result: Option<Value>,
        error: Option<Value>,
    ) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let tool_call = store
                    .tool_calls
                    .get_mut(&id)
                    .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call.status = status.to_string();
                tool_call.completed_at = Some(Utc::now());
                tool_call.result = result;
                tool_call.error = error;
                Ok(tool_call.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE tool_calls
                     SET status = $1, result = $2, error = $3, completed_at = now()
                     WHERE tenant_id = $4 AND id = $5
                     RETURNING id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at",
                )
                .bind(status)
                .bind(result)
                .bind(error)
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn get_tool_call(&self, id: Uuid) -> Result<ToolCall, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .tool_calls
                .get(&id)
                .cloned()
                .ok_or_else(|| AppError::not_found("tool call not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                     FROM tool_calls
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(self.tenant_id)
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("tool call not found"))?;
                tool_call_from_row(row)
            }
        }
    }

    pub(crate) async fn list_tool_calls(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<ToolCall>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut calls: Vec<_> = inner
                    .read()
                    .await
                    .tool_calls
                    .values()
                    .filter(|call| session_id.is_none_or(|id| call.session_id == id))
                    .cloned()
                    .collect();
                calls.sort_by_key(|call| call.created_at);
                calls.reverse();
                Ok(calls)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, event_id, tool_name, args, status, risk_level, policy_decision, result, error, started_at, completed_at, created_at
                             FROM tool_calls
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(tool_call_from_row).collect()
            }
        }
    }

    pub(crate) async fn insert_artifact(&self, artifact: Artifact) -> Result<Artifact, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .artifacts
                    .insert(artifact.id, artifact.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO artifacts (id, tenant_id, session_id, artifact_type, name, path, content, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(artifact.id)
                .bind(self.tenant_id)
                .bind(artifact.session_id)
                .bind(&artifact.artifact_type)
                .bind(&artifact.name)
                .bind(&artifact.path)
                .bind(&artifact.content)
                .bind(artifact.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(artifact)
    }

    pub(crate) async fn list_artifacts(&self, session_id: Uuid) -> Result<Vec<Artifact>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .artifacts
                .values()
                .filter(|artifact| artifact.session_id == session_id)
                .cloned()
                .collect()),
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, artifact_type, name, path, content, created_at
                     FROM artifacts
                     WHERE tenant_id = $1 AND session_id = $2
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .bind(session_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(artifact_from_row).collect()
            }
        }
    }

    pub(crate) async fn insert_approval(&self, approval: Approval) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .approvals
                    .insert(approval.id, approval.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO approvals (id, tenant_id, session_id, tool_call_id, action, risk_level, reason, evidence, status, created_at, decided_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(approval.id)
                .bind(self.tenant_id)
                .bind(approval.session_id)
                .bind(approval.tool_call_id)
                .bind(&approval.action)
                .bind(&approval.risk_level)
                .bind(&approval.reason)
                .bind(&approval.evidence)
                .bind(&approval.status)
                .bind(approval.created_at)
                .bind(approval.decided_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(approval)
    }

    pub(crate) async fn list_approvals(&self) -> Result<Vec<Approval>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                Ok(inner.read().await.approvals.values().cloned().collect())
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, session_id, tool_call_id, action, risk_level, reason, evidence, status, created_at, decided_at
                     FROM approvals
                     WHERE tenant_id = $1
                     ORDER BY created_at DESC",
                )
                .bind(self.tenant_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(approval_from_row).collect()
            }
        }
    }

    pub(crate) async fn decide_approval(
        &self,
        approval_id: Uuid,
        status: &str,
    ) -> Result<Approval, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let approval = store
                    .approvals
                    .get_mut(&approval_id)
                    .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval.status = status.to_string();
                approval.decided_at = Some(Utc::now());
                Ok(approval.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE approvals
                     SET status = $1, decided_at = now()
                     WHERE tenant_id = $2 AND id = $3
                     RETURNING id, session_id, tool_call_id, action, risk_level, reason, evidence, status, created_at, decided_at",
                )
                .bind(status)
                .bind(self.tenant_id)
                .bind(approval_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("approval not found"))?;
                approval_from_row(row)
            }
        }
    }

    pub(crate) async fn append_audit_log(&self, audit_log: AuditLog) -> Result<AuditLog, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                inner
                    .write()
                    .await
                    .audit_logs
                    .insert(audit_log.id, audit_log.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO audit_logs
                        (id, tenant_id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(audit_log.id)
                .bind(self.tenant_id)
                .bind(audit_log.session_id)
                .bind(&audit_log.actor_type)
                .bind(audit_log.actor_id)
                .bind(&audit_log.action)
                .bind(&audit_log.resource_type)
                .bind(audit_log.resource_id)
                .bind(&audit_log.details)
                .bind(audit_log.created_at)
                .execute(pool)
                .await?;
            }
        }
        Ok(audit_log)
    }

    pub(crate) async fn list_audit_logs(
        &self,
        session_id: Option<Uuid>,
    ) -> Result<Vec<AuditLog>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut logs: Vec<_> = inner
                    .read()
                    .await
                    .audit_logs
                    .values()
                    .filter(|log| session_id.is_none_or(|id| log.session_id == Some(id)))
                    .cloned()
                    .collect();
                logs.sort_by_key(|log| log.created_at);
                logs.reverse();
                Ok(logs)
            }
            StoreBackend::Postgres(pool) => {
                let rows = match session_id {
                    Some(session_id) => {
                        sqlx::query(
                            "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                             FROM audit_logs
                             WHERE tenant_id = $1 AND session_id = $2
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .bind(session_id)
                        .fetch_all(pool)
                        .await?
                    }
                    None => {
                        sqlx::query(
                            "SELECT id, session_id, actor_type, actor_id, action, resource_type, resource_id, details, created_at
                             FROM audit_logs
                             WHERE tenant_id = $1
                             ORDER BY created_at DESC",
                        )
                        .bind(self.tenant_id)
                        .fetch_all(pool)
                        .await?
                    }
                };
                rows.into_iter().map(audit_log_from_row).collect()
            }
        }
    }

    pub(crate) async fn seed_demo_agent(&self) -> Result<(), AppError> {
        let agent = Agent {
            id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid"),
            name: "Generic Orchestrator Agent".to_string(),
            kind: "orchestrator".to_string(),
            provider: "openai-compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            system_prompt: "You are a general-purpose orchestrator. Use tools through the runtime only, request approval before risky actions, and preserve an auditable timeline.".to_string(),
            tools: vec![
                "file.read".to_string(),
                "file.write".to_string(),
                "sql.get_schema".to_string(),
                "sql.query".to_string(),
                "shell.exec".to_string(),
                "codex.exec".to_string(),
                "approval.request".to_string(),
                "artifact.create".to_string(),
            ],
            created_at: Utc::now(),
        };

        match &self.store {
            StoreBackend::Memory(inner) => {
                inner.write().await.agents.insert(agent.id, agent.clone());
            }
            StoreBackend::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO agents (id, tenant_id, name, kind, provider, model, system_prompt, tools, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     ON CONFLICT (id) DO NOTHING",
                )
                .bind(agent.id)
                .bind(self.tenant_id)
                .bind(&agent.name)
                .bind(&agent.kind)
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
        Ok(())
    }
}
