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
use tokio::{process::Command, sync::RwLock};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    inner: Arc<RwLock<Store>>,
    workspace_root: PathBuf,
}

#[derive(Default)]
struct Store {
    agents: HashMap<Uuid, Agent>,
    sessions: HashMap<Uuid, Session>,
    events: HashMap<Uuid, Vec<SessionEvent>>,
    approvals: HashMap<Uuid, Approval>,
    artifacts: HashMap<Uuid, Artifact>,
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
    action: String,
    risk_level: String,
    reason: String,
    evidence: Value,
    status: String,
    created_at: DateTime<Utc>,
    decided_at: Option<DateTime<Utc>>,
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

    let state = AppState {
        inner: Arc::new(RwLock::new(Store::default())),
        workspace_root,
    };
    seed_demo_agent(&state).await;

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
        .route("/api/tools", get(list_tools))
        .route("/api/tools/{name}/execute", post(execute_tool))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}/approve", post(approve))
        .route("/api/approvals/{id}/reject", post(reject))
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

async fn list_agents(State(state): State<AppState>) -> Json<Vec<Agent>> {
    Json(state.inner.read().await.agents.values().cloned().collect())
}

async fn create_agent(
    State(state): State<AppState>,
    Json(input): Json<CreateAgent>,
) -> Json<Agent> {
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
    state
        .inner
        .write()
        .await
        .agents
        .insert(agent.id, agent.clone());
    Json(agent)
}

async fn list_sessions(State(state): State<AppState>) -> Json<Vec<Session>> {
    Json(
        state
            .inner
            .read()
            .await
            .sessions
            .values()
            .cloned()
            .collect(),
    )
}

async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSession>,
) -> Result<Json<Session>, AppError> {
    let now = Utc::now();
    let session = Session {
        id: Uuid::new_v4(),
        agent_id: input.agent_id,
        title: input.title,
        status: SessionStatus::Created,
        created_at: now,
        updated_at: now,
    };
    {
        let mut store = state.inner.write().await;
        if !store.agents.contains_key(&session.agent_id) {
            return Err(AppError::not_found("agent not found"));
        }
        store.sessions.insert(session.id, session.clone());
    }
    if let Some(message) = input.message {
        append_event(
            &state,
            session.id,
            "user",
            None,
            "user.message",
            json!({ "message": message }),
        )
        .await?;
    }
    Ok(Json(session))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Session>, AppError> {
    state
        .inner
        .read()
        .await
        .sessions
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| AppError::not_found("session not found"))
}

async fn add_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMessage>,
) -> Result<Json<SessionEvent>, AppError> {
    Ok(Json(
        append_event(
            &state,
            id,
            "user",
            None,
            "user.message",
            json!({ "message": input.message }),
        )
        .await?,
    ))
}

async fn run_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Session>, AppError> {
    set_session_status(&state, id, SessionStatus::Running).await?;

    append_event(
        &state,
        id,
        "agent",
        None,
        "manager.plan",
        json!({
            "steps": [
                "Inspect GMV by day and compare to prior baseline",
                "Break down by SKU, advertising, refunds, inventory, and customer voice",
                "Draft operating actions and route risky changes to approval"
            ]
        }),
    )
    .await?;

    let schema = demo_schema();
    append_event(
        &state,
        id,
        "tool",
        None,
        "tool.result",
        json!({"tool": "warehouse.get_schema", "summary": "Demo commerce schema loaded", "content": schema}),
    )
    .await?;

    let diagnosis = demo_gmv_diagnosis();
    append_event(
        &state,
        id,
        "tool",
        None,
        "tool.result",
        json!({"tool": "warehouse.query", "summary": "GMV decline attribution query completed", "content": diagnosis}),
    )
    .await?;

    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: id,
        artifact_type: "markdown".to_string(),
        name: "gmv-diagnosis-report.md".to_string(),
        path: None,
        content: json!({
            "markdown": "# GMV Diagnosis\n\nGMV fell 18.4% day over day. Main drivers: SKU-A stockout, ad spend drop, refund spike, and negative reviews."
        }),
        created_at: Utc::now(),
    };
    state
        .inner
        .write()
        .await
        .artifacts
        .insert(artifact.id, artifact.clone());
    append_event(
        &state,
        id,
        "system",
        Some(artifact.id),
        "artifact.created",
        json!({"artifact_id": artifact.id, "name": artifact.name, "artifact_type": artifact.artifact_type}),
    )
    .await?;

    let approval = Approval {
        id: Uuid::new_v4(),
        session_id: id,
        action: "campaign.draft.coupon".to_string(),
        risk_level: "medium".to_string(),
        reason: "A coupon for at-risk users could recover demand, but it affects more than 1,000 customers.".to_string(),
        evidence: json!({"affected_users": 1250, "suggested_discount_percent": 5, "primary_skus": ["SKU-A", "SKU-D"]}),
        status: "pending".to_string(),
        created_at: Utc::now(),
        decided_at: None,
    };
    state
        .inner
        .write()
        .await
        .approvals
        .insert(approval.id, approval.clone());
    append_event(
        &state,
        id,
        "system",
        Some(approval.id),
        "approval.requested",
        json!({"approval_id": approval.id, "action": approval.action, "risk_level": approval.risk_level, "reason": approval.reason, "evidence": approval.evidence}),
    )
    .await?;

    append_event(
        &state,
        id,
        "agent",
        None,
        "llm.response",
        json!({
            "final_report": {
                "gmv_drop": "-18.4%",
                "top_skus": ["SKU-A", "SKU-D", "SKU-F", "SKU-B", "SKU-H"],
                "drivers": ["SKU-A stockout caused lost sales", "Ad ROI fell after spend was reduced 31%", "Refund rate rose on SKU-D", "Negative review rate doubled for delayed shipments"],
                "recommendations": [
                    "Replenish SKU-A and pin substitution SKU-B until stock recovers",
                    "Restore paid search budget on campaigns with ROI > 2.5",
                    "Open a logistics incident review for delayed shipment cohort",
                    "Draft a 5% retention coupon for affected high-value customers"
                ]
            }
        }),
    )
    .await?;

    let session = set_session_status(&state, id, SessionStatus::WaitingApproval).await?;
    Ok(Json(session))
}

async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Json<Vec<SessionEvent>> {
    Json(
        state
            .inner
            .read()
            .await
            .events
            .get(&id)
            .cloned()
            .unwrap_or_default(),
    )
}

async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let events = state
        .inner
        .read()
        .await
        .events
        .get(&id)
        .cloned()
        .unwrap_or_default();
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
            name: "warehouse.get_schema",
            risk: "low",
            description: "Return demo commerce warehouse schema",
        },
        ToolDescriptor {
            name: "warehouse.query",
            risk: "medium",
            description: "Execute read-only demo SQL",
        },
        ToolDescriptor {
            name: "inventory.query",
            risk: "low",
            description: "Inspect inventory and fulfillment signals",
        },
        ToolDescriptor {
            name: "customer_voice.search",
            risk: "low",
            description: "Search tickets and reviews",
        },
        ToolDescriptor {
            name: "campaign.draft",
            risk: "medium",
            description: "Draft campaign action for approval",
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
    append_event(
        &state,
        input.session_id,
        "tool",
        None,
        "tool.call",
        json!({"tool": name, "args": input.args}),
    )
    .await?;
    match name.as_str() {
        "warehouse.get_schema" => Ok(Json(demo_schema())),
        "warehouse.query" => {
            let sql = input
                .args
                .get("sql")
                .and_then(Value::as_str)
                .unwrap_or_default();
            ensure_read_only_sql(sql)?;
            Ok(Json(json!({"rows": demo_gmv_diagnosis(), "row_count": 5})))
        }
        "codex.exec" => {
            let request: CodexRequest = serde_json::from_value(input.args)?;
            let output = run_codex(&state, input.session_id, request).await?;
            Ok(Json(output))
        }
        _ => Err(AppError::not_found("unknown tool")),
    }
}

async fn list_approvals(State(state): State<AppState>) -> Json<Vec<Approval>> {
    Json(
        state
            .inner
            .read()
            .await
            .approvals
            .values()
            .cloned()
            .collect(),
    )
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
) -> Json<Vec<Artifact>> {
    Json(
        state
            .inner
            .read()
            .await
            .artifacts
            .values()
            .filter(|artifact| artifact.session_id == id)
            .cloned()
            .collect(),
    )
}

async fn decide_approval(
    state: AppState,
    approval_id: Uuid,
    status: &str,
) -> Result<Json<Approval>, AppError> {
    let mut store = state.inner.write().await;
    let approval = store
        .approvals
        .get_mut(&approval_id)
        .ok_or_else(|| AppError::not_found("approval not found"))?;
    approval.status = status.to_string();
    approval.decided_at = Some(Utc::now());
    let updated = approval.clone();
    drop(store);
    append_event(
        &state,
        updated.session_id,
        "user",
        Some(approval_id),
        &format!("approval.{status}"),
        json!({"approval_id": approval_id, "decision": status}),
    )
    .await?;
    if status == "approved" {
        set_session_status(&state, updated.session_id, SessionStatus::Completed).await?;
        append_event(
            &state,
            updated.session_id,
            "system",
            None,
            "session.completed",
            json!({"reason": "pending approval resolved"}),
        )
        .await?;
    }
    Ok(Json(updated))
}

async fn append_event(
    state: &AppState,
    session_id: Uuid,
    actor_type: &str,
    actor_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> Result<SessionEvent, AppError> {
    let mut store = state.inner.write().await;
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

async fn set_session_status(
    state: &AppState,
    session_id: Uuid,
    status: SessionStatus,
) -> Result<Session, AppError> {
    let mut store = state.inner.write().await;
    let session = store
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| AppError::not_found("session not found"))?;
    session.status = status;
    session.updated_at = Utc::now();
    Ok(session.clone())
}

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

    append_event(
        state,
        session_id,
        "tool",
        None,
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
    append_event(
        state,
        session_id,
        "tool",
        None,
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
            "warehouse.query only accepts read-only SQL",
        ));
    }
    if !lowered.starts_with("select")
        && !lowered.starts_with("with")
        && !lowered.starts_with("explain")
    {
        return Err(AppError::bad_request(
            "warehouse.query requires SELECT, WITH, or EXPLAIN",
        ));
    }
    Ok(())
}

fn demo_schema() -> Value {
    json!({
        "tables": {
            "orders": ["id", "customer_id", "ordered_at", "status", "channel", "session_id"],
            "order_items": ["order_id", "sku_id", "quantity", "unit_price", "unit_cost"],
            "products": ["sku_id", "name", "category", "brand"],
            "inventory": ["sku_id", "available_qty", "reserved_qty", "avg_daily_sales"],
            "ad_spend": ["campaign_id", "date", "sku_id", "spend", "attributed_gmv"],
            "tickets": ["id", "sku_id", "created_at", "category", "sentiment"],
            "reviews": ["id", "sku_id", "created_at", "rating", "body"],
            "refunds": ["id", "order_id", "sku_id", "amount", "reason"]
        },
        "metrics": {
            "gmv": "sum(order_items.quantity * order_items.unit_price)",
            "refund_rate": "refunds / orders",
            "ad_roi": "attributed_gmv / ad_spend",
            "stock_days": "inventory.available_qty / avg_daily_sales"
        }
    })
}

fn demo_gmv_diagnosis() -> Value {
    json!({
        "gmv_yesterday": 81240.50,
        "gmv_prior_day": 99580.25,
        "drop_percent": 18.4,
        "top_sku_drops": [
            {"sku_id": "SKU-A", "gmv_drop": 8420.0, "driver": "stockout", "stock_days": 0.4},
            {"sku_id": "SKU-D", "gmv_drop": 4210.5, "driver": "refund spike", "refund_rate": 0.18},
            {"sku_id": "SKU-F", "gmv_drop": 3033.2, "driver": "ad spend down", "ad_spend_delta": -0.31},
            {"sku_id": "SKU-B", "gmv_drop": 2210.0, "driver": "conversion down", "conversion_delta": -0.12},
            {"sku_id": "SKU-H", "gmv_drop": 1490.8, "driver": "negative reviews", "negative_review_rate": 0.26}
        ]
    })
}

async fn seed_demo_agent(state: &AppState) {
    let agent = Agent {
        id: Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid"),
        name: "Commerce Manager Agent".to_string(),
        kind: "manager".to_string(),
        provider: "openai-compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        system_prompt: "Diagnose commerce performance using warehouse facts, route risky actions to approval, and preserve an auditable timeline.".to_string(),
        tools: vec![
            "warehouse.get_schema".to_string(),
            "warehouse.query".to_string(),
            "inventory.query".to_string(),
            "customer_voice.search".to_string(),
            "campaign.draft".to_string(),
            "codex.exec".to_string(),
        ],
        created_at: Utc::now(),
    };
    state.inner.write().await.agents.insert(agent.id, agent);
}

fn default_agent_kind() -> String {
    "manager".to_string()
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
