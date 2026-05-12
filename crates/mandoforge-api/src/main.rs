#[cfg(test)]
use std::path::Path as FsPath;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse, sse::Event, sse::KeepAlive},
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

mod authorization;
mod execution;
mod execution_queue;
mod execution_queue_broker;
mod mcp_gateway;
mod observability;
mod policy;
mod provider;
mod secrets;
mod shell_runner;
mod store_approvals;
mod store_artifacts;
mod store_audit;
mod store_backend;
mod store_entities;
mod store_eval;
mod store_events;
mod store_governance;
mod store_rows;
mod store_seed;
mod store_tool_calls;
mod store_usage_rollups;

use authorization::{
    AuthorizationRequest, Authorizer, Permission, Principal, Role, RoleBasedAuthorizer,
};
use execution::{
    ExecutionWorker, ExecutionWorkerOutcome, InlineExecutionWorker, QueueBackedExecutionWorker,
    run_execution_job,
};
#[cfg(test)]
use execution::{codex_jsonl_event_type, parse_codex_jsonl, truncate_output};
use execution_queue::ExecutionQueue;
#[cfg(test)]
use execution_queue::{ExecutionJobRequest, ExecutionJobStatus, ExecutionQueueBackend};
#[cfg(test)]
use execution_queue_broker::{BrokerExecutionQueue, BrokerQueueKind};
use mcp_gateway::{
    HttpMcpGatewayClient, McpCallRequest, McpGatewayClient, McpGatewayConfig,
    ReservedMcpGatewayClient,
};
use observability::{
    HttpTelemetryExporter, ObservabilityConfig, ReservedTelemetryExporter, TelemetryEvent,
    TelemetryExporter,
};
#[cfg(test)]
use policy::ensure_read_only_sql;
use policy::{PolicyConfig, ensure_read_only_sql_with_policy, load_policy_config};
#[cfg(test)]
use provider::parse_openai_compatible_provider_response;
use provider::{
    HarnessContext, MockProviderClient, OpenAiCompatibleProviderClient, ProviderClient,
    ProviderResponse,
};
use secrets::secret_provider_from_env;
#[cfg(test)]
use shell_runner::docker_shell_args;
use store_backend::{MemoryStore, StoreBackend};

#[derive(Clone)]
struct AppState {
    store: StoreBackend,
    execution_queue: ExecutionQueue,
    execution_worker: Arc<dyn ExecutionWorker>,
    authorizer: Arc<dyn Authorizer>,
    observability_config: ObservabilityConfig,
    telemetry_exporter: Arc<dyn TelemetryExporter>,
    mcp_gateway_config: Option<McpGatewayConfig>,
    mcp_gateway_client: Arc<dyn McpGatewayClient>,
    #[allow(dead_code)]
    workspace_root: PathBuf,
    tenant_id: Uuid,
    policy: PolicyConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionQueueBackendSelection {
    Memory,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Agent {
    id: Uuid,
    name: String,
    kind: String,
    team_id: Option<Uuid>,
    project_id: Option<Uuid>,
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
    team_id: Option<Uuid>,
    #[serde(default)]
    project_id: Option<Uuid>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct ModifyApproval {
    args: Value,
    #[serde(default)]
    comment: Option<String>,
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
    decision_payload: Value,
    status: String,
    expires_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageSummary {
    session_count: usize,
    event_count: usize,
    provider_request_count: usize,
    provider_response_count: usize,
    tool_call_count: usize,
    tool_success_count: usize,
    tool_failed_count: usize,
    approval_count: usize,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    total_tool_duration_ms: i64,
    estimated_provider_cost_cents: f64,
    by_provider: HashMap<String, ProviderUsageSummary>,
    by_tool: HashMap<String, ToolUsageSummary>,
    provider_budgets: Vec<ProviderBudgetStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProviderUsageSummary {
    request_count: usize,
    response_count: usize,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    token_cost_cents: f64,
    estimated_cost_cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ToolUsageSummary {
    call_count: usize,
    success_count: usize,
    failed_count: usize,
    total_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderBudgetStatus {
    provider_name: String,
    status: String,
    window_hours: i64,
    request_count: i64,
    daily_request_limit: Option<i64>,
    request_budget_used_percent: Option<f64>,
    estimated_cost_cents: f64,
    projected_daily_cost_cents: f64,
    daily_cost_limit_cents: Option<f64>,
    cost_budget_used_percent: Option<f64>,
    messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Organization {
    id: Uuid,
    name: String,
    slug: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateOrganization {
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Team {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    slug: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateTeam {
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    id: Uuid,
    team_id: Uuid,
    name: String,
    slug: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateProject {
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Membership {
    id: Uuid,
    user_id: String,
    organization_id: Option<Uuid>,
    team_id: Option<Uuid>,
    project_id: Option<Uuid>,
    role: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateMembership {
    user_id: String,
    #[serde(default)]
    team_id: Option<Uuid>,
    #[serde(default)]
    project_id: Option<Uuid>,
    role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderAccess {
    id: Uuid,
    team_id: Uuid,
    provider_name: String,
    model_allowlist: Vec<String>,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateProviderAccess {
    provider_name: String,
    #[serde(default)]
    model_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderRecord {
    id: Uuid,
    provider_type: String,
    name: String,
    base_url: Option<String>,
    default_model: Option<String>,
    config: Value,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateProviderRecord {
    provider_type: String,
    name: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    config: Value,
}

#[derive(Debug, Deserialize)]
struct UpdateProviderStatus {
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProviderHealth {
    provider_id: Uuid,
    name: String,
    status: String,
    healthy: bool,
    issues: Vec<String>,
    checks: Value,
    checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpServerRecord {
    id: Uuid,
    team_id: Uuid,
    name: String,
    transport: String,
    config: Value,
    tool_allowlist: Vec<String>,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateMcpServerRecord {
    name: String,
    #[serde(default = "default_mcp_transport")]
    transport: String,
    #[serde(default)]
    config: Value,
    #[serde(default)]
    tool_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalDataset {
    id: Uuid,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateEvalDataset {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalCase {
    id: Uuid,
    dataset_id: Uuid,
    input: Value,
    expected: Option<Value>,
    grading_policy: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateEvalCase {
    input: Value,
    #[serde(default)]
    expected: Option<Value>,
    #[serde(default = "empty_json_object")]
    grading_policy: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalRun {
    id: Uuid,
    dataset_id: Uuid,
    agent_id: Uuid,
    agent_version_id: Uuid,
    status: String,
    score: Option<f64>,
    details: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateEvalRun {
    agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct EvalGateRequest {
    #[serde(default)]
    min_score: Option<f64>,
    #[serde(default)]
    require_completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EvalGateDecision {
    run_id: Uuid,
    status: String,
    score: Option<f64>,
    min_score: f64,
    failure_reasons: Vec<String>,
    checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageRollup {
    id: Uuid,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    summary: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateUsageRollup {
    #[serde(default)]
    period_start: Option<DateTime<Utc>>,
    #[serde(default)]
    period_end: Option<DateTime<Utc>>,
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
struct McpCallTool;

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

    let execution_queue = execution_queue_from_env(&store, tenant_id)?;

    let state = AppState {
        store,
        execution_queue,
        execution_worker: execution_worker_from_env(),
        authorizer: Arc::new(RoleBasedAuthorizer),
        observability_config: ObservabilityConfig::from_env()
            .map_err(|error| anyhow::anyhow!(error.message))?,
        telemetry_exporter: telemetry_exporter_from_env()?,
        mcp_gateway_config: mcp_gateway_config_from_env()?,
        mcp_gateway_client: mcp_gateway_client_from_env()?,
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

fn select_execution_queue_backend(
    requested: Option<&str>,
    has_postgres: bool,
) -> Result<ExecutionQueueBackendSelection> {
    let requested = requested.unwrap_or("auto").trim().to_ascii_lowercase();
    match requested.as_str() {
        "" | "auto" => Ok(if has_postgres {
            ExecutionQueueBackendSelection::Postgres
        } else {
            ExecutionQueueBackendSelection::Memory
        }),
        "memory" => Ok(ExecutionQueueBackendSelection::Memory),
        "postgres" => {
            if has_postgres {
                Ok(ExecutionQueueBackendSelection::Postgres)
            } else {
                anyhow::bail!("MANDOFORGE_EXECUTION_QUEUE_BACKEND=postgres requires DATABASE_URL");
            }
        }
        "broker" | "redis" | "nats" => {
            anyhow::bail!(
                "MANDOFORGE_EXECUTION_QUEUE_BACKEND={requested} is reserved for a future broker-backed queue; use auto, memory, or postgres"
            );
        }
        other => {
            anyhow::bail!(
                "unsupported MANDOFORGE_EXECUTION_QUEUE_BACKEND={other}; use auto, memory, or postgres"
            );
        }
    }
}

fn execution_queue_from_env(store: &StoreBackend, tenant_id: Uuid) -> Result<ExecutionQueue> {
    let selection = select_execution_queue_backend(
        std::env::var("MANDOFORGE_EXECUTION_QUEUE_BACKEND")
            .ok()
            .as_deref(),
        matches!(store, StoreBackend::Postgres(_)),
    )?;
    match (selection, store) {
        (ExecutionQueueBackendSelection::Memory, _) => Ok(ExecutionQueue::default()),
        (ExecutionQueueBackendSelection::Postgres, StoreBackend::Postgres(pool)) => {
            Ok(ExecutionQueue::postgres(pool.clone(), tenant_id))
        }
        (ExecutionQueueBackendSelection::Postgres, StoreBackend::Memory(_)) => {
            anyhow::bail!("Postgres execution queue selected without a Postgres store")
        }
    }
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
        .route(
            "/api/organizations",
            get(list_organizations).post(create_organization),
        )
        .route(
            "/api/organizations/{id}/teams",
            get(list_teams).post(create_team),
        )
        .route(
            "/api/organizations/{id}/memberships",
            get(list_memberships).post(create_membership),
        )
        .route(
            "/api/teams/{id}/projects",
            get(list_projects).post(create_project),
        )
        .route(
            "/api/teams/{id}/provider-access",
            get(list_provider_access).post(create_provider_access),
        )
        .route(
            "/api/teams/{id}/mcp-servers",
            get(list_mcp_servers).post(create_mcp_server),
        )
        .route(
            "/api/teams/{team_id}/mcp-servers/{server_id}/discover",
            post(discover_mcp_server_tools),
        )
        .route("/api/providers", get(list_providers).post(create_provider))
        .route("/api/providers/{id}/status", patch(update_provider_status))
        .route("/api/providers/{id}/health", get(get_provider_health))
        .route(
            "/api/eval/datasets",
            get(list_eval_datasets).post(create_eval_dataset),
        )
        .route(
            "/api/eval/datasets/{id}/cases",
            get(list_eval_cases).post(create_eval_case),
        )
        .route(
            "/api/eval/datasets/{id}/runs",
            get(list_dataset_eval_runs).post(create_eval_run),
        )
        .route("/api/eval/runs", get(list_eval_runs))
        .route("/api/eval/runs/{id}/gate", post(gate_eval_run))
        .route("/api/usage", get(get_usage_summary))
        .route(
            "/api/usage/rollups",
            get(list_usage_rollups).post(create_usage_rollup),
        )
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve))
        .route("/api/approvals/{id}/reject", post(reject))
        .route("/api/approvals/{id}/expire", post(expire))
        .route("/api/approvals/{id}/modify", post(modify_approval))
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

fn telemetry_exporter_from_env() -> Result<Arc<dyn TelemetryExporter>> {
    let config = ObservabilityConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?;
    if config.is_enabled() {
        Ok(Arc::new(
            HttpTelemetryExporter::new().map_err(|error| anyhow::anyhow!(error.message))?,
        ))
    } else {
        Ok(Arc::new(ReservedTelemetryExporter))
    }
}

fn mcp_gateway_config_from_env() -> Result<Option<McpGatewayConfig>> {
    match std::env::var("MANDOFORGE_MCP_GATEWAY_URL") {
        Ok(value) if !value.trim().is_empty() => Ok(Some(
            McpGatewayConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?,
        )),
        _ => Ok(None),
    }
}

fn mcp_gateway_client_from_env() -> Result<Arc<dyn McpGatewayClient>> {
    if std::env::var("MANDOFORGE_MCP_GATEWAY_URL")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        Ok(Arc::new(
            HttpMcpGatewayClient::new().map_err(|error| anyhow::anyhow!(error.message))?,
        ))
    } else {
        Ok(Arc::new(ReservedMcpGatewayClient))
    }
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

impl AppState {
    async fn emit_telemetry_event(&self, event: &SessionEvent) {
        if !self.observability_config.is_enabled() || self.observability_config.sample_ratio <= 0.0
        {
            return;
        }
        let telemetry_event = TelemetryEvent {
            name: event.event_type.clone(),
            attributes: telemetry_attributes_for_event(event, self.tenant_id),
        };
        if let Err(error) = self
            .telemetry_exporter
            .export_event(&self.observability_config, telemetry_event)
            .await
        {
            warn!(%error.message, "telemetry export failed");
        }
    }
}

fn telemetry_attributes_for_event(event: &SessionEvent, tenant_id: Uuid) -> Value {
    let category = event.event_type.split('.').next().unwrap_or("event");
    let status = telemetry_status_for_event(event);
    let duration_ms = event
        .payload
        .get("duration_ms")
        .or_else(|| event.payload.get("latency_ms"))
        .and_then(Value::as_i64);
    let mut attributes = json!({
        "tenant_id": tenant_id,
        "session_id": event.session_id,
        "event_id": event.id,
        "seq": event.seq,
        "actor_type": event.actor_type,
        "actor_id": event.actor_id,
        "signal": {
            "type": telemetry_signal_type(category),
            "category": category,
            "span_name": format!("mandoforge.{}", event.event_type),
            "status": status,
        },
        "metrics": {
            "event_count": 1,
            "metric_name": format!("mandoforge.{}.events", category),
        }
    });
    if let Some(duration_ms) = duration_ms {
        attributes["metrics"]["duration_ms"] = json!(duration_ms);
    }
    copy_payload_key(&mut attributes, &event.payload, "provider");
    copy_payload_key(&mut attributes, &event.payload, "client");
    copy_payload_key(&mut attributes, &event.payload, "tool");
    copy_payload_key(&mut attributes, &event.payload, "tool_call_id");
    copy_payload_key(&mut attributes, &event.payload, "approval_id");
    copy_payload_key(&mut attributes, &event.payload, "worker_id");
    if let Some(tool_calls) = event.payload.get("tool_calls").and_then(Value::as_array) {
        attributes["metrics"]["tool_call_count"] = json!(tool_calls.len());
    }
    attributes
}

fn telemetry_signal_type(category: &str) -> &'static str {
    match category {
        "llm" | "tool" | "approval" | "session" | "worker" | "sandbox" | "codex" => "span",
        _ => "log",
    }
}

fn telemetry_status_for_event(event: &SessionEvent) -> &'static str {
    if event.event_type.ends_with(".failed")
        || event.event_type.ends_with(".error")
        || event.event_type.ends_with(".denied")
        || event.payload.get("status").and_then(Value::as_str) == Some("failed")
    {
        "error"
    } else if event.event_type.ends_with(".requested")
        || event.event_type.ends_with(".started")
        || event.event_type.ends_with(".call")
        || event.event_type.ends_with(".request")
    {
        "started"
    } else {
        "ok"
    }
}

fn copy_payload_key(attributes: &mut Value, payload: &Value, key: &str) {
    if let Some(value) = payload.get(key) {
        attributes[key] = value.clone();
    }
}

async fn run_migrations(pool: &PgPool) -> Result<()> {
    for path in [
        "db/migrations/0001_core.sql",
        "db/migrations/0002_generic_demo.sql",
        "db/migrations/0003_stage2_governance.sql",
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

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Agent>>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::AgentsRead,
        resource_type: "agents".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(state.list_agents_visible_to(&principal).await?))
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsWrite, "agents", None).await?;
    Ok(Json(state.create_agent(input).await?))
}

async fn list_agent_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentVersion>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.list_agent_versions(id).await?))
}

async fn get_agent_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, i32)>,
    headers: HeaderMap,
) -> Result<Json<AgentVersion>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "agent", Some(id)).await?;
    Ok(Json(state.get_agent_version(id, version).await?))
}

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Session>>, AppError> {
    let principal = principal_from_request(&state, &headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::SessionsRead,
        resource_type: "sessions".to_string(),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await?;
    Ok(Json(state.list_sessions_visible_to(&principal).await?))
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateSession>,
) -> Result<Json<Session>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "sessions",
        None,
    )
    .await?;
    Ok(Json(state.create_session(input).await?))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Session>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.get_session(id).await?))
}

async fn add_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<AddMessage>,
) -> Result<Json<SessionEvent>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsWrite,
        "session",
        Some(id),
    )
    .await?;
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
    let approved_tool_result_count = events
        .iter()
        .filter(|event| {
            event.event_type == "tool.result"
                && event
                    .payload
                    .get("content")
                    .and_then(|content| content.get("approval"))
                    .and_then(Value::as_str)
                    == Some("approved")
        })
        .count();
    Ok(HarnessContext {
        session_id,
        event_count: events.len(),
        last_user_message,
        approved_tool_result_count,
    })
}

async fn run_provider_harness(
    state: &AppState,
    session_id: Uuid,
    provider: &dyn ProviderClient,
    provider_label: &str,
) -> Result<ProviderResponse, AppError> {
    let context = build_harness_context(state, session_id).await?;
    state
        .append_event(
            "agent",
            None,
            session_id,
            "llm.request",
            json!({"provider": provider_label, "client": provider.name(), "context": context}),
        )
        .await?;
    let response = provider.complete(context).await?;
    state
        .append_event(
            "agent",
            None,
            session_id,
            "llm.response",
            json!({"provider": provider_label, "client": provider.name(), "tool_calls": &response.tool_calls, "final_message": &response.final_message, "usage": &response.usage}),
        )
        .await?;
    Ok(response)
}

async fn provider_client_for_session(
    state: &AppState,
    session_id: Uuid,
) -> Result<(String, Box<dyn ProviderClient>), AppError> {
    let session = state.get_session(session_id).await?;
    let agent = state.get_agent(session.agent_id).await?;
    if let Some(provider) = state.provider_by_name(&agent.provider).await? {
        if provider.status != "active" {
            return Err(AppError::forbidden(format!(
                "provider {} is not active",
                provider.name
            )));
        }
        enforce_provider_budget(state, &provider).await?;
        let provider_type = provider.provider_type.trim().to_ascii_lowercase();
        if matches!(provider_type.as_str(), "mock" | "mock_openai_compatible") {
            return Ok((provider.name, Box::new(MockProviderClient)));
        }
        if matches!(
            provider_type.as_str(),
            "openai_compatible" | "openai-compatible"
        ) {
            let base_url = provider.base_url.clone().ok_or_else(|| {
                AppError::bad_request("stored openai-compatible provider requires base_url")
            })?;
            let model = agent
                .model
                .trim()
                .is_empty()
                .then(|| provider.default_model.clone())
                .flatten()
                .unwrap_or(agent.model);
            let api_key = stored_provider_api_key(&provider).await?;
            return Ok((
                provider.name,
                Box::new(OpenAiCompatibleProviderClient::from_parts(
                    base_url, api_key, model,
                )?),
            ));
        }
        return Err(AppError::bad_request(format!(
            "provider type {} is not supported",
            provider.provider_type
        )));
    }
    let fallback = provider_client_from_env().await?;
    Ok((agent.provider, fallback))
}

async fn stored_provider_api_key(provider: &ProviderRecord) -> Result<String, AppError> {
    if let Some(env_key) = provider.config.get("api_key_env").and_then(Value::as_str) {
        let value = std::env::var(env_key).map_err(|_| {
            AppError::bad_request(format!(
                "stored provider {} requires env var {env_key}",
                provider.name
            ))
        })?;
        return Ok(value);
    }
    if let Some(value) = provider.config.get("api_key_ref").and_then(Value::as_str) {
        let secret_provider = secret_provider_from_env()?;
        return provider::provider_api_key_from_stored_value(value, secret_provider.as_ref()).await;
    }
    Err(AppError::bad_request(format!(
        "stored provider {} requires config.api_key_env or config.api_key_ref",
        provider.name
    )))
}

async fn enforce_provider_budget(
    state: &AppState,
    provider: &ProviderRecord,
) -> Result<(), AppError> {
    let since = Utc::now() - chrono::Duration::hours(24);
    if let Some(limit) = provider_daily_request_limit(provider) {
        let used = state
            .provider_request_count_since(&provider.name, since)
            .await?;
        if used >= limit {
            return Err(AppError::forbidden(format!(
                "provider {} exceeded daily request budget {limit}",
                provider.name
            )));
        }
    }
    if let Some(limit) = provider_daily_cost_limit_cents(provider) {
        let used = provider_estimated_cost_cents_since(state, provider, since).await?;
        let next_request_cost = provider_per_request_cost_cents(provider);
        if used + next_request_cost > limit {
            return Err(AppError::forbidden(format!(
                "provider {} exceeded daily cost budget {limit:.2} cents",
                provider.name
            )));
        }
    }
    Ok(())
}

fn provider_daily_request_limit(provider: &ProviderRecord) -> Option<i64> {
    provider
        .config
        .get("budget")
        .and_then(|budget| budget.get("daily_request_limit"))
        .and_then(Value::as_i64)
        .filter(|limit| *limit >= 0)
}

fn provider_daily_cost_limit_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("budget")
        .and_then(|budget| budget.get("daily_cost_limit_cents"))
        .and_then(Value::as_f64)
        .filter(|limit| *limit >= 0.0)
}

fn provider_per_request_cost_cents(provider: &ProviderRecord) -> f64 {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_request_cents"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn provider_prompt_token_price_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_1k_prompt_tokens_cents"))
        .and_then(Value::as_f64)
}

fn provider_completion_token_price_cents(provider: &ProviderRecord) -> Option<f64> {
    provider
        .config
        .get("pricing")
        .and_then(|pricing| pricing.get("per_1k_completion_tokens_cents"))
        .and_then(Value::as_f64)
}

async fn provider_estimated_cost_cents_since(
    state: &AppState,
    provider: &ProviderRecord,
    since: DateTime<Utc>,
) -> Result<f64, AppError> {
    let mut cost = 0.0;
    for session in state.list_sessions().await? {
        for event in state.list_events(session.id).await? {
            if event.created_at < since {
                continue;
            }
            if event.payload.get("provider").and_then(Value::as_str) != Some(provider.name.as_str())
            {
                continue;
            }
            if event.event_type == "llm.request" {
                cost += provider_per_request_cost_cents(provider);
            }
            if event.event_type == "llm.response" {
                let prompt_tokens = event
                    .payload
                    .get("usage")
                    .and_then(|usage| usage.get("prompt_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let completion_tokens = event
                    .payload
                    .get("usage")
                    .and_then(|usage| usage.get("completion_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                cost +=
                    token_cost_cents(prompt_tokens, provider_prompt_token_price_cents(provider));
                cost += token_cost_cents(
                    completion_tokens,
                    provider_completion_token_price_cents(provider),
                );
            }
        }
    }
    Ok(cost)
}

async fn provider_client_from_env() -> Result<Box<dyn ProviderClient>, AppError> {
    if let Some(provider) = OpenAiCompatibleProviderClient::from_env().await? {
        Ok(Box::new(provider))
    } else {
        Ok(Box::new(MockProviderClient))
    }
}

async fn run_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Session>, AppError> {
    authorize_session_run(&state, &headers, id).await?;
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

    let (provider_label, provider) = provider_client_for_session(&state, id).await?;
    let provider_response =
        run_provider_harness(&state, id, provider.as_ref(), &provider_label).await?;

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

async fn authorize_session_run(
    state: &AppState,
    headers: &HeaderMap,
    session_id: Uuid,
) -> Result<(), AppError> {
    authorize_request(
        state,
        headers,
        Permission::SessionsRun,
        "session",
        Some(session_id),
    )
    .await
}

async fn authorize_request(
    state: &AppState,
    headers: &HeaderMap,
    permission: Permission,
    resource_type: impl Into<String>,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission,
        resource_type: resource_type.into(),
        resource_id,
    };
    state.authorizer.authorize(&principal, &request).await?;
    enforce_resource_scope(state, &principal, &request).await
}

async fn enforce_resource_scope(
    state: &AppState,
    principal: &Principal,
    request: &AuthorizationRequest,
) -> Result<(), AppError> {
    if principal.roles.contains(&Role::Admin) {
        return Ok(());
    }
    let Some(resource_id) = request.resource_id else {
        return Ok(());
    };
    let (team_id, project_id) = match request.resource_type.as_str() {
        "agent" => {
            let agent = state.get_agent(resource_id).await?;
            (agent.team_id, agent.project_id)
        }
        "session" => {
            let session = state.get_session(resource_id).await?;
            let agent = state.get_agent(session.agent_id).await?;
            (agent.team_id, agent.project_id)
        }
        _ => (None, None),
    };
    if let Some(project_id) = project_id {
        if state
            .subject_can_access_project(&principal.subject_id, project_id)
            .await?
        {
            return Ok(());
        }
        return Err(AppError::forbidden(format!(
            "principal {} has no membership for scoped {}",
            principal.subject_id, request.resource_type
        )));
    }
    let Some(team_id) = team_id else {
        return Ok(());
    };
    if state
        .subject_can_access_team(&principal.subject_id, team_id)
        .await?
    {
        Ok(())
    } else {
        Err(AppError::forbidden(format!(
            "principal {} has no membership for scoped {}",
            principal.subject_id, request.resource_type
        )))
    }
}

async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionEvent>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_events(id).await?))
}

async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>>, AppError>
{
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    let events = state.list_events(id).await.unwrap_or_default();
    let stream = futures_util::stream::iter(events.into_iter().map(|event| {
        Ok(Event::default()
            .event(event.event_type.clone())
            .json_data(event)
            .unwrap_or_else(|_| Event::default().event("error").data("serialization failed")))
    }));
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
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
        let created_at = Utc::now();
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action,
            risk_level,
            reason,
            evidence,
            decision_payload: json!({}),
            status: "pending".to_string(),
            expires_at: approval_expires_at(created_at, input.args.get("expires_in_seconds")),
            created_at,
            decided_at: None,
        };
        let approval = state.insert_approval(approval).await?;
        state
            .append_event(
                "system",
                Some(approval.id),
                input.session_id,
                "approval.requested",
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence, "expires_at": approval.expires_at, "tool_call_id": tool_call.id}),
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
                json!({"action": approval.action, "risk_level": approval.risk_level, "expires_at": approval.expires_at}),
            ))
            .await?;
        state
            .set_session_status(input.session_id, SessionStatus::WaitingApproval)
            .await?;
        Ok(json!({"status": "approval_requested", "approval_id": approval.id}))
    }
}

#[async_trait]
impl ToolExecutor for McpCallTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "mcp.call",
            risk: "high",
            description: "Call an allowlisted MCP Gateway server tool through the audited Tool Router",
        }
    }

    async fn execute(
        &self,
        state: &AppState,
        _input: &ExecuteTool,
        _tool_call: &ToolCall,
    ) -> Result<Value, AppError> {
        let config = state
            .mcp_gateway_config
            .as_ref()
            .ok_or_else(|| AppError::bad_request("MCP gateway is not configured"))?;
        let request: McpCallRequest = serde_json::from_value(_input.args.clone())?;
        state
            .ensure_mcp_tool_allowed_for_session(_input.session_id, &request.server, &request.tool)
            .await?;
        let response = state.mcp_gateway_client.call(config, request).await?;
        Ok(json!({
            "status": "called",
            "result": response.result,
        }))
    }
}

fn tool_registry() -> HashMap<&'static str, Box<dyn ToolExecutor>> {
    let tools: Vec<Box<dyn ToolExecutor>> = vec![
        Box::new(ArtifactCreateTool),
        Box::new(ApprovalRequestTool),
        Box::new(FileReadTool),
        Box::new(McpCallTool),
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

async fn list_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolDescriptor>>, AppError> {
    authorize_request(&state, &headers, Permission::AgentsRead, "tools", None).await?;
    Ok(Json(tool_descriptors()))
}

async fn execute_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(input): Json<ExecuteTool>,
) -> Result<Json<Value>, AppError> {
    authorize_tool_execution(&state, &headers, &name).await?;
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(input.session_id),
    )
    .await?;
    Ok(Json(execute_tool_invocation(&state, &name, input).await?))
}

async fn authorize_tool_execution(
    state: &AppState,
    headers: &HeaderMap,
    tool_name: &str,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::ToolsExecute,
        resource_type: format!("tool:{tool_name}"),
        resource_id: None,
    };
    state.authorizer.authorize(&principal, &request).await
}

async fn principal_from_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Principal, AppError> {
    let explicit_subject = header_value(headers, "x-mandoforge-subject");
    let subject_id = explicit_subject.unwrap_or("demo-operator").to_string();
    let roles = if let Some(value) = header_value(headers, "x-mandoforge-roles") {
        parse_roles_header(value)?
    } else if explicit_subject.is_some() {
        state.membership_roles_for_subject(&subject_id).await?
    } else {
        vec![Role::Operator]
    };
    if roles.is_empty() {
        return Err(AppError::forbidden("principal has no roles"));
    }

    Ok(Principal {
        tenant_id: state.tenant_id,
        subject_id,
        roles,
    })
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn parse_roles_header(value: &str) -> Result<Vec<Role>, AppError> {
    value
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(|role| match role {
            "admin" => Ok(Role::Admin),
            "operator" => Ok(Role::Operator),
            "approver" => Ok(Role::Approver),
            "viewer" => Ok(Role::Viewer),
            other => Err(AppError::bad_request(format!(
                "unsupported x-mandoforge-roles value: {other}"
            ))),
        })
        .collect()
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
        let created_at = Utc::now();
        let approval = Approval {
            id: Uuid::new_v4(),
            session_id: input.session_id,
            tool_call_id: Some(tool_call.id),
            action: name.to_string(),
            risk_level: policy_decision.risk_level.clone(),
            reason: policy_decision.reason.clone(),
            evidence: json!({"tool": name, "args": input.args}),
            decision_payload: json!({}),
            status: "pending".to_string(),
            expires_at: approval_expires_at(created_at, None),
            created_at,
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
                json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence, "expires_at": approval.expires_at}),
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
                json!({"tool_call_id": approval.tool_call_id, "action": approval.action, "risk_level": approval.risk_level, "expires_at": approval.expires_at}),
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
    authorize_request(&state, &headers, Permission::Admin, "organizations", None).await?;
    Ok(Json(state.create_organization(input).await?))
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

async fn list_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderAccess>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.list_provider_access(id).await?))
}

async fn create_provider_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderAccess>,
) -> Result<Json<ProviderAccess>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.create_provider_access(id, input).await?))
}

async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderRecord>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    Ok(Json(state.list_providers().await?))
}

async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateProviderRecord>,
) -> Result<Json<ProviderRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "providers", None).await?;
    Ok(Json(state.create_provider(input).await?))
}

async fn update_provider_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateProviderStatus>,
) -> Result<Json<ProviderRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "provider", Some(id)).await?;
    let status = normalize_provider_status(&input.status)?;
    Ok(Json(state.update_provider_status(id, &status).await?))
}

fn normalize_provider_status(status: &str) -> Result<String, AppError> {
    match status.trim() {
        "active" => Ok("active".to_string()),
        "disabled" => Ok("disabled".to_string()),
        other => Err(AppError::bad_request(format!(
            "unsupported provider status: {other}"
        ))),
    }
}

async fn get_provider_health(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProviderHealth>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "provider", Some(id)).await?;
    let provider = state
        .list_providers()
        .await?
        .into_iter()
        .find(|provider| provider.id == id)
        .ok_or_else(|| AppError::not_found("provider not found"))?;
    Ok(Json(provider_health(&provider)))
}

fn provider_health(provider: &ProviderRecord) -> ProviderHealth {
    let mut issues = Vec::new();
    let provider_type = provider.provider_type.trim().to_ascii_lowercase();
    if provider.status != "active" {
        issues.push(format!("provider status is {}", provider.status));
    }
    let has_base_url = provider
        .base_url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_default_model = provider
        .default_model
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let api_key_env = provider
        .config
        .get("api_key_env")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let api_key_ref = provider
        .config
        .get("api_key_ref")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let api_key_env_present = api_key_env.is_some_and(|env_key| std::env::var(env_key).is_ok());

    match provider_type.as_str() {
        "mock" | "mock_openai_compatible" => {}
        "openai_compatible" | "openai-compatible" => {
            if !has_base_url {
                issues.push("openai-compatible provider requires base_url".to_string());
            }
            if api_key_env.is_none() && api_key_ref.is_none() {
                issues.push(
                    "openai-compatible provider requires config.api_key_env or config.api_key_ref"
                        .to_string(),
                );
            }
            if api_key_env.is_some() && !api_key_env_present {
                issues.push("configured api_key_env is not present in the environment".to_string());
            }
        }
        other => issues.push(format!("provider type {other} is not supported")),
    }

    ProviderHealth {
        provider_id: provider.id,
        name: provider.name.clone(),
        status: provider.status.clone(),
        healthy: issues.is_empty(),
        issues,
        checks: json!({
            "provider_type": provider.provider_type,
            "has_base_url": has_base_url,
            "has_default_model": has_default_model,
            "has_api_key_env": api_key_env.is_some(),
            "api_key_env_present": api_key_env_present,
            "has_api_key_ref": api_key_ref.is_some(),
        }),
        checked_at: Utc::now(),
    }
}

async fn list_mcp_servers(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<McpServerRecord>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.list_mcp_servers(id).await?))
}

async fn create_mcp_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateMcpServerRecord>,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(id)).await?;
    Ok(Json(state.create_mcp_server(id, input).await?))
}

async fn discover_mcp_server_tools(
    State(state): State<AppState>,
    Path((team_id, server_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<McpServerRecord>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "team", Some(team_id)).await?;
    let config = state
        .mcp_gateway_config
        .as_ref()
        .ok_or_else(|| AppError::bad_request("MCP gateway is not configured"))?;
    let server = state.get_mcp_server(team_id, server_id).await?;
    let tools = state
        .mcp_gateway_client
        .discover_tools(config, &server.name)
        .await?;
    let mut tool_allowlist: Vec<_> = tools
        .into_iter()
        .map(|tool| tool.name)
        .filter(|name| !name.trim().is_empty())
        .collect();
    tool_allowlist.sort();
    tool_allowlist.dedup();
    Ok(Json(
        state
            .update_mcp_server_tool_allowlist(team_id, server_id, tool_allowlist)
            .await?,
    ))
}

async fn list_eval_datasets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalDataset>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_datasets", None).await?;
    Ok(Json(state.list_eval_datasets().await?))
}

async fn create_eval_dataset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalDataset>,
) -> Result<Json<EvalDataset>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_datasets", None).await?;
    Ok(Json(state.create_eval_dataset(input).await?))
}

async fn list_eval_cases(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalCase>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_eval_cases(id).await?))
}

async fn create_eval_case(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalCase>,
) -> Result<Json<EvalCase>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_eval_case(id, input).await?))
}

async fn list_dataset_eval_runs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalRun>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_eval_runs(Some(id)).await?))
}

async fn list_eval_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvalRun>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_runs", None).await?;
    Ok(Json(state.list_eval_runs(None).await?))
}

async fn create_eval_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateEvalRun>,
) -> Result<Json<EvalRun>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::Admin,
        "eval_dataset",
        Some(id),
    )
    .await?;
    Ok(Json(state.create_eval_run(id, input).await?))
}

async fn gate_eval_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EvalGateRequest>,
) -> Result<Json<EvalGateDecision>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "eval_run", Some(id)).await?;
    let min_score = input.min_score.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&min_score) {
        return Err(AppError::bad_request(
            "eval gate min_score must be between 0.0 and 1.0",
        ));
    }
    let require_completed = input.require_completed.unwrap_or(true);
    let run = state
        .list_eval_runs(None)
        .await?
        .into_iter()
        .find(|run| run.id == id)
        .ok_or_else(|| AppError::not_found("eval run not found"))?;
    Ok(Json(build_eval_gate_decision(
        &run,
        min_score,
        require_completed,
    )))
}

fn build_eval_gate_decision(
    run: &EvalRun,
    min_score: f64,
    require_completed: bool,
) -> EvalGateDecision {
    let mut failure_reasons = Vec::new();
    let score = run.score.unwrap_or(0.0);
    if score < min_score {
        failure_reasons.push(format!(
            "score {score:.4} is below required minimum {min_score:.4}"
        ));
    }
    if require_completed && run.status != "completed" {
        failure_reasons.push(format!("eval run status is {}", run.status));
    }
    let case_count = run
        .details
        .get("case_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let passed_count = run
        .details
        .get("passed_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if case_count == 0 {
        failure_reasons.push("eval run has no cases".to_string());
    }
    if passed_count < case_count {
        failure_reasons.push(format!("{passed_count} of {case_count} eval cases passed"));
    }
    EvalGateDecision {
        run_id: run.id,
        status: if failure_reasons.is_empty() {
            "passed".to_string()
        } else {
            "failed".to_string()
        },
        score: run.score,
        min_score,
        failure_reasons,
        checked_at: Utc::now(),
    }
}

async fn get_usage_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UsageSummary>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage", None).await?;
    Ok(Json(build_usage_summary(&state).await?))
}

async fn list_usage_rollups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UsageRollup>>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_rollups", None).await?;
    Ok(Json(state.list_usage_rollups().await?))
}

async fn create_usage_rollup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateUsageRollup>,
) -> Result<Json<UsageRollup>, AppError> {
    authorize_request(&state, &headers, Permission::Admin, "usage_rollups", None).await?;
    let period_end = input.period_end.unwrap_or_else(Utc::now);
    let period_start = input
        .period_start
        .unwrap_or_else(|| period_end - chrono::Duration::hours(24));
    if period_start >= period_end {
        return Err(AppError::bad_request(
            "usage rollup period_start must be before period_end",
        ));
    }
    let summary = serde_json::to_value(build_usage_summary(&state).await?)?;
    Ok(Json(
        state
            .create_usage_rollup(period_start, period_end, summary)
            .await?,
    ))
}

async fn build_usage_summary(state: &AppState) -> Result<UsageSummary, AppError> {
    let sessions = state.list_sessions().await?;
    let tool_calls = state.list_tool_calls(None).await?;
    let approvals = state.list_approvals().await?;
    let providers = state.list_providers().await?;
    let provider_request_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_request_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();
    let provider_prompt_token_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_1k_prompt_tokens_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();
    let provider_completion_token_prices: HashMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            provider
                .config
                .get("pricing")
                .and_then(|pricing| pricing.get("per_1k_completion_tokens_cents"))
                .and_then(Value::as_f64)
                .map(|price| (provider.name.clone(), price))
        })
        .collect();

    let mut event_count = 0;
    let mut provider_request_count = 0;
    let mut provider_response_count = 0;
    let mut prompt_tokens = 0;
    let mut completion_tokens = 0;
    let mut total_tokens = 0;
    let mut by_provider = HashMap::<String, ProviderUsageSummary>::new();
    for session in &sessions {
        for event in state.list_events(session.id).await? {
            event_count += 1;
            if event.event_type == "llm.request" || event.event_type == "llm.response" {
                let provider = event
                    .payload
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let usage = by_provider.entry(provider.clone()).or_default();
                if event.event_type == "llm.request" {
                    provider_request_count += 1;
                    usage.request_count += 1;
                    usage.estimated_cost_cents += provider_request_prices
                        .get(&provider)
                        .copied()
                        .unwrap_or(0.0);
                } else {
                    provider_response_count += 1;
                    usage.response_count += 1;
                    let event_prompt_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("prompt_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let event_completion_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("completion_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let event_total_tokens = event
                        .payload
                        .get("usage")
                        .and_then(|usage| usage.get("total_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(event_prompt_tokens + event_completion_tokens);
                    usage.prompt_tokens += event_prompt_tokens;
                    usage.completion_tokens += event_completion_tokens;
                    usage.total_tokens += event_total_tokens;
                    prompt_tokens += event_prompt_tokens;
                    completion_tokens += event_completion_tokens;
                    total_tokens += event_total_tokens;
                    let token_cost = token_cost_cents(
                        event_prompt_tokens,
                        provider_prompt_token_prices.get(&provider).copied(),
                    ) + token_cost_cents(
                        event_completion_tokens,
                        provider_completion_token_prices.get(&provider).copied(),
                    );
                    usage.token_cost_cents += token_cost;
                    usage.estimated_cost_cents += token_cost;
                }
            }
        }
    }

    let mut by_tool = HashMap::<String, ToolUsageSummary>::new();
    let mut total_tool_duration_ms = 0;
    let mut tool_success_count = 0;
    let mut tool_failed_count = 0;
    for call in &tool_calls {
        let tool = by_tool.entry(call.tool_name.clone()).or_default();
        tool.call_count += 1;
        if call.status == "completed" {
            tool.success_count += 1;
            tool_success_count += 1;
        }
        if matches!(call.status.as_str(), "failed" | "denied") {
            tool.failed_count += 1;
            tool_failed_count += 1;
        }
        if let (Some(started_at), Some(completed_at)) = (call.started_at, call.completed_at) {
            let duration = completed_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .max(0);
            tool.total_duration_ms += duration;
            total_tool_duration_ms += duration;
        }
    }

    let mut estimated_provider_cost_cents: f64 = by_provider
        .values()
        .map(|usage| usage.estimated_cost_cents)
        .sum();
    if estimated_provider_cost_cents == 0.0 {
        estimated_provider_cost_cents = 0.0;
    }
    let provider_budgets = build_provider_budget_statuses(state, &providers).await?;
    Ok(UsageSummary {
        session_count: sessions.len(),
        event_count,
        provider_request_count,
        provider_response_count,
        tool_call_count: tool_calls.len(),
        tool_success_count,
        tool_failed_count,
        approval_count: approvals.len(),
        prompt_tokens,
        completion_tokens,
        total_tokens,
        total_tool_duration_ms,
        estimated_provider_cost_cents,
        by_provider,
        by_tool,
        provider_budgets,
    })
}

async fn build_provider_budget_statuses(
    state: &AppState,
    providers: &[ProviderRecord],
) -> Result<Vec<ProviderBudgetStatus>, AppError> {
    let since = Utc::now() - chrono::Duration::hours(24);
    let mut statuses = Vec::new();
    for provider in providers {
        let daily_request_limit = provider_daily_request_limit(provider);
        let daily_cost_limit_cents = provider_daily_cost_limit_cents(provider);
        if daily_request_limit.is_none() && daily_cost_limit_cents.is_none() {
            continue;
        }
        let request_count = state
            .provider_request_count_since(&provider.name, since)
            .await?;
        let estimated_cost_cents =
            provider_estimated_cost_cents_since(state, provider, since).await?;
        let request_budget_used_percent =
            daily_request_limit.map(|limit| percent_used(request_count as f64, limit as f64));
        let cost_budget_used_percent =
            daily_cost_limit_cents.map(|limit| percent_used(estimated_cost_cents, limit));
        let max_used_percent = request_budget_used_percent
            .into_iter()
            .chain(cost_budget_used_percent)
            .fold(0.0, f64::max);
        let status = if max_used_percent >= 100.0 {
            "critical"
        } else if max_used_percent >= 80.0 {
            "warning"
        } else {
            "ok"
        }
        .to_string();
        let mut messages = Vec::new();
        if let (Some(limit), Some(percent)) = (daily_request_limit, request_budget_used_percent) {
            messages.push(format!(
                "{request_count} of {limit} daily requests used ({percent:.1}%)"
            ));
        }
        if let (Some(limit), Some(percent)) = (daily_cost_limit_cents, cost_budget_used_percent) {
            messages.push(format!(
                "{estimated_cost_cents:.2} of {limit:.2} daily cost cents used ({percent:.1}%)"
            ));
        }
        statuses.push(ProviderBudgetStatus {
            provider_name: provider.name.clone(),
            status,
            window_hours: 24,
            request_count,
            daily_request_limit,
            request_budget_used_percent,
            estimated_cost_cents,
            projected_daily_cost_cents: estimated_cost_cents,
            daily_cost_limit_cents,
            cost_budget_used_percent,
            messages,
        });
    }
    statuses.sort_by(|left, right| {
        budget_rank(&right.status)
            .cmp(&budget_rank(&left.status))
            .then_with(|| {
                right
                    .projected_daily_cost_cents
                    .total_cmp(&left.projected_daily_cost_cents)
            })
            .then_with(|| left.provider_name.cmp(&right.provider_name))
    });
    Ok(statuses)
}

fn percent_used(used: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        if used > 0.0 { 100.0 } else { 0.0 }
    } else {
        (used / limit) * 100.0
    }
}

fn budget_rank(status: &str) -> i32 {
    match status {
        "critical" => 3,
        "warning" => 2,
        _ => 1,
    }
}

fn token_cost_cents(tokens: i64, price_per_1k_cents: Option<f64>) -> f64 {
    let Some(price) = price_per_1k_cents else {
        return 0.0;
    };
    (tokens.max(0) as f64 / 1000.0) * price
}

fn approval_expires_at(
    created_at: DateTime<Utc>,
    expires_in_seconds: Option<&Value>,
) -> Option<DateTime<Utc>> {
    let seconds = expires_in_seconds
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0)
        .unwrap_or(86_400);
    Some(created_at + chrono::Duration::seconds(seconds))
}

async fn list_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Approval>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "approvals",
        None,
    )
    .await?;
    Ok(Json(state.list_approvals().await?))
}

async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    decide_approval(state, id, "approved").await
}

async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    decide_approval(state, id, "rejected").await
}

async fn expire(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    Ok(Json(expire_approval_record(&state, id).await?))
}

async fn modify_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ModifyApproval>,
) -> Result<Json<Approval>, AppError> {
    authorize_approval_decision(&state, &headers, id).await?;
    let approval = state.get_approval(id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be modified",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
    if let Some(tool_call_id) = approval.tool_call_id {
        state
            .update_tool_call_args(tool_call_id, input.args.clone())
            .await?;
    }
    let updated = state
        .modify_approval(id, input.args.clone(), input.comment.clone())
        .await?;
    state
        .append_event(
            "user",
            Some(id),
            updated.session_id,
            "approval.modified",
            json!({
                "approval_id": id,
                "tool_call_id": updated.tool_call_id,
                "modified_args": input.args,
                "comment": input.comment,
            }),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "user",
            Some(id),
            "approval.modified",
            "approval",
            Some(id),
            json!({
                "tool_call_id": updated.tool_call_id,
                "action": updated.action,
                "comment": updated.decision_payload.get("comment").cloned().unwrap_or(Value::Null),
            }),
        ))
        .await?;
    Ok(Json(updated))
}

async fn authorize_approval_decision(
    state: &AppState,
    headers: &HeaderMap,
    approval_id: Uuid,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::ApprovalsDecide,
        resource_type: "approval".to_string(),
        resource_id: Some(approval_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let approval = state.get_approval(approval_id).await?;
    let session_request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::SessionsRead,
        resource_type: "session".to_string(),
        resource_id: Some(approval.session_id),
    };
    enforce_resource_scope(state, &principal, &session_request).await
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<Artifact>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_artifacts(id).await?))
}

async fn list_tool_calls(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolCall>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "tool_calls",
        None,
    )
    .await?;
    Ok(Json(state.list_tool_calls(None).await?))
}

async fn list_session_tool_calls(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolCall>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "session",
        Some(id),
    )
    .await?;
    Ok(Json(state.list_tool_calls(Some(id)).await?))
}

async fn list_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    authorize_request(&state, &headers, Permission::AuditRead, "audit_logs", None).await?;
    Ok(Json(state.list_audit_logs(None).await?))
}

async fn list_execution_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<execution_queue::ExecutionJob>>, AppError> {
    authorize_request(
        &state,
        &headers,
        Permission::SessionsRead,
        "execution_jobs",
        None,
    )
    .await?;
    Ok(Json(state.execution_queue.list().await?))
}

async fn run_execution_job_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<execution_queue::ExecutionJob>, AppError> {
    authorize_execution_job_run(&state, &headers, id).await?;
    let worker_id = headers
        .get("x-mandoforge-worker-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("api");
    let completed = run_execution_job(&state, id, worker_id).await?;
    resume_provider_after_approval(&state, completed.session_id).await?;
    Ok(Json(completed))
}

async fn authorize_execution_job_run(
    state: &AppState,
    headers: &HeaderMap,
    job_id: Uuid,
) -> Result<(), AppError> {
    let principal = principal_from_request(state, headers).await?;
    let request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::ExecutionJobsRun,
        resource_type: "execution_job".to_string(),
        resource_id: Some(job_id),
    };
    state.authorizer.authorize(&principal, &request).await?;
    let job = state.execution_queue.get(job_id).await?;
    let session_request = AuthorizationRequest {
        tenant_id: state.tenant_id,
        permission: Permission::SessionsRead,
        resource_type: "session".to_string(),
        resource_id: Some(job.session_id),
    };
    enforce_resource_scope(state, &principal, &session_request).await
}

async fn list_session_audit_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    authorize_request(&state, &headers, Permission::AuditRead, "session", Some(id)).await?;
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
    let approval = state.get_approval(approval_id).await?;
    if approval.status != "pending" {
        return Err(AppError::bad_request(
            "only pending approvals can be decided",
        ));
    }
    if approval_is_expired(&approval) {
        expire_approval_record(&state, approval_id).await?;
        return Err(AppError::bad_request("approval expired"));
    }
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
                resume_provider_after_approval(&state, updated.session_id).await?;
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

fn approval_is_expired(approval: &Approval) -> bool {
    approval.status == "pending"
        && approval
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
}

async fn expire_approval_record(state: &AppState, approval_id: Uuid) -> Result<Approval, AppError> {
    let approval = state.get_approval(approval_id).await?;
    if approval.status != "pending" {
        return Ok(approval);
    }
    let updated = state.decide_approval(approval_id, "expired").await?;
    state
        .append_event(
            "system",
            Some(approval_id),
            updated.session_id,
            "approval.expired",
            json!({"approval_id": approval_id, "decision": "expired", "expires_at": updated.expires_at}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(updated.session_id),
            "system",
            Some(approval_id),
            "approval.expired",
            "approval",
            Some(approval_id),
            json!({"tool_call_id": updated.tool_call_id, "decision": "expired", "expires_at": updated.expires_at}),
        ))
        .await?;
    Ok(updated)
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

async fn resume_provider_after_approval(
    state: &AppState,
    session_id: Uuid,
) -> Result<(), AppError> {
    let events = state.list_events(session_id).await?;
    if !events.iter().any(|event| event.event_type == "llm.request") {
        complete_session_after_approval(state, session_id).await?;
        return Ok(());
    }

    state
        .set_session_status(session_id, SessionStatus::Running)
        .await?;
    let (provider_label, provider) = provider_client_for_session(state, session_id).await?;
    let provider_response =
        run_provider_harness(state, session_id, provider.as_ref(), &provider_label).await?;
    state
        .append_event(
            "agent",
            None,
            session_id,
            "agent.plan",
            json!({"phase": "approval_resume", "steps": provider_response.plan}),
        )
        .await?;

    for tool_call in provider_response.tool_calls {
        let result = execute_tool_invocation(
            state,
            &tool_call.tool_name,
            ExecuteTool {
                session_id,
                args: tool_call.args,
            },
        )
        .await?;
        if result.get("status").and_then(Value::as_str) == Some("approval_required") {
            return Ok(());
        }
    }

    if let Some(final_message) = provider_response.final_message {
        state
            .append_event(
                "agent",
                None,
                session_id,
                "agent.final",
                json!({"message": final_message, "resumed_after_approval": true}),
            )
            .await?;
    }

    complete_session_after_approval(state, session_id).await?;
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

fn default_mcp_transport() -> String {
    "http".to_string()
}

fn default_model() -> String {
    "gpt-5.4-mini".to_string()
}

fn default_session_title() -> String {
    "Untitled session".to_string()
}

fn empty_json_object() -> Value {
    json!({})
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

        let running = queue
            .start(queued.id, "test-worker")
            .await
            .expect("start job");
        assert_eq!(running.status, ExecutionJobStatus::Running);
        assert!(running.started_at.is_some());
        assert_eq!(running.worker_id.as_deref(), Some("test-worker"));
        assert!(running.lease_expires_at.is_some());

        let completed = queue.complete(queued.id).await.expect("complete job");
        assert_eq!(completed.status, ExecutionJobStatus::Completed);
        assert!(completed.completed_at.is_some());
    }

    #[tokio::test]
    async fn broker_execution_queue_is_reserved_until_implemented() {
        for kind in [BrokerQueueKind::Redis, BrokerQueueKind::Nats] {
            let queue = BrokerExecutionQueue::new(kind);
            let request = ExecutionJobRequest {
                session_id: Uuid::new_v4(),
                approval_id: Uuid::new_v4(),
                tool_call_id: Uuid::new_v4(),
                tool_name: "file.write".to_string(),
            };

            assert!(queue.enqueue(request).await.is_err());
            assert!(queue.start(Uuid::new_v4(), "worker").await.is_err());
            assert!(queue.complete(Uuid::new_v4()).await.is_err());
            assert!(queue.fail(Uuid::new_v4()).await.is_err());
            assert!(queue.list().await.is_err());
            assert!(queue.get(Uuid::new_v4()).await.is_err());
        }
    }

    #[tokio::test]
    async fn reads_agent_versions_for_agent() {
        let app = test_app().await;
        let created: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                    "name": "Versioned Agent",
                    "kind": "orchestrator",
                    "provider": "openai-compatible",
                    "model": "gpt-5.4-mini",
                    "system_prompt": "Keep a version record.",
                    "tools": ["file.read", "sql.query"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
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
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                    "name": "Read Only Agent",
                    "kind": "orchestrator",
                    "provider": "openai-compatible",
                    "model": "gpt-5.4-mini",
                    "system_prompt": "Read only.",
                    "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
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
            }],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 7,
                "total_tokens": 18
            }
        });
        let parsed =
            parse_openai_compatible_provider_response(&response).expect("provider response parses");
        assert_eq!(parsed.plan, vec!["Read files", "Query demo data"]);
        assert_eq!(parsed.tool_calls.len(), 2);
        assert_eq!(parsed.tool_calls[0].tool_name, "file.read");
        assert_eq!(parsed.tool_calls[0].args["paths"][0], "README.md");
        assert_eq!(parsed.tool_calls[1].tool_name, "sql.query");
        assert_eq!(
            parsed.final_message.as_deref(),
            Some("{\"plan\":[\"Read files\",\"Query demo data\"]}")
        );
        let usage = parsed.usage.expect("token usage");
        assert_eq!(usage.prompt_tokens, 11);
        assert_eq!(usage.completion_tokens, 7);
        assert_eq!(usage.total_tokens, 18);
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

    #[test]
    fn selects_execution_queue_backend_fail_closed() {
        assert_eq!(
            select_execution_queue_backend(None, false).expect("auto memory"),
            ExecutionQueueBackendSelection::Memory
        );
        assert_eq!(
            select_execution_queue_backend(Some("auto"), true).expect("auto postgres"),
            ExecutionQueueBackendSelection::Postgres
        );
        assert_eq!(
            select_execution_queue_backend(Some("memory"), true).expect("forced memory"),
            ExecutionQueueBackendSelection::Memory
        );
        assert!(
            select_execution_queue_backend(Some("postgres"), false).is_err(),
            "forced postgres queue should require DATABASE_URL"
        );
        assert!(
            select_execution_queue_backend(Some("redis"), true).is_err(),
            "broker-backed queue names are reserved until implemented"
        );
    }

    async fn test_app() -> Router {
        test_app_with_worker(Arc::new(InlineExecutionWorker)).await
    }

    async fn test_app_with_worker(execution_worker: Arc<dyn ExecutionWorker>) -> Router {
        let state = AppState {
            store: StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
            execution_queue: ExecutionQueue::default(),
            execution_worker,
            authorizer: Arc::new(RoleBasedAuthorizer),
            observability_config: ObservabilityConfig {
                service_name: "mandoforge-api-test".to_string(),
                otlp_endpoint: None,
                sample_ratio: 1.0,
            },
            telemetry_exporter: Arc::new(ReservedTelemetryExporter),
            mcp_gateway_config: None,
            mcp_gateway_client: Arc::new(ReservedMcpGatewayClient),
            workspace_root: test_workspace_root(),
            tenant_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("valid uuid"),
            policy: PolicyConfig::default(),
        };
        state.seed_demo_agent().await.expect("seed demo agent");
        build_router(state)
    }

    #[derive(Default)]
    struct RecordingTelemetryExporter {
        events: tokio::sync::Mutex<Vec<TelemetryEvent>>,
    }

    #[async_trait::async_trait]
    impl TelemetryExporter for RecordingTelemetryExporter {
        async fn health_check(&self, _config: &ObservabilityConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn export_event(
            &self,
            _config: &ObservabilityConfig,
            event: TelemetryEvent,
        ) -> Result<(), AppError> {
            self.events.lock().await.push(event);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingMcpGatewayClient {
        requests: tokio::sync::Mutex<Vec<McpCallRequest>>,
    }

    #[async_trait::async_trait]
    impl McpGatewayClient for RecordingMcpGatewayClient {
        async fn health_check(&self, _config: &McpGatewayConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn call(
            &self,
            config: &McpGatewayConfig,
            request: McpCallRequest,
        ) -> Result<mcp_gateway::McpCallResponse, AppError> {
            if !config.allows_server(&request.server) {
                return Err(AppError::forbidden(format!(
                    "MCP server {} is not allowed",
                    request.server
                )));
            }
            self.requests.lock().await.push(request.clone());
            Ok(mcp_gateway::McpCallResponse {
                result: json!({
                    "server": request.server,
                    "tool": request.tool,
                    "args": request.args,
                    "status": "called"
                }),
            })
        }

        async fn discover_tools(
            &self,
            config: &McpGatewayConfig,
            server: &str,
        ) -> Result<Vec<mcp_gateway::McpToolDescriptor>, AppError> {
            if !config.allows_server(server) {
                return Err(AppError::forbidden(format!(
                    "MCP server {server} is not allowed"
                )));
            }
            Ok(vec![mcp_gateway::McpToolDescriptor {
                name: "search".to_string(),
                description: Some("Search governed docs".to_string()),
            }])
        }
    }

    #[tokio::test]
    async fn appended_session_events_export_telemetry_when_enabled() {
        let exporter = Arc::new(RecordingTelemetryExporter::default());
        let state = AppState {
            store: StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
            execution_queue: ExecutionQueue::default(),
            execution_worker: Arc::new(InlineExecutionWorker),
            authorizer: Arc::new(RoleBasedAuthorizer),
            observability_config: ObservabilityConfig {
                service_name: "mandoforge-api-test".to_string(),
                otlp_endpoint: Some("http://otel.test".to_string()),
                sample_ratio: 1.0,
            },
            telemetry_exporter: exporter.clone(),
            mcp_gateway_config: None,
            mcp_gateway_client: Arc::new(ReservedMcpGatewayClient),
            workspace_root: test_workspace_root(),
            tenant_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("valid uuid"),
            policy: PolicyConfig::default(),
        };
        state.seed_demo_agent().await.expect("seed demo agent");
        let agent = state
            .list_agents()
            .await
            .expect("list agents")
            .into_iter()
            .next()
            .expect("seeded agent");
        let session = state
            .create_session(CreateSession {
                agent_id: agent.id,
                title: "telemetry".to_string(),
                message: None,
            })
            .await
            .expect("create session");

        state
            .append_event(
                "system",
                None,
                session.id,
                "session.started",
                json!({"source": "test"}),
            )
            .await
            .expect("append event");
        state
            .append_event(
                "agent",
                None,
                session.id,
                "llm.response",
                json!({
                    "provider": "governed-mock",
                    "client": "mock-openai-compatible",
                    "tool_calls": [{"tool": "file.read"}],
                    "duration_ms": 42
                }),
            )
            .await
            .expect("append provider event");

        let events = exporter.events.lock().await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "session.started");
        assert_eq!(events[0].attributes["session_id"], session.id.to_string());
        assert_eq!(events[0].attributes["signal"]["type"], "span");
        assert_eq!(events[0].attributes["metrics"]["event_count"], 1);
        assert_eq!(events[1].name, "llm.response");
        assert_eq!(events[1].attributes["provider"], "governed-mock");
        assert_eq!(events[1].attributes["client"], "mock-openai-compatible");
        assert_eq!(events[1].attributes["metrics"]["duration_ms"], 42);
        assert_eq!(events[1].attributes["metrics"]["tool_call_count"], 1);
    }

    #[tokio::test]
    async fn mcp_call_executes_through_tool_router_and_gateway_policy() {
        let mcp_client = Arc::new(RecordingMcpGatewayClient::default());
        let state = AppState {
            store: StoreBackend::Memory(Arc::new(RwLock::new(MemoryStore::default()))),
            execution_queue: ExecutionQueue::default(),
            execution_worker: Arc::new(InlineExecutionWorker),
            authorizer: Arc::new(RoleBasedAuthorizer),
            observability_config: ObservabilityConfig {
                service_name: "mandoforge-api-test".to_string(),
                otlp_endpoint: None,
                sample_ratio: 1.0,
            },
            telemetry_exporter: Arc::new(ReservedTelemetryExporter),
            mcp_gateway_config: Some(McpGatewayConfig {
                endpoint: "http://mcp.test".to_string(),
                timeout_seconds: 5,
                allowed_servers: vec!["docs".to_string()],
            }),
            mcp_gateway_client: mcp_client.clone(),
            workspace_root: test_workspace_root(),
            tenant_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("valid uuid"),
            policy: PolicyConfig::default(),
        };
        state.seed_demo_agent().await.expect("seed demo agent");
        let organization = state
            .create_organization(CreateOrganization {
                name: "MCP Org".to_string(),
                slug: "mcp-org".to_string(),
            })
            .await
            .expect("create org");
        let team = state
            .create_team(
                organization.id,
                CreateTeam {
                    name: "MCP Team".to_string(),
                    slug: "mcp-team".to_string(),
                },
            )
            .await
            .expect("create team");
        state
            .create_provider_access(
                team.id,
                CreateProviderAccess {
                    provider_name: "openai-compatible".to_string(),
                    model_allowlist: vec!["gpt-5.4-mini".to_string()],
                },
            )
            .await
            .expect("create provider access");
        let mcp_server = state
            .create_mcp_server(
                team.id,
                CreateMcpServerRecord {
                    name: "docs".to_string(),
                    transport: "http".to_string(),
                    config: json!({"source": "test"}),
                    tool_allowlist: vec![],
                },
            )
            .await
            .expect("create mcp server");
        let scoped_agent = state
            .create_agent(CreateAgent {
                name: "MCP Scoped Agent".to_string(),
                kind: "orchestrator".to_string(),
                provider: "openai-compatible".to_string(),
                model: "gpt-5.4-mini".to_string(),
                team_id: Some(team.id),
                project_id: None,
                system_prompt: "Use governed MCP tools.".to_string(),
                tools: vec!["mcp.call".to_string()],
            })
            .await
            .expect("create scoped agent");
        let scoped_session = state
            .create_session(CreateSession {
                agent_id: scoped_agent.id,
                title: "scoped mcp call".to_string(),
                message: None,
            })
            .await
            .expect("create scoped session");
        let app = build_router(state);

        let discovered: McpServerRecord = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/teams/{}/mcp-servers/{}/discover",
                    team.id, mcp_server.id
                ))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(discovered.tool_allowlist, vec!["search".to_string()]);

        let result: Value = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/tools/mcp.call/execute")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "session_id": scoped_session.id,
                        "args": {
                            "server": "docs",
                            "tool": "search",
                            "args": {"q": "policy"}
                        }
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(result["status"], "called");
        assert_eq!(result["result"]["server"], "docs");

        let requests = mcp_client.requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].tool, "search");
        drop(requests);

        let (status, denied) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/tools/mcp.call/execute")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "session_id": scoped_session.id,
                        "args": {
                            "server": "docs",
                            "tool": "write",
                            "args": {"q": "policy"}
                        }
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            denied["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let tool_calls: Vec<ToolCall> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", scoped_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(tool_calls.iter().any(|call| {
            call.tool_name == "mcp.call"
                && call.status == "completed"
                && call.policy_decision["decision"] == "allowed"
        }));
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
        assert!(
            event_types_after_approval
                .iter()
                .filter(|event_type| **event_type == "llm.request")
                .count()
                >= 2,
            "provider should be resumed after approval: {event_types_after_approval:?}"
        );
        assert!(event_types_after_approval.contains(&"agent.final"));
        assert!(event_types_after_approval.contains(&"session.completed"));
        assert!(events_after_approval.iter().any(|event| {
            event.event_type == "llm.response"
                && event
                    .payload
                    .get("final_message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("Approved execution completed"))
        }));
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
    async fn session_run_enforces_rbac_role() {
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
                json!({"agent_id": agent.id, "title": "rbac session run"}),
            ),
        )
        .await;

        let (status, error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", session.id))
                .header("x-mandoforge-subject", "viewer-1")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let unchanged: Session = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(unchanged.status, SessionStatus::Created));
    }

    #[tokio::test]
    async fn read_routes_enforce_rbac_role_header() {
        let app = test_app().await;
        for uri in [
            "/api/agents",
            "/api/tools",
            "/api/sessions",
            "/api/approvals",
            "/api/tool-calls",
            "/api/execution-jobs",
            "/api/audit-logs",
        ] {
            let (status, error) = request_value(
                app.clone(),
                Request::builder()
                    .uri(uri)
                    .header("x-mandoforge-roles", "")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await;

            assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
            assert_eq!(error["error"], "principal has no roles");
        }

        let (status, _) = request_value(
            app,
            Request::builder()
                .uri("/api/agents")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn write_routes_enforce_rbac_role_header() {
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

        let (status, error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::from(
                    json!({"agent_id": agent.id, "title": "viewer denied session"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let session: Session = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/sessions",
                json!({"agent_id": agent.id, "title": "operator session"}),
            ),
        )
        .await;
        let (status, _) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/messages", session.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-roles", "operator")
                .body(Body::from(json!({"message": "allowed"}).to_string()))
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let (status, error) = request_value(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-roles", "operator")
                .body(Body::from(
                    json!({
                        "name": "Operator Denied Agent",
                        "description": "operators cannot create agents",
                        "model": "mock",
                        "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
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
    async fn manual_tool_execution_enforces_rbac_role() {
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
                json!({"agent_id": agent.id, "title": "rbac denied tool"}),
            ),
        )
        .await;

        let (status, error) = request_value(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/tools/sql.get_schema/execute")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "viewer-1")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::from(
                    json!({"session_id": session.id, "args": {}}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );
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
        let pending_approval = approvals
            .iter()
            .find(|approval| {
                approval.session_id == session.id
                    && approval.action == "manual.review"
                    && approval.status == "pending"
            })
            .expect("pending manual approval");
        assert!(pending_approval.expires_at.is_some());

        let expired: Approval = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{}/expire", pending_approval.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(expired.status, "expired");
        let (status, approval_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{}/approve", pending_approval.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            approval_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("only pending approvals")
        );

        let waiting: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(matches!(waiting.status, SessionStatus::WaitingApproval));

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
                .any(|event| event.event_type == "approval.expired")
        );

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
    async fn approval_decision_enforces_rbac_role() {
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
                json!({"agent_id": agent.id, "title": "rbac approval"}),
            ),
        )
        .await;

        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": "rbac.md",
                        "content": "denied"
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_result["approval_id"]
            .as_str()
            .expect("approval id");

        let (status, error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{approval_id}/approve"))
                .header("x-mandoforge-subject", "viewer-1")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let approvals: Vec<Approval> = request_json(
            app,
            Request::builder()
                .uri("/api/approvals")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let pending = approvals
            .iter()
            .find(|approval| approval.id.to_string() == approval_id)
            .expect("approval remains visible");
        assert_eq!(pending.status, "pending");
    }

    #[tokio::test]
    async fn admin_can_manage_stage2_governance_scope() {
        let app = test_app().await;

        let (status, error) = request_value(
            app.clone(),
            json_request(
                "POST",
                "/api/organizations",
                json!({"name": "Denied Org", "slug": "denied"}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let organization: Organization = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/organizations")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"name": "Platform Org", "slug": "platform"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(organization.slug, "platform");

        let team: Team = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/organizations/{}/teams", organization.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"name": "Runtime Team", "slug": "runtime"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(team.organization_id, organization.id);

        let project: Project = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/projects", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"name": "Kernel Pilot", "slug": "kernel-pilot"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(project.team_id, team.id);

        let sibling_project: Project = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/projects", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"name": "Sibling Pilot", "slug": "sibling-pilot"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(sibling_project.team_id, team.id);

        let membership: Membership = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/organizations/{}/memberships",
                    organization.id
                ))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"user_id": "approver-1", "team_id": team.id, "role": "approver"})
                        .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(membership.organization_id, Some(organization.id));
        assert_eq!(membership.team_id, Some(team.id));
        assert_eq!(membership.project_id, None);

        let project_membership: Membership = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/organizations/{}/memberships",
                    organization.id
                ))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "user_id": "project-viewer-1",
                        "team_id": team.id,
                        "project_id": project.id,
                        "role": "viewer"
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(project_membership.team_id, Some(team.id));
        assert_eq!(project_membership.project_id, Some(project.id));

        let (status, project_membership_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/organizations/{}/memberships",
                    organization.id
                ))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "user_id": "invalid-project-viewer",
                        "project_id": project.id,
                        "role": "viewer"
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            project_membership_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("project_id requires")
        );

        let derived_approver_approvals: Vec<Approval> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/approvals")
                .header("x-mandoforge-subject", "approver-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(derived_approver_approvals.is_empty());

        let (status, provider_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "blocked scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "provider": "openai-compatible",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            provider_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let provider_access: ProviderAccess = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/provider-access", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_name": "openai-compatible",
                        "model_allowlist": ["gpt-5.4-mini"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(provider_access.team_id, team.id);

        let governed_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/providers")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_type": "mock",
                        "name": "governed-mock",
                        "default_model": "gpt-5.4-mini",
                        "config": {
                            "budget": {"daily_request_limit": 1},
                            "pricing": {
                                "per_request_cents": 2.5,
                                "per_1k_prompt_tokens_cents": 1.0,
                                "per_1k_completion_tokens_cents": 2.0
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(governed_provider.name, "governed-mock");

        let governed_provider_access: ProviderAccess = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/provider-access", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_name": "governed-mock",
                        "model_allowlist": ["gpt-5.4-mini"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(governed_provider_access.provider_name, "governed-mock");

        let cost_limited_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/providers")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_type": "mock",
                        "name": "cost-limited-mock",
                        "default_model": "gpt-5.4-mini",
                        "config": {
                            "budget": {"daily_cost_limit_cents": 1.0},
                            "pricing": {"per_request_cents": 2.5}
                        }
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(cost_limited_provider.name, "cost-limited-mock");
        let cost_limited_access: ProviderAccess = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/provider-access", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_name": "cost-limited-mock",
                        "model_allowlist": ["gpt-5.4-mini"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(cost_limited_access.provider_name, "cost-limited-mock");

        let providers: Vec<ProviderRecord> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/providers")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            providers
                .iter()
                .any(|provider| provider.name == "governed-mock")
        );

        let status_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/providers")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_type": "mock",
                        "name": "status-managed-mock",
                        "default_model": "gpt-5.4-mini",
                        "config": {}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status_provider.status, "active");

        let status_provider_health: ProviderHealth = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/providers/{}/health", status_provider.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(status_provider_health.healthy);
        assert!(status_provider_health.issues.is_empty());

        let status_provider_access: ProviderAccess = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/teams/{}/provider-access", team.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_name": "status-managed-mock",
                        "model_allowlist": ["gpt-5.4-mini"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status_provider_access.provider_name, "status-managed-mock");

        let (status, invalid_status_error) = request_value(
            app.clone(),
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/providers/{}/status", status_provider.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"status": "paused"}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            invalid_status_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("unsupported provider status")
        );

        let disabled_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/providers/{}/status", status_provider.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"status": "disabled"}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(disabled_provider.status, "disabled");
        let disabled_provider_health: ProviderHealth = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/providers/{}/health", status_provider.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(!disabled_provider_health.healthy);
        assert!(
            disabled_provider_health
                .issues
                .iter()
                .any(|issue| issue.contains("disabled"))
        );

        let misconfigured_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/providers")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "provider_type": "openai-compatible",
                        "name": "misconfigured-openai-compatible",
                        "default_model": "gpt-5.4-mini",
                        "config": {}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let misconfigured_health: ProviderHealth = request_json(
            app.clone(),
            Request::builder()
                .uri(format!(
                    "/api/providers/{}/health",
                    misconfigured_provider.id
                ))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(!misconfigured_health.healthy);
        assert!(
            misconfigured_health
                .issues
                .iter()
                .any(|issue| issue.contains("base_url"))
        );
        assert!(
            misconfigured_health
                .issues
                .iter()
                .any(|issue| issue.contains("api_key_env"))
        );

        let status_agent: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "status managed scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "provider": "status-managed-mock",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read", "sql.get_schema", "sql.query", "shell.exec"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        let disabled_status_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": status_agent.id, "title": "disabled provider session"})
                        .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let (status, disabled_run_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", disabled_status_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            disabled_run_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("provider status-managed-mock is not active")
        );

        let cost_limited_agent: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "cost limited scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "provider": "cost-limited-mock",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let cost_limited_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": cost_limited_agent.id, "title": "cost limited session"})
                        .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let (status, cost_budget_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", cost_limited_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            cost_budget_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("exceeded daily cost budget")
        );

        let budget_agent: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "budget scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "project_id": project.id,
                        "provider": "governed-mock",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read", "sql.get_schema", "sql.query", "shell.exec"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        let budget_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": budget_agent.id, "title": "budget session"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let budget_run: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", budget_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(budget_run.status, SessionStatus::WaitingApproval);

        let second_budget_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": budget_agent.id, "title": "budget session over limit"})
                        .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let (status, budget_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", second_budget_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            budget_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("exceeded daily request budget")
        );

        let usage: UsageSummary = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/usage")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(usage.provider_request_count, 1);
        assert_eq!(usage.by_provider["governed-mock"].request_count, 1);
        assert_eq!(usage.prompt_tokens, 180);
        assert_eq!(usage.completion_tokens, 60);
        assert_eq!(usage.total_tokens, 240);
        assert_eq!(usage.by_provider["governed-mock"].prompt_tokens, 180);
        assert_eq!(usage.by_provider["governed-mock"].completion_tokens, 60);
        assert_eq!(usage.by_provider["governed-mock"].total_tokens, 240);
        assert!((usage.by_provider["governed-mock"].token_cost_cents - 0.3).abs() < 0.000001);
        assert!((usage.estimated_provider_cost_cents - 2.8).abs() < 0.000001);
        let governed_budget = usage
            .provider_budgets
            .iter()
            .find(|budget| budget.provider_name == "governed-mock")
            .expect("governed mock budget status");
        assert_eq!(governed_budget.status, "critical");
        assert_eq!(governed_budget.request_count, 1);
        assert_eq!(governed_budget.daily_request_limit, Some(1));
        assert_eq!(governed_budget.request_budget_used_percent, Some(100.0));
        assert!((governed_budget.estimated_cost_cents - 2.8).abs() < 0.000001);
        assert!(
            governed_budget
                .messages
                .iter()
                .any(|message| message.contains("daily requests used"))
        );

        let rollup: UsageRollup = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/usage/rollups")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(rollup.summary["provider_request_count"], 1);
        assert_eq!(rollup.summary["total_tokens"], 240);
        assert!(
            (rollup.summary["estimated_provider_cost_cents"]
                .as_f64()
                .unwrap()
                - 2.8)
                .abs()
                < 0.000001
        );
        let rollups: Vec<UsageRollup> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/usage/rollups")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(rollups.len(), 1);
        assert_eq!(rollups[0].id, rollup.id);

        let active_provider: ProviderRecord = request_json(
            app.clone(),
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/providers/{}/status", status_provider.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"status": "active"}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(active_provider.status, "active");

        let active_status_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": status_agent.id, "title": "active provider session"})
                        .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let active_run: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/run", active_status_session.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(active_run.status, SessionStatus::WaitingApproval);

        let scoped_agent: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "allowed scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "project_id": project.id,
                        "provider": "openai-compatible",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(scoped_agent.team_id, Some(team.id));
        assert_eq!(scoped_agent.project_id, Some(project.id));

        let sibling_agent: Agent = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/agents")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "sibling scoped agent",
                        "kind": "orchestrator",
                        "team_id": team.id,
                        "project_id": sibling_project.id,
                        "provider": "openai-compatible",
                        "model": "gpt-5.4-mini",
                        "tools": ["file.read"]
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(sibling_agent.project_id, Some(sibling_project.id));

        let scoped_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": scoped_agent.id, "title": "scoped session"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        let sibling_session: Session = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({"agent_id": sibling_agent.id, "title": "sibling session"}).to_string(),
                ))
                .expect("valid request"),
        )
        .await;

        let (status, scoped_error) = request_value(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", scoped_session.id))
                .header("x-mandoforge-subject", "outside-operator")
                .header("x-mandoforge-roles", "operator")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            scoped_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("no membership")
        );

        let scoped_read: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", scoped_session.id))
                .header("x-mandoforge-subject", "approver-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(scoped_read.id, scoped_session.id);

        let project_scoped_read: Session = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", scoped_session.id))
                .header("x-mandoforge-subject", "project-viewer-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(project_scoped_read.id, scoped_session.id);

        let (status, sibling_error) = request_value(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}", sibling_session.id))
                .header("x-mandoforge-subject", "project-viewer-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            sibling_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("no membership")
        );

        let outside_sessions: Vec<Session> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/sessions")
                .header("x-mandoforge-subject", "outside-operator")
                .header("x-mandoforge-roles", "operator")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            !outside_sessions
                .iter()
                .any(|session| session.id == scoped_session.id)
        );

        let approver_agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .header("x-mandoforge-subject", "approver-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            approver_agents
                .iter()
                .any(|agent| agent.id == scoped_agent.id)
        );

        let project_viewer_agents: Vec<Agent> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/agents")
                .header("x-mandoforge-subject", "project-viewer-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            project_viewer_agents
                .iter()
                .any(|agent| agent.id == scoped_agent.id)
        );
        assert!(
            !project_viewer_agents
                .iter()
                .any(|agent| agent.id == sibling_agent.id)
        );

        let project_viewer_sessions: Vec<Session> = request_json(
            app.clone(),
            Request::builder()
                .uri("/api/sessions")
                .header("x-mandoforge-subject", "project-viewer-1")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            project_viewer_sessions
                .iter()
                .any(|session| session.id == scoped_session.id)
        );
        assert!(
            !project_viewer_sessions
                .iter()
                .any(|session| session.id == sibling_session.id)
        );

        let projects: Vec<Project> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/teams/{}/projects", team.id))
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(projects.len(), 2);
        assert!(
            projects
                .iter()
                .any(|project| project.slug == "kernel-pilot")
        );
        assert!(
            projects
                .iter()
                .any(|project| project.slug == "sibling-pilot")
        );
    }

    #[tokio::test]
    async fn admin_can_create_eval_dataset_cases_and_version_bound_run() {
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

        let dataset: EvalDataset = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/eval/datasets")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "Stage 2 policy eval",
                        "description": "Checks policy and tool selection plumbing."
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(dataset.name, "Stage 2 policy eval");

        let case: EvalCase = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/cases", dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "input": {"message": "run shell command"},
                        "expected": {"tool": "shell.exec", "decision": "requires_approval"},
                        "grading_policy": {"kind": "policy"}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(case.dataset_id, dataset.id);

        let sql_case: EvalCase = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/cases", dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "input": {"sql": "UPDATE agents SET name = 'bad'"},
                        "expected": {"allowed": false},
                        "grading_policy": {"kind": "sql_safety"}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(sql_case.dataset_id, dataset.id);

        let tool_case: EvalCase = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/cases", dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "input": {"message": "read files and query data"},
                        "expected": {"required_tools": ["file.read", "sql.query"]},
                        "grading_policy": {"kind": "tool_selection"}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(tool_case.dataset_id, dataset.id);

        let run: EvalRun = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/runs", dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"agent_id": agent.id}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(run.dataset_id, dataset.id);
        assert_eq!(run.agent_id, agent.id);
        assert_eq!(run.status, "completed");
        assert_eq!(run.score, Some(1.0));
        assert_eq!(run.details["runner"], "stage2-rule-graders");
        assert_eq!(run.details["case_count"], 3);
        assert_eq!(run.details["passed_count"], 3);

        let gate: EvalGateDecision = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/runs/{}/gate", run.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"min_score": 1.0}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(gate.run_id, run.id);
        assert_eq!(gate.status, "passed");
        assert!(gate.failure_reasons.is_empty());

        let (status, gate_error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/runs/{}/gate", run.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"min_score": 1.1}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            gate_error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("min_score must be between")
        );

        let failing_dataset: EvalDataset = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/eval/datasets")
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "name": "Stage 2 failing eval",
                        "description": "Checks eval gate failure evidence."
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let _: EvalCase = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/cases", failing_dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(
                    json!({
                        "input": {"message": "read files"},
                        "expected": {"required_tools": ["production_db.write"]},
                        "grading_policy": {"kind": "tool_selection"}
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        let failing_run: EvalRun = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/datasets/{}/runs", failing_dataset.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"agent_id": agent.id}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(failing_run.status, "failed");
        let failed_gate: EvalGateDecision = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/eval/runs/{}/gate", failing_run.id))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::from(json!({"min_score": 1.0}).to_string()))
                .expect("valid request"),
        )
        .await;
        assert_eq!(failed_gate.status, "failed");
        assert!(
            failed_gate
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("score"))
        );

        let runs: Vec<EvalRun> = request_json(
            app,
            Request::builder()
                .uri("/api/eval/runs")
                .header("x-mandoforge-subject", "admin-1")
                .header("x-mandoforge-roles", "admin")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|listed| listed.id == run.id));
        assert!(runs.iter().any(|listed| listed.id == failing_run.id));
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
    async fn approval_modify_updates_waiting_tool_args_before_approve() {
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
                json!({"agent_id": agent.id, "title": "approval modify"}),
            ),
        )
        .await;

        let approval_result: Value = request_json(
            app.clone(),
            json_request(
                "POST",
                "/api/tools/file.write/execute",
                json!({
                    "session_id": session.id,
                    "args": {
                        "path": "before.md",
                        "content": "before"
                    }
                }),
            ),
        )
        .await;
        let approval_id = approval_result["approval_id"]
            .as_str()
            .expect("approval id");

        let modified: Approval = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{approval_id}/modify"))
                .header("content-type", "application/json")
                .header("x-mandoforge-subject", "approver-1")
                .header("x-mandoforge-roles", "approver")
                .body(Body::from(
                    json!({
                        "args": {
                            "path": "after.md",
                            "content": "after"
                        },
                        "comment": "tighten file name before approval"
                    })
                    .to_string(),
                ))
                .expect("valid request"),
        )
        .await;
        assert_eq!(modified.status, "pending");
        assert_eq!(
            modified.decision_payload["modified_args"]["path"],
            "after.md"
        );

        let tool_calls: Vec<ToolCall> = request_json(
            app.clone(),
            Request::builder()
                .uri(format!("/api/sessions/{}/tool-calls", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        let file_write = tool_calls
            .iter()
            .find(|call| call.tool_name == "file.write")
            .expect("file.write tool call");
        assert_eq!(file_write.args["path"], "after.md");

        let approved: Approval = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/approvals/{approval_id}/approve"))
                .header("x-mandoforge-subject", "approver-1")
                .header("x-mandoforge-roles", "approver")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(approved.status, "approved");

        let written = tokio::fs::read_to_string(
            test_workspace_root()
                .join(session.id.to_string())
                .join("after.md"),
        )
        .await
        .expect("approved modified file.write created workspace file");
        assert_eq!(written, "after");
        assert!(
            tokio::fs::metadata(
                test_workspace_root()
                    .join(session.id.to_string())
                    .join("before.md"),
            )
            .await
            .is_err(),
            "original unmodified file should not be written"
        );

        let events: Vec<SessionEvent> = request_json(
            app,
            Request::builder()
                .uri(format!("/api/sessions/{}/events", session.id))
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "approval.modified")
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
        let job_id = job.id;

        let (status, error) = request_value(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/execution-jobs/{job_id}/run"))
                .header("x-mandoforge-subject", "viewer-1")
                .header("x-mandoforge-roles", "viewer")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not allowed")
        );

        let completed: execution_queue::ExecutionJob = request_json(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/api/execution-jobs/{job_id}/run"))
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
