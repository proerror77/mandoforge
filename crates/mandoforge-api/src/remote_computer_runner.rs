use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRunnerConfig {
    pub(crate) mode: String,
    pub(crate) namespace: String,
    pub(crate) pod_template_path: String,
    pub(crate) service_account: String,
}

impl RemoteComputerRunnerConfig {
    pub(crate) fn from_env() -> Self {
        Self {
            mode: std::env::var("MANDOFORGE_REMOTE_COMPUTER_RUNNER")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "reserved".to_string()),
            namespace: std::env::var("MANDOFORGE_REMOTE_COMPUTER_NAMESPACE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "agent-os".to_string()),
            pod_template_path: std::env::var("MANDOFORGE_REMOTE_COMPUTER_TEMPLATE_PATH")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "deploy/k8s/agent-remote-computer.yaml".to_string()),
            service_account: std::env::var("MANDOFORGE_REMOTE_COMPUTER_SERVICE_ACCOUNT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "mandoforge-remote-computer".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRunnerReadiness {
    pub(crate) mode: String,
    pub(crate) configured: bool,
    pub(crate) status: String,
    pub(crate) namespace: String,
    pub(crate) pod_template_path: String,
    pub(crate) service_account: String,
    pub(crate) supported_operations: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RemoteComputerRunnerDryRunRequest {
    pub(crate) operation: Option<String>,
    pub(crate) remote_computer_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) pod_name: Option<String>,
    pub(crate) metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRunnerDryRunResponse {
    pub(crate) status: String,
    pub(crate) operation: String,
    pub(crate) configured: bool,
    pub(crate) would_create_pod: bool,
    pub(crate) would_delete_pod: bool,
    pub(crate) execution_enabled: bool,
    pub(crate) message: String,
    pub(crate) request: Value,
}

#[async_trait]
pub(crate) trait RemoteComputerRunner: Send + Sync {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness;

    async fn dry_run(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse;
}

pub(crate) struct ReservedRemoteComputerRunner;

#[async_trait]
impl RemoteComputerRunner for ReservedRemoteComputerRunner {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness {
        RemoteComputerRunnerReadiness {
            mode: config.mode.clone(),
            configured: false,
            status: "reserved".to_string(),
            namespace: config.namespace.clone(),
            pod_template_path: config.pod_template_path.clone(),
            service_account: config.service_account.clone(),
            supported_operations: vec![
                "readiness".to_string(),
                "dry_run_create".to_string(),
                "dry_run_delete".to_string(),
            ],
            message:
                "Remote Computer Kubernetes runner is reserved; no Pods are created or deleted"
                    .to_string(),
        }
    }

    async fn dry_run(
        &self,
        _config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "create".to_string());
        RemoteComputerRunnerDryRunResponse {
            status: "reserved".to_string(),
            operation,
            configured: false,
            would_create_pod: false,
            would_delete_pod: false,
            execution_enabled: false,
            message:
                "Reserved runner dry-run only; Kubernetes Pod mutation and tool execution are disabled"
                    .to_string(),
            request: json!(request),
        }
    }
}
