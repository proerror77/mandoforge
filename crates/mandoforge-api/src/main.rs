use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Component, Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
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
    policy: PolicyConfig,
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

#[derive(Debug, Clone, Serialize)]
struct HarnessContext {
    session_id: Uuid,
    event_count: usize,
    last_user_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderToolCall {
    tool_name: String,
    args: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderResponse {
    plan: Vec<String>,
    tool_calls: Vec<ProviderToolCall>,
}

#[async_trait]
trait ProviderClient: Send + Sync {
    fn name(&self) -> &'static str;

    async fn complete(&self, context: HarnessContext) -> Result<ProviderResponse, AppError>;
}

struct MockProviderClient;

struct OpenAiCompatibleProviderClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
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

#[async_trait]
trait ToolExecutor: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError>;
}

struct FileReadTool;
struct SqlSchemaTool;
struct SqlQueryTool;
struct ArtifactCreateTool;
struct ApprovalRequestTool;

#[derive(Debug, Deserialize)]
struct ExecuteTool {
    session_id: Uuid,
    args: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct PolicyConfig {
    #[serde(default)]
    blocked_tools: Vec<String>,
    #[serde(default)]
    approval_required: Vec<ApprovalRequiredRule>,
    #[serde(default)]
    allowed_tools: HashMap<String, Vec<String>>,
    #[serde(default)]
    sql_policy: SqlPolicy,
}

#[derive(Debug, Clone, Deserialize)]
struct ApprovalRequiredRule {
    tool: String,
    risk: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SqlPolicy {
    #[serde(default = "default_sql_max_rows")]
    max_rows: i64,
    #[serde(default)]
    blocked_keywords: Vec<String>,
}

#[derive(Debug, Clone)]
struct ToolPolicyDecision {
    decision: &'static str,
    risk_level: String,
    reason: String,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            blocked_tools: vec![
                "secret.read".to_string(),
                "production_db.write".to_string(),
                "system.network.unrestricted".to_string(),
                "shell.exec.unrestricted".to_string(),
            ],
            approval_required: vec![
                ApprovalRequiredRule {
                    tool: "shell.exec".to_string(),
                    risk: "high".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "codex.exec".to_string(),
                    risk: "high".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "file.write".to_string(),
                    risk: "medium".to_string(),
                },
                ApprovalRequiredRule {
                    tool: "http.request".to_string(),
                    risk: "high".to_string(),
                },
            ],
            allowed_tools: HashMap::from([(
                "generic-orchestrator-agent".to_string(),
                vec![
                    "file.read".to_string(),
                    "file.write".to_string(),
                    "sql.get_schema".to_string(),
                    "sql.query".to_string(),
                    "shell.exec".to_string(),
                    "codex.exec".to_string(),
                    "approval.request".to_string(),
                    "artifact.create".to_string(),
                ],
            )]),
            sql_policy: SqlPolicy {
                max_rows: default_sql_max_rows(),
                blocked_keywords: vec![
                    "INSERT".to_string(),
                    "UPDATE".to_string(),
                    "DELETE".to_string(),
                    "DROP".to_string(),
                    "ALTER".to_string(),
                    "CREATE".to_string(),
                    "TRUNCATE".to_string(),
                    "GRANT".to_string(),
                    "REVOKE".to_string(),
                    "COPY".to_string(),
                    "CALL".to_string(),
                    "DO".to_string(),
                ],
            },
        }
    }
}

impl Default for SqlPolicy {
    fn default() -> Self {
        Self {
            max_rows: default_sql_max_rows(),
            blocked_keywords: PolicyConfig::default().sql_policy.blocked_keywords,
        }
    }
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
    let policy = load_policy_config("config/policy.stage1.yaml").await?;

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
        policy,
    };
    state
        .seed_demo_agent()
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;

    let app = build_router(state);

    let addr: SocketAddr = std::env::var("MANDOFORGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .context("invalid MANDOFORGE_ADDR")?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "mandoforge api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
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
        .with_state(state)
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn load_policy_config(path: &str) -> Result<PolicyConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read policy config {path}"))?;
    serde_yml::from_str(&content).with_context(|| format!("failed to parse policy config {path}"))
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

    async fn get_tool_call(&self, id: Uuid) -> Result<ToolCall, AppError> {
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

#[async_trait]
impl ProviderClient for MockProviderClient {
    fn name(&self) -> &'static str {
        "mock-openai-compatible"
    }

    async fn complete(&self, _context: HarnessContext) -> Result<ProviderResponse, AppError> {
        Ok(ProviderResponse {
            plan: vec![
                "Read README and Stage 1 policy/config from the workspace".to_string(),
                "Query generic_demo.platform_events for recent session health".to_string(),
                "Request approval before shell execution or writing diagnostics.md".to_string(),
                "Create diagnostics.md as an artifact and emit a final summary".to_string(),
            ],
            tool_calls: vec![
                ProviderToolCall {
                    tool_name: "file.read".to_string(),
                    args: json!({"paths": ["README.md", "config/policy.stage1.yaml"]}),
                },
                ProviderToolCall {
                    tool_name: "sql.get_schema".to_string(),
                    args: json!({"schema": "generic_demo"}),
                },
                ProviderToolCall {
                    tool_name: "sql.query".to_string(),
                    args: json!({"sql": "select event_type, status, count(*) from generic_demo.platform_events where created_at >= now() - interval '24 hours' group by event_type, status"}),
                },
                ProviderToolCall {
                    tool_name: "shell.exec".to_string(),
                    args: json!({"command": "pwd"}),
                },
            ],
        })
    }
}

impl OpenAiCompatibleProviderClient {
    fn from_env() -> Result<Option<Self>, AppError> {
        let Ok(base_url) = std::env::var("MANDOFORGE_PROVIDER_BASE_URL") else {
            return Ok(None);
        };
        let Ok(api_key) = std::env::var("MANDOFORGE_PROVIDER_API_KEY") else {
            return Ok(None);
        };
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() || api_key.trim().is_empty() {
            return Ok(None);
        }
        let model = std::env::var("MANDOFORGE_PROVIDER_MODEL")
            .unwrap_or_else(|_| default_model())
            .trim()
            .to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Some(Self {
            base_url,
            api_key,
            model,
            client,
        }))
    }
}

#[async_trait]
impl ProviderClient for OpenAiCompatibleProviderClient {
    fn name(&self) -> &'static str {
        "openai-compatible-http"
    }

    async fn complete(&self, context: HarnessContext) -> Result<ProviderResponse, AppError> {
        let endpoint = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are MandoForge's Stage 1 provider harness. Return tool calls only for the supplied generic runtime session. Use available tools through the runtime policy path."
                },
                {
                    "role": "user",
                    "content": serde_json::to_string(&context)?
                }
            ],
            "tools": provider_tool_schemas(),
            "tool_choice": "auto"
        });
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "provider request failed with status {status}: {}",
                redact_provider_error(&value)
            )));
        }
        parse_openai_compatible_provider_response(&value)
    }
}

fn provider_tool_schemas() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "file.read",
                "description": "Read summarized files from the session workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {"paths": {"type": "array", "items": {"type": "string"}}},
                    "required": ["paths"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "sql.get_schema",
                "description": "Return the generic demo database schema.",
                "parameters": {
                    "type": "object",
                    "properties": {"schema": {"type": "string"}},
                    "required": ["schema"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "sql.query",
                "description": "Execute read-only SQL against generic demo data.",
                "parameters": {
                    "type": "object",
                    "properties": {"sql": {"type": "string"}},
                    "required": ["sql"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "shell.exec",
                "description": "Request approval to run a shell command in the session workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {"command": {"type": "string"}},
                    "required": ["command"]
                }
            }
        }
    ])
}

fn parse_openai_compatible_provider_response(value: &Value) -> Result<ProviderResponse, AppError> {
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| AppError::bad_request("provider response missing choices[0].message"))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(parse_provider_tool_call)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let plan = provider_plan_from_content(content).unwrap_or_else(|| {
        vec![format!(
            "Provider returned {} runtime tool call(s)",
            tool_calls.len()
        )]
    });
    Ok(ProviderResponse { plan, tool_calls })
}

fn parse_provider_tool_call(value: &Value) -> Option<ProviderToolCall> {
    let function = value.get("function")?;
    let tool_name = function.get("name")?.as_str()?.to_string();
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let args =
        serde_json::from_str(arguments).unwrap_or_else(|_| json!({"raw_arguments": arguments}));
    Some(ProviderToolCall { tool_name, args })
}

fn provider_plan_from_content(content: &str) -> Option<Vec<String>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(plan) = value.get("plan").and_then(Value::as_array) {
            let steps: Vec<_> = plan
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            if !steps.is_empty() {
                return Some(steps);
            }
        }
    }
    Some(
        trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn redact_provider_error(value: &Value) -> String {
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("provider returned an error")
        .to_string()
}

async fn build_harness_context(
    state: &AppState,
    session_id: Uuid,
) -> Result<HarnessContext, AppError> {
    let events = state.list_events(session_id).await?;
    let last_user_message = events
        .iter()
        .rev()
        .find(|event| event.event_type == "user.message")
        .and_then(|event| event.payload.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(HarnessContext {
        session_id,
        event_count: events.len(),
        last_user_message,
    })
}

async fn run_provider_harness(
    state: &AppState,
    session_id: Uuid,
    provider: &dyn ProviderClient,
) -> Result<ProviderResponse, AppError> {
    let context = build_harness_context(state, session_id).await?;
    let provider_name = provider.name();
    state
        .append_event(
            "agent",
            None,
            session_id,
            "llm.request",
            json!({"provider": provider_name, "context": context}),
        )
        .await?;
    let response = provider.complete(context).await?;
    state
        .append_event(
            "agent",
            None,
            session_id,
            "llm.response",
            json!({"provider": provider_name, "tool_calls": response.tool_calls}),
        )
        .await?;
    Ok(response)
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

    let provider: Box<dyn ProviderClient> =
        if let Some(provider) = OpenAiCompatibleProviderClient::from_env()? {
            Box::new(provider)
        } else {
            Box::new(MockProviderClient)
        };
    let provider_response = run_provider_harness(&state, id, provider.as_ref()).await?;

    state
        .append_event(
            "agent",
            None,
            id,
            "agent.plan",
            json!({
                "steps": provider_response.plan
            }),
        )
        .await?;

    let mut waiting_for_approval = false;
    for tool_call in provider_response.tool_calls {
        let result = execute_tool_invocation(
            &state,
            &tool_call.tool_name,
            ExecuteTool {
                session_id: id,
                args: tool_call.args,
            },
        )
        .await?;
        if result.get("status").and_then(Value::as_str) == Some("approval_required") {
            waiting_for_approval = true;
            break;
        }
    }

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
                    "Add live external provider transport behind the ProviderClient trait",
                    "Add Docker-backed sandbox execution for shell workers",
                    "Run Postgres-backed sql.query integration verification"
                ]
            }
        }),
    )
    .await?;

    let session = if waiting_for_approval {
        state
            .set_session_status(id, SessionStatus::WaitingApproval)
            .await?
    } else {
        let session = state
            .set_session_status(id, SessionStatus::Completed)
            .await?;
        state
            .append_event(
                "system",
                None,
                id,
                "session.completed",
                json!({"reason": "provider tool loop completed"}),
            )
            .await?;
        session
    };
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

#[async_trait]
impl ToolExecutor for FileReadTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "file.read",
            risk: "low",
            description: "Read files inside the session workspace",
        }
    }

    async fn execute(
        &self,
        _state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        Ok(generic_file_read_summary())
    }
}

#[async_trait]
impl ToolExecutor for SqlSchemaTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "sql.get_schema",
            risk: "low",
            description: "Return generic demo SQL schema",
        }
    }

    async fn execute(
        &self,
        _state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        Ok(generic_schema())
    }
}

#[async_trait]
impl ToolExecutor for SqlQueryTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "sql.query",
            risk: "medium",
            description: "Execute read-only SQL against generic demo data",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let sql = input
            .args
            .get("sql")
            .and_then(Value::as_str)
            .unwrap_or_default();
        ensure_read_only_sql_with_policy(sql, &state.policy.sql_policy)?;
        match &state.store {
            StoreBackend::Postgres(pool) => {
                execute_postgres_sql_query(pool, sql, state.policy.sql_policy.max_rows).await
            }
            StoreBackend::Memory(_) => Ok(json!({"rows": generic_diagnostics(), "row_count": 4})),
        }
    }
}

#[async_trait]
impl ToolExecutor for ArtifactCreateTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "artifact.create",
            risk: "low",
            description: "Create a session artifact from normalized tool output",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let name = input
            .args
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("artifact.json")
            .to_string();
        let artifact_type = input
            .args
            .get("artifact_type")
            .or_else(|| input.args.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("json")
            .to_string();
        let path = input
            .args
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string);
        let content = input
            .args
            .get("content")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            artifact_type,
            name,
            path,
            content,
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                input.session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type, "tool_call_id": tool_call.id}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "artifact.created",
                "artifact",
                Some(artifact.id),
                json!({"name": artifact.name, "artifact_type": artifact.artifact_type}),
            ))
            .await?;
        Ok(
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
        )
    }
}

#[async_trait]
impl ToolExecutor for ApprovalRequestTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "approval.request",
            risk: "low",
            description: "Create an approval request linked to the current tool call",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        input: &ExecuteTool,
        tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let action = input
            .args
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("manual.approval")
            .to_string();
        let risk_level = input
            .args
            .get("risk_level")
            .or_else(|| input.args.get("risk"))
            .and_then(Value::as_str)
            .unwrap_or("medium")
            .to_string();
        let reason = input
            .args
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Tool requested human approval.")
            .to_string();
        let evidence = input
            .args
            .get("evidence")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action,
            risk_level,
            reason,
            evidence,
            status: "pending".to_string(),
            created_at: Utc::now(),
            decided_at: None,
        };
        let approval = state.insert_approval(approval).await?;
        state
            .append_event(
                "system",
                Some(approval.id),
                input.session_id,
                "approval.requested",
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence, "tool_call_id": tool_call.id}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "approval.requested",
                "approval",
                Some(approval.id),
                json!({"action": approval.action, "risk_level": approval.risk_level}),
            ))
            .await?;
        state
            .set_session_status(input.session_id, SessionStatus::WaitingApproval)
            .await?;
        Ok(json!({"status": "approval_requested", "approval_id": approval.id}))
    }
}

fn tool_registry() -> HashMap<&'static str, Box<dyn ToolExecutor>> {
    let tools: Vec<Box<dyn ToolExecutor>> = vec![
        Box::new(ArtifactCreateTool),
        Box::new(ApprovalRequestTool),
        Box::new(FileReadTool),
        Box::new(SqlSchemaTool),
        Box::new(SqlQueryTool),
    ];
    tools
        .into_iter()
        .map(|tool| (tool.descriptor().name, tool))
        .collect()
}

fn tool_descriptors() -> Vec<ToolDescriptor> {
    let mut descriptors: Vec<_> = tool_registry()
        .into_values()
        .map(|tool| tool.descriptor())
        .collect();
    descriptors.extend([
        ToolDescriptor {
            name: "file.write",
            risk: "medium",
            description: "Write files inside the session workspace after approval",
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
    ]);
    descriptors.sort_by_key(|descriptor| descriptor.name);
    descriptors
}

async fn list_tools() -> Json<Vec<ToolDescriptor>> {
    Json(tool_descriptors())
}

async fn execute_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(input): Json<ExecuteTool>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(execute_tool_invocation(&state, &name, input).await?))
}

async fn execute_tool_invocation(
    state: &AppState,
    name: &str,
    input: ExecuteTool,
) -> Result<Value, AppError> {
    let policy_decision = state.policy.evaluate_tool(&name);
    let call_event = state
        .append_event(
            "tool",
            None,
            input.session_id,
            "tool.call",
            json!({"tool": name, "args": input.args.clone()}),
        )
        .await?;
    let tool_call = state
        .insert_tool_call(ToolCall {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            event_id: Some(call_event.id),
            tool_name: name.to_string(),
            args: input.args.clone(),
            status: (match policy_decision.decision {
                "allowed" => "running",
                "requires_approval" => "waiting_approval",
                "denied" => "denied",
                _ => "denied",
            })
            .to_string(),
            risk_level: policy_decision.risk_level.clone(),
            policy_decision: json!({
                "decision": policy_decision.decision,
                "reason": policy_decision.reason.clone(),
            }),
            result: None,
            error: None,
            started_at: if policy_decision.decision == "allowed" {
                Some(Utc::now())
            } else {
                None
            },
            completed_at: None,
            created_at: Utc::now(),
        })
        .await?;

    if policy_decision.decision == "denied" {
        let result = json!({"status": "denied", "reason": policy_decision.reason.clone()});
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "policy.denied",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "denied", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                Some(tool_call.id),
                "policy.denied",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "risk_level": policy_decision.risk_level.clone(), "status": "denied"}),
            ))
            .await?;
        return Err(AppError::forbidden(
            result["reason"].as_str().unwrap_or("tool denied"),
        ));
    }

    if policy_decision.decision == "requires_approval" {
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action: name.to_string(),
            risk_level: policy_decision.risk_level.clone(),
            reason: policy_decision.reason.clone(),
            evidence: json!({"tool": name, "args": input.args}),
            status: "pending".to_string(),
            created_at: Utc::now(),
            decided_at: None,
        };
        let approval = state.insert_approval(approval).await?;
        let result = json!({
            "status": "approval_required",
            "approval_id": approval.id,
            "reason": policy_decision.reason.clone()
        });
        state
            .append_event(
                "system",
                Some(tool_call.id),
                input.session_id,
                "policy.requires_approval",
                json!({"tool_call_id": tool_call.id, "tool": name, "approval_id": approval.id, "content": result}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "waiting_approval", Some(result.clone()), None)
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                Some(tool_call.id),
                "policy.requires_approval",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "risk_level": policy_decision.risk_level.clone(), "status": "waiting_approval", "approval_id": approval.id}),
            ))
            .await?;
        state
            .append_event(
                "system",
                Some(approval.id),
                input.session_id,
                "approval.requested",
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence}),
            )
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "system",
                None,
                "approval.requested",
                "approval",
                Some(approval.id),
                json!({"tool_call_id": approval.tool_call_id, "action": approval.action, "risk_level": approval.risk_level}),
            ))
            .await?;
        state
            .set_session_status(input.session_id, SessionStatus::WaitingApproval)
            .await?;
        return Ok(result);
    }

    state
        .append_event(
            "system",
            Some(tool_call.id),
            input.session_id,
            "policy.allowed",
            json!({"tool_call_id": tool_call.id, "tool": name, "risk_level": policy_decision.risk_level.clone()}),
        )
        .await?;

    let registry = tool_registry();
    let Some(executor) = registry.get(name) else {
        let error_payload = json!({"error": "unknown tool"});
        state
            .append_event(
                "tool",
                Some(tool_call.id),
                input.session_id,
                "tool.error",
                json!({"tool_call_id": tool_call.id, "tool": name, "content": error_payload}),
            )
            .await?;
        state
            .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
            .await?;
        state
            .append_audit_log(new_audit_log(
                Some(input.session_id),
                "tool",
                Some(tool_call.id),
                "tool.failed",
                "tool_call",
                Some(tool_call.id),
                json!({"tool": name, "error": error_payload}),
            ))
            .await?;
        return Err(AppError::not_found("unknown tool"));
    };
    let result = match executor.execute(&state, &input, &tool_call).await {
        Ok(result) => result,
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    input.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(input.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": name, "error": error_payload}),
                ))
                .await?;
            return Err(error);
        }
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
            json!({"tool": name, "risk_level": policy_decision.risk_level, "status": status}),
        ))
        .await?;
    Ok(result)
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

async fn execute_approved_tool(state: &AppState, approval: &Approval) -> Result<(), AppError> {
    let Some(tool_call_id) = approval.tool_call_id else {
        return Ok(());
    };
    let tool_call = state.get_tool_call(tool_call_id).await?;
    match tool_call.tool_name.as_str() {
        "file.write" => execute_approved_file_write(state, approval, &tool_call).await,
        "shell.exec" => execute_approved_shell(state, approval, &tool_call).await,
        "codex.exec" => execute_approved_codex(state, approval, &tool_call).await,
        _ => {
            state
                .update_tool_call_status(
                    tool_call_id,
                    "completed",
                    Some(json!({"approval": "approved"})),
                    None,
                )
                .await?;
            Ok(())
        }
    }
}

async fn execute_approved_file_write(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let relative_path = tool_call
        .args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("diagnostics.md");
    let content = tool_call
        .args
        .get("content")
        .or_else(|| tool_call.args.get("markdown"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let workspace = session_workspace(state, approval.session_id).await?;
    let output_path = safe_workspace_path(&workspace, relative_path)?;
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&output_path, content).await?;

    let result = json!({
        "approval": "approved",
        "path": relative_path,
        "bytes": content.len(),
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: approval.session_id,
        artifact_type: "file".to_string(),
        name: FsPath::new(relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(relative_path)
            .to_string(),
        path: Some(relative_path.to_string()),
        content: json!({"text": content}),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
            "system",
            Some(artifact.id),
            approval.session_id,
            "artifact.created",
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "path": relative_path, "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_shell(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let command = tool_call
        .args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("shell.exec requires command"))?;
    let workspace = session_workspace(state, approval.session_id).await?;
    let runner = shell_runner();
    let mut process = shell_command(&runner, &workspace, command);
    let output = tokio::time::timeout(Duration::from_secs(30), process.output())
        .await
        .map_err(|_| AppError::bad_request("shell.exec timed out"))??;

    let result = json!({
        "approval": "approved",
        "command": command,
        "runner": runner,
        "workspace": workspace.display().to_string(),
        "exit_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
        "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "command": command, "runner": runner, "exit_code": output.status.code(), "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

fn shell_runner() -> String {
    std::env::var("MANDOFORGE_SHELL_RUNNER")
        .unwrap_or_else(|_| "host".to_string())
        .trim()
        .to_string()
}

fn shell_command(runner: &str, workspace: &FsPath, command: &str) -> Command {
    if runner == "docker" {
        let image = std::env::var("MANDOFORGE_SHELL_DOCKER_IMAGE")
            .unwrap_or_else(|_| "alpine:3.20".to_string());
        let mut process = Command::new("docker");
        process.args(docker_shell_args(workspace, &image, command));
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command).current_dir(workspace);
        process
    }
}

fn docker_shell_args(workspace: &FsPath, image: &str, command: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network".to_string(),
        "none".to_string(),
        "--cpus".to_string(),
        "1".to_string(),
        "--memory".to_string(),
        "512m".to_string(),
        "-v".to_string(),
        format!("{}:/workspace", workspace.display()),
        "-w".to_string(),
        "/workspace".to_string(),
        image.to_string(),
        "sh".to_string(),
        "-lc".to_string(),
        command.to_string(),
    ]
}

async fn execute_approved_codex(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let request: CodexRequest = serde_json::from_value(tool_call.args.clone())?;
    match run_codex(state, approval.session_id, request).await {
        Ok(result) => {
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.result",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.completed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "resumed_after_approval": true}),
                ))
                .await?;
            Ok(())
        }
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "error": error_payload, "resumed_after_approval": true}),
                ))
                .await?;
            Err(error)
        }
    }
}

async fn session_workspace(state: &AppState, session_id: Uuid) -> Result<PathBuf, AppError> {
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    Ok(workspace)
}

fn safe_workspace_path(workspace: &FsPath, relative_path: &str) -> Result<PathBuf, AppError> {
    let path = FsPath::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(AppError::bad_request(
            "file.write path must stay inside the session workspace",
        ));
    }
    Ok(workspace.join(path))
}

async fn execute_postgres_sql_query(
    pool: &PgPool,
    sql: &str,
    max_rows: i64,
) -> Result<Value, AppError> {
    let query = wrap_read_only_sql_for_json(sql, max_rows);
    let rows: Value = sqlx::query_scalar(&query).fetch_one(pool).await?;
    let row_count = rows.as_array().map_or(0, Vec::len);
    Ok(json!({"rows": rows, "row_count": row_count}))
}

fn wrap_read_only_sql_for_json(sql: &str, max_rows: i64) -> String {
    let bounded_max_rows = max_rows.clamp(1, 5_000);
    let inner = sql.trim().trim_end_matches(';').trim();
    format!(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb) \
         FROM (SELECT * FROM ({inner}) AS query_result LIMIT {bounded_max_rows}) AS t"
    )
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
        execute_approved_tool(&state, &updated).await?;
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
    for event in parse_codex_jsonl(&stdout) {
        state
            .append_event(
                "tool",
                None,
                session_id,
                "codex.event",
                json!({"codex_event_type": codex_jsonl_event_type(&event), "event": event}),
            )
            .await?;
    }
    if !final_message.trim().is_empty() {
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id,
            artifact_type: "markdown".to_string(),
            name: "codex-final-message.md".to_string(),
            path: Some("codex-final-message.md".to_string()),
            content: json!({"markdown": final_message.clone()}),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
            )
            .await?;
    }
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

fn parse_codex_jsonl(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(line).ok()
        })
        .collect()
}

fn codex_jsonl_event_type(event: &Value) -> String {
    event
        .get("type")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn ensure_read_only_sql_with_policy(sql: &str, policy: &SqlPolicy) -> Result<(), AppError> {
    let lowered = sql.trim().to_lowercase();
    if lowered.matches(';').count() > 1 {
        return Err(AppError::bad_request("only one SQL statement is allowed"));
    }
    if policy.blocked_keywords.iter().any(|keyword| {
        let keyword = keyword.to_lowercase();
        lowered.starts_with(&keyword) || lowered.contains(&format!(" {keyword} "))
    }) {
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

#[cfg(test)]
fn ensure_read_only_sql(sql: &str) -> Result<(), AppError> {
    ensure_read_only_sql_with_policy(sql, &PolicyConfig::default().sql_policy)
}

impl PolicyConfig {
    fn evaluate_tool(&self, name: &str) -> ToolPolicyDecision {
        if self.blocked_tools.iter().any(|tool| tool == name) {
            return ToolPolicyDecision {
                decision: "denied",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!("{name} is blocked by config/policy.stage1.yaml"),
            };
        }

        if let Some(rule) = self.approval_required.iter().find(|rule| rule.tool == name) {
            return ToolPolicyDecision {
                decision: "requires_approval",
                risk_level: rule.risk.clone(),
                reason: format!("{name} requires approval by config/policy.stage1.yaml"),
            };
        }

        if self
            .allowed_tools
            .values()
            .any(|tools| tools.iter().any(|tool| tool == name))
        {
            return ToolPolicyDecision {
                decision: "allowed",
                risk_level: tool_risk_level(name).to_string(),
                reason: format!("{name} is allowed by config/policy.stage1.yaml"),
            };
        }

        ToolPolicyDecision {
            decision: "denied",
            risk_level: "unknown".to_string(),
            reason: format!("{name} is not allowed by config/policy.stage1.yaml"),
        }
    }
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

fn default_sql_max_rows() -> i64 {
    500
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

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
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

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        error!(%error, "provider transport error");
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: "provider transport error".to_string(),
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
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

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

    #[test]
    fn registry_contains_allowed_stage1_tools() {
        let registry = tool_registry();
        for expected in [
            "artifact.create",
            "approval.request",
            "file.read",
            "sql.get_schema",
            "sql.query",
        ] {
            assert!(registry.contains_key(expected), "missing {expected}");
        }

        let descriptors = tool_descriptors();
        for expected in ["file.write", "shell.exec", "codex.exec"] {
            assert!(
                descriptors
                    .iter()
                    .any(|descriptor| descriptor.name == expected),
                "missing descriptor {expected}"
            );
        }
    }

    #[test]
    fn wraps_read_only_sql_for_json_with_row_limit() {
        let wrapped =
            wrap_read_only_sql_for_json("select event_type from generic_demo.platform_events;", 25);
        assert!(wrapped.contains("jsonb_agg"));
        assert!(wrapped.contains("LIMIT 25"));
        assert!(wrapped.contains("select event_type from generic_demo.platform_events"));
        assert!(!wrapped.contains("platform_events;"));
    }

    #[test]
    fn parses_codex_jsonl_events_and_ignores_non_json_lines() {
        let events = parse_codex_jsonl(
            r#"{"type":"session.started","id":"a"}
not json
{"msg":"agent.message","text":"done"}
"#,
        );
        assert_eq!(events.len(), 2);
        assert_eq!(codex_jsonl_event_type(&events[0]), "session.started");
        assert_eq!(codex_jsonl_event_type(&events[1]), "agent.message");
    }

    #[test]
    fn parses_openai_compatible_tool_calls() {
        let response = json!({
            "choices": [{
                "message": {
                    "content": "{\"plan\":[\"Read files\",\"Query demo data\"]}",
                    "tool_calls": [
                        {
                            "type": "function",
                            "function": {
                                "name": "file.read",
                                "arguments": "{\"paths\":[\"README.md\"]}"
                            }
                        },
                        {
                            "type": "function",
                            "function": {
                                "name": "sql.query",
                                "arguments": "{\"sql\":\"select * from generic_demo.platform_events limit 1\"}"
                            }
                        }
                    ]
                }
            }]
        });
        let parsed =
            parse_openai_compatible_provider_response(&response).expect("provider response parses");
        assert_eq!(parsed.plan, vec!["Read files", "Query demo data"]);
        assert_eq!(parsed.tool_calls.len(), 2);
        assert_eq!(parsed.tool_calls[0].tool_name, "file.read");
        assert_eq!(parsed.tool_calls[0].args["paths"][0], "README.md");
        assert_eq!(parsed.tool_calls[1].tool_name, "sql.query");
    }

    #[test]
    fn builds_docker_shell_runner_args_with_workspace_sandbox() {
        let workspace = FsPath::new("/tmp/mandoforge-session");
        let args = docker_shell_args(workspace, "alpine:3.20", "pwd");
        assert_eq!(args[0], "run");
        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"none".to_string()));
        assert!(args.contains(&"--memory".to_string()));
        assert!(args.contains(&"512m".to_string()));
        assert!(args.contains(&"/workspace".to_string()));
        assert!(args.contains(&"alpine:3.20".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("pwd"));
    }

    async fn test_app() -> Router {
        let state = AppState {
            store: StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
            workspace_root: test_workspace_root(),
            tenant_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("valid uuid"),
            policy: PolicyConfig::default(),
        };
        state.seed_demo_agent().await.expect("seed demo agent");
        build_router(state)
    }

    fn test_workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.mandoforge/test-workspaces")
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(app: Router, request: Request<Body>) -> T {
        let response = app.oneshot(request).await.expect("request succeeds");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&body).expect("json response")
    }

    async fn request_value(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.expect("request succeeds");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
        (status, value)
    }

    fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("valid request")
    }

    #[tokio::test]
    async fn generic_runtime_diagnostics_replay_api_flow() {
        let app = test_app().await;

        let agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let agent = agents
            .iter()
            .find(|agent| agent.name == "Generic Orchestrator Agent")
            .expect("seeded generic orchestrator agent");

        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({
                    "agent_id": agent.id,
                    "title": "Generic runtime diagnostics",
                    "message": "Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md."
                }),
            ),
        )
        .await;
        assert!(matches!(session.status, SessionStatus::Created));

        let running: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(running.status, SessionStatus::WaitingApproval));

        let events: Vec<SessionEvent> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let event_types: Vec<_> = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        for expected in [
            "user.message",
            "llm.request",
            "agent.plan",
            "tool.call",
            "tool.result",
            "policy.allowed",
            "policy.requires_approval",
            "approval.requested",
            "artifact.created",
            "llm.response",
        ] {
            assert!(
                event_types.contains(&expected),
                "missing event type {expected}: {event_types:?}"
            );
        }
        assert!(events.iter().any(|event| {
            event.event_type == "llm.response"
                && event
                    .payload
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|calls| calls.iter().any(|call| call["tool_name"] == "shell.exec"))
        }));

        let artifacts: Vec<Artifact> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/artifacts", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.name == "diagnostics.md")
        );

        let tool_calls: Vec<ToolCall> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        for expected in ["file.read", "sql.get_schema", "sql.query", "shell.exec"] {
            assert!(
                tool_calls.iter().any(|call| call.tool_name == expected),
                "missing tool call {expected}"
            );
        }
        assert!(tool_calls.iter().any(|call| call.tool_name == "shell.exec"
            && call.status == "waiting_approval"
            && call.policy_decision["decision"] == "requires_approval"));

        let audit_logs: Vec<AuditLog> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/audit-logs", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        for expected in [
            "session.started",
            "tool.completed",
            "artifact.created",
            "policy.requires_approval",
            "approval.requested",
        ] {
            assert!(
                audit_logs.iter().any(|log| log.action == expected),
                "missing audit log {expected}"
            );
        }

        let approvals: Vec<Approval> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/approvals")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let approval = approvals
            .iter()
            .find(|approval| approval.session_id == session.id && approval.status == "pending")
            .expect("pending shell approval");

        let approved: Approval = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{}/approve", approval.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(approved.status, "approved");

        let completed: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(completed.status, SessionStatus::Completed));

        let events_after_approval: Vec<SessionEvent> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let event_types_after_approval: Vec<_> = events_after_approval
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        assert!(event_types_after_approval.contains(&"approval.approved"));
        assert!(event_types_after_approval.contains(&"session.completed"));
        let tool_calls_after_approval: Vec<ToolCall> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let shell_call = tool_calls_after_approval
            .iter()
            .find(|call| call.tool_name == "shell.exec")
            .expect("shell call after approval");
        assert_eq!(shell_call.status, "completed");
        assert!(
            shell_call
                .result
                .as_ref()
                .and_then(|result| result["stdout"].as_str())
                .unwrap_or_default()
                .contains(&session.id.to_string())
        );
    }

    #[tokio::test]
    async fn loads_stage1_policy_from_yaml() {
        let policy_path = format!(
            "{}/../../config/policy.stage1.yaml",
            env!("CARGO_MANIFEST_DIR")
        );
        let policy = load_policy_config(&policy_path)
            .await
            .expect("load stage 1 policy");

        let shell = policy.evaluate_tool("shell.exec");
        assert_eq!(shell.decision, "requires_approval");
        assert_eq!(shell.risk_level, "high");

        let secret = policy.evaluate_tool("secret.read");
        assert_eq!(secret.decision, "denied");

        let file_read = policy.evaluate_tool("file.read");
        assert_eq!(file_read.decision, "allowed");

        let schema = policy.evaluate_tool("sql.get_schema");
        assert_eq!(schema.decision, "allowed");
    }

    #[tokio::test]
    async fn manual_tool_execution_uses_policy_for_approval_and_denial() {
        let app = test_app().await;
        let agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let agent = agents.first().expect("seeded agent");
        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": agent.id, "title": "manual tools"}),
            ),
        )
        .await;

        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/shell.exec/execute",
                json!({"session_id": session.id, "args": {"command": "pwd"}}),
            ),
        )
        .await;
        assert_eq!(approval_result["status"], "approval_required");
        assert!(approval_result.get("approval_id").is_some());

        let approvals: Vec<Approval> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/approvals")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(approvals.iter().any(|approval| {
            approval.session_id == session.id
                && approval.action == "shell.exec"
                && approval.status == "pending"
        }));

        let waiting: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(waiting.status, SessionStatus::WaitingApproval));

        let (status, error) = request_value(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/secret.read/execute",
                json!({"session_id": session.id, "args": {"name": "api_key"}}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("blocked")
        );

        let events: Vec<SessionEvent> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "policy.denied")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "approval.requested")
        );

        let tool_calls: Vec<ToolCall> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(tool_calls.iter().any(|call| {
            call.tool_name == "shell.exec"
                && call.status == "waiting_approval"
                && call.policy_decision["decision"] == "requires_approval"
        }));
        assert!(tool_calls.iter().any(|call| {
            call.tool_name == "secret.read"
                && call.status == "denied"
                && call.policy_decision["decision"] == "denied"
        }));
    }

    #[tokio::test]
    async fn sql_query_policy_failure_records_failed_tool_call() {
        let app = test_app().await;
        let agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let agent = agents.first().expect("seeded agent");
        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": agent.id, "title": "bad sql"}),
            ),
        )
        .await;

        let (status, error) = request_value(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/sql.query/execute",
                json!({"session_id": session.id, "args": {"sql": "delete from generic_demo.platform_events"}}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("read-only")
        );

        let events: Vec<SessionEvent> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(events.iter().any(|event| event.event_type == "tool.error"));

        let tool_calls: Vec<ToolCall> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(tool_calls.iter().any(|call| {
            call.tool_name == "sql.query" && call.status == "failed" && call.error.is_some()
        }));
    }

    #[tokio::test]
    async fn artifact_and_approval_tools_execute_through_registry() {
        let app = test_app().await;
        let agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let agent = agents.first().expect("seeded agent");
        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": agent.id, "title": "artifact and approval tools"}),
            ),
        )
        .await;

        let artifact_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/artifact.create/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "name": "summary.json",
                        "artifact_type": "json",
                        "content": {"summary": "ok"}
                    }
                }),
            ),
        )
        .await;
        assert_eq!(artifact_result["name"], "summary.json");

        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/approval.request/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "action": "manual.review",
                        "risk_level": "medium",
                        "reason": "Review generated summary",
                        "evidence": {"artifact": "summary.json"}
                    }
                }),
            ),
        )
        .await;
        assert_eq!(approval_result["status"], "approval_requested");

        let artifacts: Vec<Artifact> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/artifacts", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.name == "summary.json")
        );

        let approvals: Vec<Approval> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/approvals")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(approvals.iter().any(|approval| {
            approval.session_id == session.id
                && approval.action == "manual.review"
                && approval.status == "pending"
        }));

        let waiting: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(waiting.status, SessionStatus::WaitingApproval));

        let tool_calls: Vec<ToolCall> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        for expected in ["artifact.create", "approval.request"] {
            assert!(tool_calls.iter().any(|call| {
                call.tool_name == expected
                    && call.status == "completed"
                    && call.policy_decision["decision"] == "allowed"
            }));
        }
    }

    #[tokio::test]
    async fn approving_file_write_resumes_tool_and_creates_artifact() {
        let app = test_app().await;
        let agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let agent = agents.first().expect("seeded agent");
        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": agent.id, "title": "file write approval"}),
            ),
        )
        .await;

        let content = "# Diagnostics\n\nApproved write.";
        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": "diagnostics.md",
                        "content": content
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_result["approval_id"]
            .as_str()
            .expect("approval id");

        let approved: Approval = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{approval_id}/approve"))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(approved.status, "approved");

        let written = tokio::fs::read_to_string(
            test_workspace_root()
                .join(session.id.to_string())
                .join("diagnostics.md"),
        )
        .await
        .expect("approved file.write created workspace file");
        assert_eq!(written, content);

        let artifacts: Vec<Artifact> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/artifacts", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(artifacts.iter().any(|artifact| {
            artifact.name == "diagnostics.md" && artifact.path.as_deref() == Some("diagnostics.md")
        }));

        let tool_calls: Vec<ToolCall> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(tool_calls.iter().any(|call| {
            call.tool_name == "file.write"
                && call.status == "completed"
                && call
                    .result
                    .as_ref()
                    .and_then(|result| result["path"].as_str())
                    == Some("diagnostics.md")
        }));

        let events: Vec<SessionEvent> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(events.iter().any(|event| event.event_type == "tool.result"));
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "artifact.created")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "session.completed")
        );
    }
}
