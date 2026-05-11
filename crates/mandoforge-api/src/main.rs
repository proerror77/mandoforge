use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse, sse::Event, sse::KeepAlive},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{
    PgPool, Row,
    postgres::{PgPoolOptions, PgRow},
};
use tokio::{process::Command, sync::RwLock};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    store: StoreBackend,
    #[allow(dead_code)]
    workspace_root: PathBuf,
    tenant_id: Uuid,
}

#[derive(Default)]
struct MemoryStore {
    agents: HashMap<Uuid, Agent>,
    sessions: HashMap<Uuid, Session>,
    events: HashMap<Uuid, Vec<SessionEvent>>,
    approvals: HashMap<Uuid, Approval>,
    artifacts: HashMap<Uuid, Artifact>,
    tool_calls: HashMap<Uuid, ToolCall>,
    audit_logs: HashMap<Uuid, AuditLog>,
}

#[derive(Clone)]
enum StoreBackend {
    Memory(Arc<RwLock<MemoryStore>>),
    Postgres(PgPool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Agent {
    id: Uuid,
    name: String,
    kind: String,
    provider: String,
    model: String,
    system_prompt: String,
    tools: Vec<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateAgent {
    name: String,
    #[serde(default = "default_agent_kind")]
    kind: String,
    #[serde(default = "default_provider")]
    provider: String,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default)]
    system_prompt: String,
    #[serde(default)]
    tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    id: Uuid,
    agent_id: Uuid,
    title: String,
    status: SessionStatus,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionStatus {
    Created,
    Running,
    WaitingApproval,
    Completed,
    Failed,
}

impl SessionStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl From<String> for SessionStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "running" => Self::Running,
            "waiting_approval" => Self::WaitingApproval,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Created,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateSession {
    agent_id: Uuid,
    #[serde(default = "default_session_title")]
    title: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddMessage {
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionEvent {
    id: Uuid,
    session_id: Uuid,
    seq: i64,
    parent_event_id: Option<Uuid>,
    actor_type: String,
    actor_id: Option<Uuid>,
    event_type: String,
    payload: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Approval {
    id: Uuid,
    session_id: Uuid,
    tool_call_id: Option<Uuid>,
    action: String,
    risk_level: String,
    reason: String,
    evidence: Value,
    status: String,
    created_at: DateTime<Utc>,
    decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    id: Uuid,
    session_id: Uuid,
    event_id: Option<Uuid>,
    tool_name: String,
    args: Value,
    status: String,
    risk_level: String,
    policy_decision: Value,
    result: Option<Value>,
    error: Option<Value>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditLog {
    id: Uuid,
    session_id: Option<Uuid>,
    actor_type: String,
    actor_id: Option<Uuid>,
    action: String,
    resource_type: String,
    resource_id: Option<Uuid>,
    details: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Artifact {
    id: Uuid,
    session_id: Uuid,
    artifact_type: String,
    name: String,
    path: Option<String>,
    content: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ToolDescriptor {
    name: &'static str,
    risk: &'static str,
    description: &'static str,
}

#[derive(Debug, Deserialize)]
struct ExecuteTool {
    session_id: Uuid,
    args: Value,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexRequest {
    task: String,
    #[serde(default = "default_sandbox")]
    sandbox_mode: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let workspace_root = std::env::var("MANDOFORGE_WORKSPACE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".mandoforge/workspaces"));
    tokio::fs::create_dir_all(&workspace_root).await?;

    let tenant_id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("valid uuid");
    let store = match std::env::var("DATABASE_URL") {
        Ok(database_url) if !database_url.trim().is_empty() => {
            let pool = PgPoolOptions::new()
                .max_connections(8)
                .connect(&database_url)
                .await
                .context("failed to connect to Postgres")?;
            run_migrations(&pool).await?;
            seed_demo_tenant(&pool, tenant_id).await?;
            StoreBackend::Postgres(pool)
        }
        _ => StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
    };

    let state = AppState {
        store,
        workspace_root,
        tenant_id,
    };
    state
        .seed_demo_agent()
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/messages", post(add_message))
        .route("/api/sessions/{id}/run", post(run_session))
        .route("/api/sessions/{id}/events", get(list_events))
        .route("/api/sessions/{id}/stream", get(stream_events))
        .route("/api/sessions/{id}/artifacts", get(list_artifacts))
        .route(
            "/api/sessions/{id}/tool-calls",
            get(list_session_tool_calls),
        )
        .route(
            "/api/sessions/{id}/audit-logs",
            get(list_session_audit_logs),
        )
        .route("/api/tools", get(list_tools))
        .route("/api/tools/{name}/execute", post(execute_tool))
        .route("/api/tool-calls", get(list_tool_calls))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve))
        .route("/api/approvals/{id}/reject", post(reject))
        .route("/api/audit-logs", get(list_audit_logs))
        .fallback_service(ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("MANDOFORGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .context("invalid MANDOFORGE_ADDR")?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "mandoforge api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn run_migrations(pool: &PgPool) -> Result<()> {
    for path in [
        "db/migrations/0001_core.sql",
        "db/migrations/0002_generic_demo.sql",
    ] {
        let sql = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read migration {path}"))?;
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .with_context(|| format!("failed to execute migration {path}"))?;
    }
    Ok(())
}

async fn seed_demo_tenant(pool: &PgPool, tenant_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO tenants (id, name, slug)
         VALUES ($1, 'Demo Tenant', 'default')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;

    if let Ok(seed_sql) = tokio::fs::read_to_string("db/seed/generic_demo.sql").await {
        sqlx::raw_sql(&seed_sql).execute(pool).await?;
    }
    Ok(())
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
    async fn list_agents(&self) -> Result<Vec<Agent>, AppError> {
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

    async fn create_agent(&self, input: CreateAgent) -> Result<Agent, AppError> {
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
                self.insert_agent_version(&agent, 1).await?;
            }
        }
        Ok(agent)
    }

    async fn insert_agent_version(&self, agent: &Agent, version: i32) -> Result<(), AppError> {
        if let StoreBackend::Postgres(pool) = &self.store {
            sqlx::query(
                    "INSERT INTO agent_versions (agent_id, version, model, system_prompt, tools, tool_names, approval_policy)
                 VALUES ($1, $2, $3, $4, $5, $5, '{}')
                 ON CONFLICT (agent_id, version) DO NOTHING",
            )
            .bind(agent.id)
            .bind(version)
            .bind(&agent.model)
            .bind(&agent.system_prompt)
            .bind(json!(agent.tools))
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<Session>, AppError> {
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

    async fn create_session(&self, input: CreateSession) -> Result<Session, AppError> {
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

    async fn agent_exists(&self, agent_id: Uuid) -> Result<bool, AppError> {
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

    async fn get_session(&self, id: Uuid) -> Result<Session, AppError> {
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

    async fn set_session_status(
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

    async fn list_events(&self, session_id: Uuid) -> Result<Vec<SessionEvent>, AppError> {
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

    async fn append_event(
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

    async fn insert_tool_call(&self, tool_call: ToolCall) -> Result<ToolCall, AppError> {
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

    async fn update_tool_call_status(
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

    async fn list_tool_calls(&self, session_id: Option<Uuid>) -> Result<Vec<ToolCall>, AppError> {
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

    async fn insert_artifact(&self, artifact: Artifact) -> Result<Artifact, AppError> {
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

    async fn list_artifacts(&self, session_id: Uuid) -> Result<Vec<Artifact>, AppError> {
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

    async fn insert_approval(&self, approval: Approval) -> Result<Approval, AppError> {
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

    async fn list_approvals(&self) -> Result<Vec<Approval>, AppError> {
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

    async fn decide_approval(&self, approval_id: Uuid, status: &str) -> Result<Approval, AppError> {
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

    async fn append_audit_log(&self, audit_log: AuditLog) -> Result<AuditLog, AppError> {
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

    async fn list_audit_logs(&self, session_id: Option<Uuid>) -> Result<Vec<AuditLog>, AppError> {
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

    async fn seed_demo_agent(&self) -> Result<(), AppError> {
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
                inner.write().await.agents.insert(agent.id, agent);
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
                self.insert_agent_version(&agent, 1).await?;
            }
        }
        Ok(())
    }
}

async fn list_agents(State(state): State<AppState>) -> Result<Json<Vec<Agent>>, AppError> {
    Ok(Json(state.list_agents().await?))
}

async fn create_agent(
    State(state): State<AppState>,
    Json(input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    Ok(Json(state.create_agent(input).await?))
}

async fn list_sessions(State(state): State<AppState>) -> Result<Json<Vec<Session>>, AppError> {
    Ok(Json(state.list_sessions().await?))
}

async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSession>,
) -> Result<Json<Session>, AppError> {
    Ok(Json(state.create_session(input).await?))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Session>, AppError> {
    Ok(Json(state.get_session(id).await?))
}

async fn add_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMessage>,
) -> Result<Json<SessionEvent>, AppError> {
    Ok(Json(
        state
            .append_event(
                "user",
                None,
                id,
                "user.message",
                json!({ "message": input.message }),
            )
            .await?,
    ))
}

fn new_audit_log(
    session_id: Option<Uuid>,
    actor_type: &str,
    actor_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    details: Value,
) -> AuditLog {
    AuditLog {
        id: Uuid::new_v4(),
        session_id,
        actor_type: actor_type.to_string(),
        actor_id,
        action: action.to_string(),
        resource_type: resource_type.to_string(),
        resource_id,
        details,
        created_at: Utc::now(),
    }
}

async fn record_completed_tool_call(
    state: &AppState,
    session_id: Uuid,
    tool_name: &str,
    risk_level: &str,
    args: Value,
    content: Value,
    summary: &str,
) -> Result<ToolCall, AppError> {
    let call_event = state
        .append_event(
            "tool",
            None,
            session_id,
            "tool.call",
            json!({"tool": tool_name, "args": args}),
        )
        .await?;
    let tool_call = state
        .insert_tool_call(ToolCall {
            id: Uuid::new_v4(),
            session_id,
            event_id: Some(call_event.id),
            tool_name: tool_name.to_string(),
            args,
            status: "running".to_string(),
            risk_level: risk_level.to_string(),
            policy_decision: json!({"decision": "allowed"}),
            result: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
            created_at: Utc::now(),
        })
        .await?;
    state
        .append_event(
            "system",
            None,
            session_id,
            "policy.allowed",
            json!({"tool_call_id": tool_call.id, "tool": tool_name, "risk_level": risk_level}),
        )
        .await?;
    let result = json!({"tool": tool_name, "summary": summary, "content": content});
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            session_id,
            "tool.result",
            result.clone(),
        )
        .await?;
    let tool_call = state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_name, "risk_level": risk_level, "summary": summary}),
        ))
        .await?;
    Ok(tool_call)
}

async fn run_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Session>, AppError> {
    state.set_session_status(id, SessionStatus::Running).await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "session.started",
            "session",
            Some(id),
            json!({"status": "running"}),
        ))
        .await?;

    state
        .append_event(
            "agent",
            None,
            id,
            "agent.plan",
            json!({
                "steps": [
                    "Read README and Stage 1 policy/config from the workspace",
                    "Query generic_demo.platform_events for recent session health",
                    "Request approval before shell execution or writing diagnostics.md",
                    "Create diagnostics.md as an artifact and emit a final summary"
                ]
            }),
        )
        .await?;

    record_completed_tool_call(
        &state,
        id,
        "file.read",
        "low",
        json!({"paths": ["README.md", "config/policy.stage1.yaml"]}),
        generic_file_read_summary(),
        "Read workspace README and config policy summary",
    )
    .await?;

    record_completed_tool_call(
        &state,
        id,
        "sql.get_schema",
        "low",
        json!({"schema": "generic_demo"}),
        generic_schema(),
        "Generic demo SQL schema loaded",
    )
    .await?;

    record_completed_tool_call(
        &state,
        id,
        "sql.query",
        "medium",
        json!({"sql": "select event_type, status, count(*) from generic_demo.platform_events where created_at >= now() - interval '24 hours' group by event_type, status"}),
        generic_diagnostics(),
        "Recent platform_events status query completed",
    )
    .await?;

    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: id,
        artifact_type: "markdown".to_string(),
        name: "diagnostics.md".to_string(),
        path: None,
        content: json!({
            "markdown": "# Runtime Diagnostics\n\nThe generic runtime processed recent platform events, confirmed approval gating for shell execution, and produced a replayable diagnostics artifact."
        }),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
        "system",
        Some(artifact.id),
        id,
        "artifact.created",
        json!({"artifact_id": artifact.id, "name": artifact.name, "artifact_type": artifact.artifact_type}),
    )
    .await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "artifact.created",
            "artifact",
            Some(artifact.id),
            json!({"name": artifact.name, "artifact_type": artifact.artifact_type}),
        ))
        .await?;

    let shell_call_event = state
        .append_event(
            "tool",
            None,
            id,
            "tool.call",
            json!({"tool": "shell.exec", "args": {"command": "ls -la && test -f README.md"}}),
        )
        .await?;
    let shell_tool_call = state
        .insert_tool_call(ToolCall {
            id: Uuid::new_v4(),
            session_id: id,
            event_id: Some(shell_call_event.id),
            tool_name: "shell.exec".to_string(),
            args: json!({"command": "ls -la && test -f README.md"}),
            status: "waiting_approval".to_string(),
            risk_level: "high".to_string(),
            policy_decision: json!({
                "decision": "requires_approval",
                "reason": "shell.exec requires human approval in Stage 1",
            }),
            result: None,
            error: None,
            started_at: None,
            completed_at: None,
            created_at: Utc::now(),
        })
        .await?;
    state
        .append_event(
            "system",
            None,
            id,
            "policy.requires_approval",
            json!({"tool_call_id": shell_tool_call.id, "tool": "shell.exec", "risk_level": "high"}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "policy.requires_approval",
            "tool_call",
            Some(shell_tool_call.id),
            json!({"tool": "shell.exec", "risk_level": "high"}),
        ))
        .await?;

    let approval = Approval {
        id: Uuid::new_v4(),
        session_id: id,
        tool_call_id: Some(shell_tool_call.id),
        action: "shell.exec".to_string(),
        risk_level: "high".to_string(),
        reason: "The diagnostics task requested a shell command. Stage 1 policy requires human approval before shell execution.".to_string(),
        evidence: json!({"command": "ls -la && test -f README.md", "sandbox_mode": "workspace_write", "network": "disabled"}),
        status: "pending".to_string(),
        created_at: Utc::now(),
        decided_at: None,
    };
    let approval = state.insert_approval(approval).await?;
    state
        .append_event(
        "system",
        Some(approval.id),
        id,
        "approval.requested",
        json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence}),
    )
    .await?;
    state
        .append_audit_log(new_audit_log(
            Some(id),
            "system",
            None,
            "approval.requested",
            "approval",
            Some(approval.id),
            json!({"tool_call_id": approval.tool_call_id, "action": approval.action, "risk_level": approval.risk_level}),
        ))
        .await?;

    state
        .append_event(
        "agent",
        None,
        id,
            "llm.response",
            json!({
                "final_report": {
                "summary": "Generic Runtime Diagnostics Demo reached the approval gate and produced a replayable artifact.",
                "files_read": ["README.md", "config/policy.stage1.yaml"],
                "sql_tables": ["generic_demo.platform_events", "generic_demo.sample_documents", "generic_demo.sample_metrics"],
                "policy_events": ["policy.requires_approval for shell.exec"],
                "artifacts": ["diagnostics.md"],
                "next_steps": [
                    "Implement real Tool trait execution for file.read, sql.query, shell.exec, and file.write",
                    "Persist tool_calls and audit_logs for every tool path",
                    "Replace mock harness events with provider-driven tool-call parsing",
                    "Wire approval resume to continue the same session after human decision"
                ]
            }
        }),
    )
    .await?;

    let session = state
        .set_session_status(id, SessionStatus::WaitingApproval)
        .await?;
    Ok(Json(session))
}

async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<SessionEvent>>, AppError> {
    Ok(Json(state.list_events(id).await?))
}

async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let events = state.list_events(id).await.unwrap_or_default();
    let stream = futures_util::stream::iter(events.into_iter().map(|event| {
        Ok(Event::default()
            .event(event.event_type.clone())
            .json_data(event)
            .unwrap_or_else(|_| Event::default().event("error").data("serialization failed")))
    }));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn list_tools() -> Json<Vec<ToolDescriptor>> {
    Json(vec![
        ToolDescriptor {
            name: "file.read",
            risk: "low",
            description: "Read files inside the session workspace",
        },
        ToolDescriptor {
            name: "file.write",
            risk: "medium",
            description: "Write files inside the session workspace after approval",
        },
        ToolDescriptor {
            name: "sql.get_schema",
            risk: "low",
            description: "Return generic demo SQL schema",
        },
        ToolDescriptor {
            name: "sql.query",
            risk: "medium",
            description: "Execute read-only SQL against generic demo data",
        },
        ToolDescriptor {
            name: "shell.exec",
            risk: "high",
            description: "Run a shell command in a controlled workspace after approval",
        },
        ToolDescriptor {
            name: "codex.exec",
            risk: "high",
            description: "Run Codex CLI in a session workspace",
        },
    ])
}

async fn execute_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(input): Json<ExecuteTool>,
) -> Result<Json<Value>, AppError> {
    let risk_level = tool_risk_level(&name);
    let requires_approval = matches!(name.as_str(), "file.write" | "shell.exec" | "codex.exec");
    let call_event = state
        .append_event(
            "tool",
            None,
            input.session_id,
            "tool.call",
            json!({"tool": name, "args": input.args}),
        )
        .await?;
    let tool_call = state
        .insert_tool_call(ToolCall {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            event_id: Some(call_event.id),
            tool_name: name.clone(),
            args: input.args.clone(),
            status: if requires_approval {
                "waiting_approval".to_string()
            } else {
                "running".to_string()
            },
            risk_level: risk_level.to_string(),
            policy_decision: if requires_approval {
                json!({"decision": "requires_approval", "reason": format!("{name} requires approval in Stage 1")})
            } else {
                json!({"decision": "allowed"})
            },
            result: None,
            error: None,
            started_at: Some(Utc::now()),
            completed_at: None,
            created_at: Utc::now(),
        })
        .await?;
    if requires_approval {
        let result = json!({"status": "approval_required", "reason": format!("{name} requires approval in Stage 1")});
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "policy.requires_approval",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "waiting_approval", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "tool.waiting_approval",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "risk_level": risk_level, "status": "waiting_approval"}),
            ))
            .await?;
        return Ok(Json(result));
    }
    let result = match name.as_str() {
        "sql.get_schema" => generic_schema(),
        "sql.query" => {
            let sql = input
                .args
                .get("sql")
                .and_then(Value::as_str)
                .unwrap_or_default();
            ensure_read_only_sql(sql)?;
            json!({"rows": generic_diagnostics(), "row_count": 4})
        }
        "file.read" => generic_file_read_summary(),
        _ => return Err(AppError::not_found("unknown tool")),
    };
    let status = if result.get("status").and_then(Value::as_str) == Some("approval_required") {
        "waiting_approval"
    } else {
        "completed"
    };
    let event_type = if status == "waiting_approval" {
        "policy.requires_approval"
    } else {
        "tool.result"
    };
    state
        .append_event(
            if status == "waiting_approval" {
                "system"
            } else {
                "tool"
            },
            Some(tool_call.id),
            input.session_id,
            event_type,
            json!({"tool_call_id": tool_call.id, "tool": name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, status, Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(input.session_id),
            "tool",
            Some(tool_call.id),
            if status == "waiting_approval" {
                "tool.waiting_approval"
            } else {
                "tool.completed"
            },
            "tool_call",
            Some(tool_call.id),
            json!({"tool": name, "risk_level": risk_level, "status": status}),
        ))
        .await?;
    Ok(Json(result))
}

async fn list_approvals(State(state): State<AppState>) -> Result<Json<Vec<Approval>>, AppError> {
    Ok(Json(state.list_approvals().await?))
}

async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Approval>, AppError> {
    decide_approval(state, id, "approved").await
}

async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Approval>, AppError> {
    decide_approval(state, id, "rejected").await
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Artifact>>, AppError> {
    Ok(Json(state.list_artifacts(id).await?))
}

async fn list_tool_calls(State(state): State<AppState>) -> Result<Json<Vec<ToolCall>>, AppError> {
    Ok(Json(state.list_tool_calls(None).await?))
}

async fn list_session_tool_calls(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ToolCall>>, AppError> {
    Ok(Json(state.list_tool_calls(Some(id)).await?))
}

async fn list_audit_logs(State(state): State<AppState>) -> Result<Json<Vec<AuditLog>>, AppError> {
    Ok(Json(state.list_audit_logs(None).await?))
}

async fn list_session_audit_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    Ok(Json(state.list_audit_logs(Some(id)).await?))
}

async fn decide_approval(
    state: AppState,
    approval_id: Uuid,
    status: &str,
) -> Result<Json<Approval>, AppError> {
    let updated = state.decide_approval(approval_id, status).await?;
    state
        .append_event(
            "user",
            Some(approval_id),
            updated.session_id,
            &format!("approval.{status}"),
            json!({"approval_id": approval_id, "decision": status}),
        )
        .await?;
    if status == "approved" {
        if let Some(tool_call_id) = updated.tool_call_id {
            state
                .update_tool_call_status(
                    tool_call_id,
                    "completed",
                    Some(json!({"approval": "approved"})),
                    None,
                )
                .await?;
        }
        state
            .set_session_status(updated.session_id, SessionStatus::Completed)
            .await?;
        state
            .append_event(
                "system",
                None,
                updated.session_id,
                "session.completed",
                json!({"reason": "pending approval resolved"}),
            )
            .await?;
    }
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "user",
            Some(approval_id),
            &format!("approval.{status}"),
            "approval",
            Some(approval_id),
            json!({"tool_call_id": updated.tool_call_id, "decision": status}),
        ))
        .await?;
    Ok(Json(updated))
}

#[allow(dead_code)]
async fn run_codex(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
) -> Result<Value, AppError> {
    if request.sandbox_mode != "read-only" && request.sandbox_mode != "workspace-write" {
        return Err(AppError::bad_request(
            "codex sandbox mode requires approval",
        ));
    }
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let last_message = workspace.join("last_message.md");

    state
        .append_event(
            "tool",
            None,
            session_id,
            "codex.task.started",
            json!({"task": request.task, "sandbox_mode": request.sandbox_mode, "workspace": workspace}),
        )
        .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(180),
        Command::new("codex")
            .arg("exec")
            .arg("--sandbox")
            .arg(&request.sandbox_mode)
            .arg("--json")
            .arg("--output-last-message")
            .arg(&last_message)
            .arg("--cd")
            .arg(&workspace)
            .arg(&request.task)
            .output(),
    )
    .await
    .map_err(|_| AppError::bad_request("codex exec timed out"))??;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let final_message = tokio::fs::read_to_string(&last_message)
        .await
        .unwrap_or_default();
    let event_type = if output.status.success() {
        "codex.task.completed"
    } else {
        "codex.task.failed"
    };
    state
        .append_event(
            "tool",
            None,
            session_id,
            event_type,
            json!({"exit_code": output.status.code(), "stdout": stdout, "stderr": stderr, "final_message": final_message}),
        )
        .await?;
    Ok(
        json!({"status": output.status.code(), "stdout": stdout, "stderr": stderr, "final_message": final_message}),
    )
}

fn ensure_read_only_sql(sql: &str) -> Result<(), AppError> {
    let lowered = sql.trim().to_lowercase();
    if lowered.matches(';').count() > 1 {
        return Err(AppError::bad_request("only one SQL statement is allowed"));
    }
    let blocked = [
        "insert", "update", "delete", "drop", "alter", "create", "truncate", "grant", "revoke",
        "copy", "call", "do",
    ];
    if blocked
        .iter()
        .any(|keyword| lowered.starts_with(keyword) || lowered.contains(&format!(" {keyword} ")))
    {
        return Err(AppError::bad_request(
            "sql.query only accepts read-only SQL",
        ));
    }
    if !lowered.starts_with("select")
        && !lowered.starts_with("with")
        && !lowered.starts_with("explain")
    {
        return Err(AppError::bad_request(
            "sql.query requires SELECT, WITH, or EXPLAIN",
        ));
    }
    Ok(())
}

fn tool_risk_level(name: &str) -> &'static str {
    match name {
        "file.read" | "sql.get_schema" | "approval.request" | "artifact.create" => "low",
        "file.write" | "sql.query" => "medium",
        "shell.exec" | "codex.exec" | "http.request" => "high",
        _ => "unknown",
    }
}

fn generic_file_read_summary() -> Value {
    json!({
        "files": [
            {
                "path": "README.md",
                "summary": "Rust-native Managed Agents runtime prototype with Postgres-backed event log and approval timeline."
            },
            {
                "path": "config/policy.stage1.yaml",
                "summary": "Generic Stage 1 policy requiring approval for shell.exec, codex.exec, file.write, and http.request."
            }
        ]
    })
}

fn generic_schema() -> Value {
    json!({
        "tables": {
            "generic_demo.platform_events": ["id", "session_id", "event_type", "status", "latency_ms", "payload", "created_at"],
            "generic_demo.sample_documents": ["id", "title", "body", "metadata", "created_at"],
            "generic_demo.sample_metrics": ["id", "metric_name", "metric_value", "dimensions", "observed_at"]
        },
        "metrics": {
            "sessions_started_24h": "count(*) where event_type = 'session.started'",
            "sessions_completed_24h": "count(*) where event_type = 'session.completed'",
            "approvals_requested_24h": "count(*) where event_type = 'policy.requires_approval'",
            "p95_latency_ms": "percentile_cont(0.95) within group (order by latency_ms)"
        }
    })
}

fn generic_diagnostics() -> Value {
    json!({
        "window": "24h",
        "sessions_started": 12,
        "sessions_completed": 9,
        "sessions_failed": 1,
        "approvals_requested": 3,
        "tool_success_rate": 0.91,
        "notable_events": [
            {"event_type": "policy.requires_approval", "status": "waiting_approval", "tool": "shell.exec"},
            {"event_type": "artifact.created", "status": "ok", "artifact": "diagnostics.md"},
            {"event_type": "session.failed", "status": "failed", "reason": "tool timeout"}
        ]
    })
}

fn default_agent_kind() -> String {
    "orchestrator".to_string()
}

fn default_provider() -> String {
    "openai-compatible".to_string()
}

fn default_model() -> String {
    "gpt-5.4-mini".to_string()
}

fn default_session_title() -> String {
    "Untitled session".to_string()
}

#[allow(dead_code)]
fn default_sandbox() -> String {
    "workspace-write".to_string()
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        error!(%error, "internal error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::from(anyhow::Error::from(error))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::bad_request(error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        error!(%error, "database error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl From<tokio::time::error::Elapsed> for AppError {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Self::bad_request("operation timed out")
    }
}

impl From<StatusCode> for AppError {
    fn from(status: StatusCode) -> Self {
        Self {
            status,
            message: status.to_string(),
        }
    }
}

impl From<anyhow::Error> for Box<AppError> {
    fn from(error: anyhow::Error) -> Self {
        Box::new(AppError::from(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_read_only_sql() {
        assert!(
            ensure_read_only_sql("select * from generic_demo.platform_events limit 10").is_ok()
        );
        assert!(ensure_read_only_sql("with daily as (select 1) select * from daily").is_ok());
        assert!(ensure_read_only_sql("explain select * from generic_demo.platform_events").is_ok());
    }

    #[test]
    fn rejects_write_sql() {
        for sql in [
            "update products set price = price * 0.9",
            "delete from orders",
            "drop table orders",
            "insert into products values ('x')",
            "select * from orders; delete from orders;",
        ] {
            assert!(ensure_read_only_sql(sql).is_err(), "{sql}");
        }
    }
}
