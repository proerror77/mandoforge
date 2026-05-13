use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRunnerConfig {
    pub(crate) mode: String,
    pub(crate) namespace: String,
    pub(crate) pod_template_path: String,
    pub(crate) service_account: String,
    pub(crate) kubeconfig_path: Option<String>,
    pub(crate) in_cluster: bool,
    pub(crate) mutation_enabled: bool,
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
            kubeconfig_path: std::env::var("MANDOFORGE_REMOTE_COMPUTER_KUBECONFIG")
                .ok()
                .or_else(|| std::env::var("KUBECONFIG").ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            in_cluster: env_flag("MANDOFORGE_REMOTE_COMPUTER_IN_CLUSTER"),
            mutation_enabled: env_flag("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED"),
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
    pub(crate) client_configured: bool,
    pub(crate) mutation_enabled: bool,
    pub(crate) dry_run_only: bool,
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

pub(crate) struct KubernetesRemoteComputerRunner;

pub(crate) fn remote_computer_runner_for_config(
    config: &RemoteComputerRunnerConfig,
) -> Box<dyn RemoteComputerRunner> {
    match config.mode.as_str() {
        "kubernetes" | "k8s" => Box::new(KubernetesRemoteComputerRunner),
        _ => Box::new(ReservedRemoteComputerRunner),
    }
}

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
            client_configured: false,
            mutation_enabled: false,
            dry_run_only: true,
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

#[async_trait]
impl RemoteComputerRunner for KubernetesRemoteComputerRunner {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness {
        let client_configured = kubernetes_client_configured(config);
        let template_present = Path::new(&config.pod_template_path).exists();
        let configured = client_configured && template_present;
        let status = if configured {
            "dry_run_ready"
        } else if !template_present {
            "template_missing"
        } else {
            "client_missing"
        };
        RemoteComputerRunnerReadiness {
            mode: config.mode.clone(),
            configured,
            status: status.to_string(),
            namespace: config.namespace.clone(),
            pod_template_path: config.pod_template_path.clone(),
            service_account: config.service_account.clone(),
            client_configured,
            mutation_enabled: config.mutation_enabled,
            dry_run_only: true,
            supported_operations: vec![
                "readiness".to_string(),
                "dry_run_create".to_string(),
                "dry_run_delete".to_string(),
            ],
            message: if configured {
                "Kubernetes Remote Computer adapter is configured for dry-run planning; Pod mutation remains disabled in this skeleton"
            } else if !template_present {
                "Kubernetes Remote Computer adapter is selected, but the Pod template is missing"
            } else {
                "Kubernetes Remote Computer adapter is selected, but kubeconfig or in-cluster config is missing"
            }
            .to_string(),
        }
    }

    async fn dry_run(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "create".to_string());
        let readiness = self.readiness(config);
        let operation_is_create = operation == "create" || operation == "dry_run_create";
        let operation_is_delete = operation == "delete" || operation == "dry_run_delete";
        RemoteComputerRunnerDryRunResponse {
            status: if readiness.configured {
                "dry_run_ready".to_string()
            } else {
                "blocked".to_string()
            },
            operation,
            configured: readiness.configured,
            would_create_pod: readiness.configured && operation_is_create,
            would_delete_pod: readiness.configured && operation_is_delete,
            execution_enabled: false,
            message: if readiness.configured {
                "Kubernetes adapter dry-run calculated Pod intent only; no Kubernetes API mutation or tool execution was performed"
            } else {
                "Kubernetes adapter dry-run is blocked until template and client configuration are present"
            }
            .to_string(),
            request: json!(request),
        }
    }
}

fn kubernetes_client_configured(config: &RemoteComputerRunnerConfig) -> bool {
    config.in_cluster
        || config
            .kubeconfig_path
            .as_deref()
            .is_some_and(|path| Path::new(path).exists())
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pod_template_path() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../deploy/k8s/agent-remote-computer.yaml")
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn selects_kubernetes_runner_only_when_requested() {
        let reserved = RemoteComputerRunnerConfig {
            mode: "reserved".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            in_cluster: false,
            mutation_enabled: false,
        };
        assert_eq!(
            remote_computer_runner_for_config(&reserved)
                .readiness(&reserved)
                .status,
            "reserved"
        );

        let kubernetes = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            in_cluster: true,
            ..reserved
        };
        assert_eq!(
            remote_computer_runner_for_config(&kubernetes)
                .readiness(&kubernetes)
                .status,
            "dry_run_ready"
        );
    }

    #[tokio::test]
    async fn kubernetes_runner_dry_run_never_enables_execution() {
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            in_cluster: true,
            mutation_enabled: true,
        };
        let runner = KubernetesRemoteComputerRunner;
        let response = runner
            .dry_run(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("create".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(response.status, "dry_run_ready");
        assert!(response.would_create_pod);
        assert!(!response.would_delete_pod);
        assert!(!response.execution_enabled);
    }
}
