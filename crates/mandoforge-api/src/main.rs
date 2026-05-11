#[cfg(test)]
use std::path::Path as FsPath;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

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
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use uuid::Uuid;

mod execution;
mod execution_queue;
mod policy;
mod provider;
mod shell_runner;
mod store;

use execution::{
    ExecutionWorker, ExecutionWorkerOutcome, InlineExecutionWorker, QueueBackedExecutionWorker,
    run_execution_job,
};
#[cfg(test)]
use execution::{codex_jsonl_event_type, parse_codex_jsonl, truncate_output};
use execution_queue::ExecutionQueue;
#[cfg(test)]
use execution_queue::{ExecutionJobRequest, ExecutionJobStatus};
#[cfg(test)]
use policy::ensure_read_only_sql;
use policy::{PolicyConfig, ensure_read_only_sql_with_policy, load_policy_config};
#[cfg(test)]
use provider::parse_openai_compatible_provider_response;
use provider::{
    HarnessContext, MockProviderClient, OpenAiCompatibleProviderClient, ProviderClient,
    ProviderResponse,
};
#[cfg(test)]
use shell_runner::docker_shell_args;
use store::{MemoryStore, StoreBackend};

#[derive(Clone)]
struct AppState {
    store: StoreBackend,
    execution_queue: ExecutionQueue,
    execution_worker: Arc<dyn ExecutionWorker>,
    #[allow(dead_code)]
    workspace_root: PathBuf,
    tenant_id: Uuid,
    policy: PolicyConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentVersion {
    id: Uuid,
    agent_id: Uuid,
    version: i32,
    model: String,
    system_prompt: String,
    tools: Vec<String>,
    tool_names: Vec<String>,
    runtime_config: Value,
    approval_policy: Value,
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
    agent_version_id: Option<Uuid>,
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

    let execution_queue = match &store {
        StoreBackend::Memory(_) => ExecutionQueue::default(),
        StoreBackend::Postgres(pool) => ExecutionQueue::postgres(pool.clone(), tenant_id),
    };

    let state = AppState {
        store,
        execution_queue,
        execution_worker: execution_worker_from_env(),
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
        .route("/api/agents/{id}/versions", get(list_agent_versions))
        .route(
            "/api/agents/{id}/versions/{version}",
            get(get_agent_version),
        )
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
        .route("/api/execution-jobs", get(list_execution_jobs))
        .route(
            "/api/execution-jobs/{id}/run",
            post(run_execution_job_route),
        )
        .route("/api/audit-logs", get(list_audit_logs))
        .fallback_service(ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn execution_worker_from_env() -> Arc<dyn ExecutionWorker> {
    match std::env::var("MANDOFORGE_EXECUTION_WORKER")
        .unwrap_or_else(|_| "inline".to_string())
        .as_str()
    {
        "queue" | "queued" | "external" => Arc::new(QueueBackedExecutionWorker),
        _ => Arc::new(InlineExecutionWorker),
    }
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

async fn list_agents(State(state): State<AppState>) -> Result<Json<Vec<Agent>>, AppError> {
    Ok(Json(state.list_agents().await?))
}

async fn create_agent(
    State(state): State<AppState>,
    Json(input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    Ok(Json(state.create_agent(input).await?))
}

async fn list_agent_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AgentVersion>>, AppError> {
    Ok(Json(state.list_agent_versions(id).await?))
}

async fn get_agent_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, i32)>,
) -> Result<Json<AgentVersion>, AppError> {
    Ok(Json(state.get_agent_version(id, version).await?))
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
    let agent_version = state.agent_version_for_session(input.session_id).await?;
    let policy_decision = state
        .policy
        .evaluate_tool_for_agent_version(name, &agent_version);
    let call_event = state
        .append_event(
            "tool",
            None,
            input.session_id,
            "tool.call",
            json!({"tool": name, "args": input.args.clone(), "agent_version_id": agent_version.id, "agent_version": agent_version.version}),
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
                "agent_version_id": agent_version.id,
                "agent_version": agent_version.version,
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

async fn list_execution_jobs(
    State(state): State<AppState>,
) -> Result<Json<Vec<execution_queue::ExecutionJob>>, AppError> {
    Ok(Json(state.execution_queue.list().await?))
}

async fn run_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<execution_queue::ExecutionJob>, AppError> {
    let completed = run_execution_job(&state, id).await?;
    complete_session_after_approval(&state, completed.session_id).await?;
    Ok(Json(completed))
}

async fn list_session_audit_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    Ok(Json(state.list_audit_logs(Some(id)).await?))
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
        let outcome = state
            .execution_worker
            .execute_approved_tool(&state, &updated)
            .await?;
        match outcome {
            ExecutionWorkerOutcome::Completed => {
                complete_session_after_approval(&state, updated.session_id).await?;
            }
            ExecutionWorkerOutcome::Queued => {
                state
                    .set_session_status(updated.session_id, SessionStatus::Running)
                    .await?;
            }
        }
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

async fn complete_session_after_approval(
    state: &AppState,
    session_id: Uuid,
) -> Result<(), AppError> {
    state
        .set_session_status(session_id, SessionStatus::Completed)
        .await?;
    state
        .append_event(
            "system",
            None,
            session_id,
            "session.completed",
            json!({"reason": "pending approval resolved"}),
        )
        .await?;
    Ok(())
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
    fn truncates_execution_output_on_utf8_boundary() {
        let output = truncate_output("abc你好", 4);
        assert_eq!(output.text, "abc");
        assert_eq!(output.original_bytes, 9);
        assert!(output.truncated);

        let unchanged = truncate_output("short", 64);
        assert_eq!(unchanged.text, "short");
        assert_eq!(unchanged.original_bytes, 5);
        assert!(!unchanged.truncated);
    }

    #[tokio::test]
    async fn execution_queue_tracks_job_lifecycle() {
        let queue = ExecutionQueue::default();
        let request = ExecutionJobRequest {
            session_id: Uuid::new_v4(),
            approval_id: Uuid::new_v4(),
            tool_call_id: Uuid::new_v4(),
            tool_name: "codex.exec".to_string(),
        };

        let queued = queue.enqueue(request).await.expect("queue job");
        assert_eq!(queued.status, ExecutionJobStatus::Queued);
        assert_eq!(queue.list().await.expect("list jobs").len(), 1);

        let running = queue.start(queued.id).await.expect("start job");
        assert_eq!(running.status, ExecutionJobStatus::Running);
        assert!(running.started_at.is_some());

        let completed = queue.complete(queued.id).await.expect("complete job");
        assert_eq!(completed.status, ExecutionJobStatus::Completed);
        assert!(completed.completed_at.is_some());
    }

    #[tokio::test]
    async fn reads_agent_versions_for_agent() {
        let app = test_app().await;
        let created: Agent = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/agents",
                json!({
                    "name": "Versioned Agent",
                    "kind": "orchestrator",
                    "provider": "openai-compatible",
                    "model": "gpt-5.4-mini",
                    "system_prompt": "Keep a version record.",
                    "tools": ["file.read", "sql.query"]
                }),
            ),
        )
        .await;

        let versions: Vec<AgentVersion> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/agents/{}/versions", created.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].agent_id, created.id);
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[0].model, created.model);
        assert_eq!(versions[0].tool_names, created.tools);

        let version: AgentVersion = request_json(
            app,
            Request::builder()
                .uri(format!("/api/agents/{}/versions/1", created.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(version.id, versions[0].id);
        assert_eq!(version.system_prompt, created.system_prompt);
    }

    #[tokio::test]
    async fn sessions_bind_agent_version_and_enforce_tool_allowlist() {
        let app = test_app().await;
        let created: Agent = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/agents",
                json!({
                    "name": "Read Only Agent",
                    "kind": "orchestrator",
                    "provider": "openai-compatible",
                    "model": "gpt-5.4-mini",
                    "system_prompt": "Read only.",
                    "tools": ["file.read"]
                }),
            ),
        )
        .await;
        let versions: Vec<AgentVersion> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/agents/{}/versions", created.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let version = versions.first().expect("agent version");
        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": created.id, "title": "version policy"}),
            ),
        )
        .await;
        assert_eq!(session.agent_version_id, Some(version.id));

        let (status, error) = request_value(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/sql.get_schema/execute",
                json!({"session_id": session.id, "args": {}}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            error["error"].as_str(),
            Some("sql.get_schema is not enabled for agent version 1")
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
            call.tool_name == "sql.get_schema"
                && call.status == "denied"
                && call.policy_decision["decision"] == "denied"
                && call.policy_decision["agent_version_id"] == json!(version.id)
        }));
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
        test_app_with_worker(Arc::new(InlineExecutionWorker)).await
    }

    async fn test_app_with_worker(execution_worker: Arc<dyn ExecutionWorker>) -> Router {
        let state = AppState {
            store: StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
            execution_queue: ExecutionQueue::default(),
            execution_worker,
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

    #[tokio::test]
    async fn queue_backed_worker_defers_approved_tool_until_job_run() {
        let app = test_app_with_worker(Arc::new(QueueBackedExecutionWorker)).await;
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
                json!({"agent_id": agent.id, "title": "queued file write approval"}),
            ),
        )
        .await;

        let relative_path = format!("queued-{}.md", Uuid::new_v4());
        let content = "# Queued\n\nWorker drain.";
        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": relative_path,
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

        let workspace_file = test_workspace_root()
            .join(session.id.to_string())
            .join(&relative_path);
        assert!(
            tokio::fs::metadata(&workspace_file).await.is_err(),
            "queued approval should not run inline"
        );

        let jobs: Vec<execution_queue::ExecutionJob> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/execution-jobs")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let job = jobs
            .iter()
            .find(|job| job.approval_id == approved.id)
            .expect("execution job queued");
        assert_eq!(job.status, ExecutionJobStatus::Queued);

        let completed: execution_queue::ExecutionJob = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/execution-jobs/{}/run", job.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(completed.status, ExecutionJobStatus::Completed);

        let written = tokio::fs::read_to_string(workspace_file)
            .await
            .expect("queued worker run wrote file");
        assert_eq!(written, content);

        let completed_session: Session = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(completed_session.status, SessionStatus::Completed));
    }
}
