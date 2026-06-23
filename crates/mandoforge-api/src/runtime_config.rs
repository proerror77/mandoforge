use std::sync::Arc;

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::{
    BrokerExecutionQueue, BrokerQueueConfig, BrokerQueueKind, CodexAppServerClient,
    CodexAppServerConfig, CostAlertSmtpConfig, DEFAULT_TENANT_ID, EvalJudgeClient,
    EvalJudgeConfig, ExecutionQueue, ExecutionWorker, HttpCodexAppServerClient,
    HttpEvalJudgeClient, HttpMcpGatewayClient, HttpTelemetryExporter, InlineExecutionWorker,
    McpGatewayClient, McpGatewayConfig, ObservabilityConfig, QueueBackedExecutionWorker,
    ReservedCodexAppServerClient, ReservedMcpGatewayClient, ReservedTelemetryExporter,
    StoreBackend, TelemetryExporter, TenantRuntimeMode, WsCodexAppServerClient,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionQueueBackendSelection {
    Memory,
    Postgres,
    Redis,
    Nats,
    NatsJetstream,
}

pub(crate) fn select_execution_queue_backend(
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
        "redis" => Ok(ExecutionQueueBackendSelection::Redis),
        "nats" => Ok(ExecutionQueueBackendSelection::Nats),
        "nats_jetstream" | "jetstream" => Ok(ExecutionQueueBackendSelection::NatsJetstream),
        "broker" => {
            anyhow::bail!(
                "MANDOFORGE_EXECUTION_QUEUE_BACKEND={requested} is reserved for a future broker-backed queue; use auto, memory, postgres, redis, nats, or nats_jetstream"
            );
        }
        other => {
            anyhow::bail!(
                "unsupported MANDOFORGE_EXECUTION_QUEUE_BACKEND={other}; use auto, memory, postgres, redis, nats, or nats_jetstream"
            );
        }
    }
}

pub(crate) fn runtime_tenant_id_from_env() -> Result<Uuid> {
    runtime_tenant_id_from_lookup(|key| std::env::var(key).ok())
}

pub(crate) fn runtime_tenant_id_from_lookup<F>(lookup: F) -> Result<Uuid>
where
    F: Fn(&str) -> Option<String>,
{
    let raw = lookup("MANDOFORGE_TENANT_ID")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_TENANT_ID.to_string());
    Uuid::parse_str(raw.trim()).with_context(|| "MANDOFORGE_TENANT_ID must be a valid UUID")
}

pub(crate) fn tenant_runtime_mode_from_env() -> Result<TenantRuntimeMode> {
    tenant_runtime_mode_from_lookup(|key| std::env::var(key).ok())
}

pub(crate) fn tenant_runtime_mode_from_lookup<F>(lookup: F) -> Result<TenantRuntimeMode>
where
    F: Fn(&str) -> Option<String>,
{
    let raw = lookup("MANDOFORGE_TENANT_ROUTING_MODE")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "single_runtime_tenant".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "single_runtime_tenant" | "single" | "default" => {
            Ok(TenantRuntimeMode::SingleRuntimeTenant)
        }
        "tenant_routed" | "tenant-routed" | "routed" => Ok(TenantRuntimeMode::TenantRouted),
        other => anyhow::bail!(
            "unsupported MANDOFORGE_TENANT_ROUTING_MODE={other}; use single_runtime_tenant or tenant_routed"
        ),
    }
}

pub(crate) fn execution_queue_from_env(
    store: &StoreBackend,
    tenant_id: Uuid,
) -> Result<ExecutionQueue> {
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
        (ExecutionQueueBackendSelection::Redis, _) => {
            let config = BrokerQueueConfig::from_env(BrokerQueueKind::Redis)
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(ExecutionQueue::broker(Arc::new(
                BrokerExecutionQueue::redis(config),
            )))
        }
        (ExecutionQueueBackendSelection::Nats, _) => {
            let config = BrokerQueueConfig::from_env(BrokerQueueKind::Nats)
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(ExecutionQueue::broker(Arc::new(
                BrokerExecutionQueue::nats(config),
            )))
        }
        (ExecutionQueueBackendSelection::NatsJetstream, _) => {
            let config = BrokerQueueConfig::from_env(BrokerQueueKind::NatsJetstream)
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(ExecutionQueue::broker(Arc::new(
                BrokerExecutionQueue::nats_jetstream(config),
            )))
        }
    }
}

pub(crate) fn execution_worker_from_env() -> Arc<dyn ExecutionWorker> {
    match std::env::var("MANDOFORGE_EXECUTION_WORKER")
        .unwrap_or_else(|_| "inline".to_string())
        .as_str()
    {
        "queue" | "queued" | "external" => Arc::new(QueueBackedExecutionWorker),
        _ => Arc::new(InlineExecutionWorker),
    }
}

pub(crate) fn telemetry_exporter_from_env() -> Result<Arc<dyn TelemetryExporter>> {
    let config = ObservabilityConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?;
    if config.is_enabled() {
        Ok(Arc::new(
            HttpTelemetryExporter::new().map_err(|error| anyhow::anyhow!(error.message))?,
        ))
    } else {
        Ok(Arc::new(ReservedTelemetryExporter))
    }
}

pub(crate) fn mcp_gateway_client_from_env() -> Result<Arc<dyn McpGatewayClient>> {
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

pub(crate) fn codex_app_server_client_from_env() -> Result<Arc<dyn CodexAppServerClient>> {
    let endpoint = std::env::var("MANDOFORGE_CODEX_APP_SERVER_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if endpoint
        .as_deref()
        .is_some_and(|value| value.starts_with("ws://") || value.starts_with("wss://"))
    {
        Ok(Arc::new(WsCodexAppServerClient))
    } else if endpoint.is_some() {
        Ok(Arc::new(
            HttpCodexAppServerClient::new().map_err(|error| anyhow::anyhow!(error.message))?,
        ))
    } else {
        Ok(Arc::new(ReservedCodexAppServerClient))
    }
}

pub(crate) fn eval_judge_client_from_env() -> Result<Arc<dyn EvalJudgeClient>> {
    Ok(Arc::new(
        HttpEvalJudgeClient::new().map_err(|error| anyhow::anyhow!(error.message))?,
    ))
}

pub(crate) fn cost_alert_webhook_url_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_COST_ALERT_WEBHOOK_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn cost_alert_email_relay_url_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_COST_ALERT_EMAIL_RELAY_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn cost_alert_smtp_config_from_env() -> Option<CostAlertSmtpConfig> {
    let addr = std::env::var("MANDOFORGE_COST_ALERT_SMTP_ADDR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let from = std::env::var("MANDOFORGE_COST_ALERT_SMTP_FROM")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let helo_domain = std::env::var("MANDOFORGE_COST_ALERT_SMTP_HELO")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "mandoforge.local".to_string());
    Some(CostAlertSmtpConfig {
        addr,
        from,
        helo_domain,
    })
}

pub(crate) fn approval_webhook_url_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_APPROVAL_WEBHOOK_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn approval_slack_webhook_url_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_APPROVAL_SLACK_WEBHOOK_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn approval_email_relay_url_from_env() -> Option<String> {
    std::env::var("MANDOFORGE_APPROVAL_EMAIL_RELAY_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn mcp_gateway_config_from_env() -> Result<Option<McpGatewayConfig>> {
    match std::env::var("MANDOFORGE_MCP_GATEWAY_URL") {
        Ok(value) if !value.trim().is_empty() => Ok(Some(
            McpGatewayConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?,
        )),
        _ => Ok(None),
    }
}

pub(crate) fn codex_app_server_config_from_env() -> Result<Option<CodexAppServerConfig>> {
    match std::env::var("MANDOFORGE_CODEX_APP_SERVER_URL") {
        Ok(value) if !value.trim().is_empty() => Ok(Some(
            CodexAppServerConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?,
        )),
        _ => Ok(None),
    }
}

pub(crate) fn eval_judge_config_from_env() -> Result<Option<EvalJudgeConfig>> {
    match std::env::var("MANDOFORGE_EVAL_JUDGE_URL") {
        Ok(value) if !value.trim().is_empty() => Ok(Some(
            EvalJudgeConfig::from_env().map_err(|error| anyhow::anyhow!(error.message))?,
        )),
        _ => Ok(None),
    }
}
