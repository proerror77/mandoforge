use std::{path::PathBuf, sync::Arc};

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    CostAlertSmtpConfig, PolicyRuntime, TenantRuntimeMode,
    authorization::Authorizer,
    codex_app_server::{CodexAppServerClient, CodexAppServerConfig},
    eval_judge::{EvalJudgeClient, EvalJudgeConfig},
    execution::ExecutionWorker,
    execution_queue::ExecutionQueue,
    mcp_gateway::{McpGatewayClient, McpGatewayConfig},
    observability::{ObservabilityConfig, TelemetryExporter},
    store_backend::StoreBackend,
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) store: StoreBackend,
    pub(crate) execution_queue: ExecutionQueue,
    pub(crate) execution_worker: Arc<dyn ExecutionWorker>,
    pub(crate) authorizer: Arc<dyn Authorizer>,
    pub(crate) observability_config: ObservabilityConfig,
    pub(crate) telemetry_exporter: Arc<dyn TelemetryExporter>,
    pub(crate) mcp_gateway_config: Option<McpGatewayConfig>,
    pub(crate) mcp_gateway_client: Arc<dyn McpGatewayClient>,
    pub(crate) codex_app_server_config: Option<CodexAppServerConfig>,
    pub(crate) codex_app_server_client: Arc<dyn CodexAppServerClient>,
    pub(crate) eval_judge_config: Option<EvalJudgeConfig>,
    pub(crate) eval_judge_client: Arc<dyn EvalJudgeClient>,
    pub(crate) cost_alert_webhook_url: Option<String>,
    pub(crate) cost_alert_email_relay_url: Option<String>,
    pub(crate) cost_alert_smtp_config: Option<CostAlertSmtpConfig>,
    pub(crate) approval_webhook_url: Option<String>,
    #[allow(dead_code)]
    pub(crate) workspace_root: PathBuf,
    pub(crate) tenant_id: Uuid,
    pub(crate) tenant_runtime_mode: TenantRuntimeMode,
    pub(crate) policy: Arc<RwLock<PolicyRuntime>>,
}
