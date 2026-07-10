use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use rustls_pki_types::pem::PemObject;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;

use crate::{SANDBOX_RUNTIME_EXECUTABLE, SANDBOX_RUNTIME_SUBCOMMAND, SandboxRuntimeRequest};

const DEFAULT_KUBERNETES_EXEC_TIMEOUT_SECONDS: u64 = 910;
const MAX_KUBERNETES_EXEC_CAPTURE_BYTES: usize = 1024 * 1024;
const IN_CLUSTER_KUBERNETES_API_URL: &str = "https://kubernetes.default.svc";
const IN_CLUSTER_SERVICE_ACCOUNT_TOKEN: &str =
    "/var/run/secrets/kubernetes.io/serviceaccount/token";
const IN_CLUSTER_SERVICE_ACCOUNT_CA: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";
const DEFAULT_AGENT_SANDBOX_WARM_POOL: &str = "mandoforge-agent-runtime";
const DEFAULT_AGENT_SANDBOX_TTL_SECONDS: u64 = 1800;
const MAX_AGENT_SANDBOX_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;
const AGENT_SANDBOX_POD_NAME_ANNOTATION: &str = "agents.x-k8s.io/pod-name";
const AGENT_SANDBOX_NAME_ANNOTATION: &str = "agents.x-k8s.io/sandbox-name";
const AGENT_SANDBOX_NAME_LABEL: &str = "agents.x-k8s.io/sandbox-name";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRunnerConfig {
    pub(crate) mode: String,
    pub(crate) namespace: String,
    pub(crate) pod_template_path: String,
    pub(crate) service_account: String,
    pub(crate) kubeconfig_path: Option<String>,
    pub(crate) kube_api_url: Option<String>,
    pub(crate) bearer_token_path: Option<String>,
    pub(crate) in_cluster: bool,
    pub(crate) mutation_enabled: bool,
    pub(crate) live_mutation_enabled: bool,
    pub(crate) execution_enabled: bool,
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
            kube_api_url: std::env::var("MANDOFORGE_REMOTE_COMPUTER_KUBE_API_URL")
                .ok()
                .map(|value| value.trim().trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty()),
            bearer_token_path: std::env::var("MANDOFORGE_REMOTE_COMPUTER_BEARER_TOKEN_PATH")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            in_cluster: env_flag("MANDOFORGE_REMOTE_COMPUTER_IN_CLUSTER"),
            mutation_enabled: env_flag("MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED"),
            live_mutation_enabled: env_flag("MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED"),
            execution_enabled: env_flag("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED"),
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
    pub(crate) api_server_configured: bool,
    pub(crate) bearer_token_configured: bool,
    pub(crate) mutation_enabled: bool,
    pub(crate) live_mutation_enabled: bool,
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
    pub(crate) live_probe_attempted: bool,
    pub(crate) live_probe_status_code: Option<u16>,
    pub(crate) live_mutation_attempted: bool,
    pub(crate) live_mutation_status_code: Option<u16>,
    pub(crate) kubernetes_api_path: Option<String>,
    pub(crate) namespace: Option<String>,
    pub(crate) pod_name: Option<String>,
    pub(crate) pod_template_path: Option<String>,
    pub(crate) execution_enabled: bool,
    pub(crate) message: String,
    pub(crate) request: Value,
    pub(crate) exec_result: Option<Value>,
}

#[async_trait]
pub(crate) trait RemoteComputerRunner: Send + Sync {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness;

    async fn dry_run(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse;

    async fn mutate(
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
        "kubernetes" | "k8s" | "agent-sandbox" | "k8s-agent-sandbox" => {
            Box::new(KubernetesRemoteComputerRunner)
        }
        _ => Box::new(ReservedRemoteComputerRunner),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentSandboxBinding {
    pub(crate) sandbox_name: String,
    pub(crate) pod_name: String,
}

fn request_metadata(
    request: &RemoteComputerRunnerDryRunRequest,
) -> Option<&serde_json::Map<String, Value>> {
    request.metadata.as_ref().and_then(Value::as_object)
}

fn request_namespace(
    config: &RemoteComputerRunnerConfig,
    request: &RemoteComputerRunnerDryRunRequest,
) -> String {
    request_metadata(request)
        .and_then(|metadata| metadata.get("namespace"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| valid_kubernetes_name(value))
        .map(ToString::to_string)
        .unwrap_or_else(|| config.namespace.clone())
}

fn request_runtime_substrate(
    config: &RemoteComputerRunnerConfig,
    request: &RemoteComputerRunnerDryRunRequest,
) -> &'static str {
    let Some(metadata) = request_metadata(request) else {
        return if remote_computer_agent_sandbox_requested(config) {
            "agent-sandbox"
        } else {
            "kubernetes-pod"
        };
    };
    if let Some(substrate) = metadata
        .get("runtime_identity")
        .and_then(Value::as_object)
        .and_then(|identity| identity.get("substrate"))
        .and_then(Value::as_str)
    {
        match substrate.trim() {
            "agent-sandbox" => return "agent-sandbox",
            "kubernetes-pod" => return "kubernetes-pod",
            _ => {}
        }
    }
    if let Some(substrate) = metadata.get("runtime_substrate").and_then(Value::as_str) {
        match substrate.trim() {
            "agent-sandbox" => return "agent-sandbox",
            "kubernetes-pod" => return "kubernetes-pod",
            _ => {}
        }
    }
    if remote_computer_agent_sandbox_requested(config) {
        "agent-sandbox"
    } else {
        "kubernetes-pod"
    }
}

fn request_agent_sandbox_requested(
    config: &RemoteComputerRunnerConfig,
    request: &RemoteComputerRunnerDryRunRequest,
) -> bool {
    request_runtime_substrate(config, request) == "agent-sandbox"
}

fn agent_sandbox_claim_lifecycle_deadline(
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<Option<String>, String> {
    let Some(value) = request_metadata(request)
        .and_then(|metadata| metadata.get("lifecycle_deadline"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| "metadata.lifecycle_deadline must be RFC3339".to_string())?;
    Ok(Some(parsed.with_timezone(&Utc).to_rfc3339()))
}

fn agent_sandbox_claim_ttl_seconds(request: &RemoteComputerRunnerDryRunRequest) -> u64 {
    let configured = request_metadata(request)
        .and_then(|metadata| metadata.get("sandbox_ttl_seconds"))
        .and_then(Value::as_u64)
        .or_else(|| {
            std::env::var("MANDOFORGE_AGENT_SANDBOX_TTL_SECONDS")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
        .unwrap_or(DEFAULT_AGENT_SANDBOX_TTL_SECONDS)
        .min(MAX_AGENT_SANDBOX_TTL_SECONDS);
    let readiness_floor = request_metadata(request)
        .and_then(|metadata| metadata.get("readiness_timeout_seconds"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let lease_floor = request_metadata(request)
        .and_then(|metadata| metadata.get("initial_lease_seconds"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let minimum = readiness_floor
        .max(lease_floor)
        .saturating_add(60)
        .min(MAX_AGENT_SANDBOX_TTL_SECONDS);
    configured.clamp(minimum, MAX_AGENT_SANDBOX_TTL_SECONDS)
}

fn mutation_is_idempotent_success(
    operation_is_create: bool,
    operation_is_delete: bool,
    status_code: u16,
) -> bool {
    (operation_is_create && status_code == 409)
        || (operation_is_delete && matches!(status_code, 404 | 410))
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
            api_server_configured: false,
            bearer_token_configured: false,
            mutation_enabled: false,
            live_mutation_enabled: false,
            dry_run_only: true,
            supported_operations: vec![
                "readiness".to_string(),
                "dry_run_create".to_string(),
                "dry_run_delete".to_string(),
                "dry_run_exec".to_string(),
                "dry_run_probe".to_string(),
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
            live_probe_attempted: false,
            live_probe_status_code: None,
            live_mutation_attempted: false,
            live_mutation_status_code: None,
            kubernetes_api_path: None,
            namespace: None,
            pod_name: None,
            pod_template_path: None,
            execution_enabled: false,
            message:
                "Reserved runner dry-run only; Kubernetes Pod mutation and tool execution are disabled"
                    .to_string(),
            request: remote_computer_runner_request_projection(&request),
            exec_result: None,
        }
    }

    async fn mutate(
        &self,
        _config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "create".to_string());
        RemoteComputerRunnerDryRunResponse {
            status: "blocked".to_string(),
            operation,
            configured: false,
            would_create_pod: false,
            would_delete_pod: false,
            live_probe_attempted: false,
            live_probe_status_code: None,
            live_mutation_attempted: false,
            live_mutation_status_code: None,
            kubernetes_api_path: None,
            namespace: None,
            pod_name: None,
            pod_template_path: None,
            execution_enabled: false,
            message:
                "Reserved runner blocks live Kubernetes mutation; no Pods or tool execution were started"
                    .to_string(),
            request: remote_computer_runner_request_projection(&request),
            exec_result: None,
        }
    }
}

#[async_trait]
impl RemoteComputerRunner for KubernetesRemoteComputerRunner {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness {
        let agent_sandbox = remote_computer_agent_sandbox_requested(config);
        let client_access = kubernetes_client_access(config);
        let client_configured = client_access.is_some();
        let api_server_configured = config.kube_api_url.is_some() || config.in_cluster;
        let bearer_token_configured = kubernetes_bearer_token_configured(config);
        let template_present = Path::new(&config.pod_template_path).exists();
        let configured = client_configured && (agent_sandbox || template_present);
        let status = if configured {
            "dry_run_ready"
        } else if !agent_sandbox && !template_present {
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
            api_server_configured,
            bearer_token_configured,
            mutation_enabled: config.mutation_enabled,
            live_mutation_enabled: config.live_mutation_enabled,
            dry_run_only: !(config.mutation_enabled && config.live_mutation_enabled),
            supported_operations: vec![
                "readiness".to_string(),
                "dry_run_create".to_string(),
                "dry_run_delete".to_string(),
                "dry_run_exec".to_string(),
                "dry_run_probe".to_string(),
                "live_create".to_string(),
                "live_delete".to_string(),
                "live_exec".to_string(),
                "live_create_sandbox_claim".to_string(),
                "live_delete_sandbox_claim".to_string(),
            ],
            message: if agent_sandbox && configured && config.mutation_enabled && config.live_mutation_enabled {
                "Kubernetes Agent Sandbox adapter is configured for explicit live SandboxClaim create/delete and optional Pod exec"
            } else if agent_sandbox && configured {
                "Kubernetes Agent Sandbox adapter is configured for dry-run planning; SandboxClaim mutation remains disabled until both mutation gates are enabled"
            } else if configured && config.mutation_enabled && config.live_mutation_enabled {
                "Kubernetes Remote Computer adapter is configured for explicit live Pod create/delete and optional Pod exec"
            } else if configured {
                "Kubernetes Remote Computer adapter is configured for dry-run planning; Pod mutation remains disabled until both mutation gates are enabled"
            } else if !agent_sandbox && !template_present {
                "Kubernetes Remote Computer adapter is selected, but the Pod template is missing"
            } else if api_server_configured && !bearer_token_configured {
                "Kubernetes Remote Computer adapter has an API server URL, but no readable bearer token is configured"
            } else if config.kubeconfig_path.is_some() {
                "Kubernetes Remote Computer adapter has a kubeconfig path, but live kubeconfig transport is not implemented; configure API server URL and bearer token"
            } else {
                "Kubernetes Remote Computer adapter is selected, but API server URL and bearer token configuration are missing"
            }
            .to_string(),
        }
    }

    async fn dry_run(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let agent_sandbox = request_agent_sandbox_requested(config, &request);
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "create".to_string());
        let readiness = self.readiness(config);
        let namespace = request_namespace(config, &request);
        let operation_is_create = operation == "create" || operation == "dry_run_create";
        let operation_is_delete = operation == "delete" || operation == "dry_run_delete";
        let operation_is_probe = operation == "probe" || operation == "dry_run_probe";
        let operation_is_exec = operation == "exec" || operation == "dry_run_exec";
        let pod_name = request
            .pod_name
            .clone()
            .filter(|pod_name| !pod_name.trim().is_empty())
            .unwrap_or_else(|| "agent-remote-computer-dry-run".to_string());
        let kubernetes_api_path = if operation_is_create {
            if agent_sandbox {
                Some(format!(
                    "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims",
                    namespace
                ))
            } else {
                Some(format!("/api/v1/namespaces/{}/pods", namespace))
            }
        } else if operation_is_delete {
            if agent_sandbox {
                Some(format!(
                    "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims/{}",
                    namespace, pod_name
                ))
            } else {
                Some(format!(
                    "/api/v1/namespaces/{}/pods/{}",
                    namespace, pod_name
                ))
            }
        } else if operation_is_probe {
            Some("/version".to_string())
        } else if operation_is_exec {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}/exec",
                namespace, pod_name
            ))
        } else {
            None
        };
        let probe_result = if operation_is_probe && readiness.configured {
            Some(probe_kubernetes_version(config).await)
        } else {
            None
        };
        let live_probe_status_code = probe_result
            .as_ref()
            .and_then(|result| result.as_ref().ok().map(|(status_code, _)| *status_code));
        let probe_failed_message = probe_result
            .as_ref()
            .and_then(|result| result.as_ref().err().cloned());
        let status = if let Some(result) = &probe_result {
            if result.is_ok() {
                "probe_ok"
            } else {
                "probe_failed"
            }
        } else if readiness.configured {
            "dry_run_ready"
        } else {
            "blocked"
        };
        RemoteComputerRunnerDryRunResponse {
            status: status.to_string(),
            operation,
            configured: readiness.configured,
            would_create_pod: readiness.configured && operation_is_create,
            would_delete_pod: readiness.configured && operation_is_delete,
            live_probe_attempted: probe_result.is_some(),
            live_probe_status_code,
            live_mutation_attempted: false,
            live_mutation_status_code: None,
            kubernetes_api_path,
            namespace: Some(namespace),
            pod_name: Some(pod_name),
            pod_template_path: Some(config.pod_template_path.clone()),
            execution_enabled: false,
            message: if let Some(message) = probe_failed_message {
                format!("Kubernetes API probe failed: {message}")
            } else if probe_result.is_some() {
                "Kubernetes API probe succeeded; no Kubernetes mutation or tool execution was performed"
                    .to_string()
            } else if operation_is_exec && readiness.configured {
                "Kubernetes adapter dry-run calculated Pod exec intent only; no command was executed"
                    .to_string()
            } else if readiness.configured {
                "Kubernetes adapter dry-run calculated Pod intent only; no Kubernetes API mutation or tool execution was performed"
                    .to_string()
            } else {
                "Kubernetes adapter dry-run is blocked until template and client configuration are present"
                    .to_string()
            },
            request: remote_computer_runner_request_projection(&request),
            exec_result: None,
        }
    }

    async fn mutate(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let agent_sandbox = request_agent_sandbox_requested(config, &request);
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "live_create".to_string());
        let readiness = self.readiness(config);
        let namespace = request_namespace(config, &request);
        let operation_is_create = operation == "create" || operation == "live_create";
        let operation_is_delete = operation == "delete" || operation == "live_delete";
        let operation_is_exec = operation == "exec" || operation == "live_exec";
        let pod_name = request
            .pod_name
            .clone()
            .filter(|pod_name| valid_kubernetes_name(pod_name))
            .unwrap_or_else(|| live_pod_name(&request));
        let kubernetes_api_path = if operation_is_create {
            if agent_sandbox {
                Some(format!(
                    "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims",
                    namespace
                ))
            } else {
                Some(format!("/api/v1/namespaces/{}/pods", namespace))
            }
        } else if operation_is_delete {
            if agent_sandbox {
                Some(format!(
                    "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims/{}",
                    namespace, pod_name
                ))
            } else {
                Some(format!(
                    "/api/v1/namespaces/{}/pods/{}",
                    namespace, pod_name
                ))
            }
        } else if operation_is_exec {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}/exec",
                namespace, pod_name
            ))
        } else {
            None
        };
        let client_access = kubernetes_client_access(config);
        let gates_open = readiness.configured
            && config.mutation_enabled
            && config.live_mutation_enabled
            && client_access.is_some();
        let mutation_result = if gates_open && (operation_is_create || operation_is_delete) {
            if agent_sandbox {
                Some(
                    call_agent_sandbox_claim_mutation(
                        config,
                        &namespace,
                        operation_is_create,
                        &pod_name,
                        &request,
                    )
                    .await,
                )
            } else {
                Some(
                    call_kubernetes_mutation(
                        config,
                        &namespace,
                        operation_is_create,
                        &pod_name,
                        &request,
                    )
                    .await,
                )
            }
        } else {
            None
        };
        let exec_stdin = parse_kubernetes_exec_stdin(request.metadata.as_ref());
        let exec_gates_open = readiness.configured
            && config.execution_enabled
            && config.live_mutation_enabled
            && client_access.is_some();
        let exec_result = if exec_gates_open && operation_is_exec {
            Some(match &exec_stdin {
                Ok(stdin) => call_kubernetes_exec(config, &namespace, &pod_name, stdin).await,
                Err(error) => Err(error.clone()),
            })
        } else {
            None
        };
        let exec_failed_message = exec_result.as_ref().and_then(|result| match result {
            Ok(result) => result.status_failure.clone(),
            Err(error) => Some(error.clone()),
        });
        let exec_result_payload = exec_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(KubernetesExecResult::to_json);
        let normalized_mutation_result = mutation_result.as_ref().map(|result| match result {
            Ok(result) => Ok(result.clone()),
            Err(error)
                if error.status_code.is_some_and(|status_code| {
                    mutation_is_idempotent_success(
                        operation_is_create,
                        operation_is_delete,
                        status_code,
                    )
                }) =>
            {
                Ok((
                    error.status_code.expect("checked idempotent status code"),
                    json!({"status": "converged"}),
                ))
            }
            Err(error) => Err(error.clone()),
        });
        let live_mutation_status_code =
            normalized_mutation_result
                .as_ref()
                .and_then(|result| match result {
                    Ok((status_code, _)) => Some(*status_code),
                    Err(error) => error.status_code,
                });
        let mutation_failed_message = normalized_mutation_result
            .as_ref()
            .and_then(|result| result.as_ref().err().map(ToString::to_string));
        let status = if let Some(result) = &exec_result {
            if result
                .as_ref()
                .is_ok_and(|result| result.status_failure.is_none())
            {
                "exec_ok"
            } else {
                "exec_failed"
            }
        } else if let Some(result) = &normalized_mutation_result {
            if result.is_ok() {
                "mutation_ok"
            } else {
                "mutation_failed"
            }
        } else {
            "blocked"
        };
        RemoteComputerRunnerDryRunResponse {
            status: status.to_string(),
            operation,
            configured: readiness.configured,
            would_create_pod: readiness.configured && operation_is_create,
            would_delete_pod: readiness.configured && operation_is_delete,
            live_probe_attempted: false,
            live_probe_status_code: None,
            live_mutation_attempted: mutation_result.is_some(),
            live_mutation_status_code,
            kubernetes_api_path,
            namespace: Some(namespace),
            pod_name: Some(pod_name),
            pod_template_path: Some(config.pod_template_path.clone()),
            execution_enabled: exec_result.as_ref().is_some_and(|result| {
                result
                    .as_ref()
                    .is_ok_and(|result| result.status_failure.is_none())
            }),
            message: if let Some(message) = exec_failed_message {
                format!("Kubernetes Pod exec failed: {message}")
            } else if exec_result.is_some() {
                "Kubernetes Pod exec completed through the WebSocket transport; no execution job was created"
                    .to_string()
            } else if let Some(message) = mutation_failed_message {
                format!("Kubernetes mutation failed: {message}")
            } else if operation_is_create && live_mutation_status_code == Some(409) {
                "Kubernetes mutation converged on an existing resource; no tool execution or execution job was started"
                    .to_string()
            } else if operation_is_delete
                && live_mutation_status_code
                    .is_some_and(|status_code| matches!(status_code, 404 | 410))
            {
                "Kubernetes mutation converged on an absent resource; no tool execution or execution job was started"
                    .to_string()
            } else if mutation_result.is_some() && agent_sandbox {
                "Kubernetes Agent Sandbox claim mutation completed; no tool execution or execution job was started"
                    .to_string()
            } else if mutation_result.is_some() {
                "Kubernetes Pod mutation completed; no tool execution or execution job was started"
                    .to_string()
            } else if !readiness.configured {
                "Kubernetes mutation is blocked until template and client configuration are present"
                    .to_string()
            } else if !config.mutation_enabled || !config.live_mutation_enabled {
                "Kubernetes mutation is blocked until both mutation gates are explicitly enabled"
                    .to_string()
            } else if client_access.is_none() {
                "Kubernetes mutation requires a supported API server URL and readable bearer token; kubeconfig mutation is not implemented"
                    .to_string()
            } else if operation_is_exec && exec_stdin.is_err() {
                format!(
                    "Kubernetes Pod exec is blocked: {}",
                    exec_stdin.as_ref().expect_err("checked exec stdin error")
                )
            } else if operation_is_exec && !config.execution_enabled {
                "Kubernetes Pod exec is blocked until MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED is explicitly enabled".to_string()
            } else if operation_is_exec && !config.live_mutation_enabled {
                "Kubernetes Pod exec is blocked until the live mutation gate is explicitly enabled"
                    .to_string()
            } else if !(operation_is_create || operation_is_delete || operation_is_exec) {
                "Kubernetes runner only supports live_create, live_delete, and live_exec"
                    .to_string()
            } else {
                "Kubernetes mutation was blocked by the runner policy".to_string()
            },
            request: remote_computer_runner_request_projection(&request),
            exec_result: exec_result_payload,
        }
    }
}

fn remote_computer_agent_sandbox_requested(config: &RemoteComputerRunnerConfig) -> bool {
    matches!(config.mode.as_str(), "agent-sandbox" | "k8s-agent-sandbox")
}

async fn probe_kubernetes_version(
    config: &RemoteComputerRunnerConfig,
) -> Result<(u16, Value), String> {
    let access = kubernetes_client_access(config)
        .ok_or_else(|| "supported Kubernetes API client is not configured".to_string())?;
    let token = tokio::fs::read_to_string(&access.bearer_token_path)
        .await
        .map_err(|err| format!("failed to read bearer token: {err}"))?;
    let response = kubernetes_http_client(&access, Duration::from_secs(10))
        .await?
        .get(format!("{}/version", access.api_url))
        .bearer_auth(token.trim())
        .send()
        .await
        .map_err(|err| format!("failed to call Kubernetes /version: {err}"))?;
    let status_code = response.status().as_u16();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("failed to parse Kubernetes /version response: {err}"))?;
    if !(200..300).contains(&status_code) {
        return Err(format!("Kubernetes /version returned HTTP {status_code}"));
    }
    Ok((status_code, body))
}

async fn call_kubernetes_mutation(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    create: bool,
    pod_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<(u16, Value), KubernetesMutationError> {
    let access = kubernetes_client_access(config).ok_or_else(|| {
        KubernetesMutationError::without_status("supported Kubernetes API client is not configured")
    })?;
    let token = tokio::fs::read_to_string(&access.bearer_token_path)
        .await
        .map_err(|err| {
            KubernetesMutationError::without_status(format!("failed to read bearer token: {err}"))
        })?;
    let client = kubernetes_http_client(&access, Duration::from_secs(10))
        .await
        .map_err(KubernetesMutationError::without_status)?;
    let url = if create {
        format!("{}/api/v1/namespaces/{}/pods", access.api_url, namespace)
    } else {
        format!(
            "{}/api/v1/namespaces/{}/pods/{}",
            access.api_url, namespace, pod_name
        )
    };
    let request = if create {
        client.post(url).bearer_auth(token.trim()).json(
            &build_kubernetes_pod_request(config, pod_name, request)
                .map_err(KubernetesMutationError::without_status)?,
        )
    } else {
        client.delete(url).bearer_auth(token.trim())
    };
    let response = request.send().await.map_err(|err| {
        KubernetesMutationError::without_status(format!("failed to call Kubernetes Pod API: {err}"))
    })?;
    let status_code = response.status().as_u16();
    let body = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({"status_code": status_code}));
    if !(200..300).contains(&status_code) {
        return Err(KubernetesMutationError::with_status(
            status_code,
            format!("Kubernetes Pod API returned HTTP {status_code}"),
        ));
    }
    Ok((status_code, body))
}

async fn call_agent_sandbox_claim_mutation(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    create: bool,
    claim_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<(u16, Value), KubernetesMutationError> {
    let access = kubernetes_client_access(config).ok_or_else(|| {
        KubernetesMutationError::without_status("supported Kubernetes API client is not configured")
    })?;
    let token = tokio::fs::read_to_string(&access.bearer_token_path)
        .await
        .map_err(|err| {
            KubernetesMutationError::without_status(format!("failed to read bearer token: {err}"))
        })?;
    let client = kubernetes_http_client(&access, Duration::from_secs(10))
        .await
        .map_err(KubernetesMutationError::without_status)?;
    let url = if create {
        format!(
            "{}/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims",
            access.api_url,
            percent_encode(namespace)
        )
    } else {
        format!(
            "{}/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims/{}",
            access.api_url,
            percent_encode(namespace),
            percent_encode(claim_name)
        )
    };
    let request = if create {
        client.post(url).bearer_auth(token.trim()).json(
            &build_agent_sandbox_claim_request(namespace, claim_name, request)
                .map_err(KubernetesMutationError::without_status)?,
        )
    } else {
        client.delete(url).bearer_auth(token.trim())
    };
    let response = request.send().await.map_err(|err| {
        KubernetesMutationError::without_status(format!(
            "failed to call Kubernetes Agent Sandbox API: {err}"
        ))
    })?;
    let status_code = response.status().as_u16();
    let body = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({"status_code": status_code}));
    if !(200..300).contains(&status_code) {
        return Err(KubernetesMutationError::with_status(
            status_code,
            format!("Kubernetes Agent Sandbox API returned HTTP {status_code}"),
        ));
    }
    Ok((status_code, body))
}

pub(crate) async fn poll_agent_sandbox_binding(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    claim_name: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<AgentSandboxBinding, String> {
    let access = kubernetes_client_access(config)
        .ok_or_else(|| "supported Kubernetes API client is not configured".to_string())?;
    let poll_interval = interval.max(Duration::from_millis(100));
    let deadline = Instant::now() + timeout;
    let client = kubernetes_http_client(&access, Duration::from_secs(10)).await?;
    loop {
        if Instant::now() >= deadline {
            return Err(format!(
                "SandboxClaim did not expose a bound Pod within {:.0}s",
                timeout.as_secs_f64()
            ));
        }
        let token = tokio::fs::read_to_string(&access.bearer_token_path)
            .await
            .map_err(|err| format!("failed to read bearer token: {err}"))?;
        let claim_url = format!(
            "{}/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/{}/sandboxclaims/{}",
            access.api_url,
            percent_encode(namespace),
            percent_encode(claim_name)
        );
        let response = client
            .get(&claim_url)
            .bearer_auth(token.trim())
            .send()
            .await
            .map_err(|err| format!("failed to GET SandboxClaim status: {err}"))?;
        let status_code = response.status().as_u16();
        if status_code == 404 {
            tokio::time::sleep(poll_interval).await;
            continue;
        } else if !(200..300).contains(&status_code) {
            return Err(format!(
                "SandboxClaim status GET returned HTTP {status_code}"
            ));
        }
        let claim: Value = response
            .json()
            .await
            .map_err(|err| format!("failed to parse SandboxClaim status: {err}"))?;
        if let Some(error) = sandbox_terminal_condition_error("SandboxClaim", &claim) {
            return Err(error);
        }
        let Some(sandbox_name) = resolved_sandbox_name_from_claim(&claim) else {
            tokio::time::sleep(poll_interval).await;
            continue;
        };
        let sandbox_url = format!(
            "{}/apis/agents.x-k8s.io/v1beta1/namespaces/{}/sandboxes/{}",
            access.api_url,
            percent_encode(namespace),
            percent_encode(&sandbox_name)
        );
        let sandbox_response = client
            .get(&sandbox_url)
            .bearer_auth(token.trim())
            .send()
            .await
            .map_err(|err| format!("failed to GET Agent Sandbox status: {err}"))?;
        let sandbox_status_code = sandbox_response.status().as_u16();
        if sandbox_status_code == 404 {
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        if !(200..300).contains(&sandbox_status_code) {
            return Err(format!(
                "Agent Sandbox status GET returned HTTP {sandbox_status_code}"
            ));
        }
        let sandbox: Value = sandbox_response
            .json()
            .await
            .map_err(|err| format!("failed to parse Agent Sandbox status: {err}"))?;
        if let Some(error) = sandbox_terminal_condition_error("Sandbox", &sandbox) {
            return Err(error);
        }
        if let Some(pod_name) = sandbox
            .pointer(&format!(
                "/metadata/annotations/{}",
                AGENT_SANDBOX_POD_NAME_ANNOTATION.replace('/', "~1")
            ))
            .and_then(Value::as_str)
            .filter(|value| valid_kubernetes_name(value))
        {
            return Ok(AgentSandboxBinding {
                sandbox_name,
                pod_name: pod_name.to_string(),
            });
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn resolved_sandbox_name_from_claim(claim: &Value) -> Option<String> {
    claim
        .pointer("/status/sandbox/name")
        .and_then(Value::as_str)
        .or_else(|| {
            claim
                .pointer("/status/sandbox/Name")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            claim
                .pointer(&format!(
                    "/metadata/annotations/{}",
                    AGENT_SANDBOX_NAME_ANNOTATION.replace('/', "~1")
                ))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            claim
                .pointer(&format!(
                    "/metadata/labels/{}",
                    AGENT_SANDBOX_NAME_LABEL.replace('/', "~1")
                ))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| valid_kubernetes_name(value))
        .map(ToString::to_string)
}

fn sandbox_terminal_condition_error(kind: &str, resource: &Value) -> Option<String> {
    let conditions = resource.pointer("/status/conditions")?.as_array()?;
    for condition in conditions {
        let condition_type = condition
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = condition
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let reason = condition
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown reason");
        let message = condition
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let lower_reason = reason.to_ascii_lowercase();
        let terminal_ready_failure = condition_type == "Ready"
            && status == "False"
            && ["fail", "error", "invalid", "reject", "deny", "expire"]
                .iter()
                .any(|needle| lower_reason.contains(needle));
        let terminal_finished = condition_type == "Finished" && status == "True";
        if terminal_ready_failure || terminal_finished {
            let detail = if message.is_empty() {
                reason.to_string()
            } else {
                format!("{reason}: {message}")
            };
            return Some(format!(
                "{kind} reported terminal {condition_type} condition: {detail}"
            ));
        }
    }
    None
}

#[derive(Debug, Clone)]
struct KubernetesMutationError {
    status_code: Option<u16>,
    message: String,
}

impl KubernetesMutationError {
    fn with_status(status_code: u16, message: impl Into<String>) -> Self {
        Self {
            status_code: Some(status_code),
            message: message.into(),
        }
    }

    fn without_status(message: impl Into<String>) -> Self {
        Self {
            status_code: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for KubernetesMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Debug, Clone)]
struct KubernetesExecResult {
    handshake_status_code: u16,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
    status: Option<Value>,
    status_failure: Option<String>,
}

#[derive(Debug, Clone)]
struct KubernetesExecStdin {
    bytes: Vec<u8>,
    timeout_seconds: u64,
}

impl KubernetesExecResult {
    fn to_json(&self) -> Value {
        json!({
            "handshake_status_code": self.handshake_status_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "stdout_truncated": self.stdout_truncated,
            "stderr_truncated": self.stderr_truncated,
            "status": self.status,
            "status_failure": self.status_failure
        })
    }
}

async fn call_kubernetes_exec(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    pod_name: &str,
    stdin: &KubernetesExecStdin,
) -> Result<KubernetesExecResult, String> {
    call_kubernetes_exec_with_timeout(
        config,
        namespace,
        pod_name,
        &stdin.bytes,
        Duration::from_secs(
            kubernetes_exec_timeout_seconds().min(stdin.timeout_seconds.saturating_add(10)),
        ),
    )
    .await
}

async fn call_kubernetes_exec_with_timeout(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    pod_name: &str,
    stdin: &[u8],
    timeout: Duration,
) -> Result<KubernetesExecResult, String> {
    let access = kubernetes_client_access(config)
        .ok_or_else(|| "supported Kubernetes API client is not configured".to_string())?;
    let token = tokio::fs::read_to_string(&access.bearer_token_path)
        .await
        .map_err(|err| format!("failed to read bearer token: {err}"))?;
    let websocket_url = kubernetes_exec_websocket_url(&access.api_url, namespace, pod_name);
    let mut request = websocket_url
        .into_client_request()
        .map_err(|err| format!("failed to build Kubernetes exec WebSocket request: {err}"))?;
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", token.trim())
            .parse()
            .map_err(|err| format!("failed to build authorization header: {err}"))?,
    );
    request.headers_mut().insert(
        "sec-websocket-protocol",
        "v4.channel.k8s.io"
            .parse()
            .map_err(|err| format!("failed to build Kubernetes exec protocol header: {err}"))?,
    );
    let connector = kubernetes_exec_tls_connector(&access).await?;
    let (mut socket, response) = connect_async_tls_with_config(request, None, false, connector)
        .await
        .map_err(|err| format!("failed to open Kubernetes exec WebSocket: {err}"))?;
    let handshake_status_code = response.status().as_u16();
    let mut stdin_frame = Vec::with_capacity(stdin.len() + 1);
    stdin_frame.push(0);
    stdin_frame.extend_from_slice(stdin);
    socket
        .send(Message::Binary(stdin_frame))
        .await
        .map_err(|error| format!("failed to send Kubernetes exec stdin: {error}"))?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;
    let mut status = None;
    let deadline = Instant::now() + timeout;
    loop {
        let now = Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("Kubernetes exec WebSocket timed out".to_string());
        }
        let Some(message) = tokio::time::timeout(deadline - now, socket.next())
            .await
            .map_err(|_| "Kubernetes exec WebSocket timed out".to_string())?
        else {
            break;
        };
        let message = message.map_err(|err| format!("Kubernetes exec WebSocket error: {err}"))?;
        match message {
            Message::Binary(frame) if !frame.is_empty() => {
                let channel = frame[0];
                let payload = &frame[1..];
                match channel {
                    1 => append_bounded_exec_output(&mut stdout, payload, &mut stdout_truncated),
                    2 => append_bounded_exec_output(&mut stderr, payload, &mut stderr_truncated),
                    3 => {
                        status = serde_json::from_slice(payload).ok();
                        break;
                    }
                    _ => {}
                }
            }
            Message::Text(text) => {
                append_bounded_exec_output(&mut stdout, text.as_bytes(), &mut stdout_truncated);
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = socket.close(None).await;
    if status.is_none() {
        return Err("Kubernetes exec WebSocket closed without status".to_string());
    }
    let status_failure =
        kubernetes_exec_status_failure(status.as_ref().expect("status checked above"));
    Ok(KubernetesExecResult {
        handshake_status_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        stdout_truncated,
        stderr_truncated,
        status,
        status_failure,
    })
}

fn append_bounded_exec_output(target: &mut Vec<u8>, payload: &[u8], truncated: &mut bool) {
    let remaining = MAX_KUBERNETES_EXEC_CAPTURE_BYTES.saturating_sub(target.len());
    if remaining == 0 {
        *truncated = true;
        return;
    }
    let take = remaining.min(payload.len());
    target.extend_from_slice(&payload[..take]);
    if take < payload.len() {
        *truncated = true;
    }
}

/// Polls the Kubernetes Pod status endpoint until the Pod reaches `Running` phase,
/// a terminal phase (`Failed`, `Succeeded`, `Unknown`), or the timeout elapses.
#[cfg(test)]
async fn poll_kubernetes_pod_running(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<(), String> {
    poll_kubernetes_pod_running_in_namespace(config, &config.namespace, pod_name, timeout, interval)
        .await
}

pub(crate) async fn poll_kubernetes_pod_running_in_namespace(
    config: &RemoteComputerRunnerConfig,
    namespace: &str,
    pod_name: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<(), String> {
    let access = kubernetes_client_access(config)
        .ok_or_else(|| "supported Kubernetes API client is not configured".to_string())?;
    let poll_interval = interval.max(Duration::from_millis(100));
    let deadline = Instant::now() + timeout;
    let client = kubernetes_http_client(&access, Duration::from_secs(10)).await?;
    loop {
        if Instant::now() >= deadline {
            return Err(format!(
                "Pod did not reach Running within {:.0}s",
                timeout.as_secs_f64()
            ));
        }
        let token = tokio::fs::read_to_string(&access.bearer_token_path)
            .await
            .map_err(|err| format!("failed to read bearer token: {err}"))?;
        let url = format!(
            "{}/api/v1/namespaces/{}/pods/{}",
            access.api_url,
            percent_encode(namespace),
            percent_encode(pod_name)
        );
        let response = client
            .get(&url)
            .bearer_auth(token.trim())
            .send()
            .await
            .map_err(|err| format!("failed to GET Pod status: {err}"))?;
        let status_code = response.status().as_u16();
        if status_code == 404 {
            // Pod not yet visible — may happen in the first few hundred ms
        } else if !(200..300).contains(&status_code) {
            return Err(format!("Pod status GET returned HTTP {status_code}"));
        } else {
            let body: Value = response
                .json()
                .await
                .map_err(|err| format!("failed to parse Pod status: {err}"))?;
            let phase = body
                .pointer("/status/phase")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            match phase {
                "Running" => return Ok(()),
                "Failed" | "Succeeded" | "Unknown" => {
                    return Err(format!("Pod entered terminal phase: {phase}"));
                }
                _ => {} // Pending or ContainerCreating — keep polling
            }
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn kubernetes_exec_timeout_seconds() -> u64 {
    std::env::var("MANDOFORGE_REMOTE_COMPUTER_EXEC_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_KUBERNETES_EXEC_TIMEOUT_SECONDS)
}

fn kubernetes_exec_websocket_url(api_url: &str, namespace: &str, pod_name: &str) -> String {
    let base = if let Some(rest) = api_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = api_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        api_url.to_string()
    };
    format!(
        "{base}/api/v1/namespaces/{}/pods/{}/exec?container=remote-computer&stdout=true&stderr=true&stdin=true&tty=false&command={}&command={}",
        percent_encode(namespace),
        percent_encode(pod_name),
        percent_encode(SANDBOX_RUNTIME_EXECUTABLE),
        percent_encode(SANDBOX_RUNTIME_SUBCOMMAND),
    )
}

fn parse_kubernetes_exec_stdin(metadata: Option<&Value>) -> Result<KubernetesExecStdin, String> {
    let request = metadata
        .and_then(|metadata| metadata.get("sandbox_runtime_request"))
        .cloned()
        .ok_or_else(|| "metadata.sandbox_runtime_request is required".to_string())?;
    let request: SandboxRuntimeRequest = serde_json::from_value(request)
        .map_err(|error| format!("metadata.sandbox_runtime_request is invalid: {error}"))?;
    Ok(KubernetesExecStdin {
        bytes: request.to_stdin_bytes()?,
        timeout_seconds: request.timeout_seconds,
    })
}

fn remote_computer_runner_request_projection(request: &RemoteComputerRunnerDryRunRequest) -> Value {
    let operation = request.operation.as_deref().unwrap_or_default();
    if !matches!(operation, "exec" | "live_exec") {
        return json!(request);
    }
    let metadata = request.metadata.as_ref().and_then(Value::as_object);
    let runtime_request = metadata
        .and_then(|metadata| metadata.get("sandbox_runtime_request"))
        .cloned()
        .and_then(|value| serde_json::from_value::<SandboxRuntimeRequest>(value).ok());
    json!({
        "operation": request.operation,
        "remote_computer_id": request.remote_computer_id,
        "session_id": request.session_id,
        "pod_name": request.pod_name,
        "metadata": {
            "namespace": metadata.and_then(|metadata| metadata.get("namespace")),
            "runtime_substrate": metadata.and_then(|metadata| metadata.get("runtime_substrate")),
            "tool_call_id": metadata.and_then(|metadata| metadata.get("tool_call_id")),
            "sandbox_runtime": runtime_request.as_ref().map(|request| json!({
                "version": request.version,
                "operation": request.operation_name(),
                "workspace_path": request.workspace_path,
                "timeout_seconds": request.timeout_seconds,
                "environment_key_count": request.environment.len(),
                "stdin_bytes": request.to_stdin_bytes().map(|bytes| bytes.len()).unwrap_or(0),
                "redacted": true,
            })),
        }
    })
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            encoded.push(ch);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[derive(Debug, Clone)]
struct KubernetesClientAccess {
    api_url: String,
    bearer_token_path: PathBuf,
    ca_cert_path: Option<PathBuf>,
}

fn kubernetes_client_access(config: &RemoteComputerRunnerConfig) -> Option<KubernetesClientAccess> {
    if let (Some(api_url), Some(token_path)) = (
        config
            .kube_api_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        config
            .bearer_token_path
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.exists()),
    ) {
        return Some(KubernetesClientAccess {
            api_url: api_url.trim_end_matches('/').to_string(),
            bearer_token_path: token_path,
            ca_cert_path: in_cluster_ca_cert_path(config.in_cluster),
        });
    }
    if config.in_cluster {
        let token_path = config
            .bearer_token_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(IN_CLUSTER_SERVICE_ACCOUNT_TOKEN));
        if token_path.exists() {
            return Some(KubernetesClientAccess {
                api_url: config
                    .kube_api_url
                    .as_deref()
                    .unwrap_or(IN_CLUSTER_KUBERNETES_API_URL)
                    .trim_end_matches('/')
                    .to_string(),
                bearer_token_path: token_path,
                ca_cert_path: in_cluster_ca_cert_path(true),
            });
        }
    }
    None
}

fn in_cluster_ca_cert_path(in_cluster: bool) -> Option<PathBuf> {
    in_cluster.then(|| PathBuf::from(IN_CLUSTER_SERVICE_ACCOUNT_CA))
}

async fn kubernetes_http_client(
    access: &KubernetesClientAccess,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(timeout);
    if let Some(ca_cert_path) = &access.ca_cert_path {
        let ca = tokio::fs::read(ca_cert_path)
            .await
            .map_err(|err| format!("failed to read Kubernetes service-account CA: {err}"))?;
        let certificate = reqwest::Certificate::from_pem(&ca)
            .map_err(|err| format!("failed to parse Kubernetes service-account CA: {err}"))?;
        builder = builder.add_root_certificate(certificate);
    }
    builder
        .build()
        .map_err(|err| format!("failed to build Kubernetes HTTP client: {err}"))
}

async fn kubernetes_exec_tls_connector(
    access: &KubernetesClientAccess,
) -> Result<Option<Connector>, String> {
    let Some(ca_cert_path) = &access.ca_cert_path else {
        return Ok(None);
    };
    let ca = tokio::fs::read(ca_cert_path)
        .await
        .map_err(|err| format!("failed to read Kubernetes service-account CA: {err}"))?;
    let certs = rustls_pki_types::CertificateDer::pem_slice_iter(&ca)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to parse Kubernetes service-account CA: {err}"))?;
    let mut root_store = rustls::RootCertStore::empty();
    let (added, _ignored) = root_store.add_parsable_certificates(certs);
    if added == 0 {
        return Err(
            "Kubernetes service-account CA did not contain a usable certificate".to_string(),
        );
    }
    Ok(Some(Connector::Rustls(Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    ))))
}

fn kubernetes_exec_status_failure(status: &Value) -> Option<String> {
    if status.get("status").and_then(Value::as_str) == Some("Success") {
        return None;
    }
    let status_label = status
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("Failure");
    let reason = status.get("reason").and_then(Value::as_str);
    let message = status.get("message").and_then(Value::as_str);
    let exit_code = status.get("exitCode").and_then(Value::as_i64).or_else(|| {
        status
            .pointer("/details/causes")
            .and_then(Value::as_array)
            .and_then(|causes| {
                causes
                    .iter()
                    .find(|cause| cause.get("reason").and_then(Value::as_str) == Some("ExitCode"))
                    .and_then(|cause| cause.get("message"))
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<i64>().ok())
            })
    });
    let mut parts = vec![format!("Kubernetes exec status reported {status_label}")];
    if let Some(exit_code) = exit_code {
        parts.push(format!("exit_code={exit_code}"));
    }
    if let Some(reason) = reason {
        parts.push(format!("reason={reason}"));
    }
    if let Some(message) = message {
        parts.push(format!("message={message}"));
    }
    Some(parts.join(", "))
}

fn build_kubernetes_pod_request(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<Value, String> {
    let session_id = request
        .session_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let remote_computer_id = request
        .remote_computer_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let assignment_id = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("assignment_id"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let artifact_discovery_enabled = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("artifact_discovery_enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        .to_string();
    let session_workspace_path = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("session_workspace_path"))
        .and_then(|value| value.as_str())
        .unwrap_or("/workspace")
        .to_string();
    let artifact_dir = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("artifact_dir"))
        .and_then(|value| value.as_str())
        .unwrap_or("/workspace/artifacts")
        .to_string();
    let mut pod = load_kubernetes_pod_template(config)?;
    patch_pod_metadata(&mut pod, config, pod_name, request)?;
    patch_pod_spec(&mut pod, config)?;
    patch_container_env(
        &mut pod,
        "remote-computer",
        &[
            ("MANDOFORGE_REMOTE_COMPUTER_MODE", "session-pod"),
            ("MANDOFORGE_SESSION_ID", &session_id),
            ("MANDOFORGE_REMOTE_COMPUTER_ID", &remote_computer_id),
            ("MANDOFORGE_SESSION_WORKSPACE", &session_workspace_path),
            ("MANDOFORGE_WORKSPACE_ROOT", "/workspace"),
        ],
    )?;
    patch_container_env(
        &mut pod,
        "artifact-discovery",
        &[
            (
                "MANDOFORGE_ARTIFACT_DISCOVERY_ENABLED",
                &artifact_discovery_enabled,
            ),
            ("MANDOFORGE_SESSION_ID", &session_id),
            ("MANDOFORGE_REMOTE_COMPUTER_ID", &remote_computer_id),
            ("MANDOFORGE_ASSIGNMENT_ID", &assignment_id),
            ("MANDOFORGE_ARTIFACT_DIR", &artifact_dir),
        ],
    )?;
    Ok(pod)
}

fn build_agent_sandbox_claim_request(
    namespace: &str,
    claim_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<Value, String> {
    let warm_pool = agent_sandbox_warm_pool_name(request);
    let ttl_seconds = agent_sandbox_claim_ttl_seconds(request);
    let shutdown_time = agent_sandbox_claim_lifecycle_deadline(request)?.unwrap_or_else(|| {
        (Utc::now() + chrono::Duration::seconds(ttl_seconds as i64)).to_rfc3339()
    });
    let mut labels = serde_json::Map::new();
    labels.insert("app.kubernetes.io/name".to_string(), json!("mandoforge"));
    labels.insert(
        "mandoforge.io/runtime-substrate".to_string(),
        json!("agent-sandbox"),
    );
    insert_optional_pod_tracking_label(&mut labels, "mandoforge.io/session-id", request.session_id);
    insert_optional_pod_tracking_label(
        &mut labels,
        "mandoforge.io/remote-computer-id",
        request.remote_computer_id,
    );
    insert_optional_pod_tracking_metadata_label(
        &mut labels,
        request,
        "tenant_id",
        "mandoforge.io/tenant-id",
    );
    insert_optional_pod_tracking_metadata_label(
        &mut labels,
        request,
        "cache_scope",
        "mandoforge.io/cache-scope",
    );
    insert_optional_pod_tracking_metadata_label(
        &mut labels,
        request,
        "workspace_seed",
        "mandoforge.io/workspace-seed",
    );
    let mut annotations = serde_json::Map::new();
    annotations.insert(
        "mandoforge.io/lifecycle".to_string(),
        json!("session-bound-agent-sandbox"),
    );
    insert_optional_pod_tracking_annotation(
        &mut annotations,
        "mandoforge.io/session-id",
        request.session_id,
    );
    insert_optional_pod_tracking_annotation(
        &mut annotations,
        "mandoforge.io/remote-computer-id",
        request.remote_computer_id,
    );
    insert_optional_pod_tracking_metadata_annotation(
        &mut annotations,
        request,
        "tenant_id",
        "mandoforge.io/tenant-id",
    );
    insert_optional_pod_tracking_metadata_annotation(
        &mut annotations,
        request,
        "cache_scope",
        "mandoforge.io/cache-scope",
    );
    insert_optional_pod_tracking_metadata_annotation(
        &mut annotations,
        request,
        "workspace_seed",
        "mandoforge.io/workspace-seed",
    );
    Ok(json!({
        "apiVersion": "extensions.agents.x-k8s.io/v1beta1",
        "kind": "SandboxClaim",
        "metadata": {
            "name": claim_name,
            "namespace": namespace,
            "labels": labels,
            "annotations": annotations
        },
        "spec": {
            "warmPoolRef": {
                "name": warm_pool
            },
            "lifecycle": {
                "shutdownTime": shutdown_time,
                "ttlSecondsAfterFinished": ttl_seconds,
                "shutdownPolicy": "Delete"
            },
            "additionalPodMetadata": {
                "labels": labels,
                "annotations": annotations
            }
        }
    }))
}

fn agent_sandbox_warm_pool_name(request: &RemoteComputerRunnerDryRunRequest) -> String {
    request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("sandbox_warm_pool"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| valid_kubernetes_name(value))
        .map(ToString::to_string)
        .or_else(|| {
            std::env::var("MANDOFORGE_AGENT_SANDBOX_WARM_POOL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| valid_kubernetes_name(value))
        })
        .unwrap_or_else(|| DEFAULT_AGENT_SANDBOX_WARM_POOL.to_string())
}

fn load_kubernetes_pod_template(config: &RemoteComputerRunnerConfig) -> Result<Value, String> {
    let content = std::fs::read_to_string(&config.pod_template_path)
        .map_err(|err| format!("failed to read Pod template: {err}"))?;
    let document: Value = serde_yaml::from_str(&content)
        .map_err(|err| format!("failed to parse Pod template YAML: {err}"))?;
    if document.get("kind").and_then(Value::as_str) == Some("Pod") {
        return Ok(document);
    }
    let template = document
        .pointer("/spec/template")
        .cloned()
        .ok_or_else(|| "Pod template YAML must be a Pod or contain spec.template".to_string())?;
    let metadata = template
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let spec = template
        .get("spec")
        .cloned()
        .ok_or_else(|| "Pod template spec.template.spec is required".to_string())?;
    Ok(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": metadata,
        "spec": spec
    }))
}

fn patch_pod_metadata(
    pod: &mut Value,
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<(), String> {
    let object = pod
        .as_object_mut()
        .ok_or_else(|| "Pod template root must be an object".to_string())?;
    object.insert("apiVersion".to_string(), json!("v1"));
    object.insert("kind".to_string(), json!("Pod"));
    let metadata = object
        .entry("metadata")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Pod template metadata must be an object".to_string())?;
    metadata.insert("name".to_string(), json!(pod_name));
    metadata.insert(
        "namespace".to_string(),
        json!(request_namespace(config, request)),
    );
    let labels = metadata
        .entry("labels")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Pod template metadata.labels must be an object".to_string())?;
    labels.insert("app".to_string(), json!("mandoforge-agent-remote-computer"));
    labels.insert("mandoforge.io/runner".to_string(), json!("remote-computer"));
    insert_optional_pod_tracking_label(labels, "mandoforge.io/session-id", request.session_id);
    insert_optional_pod_tracking_label(
        labels,
        "mandoforge.io/remote-computer-id",
        request.remote_computer_id,
    );
    insert_optional_pod_tracking_metadata_label(
        labels,
        request,
        "tenant_id",
        "mandoforge.io/tenant-id",
    );
    insert_optional_pod_tracking_metadata_label(
        labels,
        request,
        "lease_id",
        "mandoforge.io/lease-id",
    );
    let annotations = metadata
        .entry("annotations")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Pod template metadata.annotations must be an object".to_string())?;
    annotations.insert(
        "mandoforge.io/template-path".to_string(),
        json!(config.pod_template_path),
    );
    annotations.insert(
        "mandoforge.io/lifecycle".to_string(),
        json!("session-bound"),
    );
    insert_optional_pod_tracking_annotation(
        annotations,
        "mandoforge.io/session-id",
        request.session_id,
    );
    insert_optional_pod_tracking_annotation(
        annotations,
        "mandoforge.io/remote-computer-id",
        request.remote_computer_id,
    );
    insert_optional_pod_tracking_metadata_annotation(
        annotations,
        request,
        "tenant_id",
        "mandoforge.io/tenant-id",
    );
    insert_optional_pod_tracking_metadata_annotation(
        annotations,
        request,
        "lease_id",
        "mandoforge.io/lease-id",
    );
    Ok(())
}

fn insert_optional_pod_tracking_label(
    labels: &mut serde_json::Map<String, Value>,
    key: &str,
    id: Option<Uuid>,
) {
    if let Some(id) = id {
        labels.insert(key.to_string(), json!(id.to_string()));
    }
}

fn insert_optional_pod_tracking_metadata_label(
    labels: &mut serde_json::Map<String, Value>,
    request: &RemoteComputerRunnerDryRunRequest,
    metadata_key: &str,
    label_key: &str,
) {
    let Some(value) = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get(metadata_key))
        .and_then(Value::as_str)
        .filter(|value| valid_kubernetes_label_value(value))
    else {
        return;
    };
    labels.insert(label_key.to_string(), json!(value));
}

fn insert_optional_pod_tracking_annotation(
    annotations: &mut serde_json::Map<String, Value>,
    key: &str,
    id: Option<Uuid>,
) {
    if let Some(id) = id {
        annotations.insert(key.to_string(), json!(id.to_string()));
    }
}

fn insert_optional_pod_tracking_metadata_annotation(
    annotations: &mut serde_json::Map<String, Value>,
    request: &RemoteComputerRunnerDryRunRequest,
    metadata_key: &str,
    annotation_key: &str,
) {
    let Some(value) = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get(metadata_key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    annotations.insert(annotation_key.to_string(), json!(value));
}

fn patch_pod_spec(pod: &mut Value, config: &RemoteComputerRunnerConfig) -> Result<(), String> {
    let spec = pod
        .get_mut("spec")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Pod template spec must be an object".to_string())?;
    spec.insert(
        "serviceAccountName".to_string(),
        json!(config.service_account),
    );
    spec.insert("automountServiceAccountToken".to_string(), json!(false));
    spec.insert("restartPolicy".to_string(), json!("Never"));
    Ok(())
}

fn patch_container_env(
    pod: &mut Value,
    container_name: &str,
    entries: &[(&str, &str)],
) -> Result<(), String> {
    let containers = pod
        .pointer_mut("/spec/containers")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Pod template spec.containers must be an array".to_string())?;
    let Some(container) = containers
        .iter_mut()
        .find(|container| container.get("name").and_then(Value::as_str) == Some(container_name))
    else {
        return Ok(());
    };
    let container = container
        .as_object_mut()
        .ok_or_else(|| "Pod template container must be an object".to_string())?;
    let env = container
        .entry("env")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| "Pod template container env must be an array".to_string())?;
    for (name, value) in entries {
        upsert_container_env(env, name, value);
    }
    Ok(())
}

fn upsert_container_env(env: &mut Vec<Value>, name: &str, value: &str) {
    if let Some(existing) = env
        .iter_mut()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
        .and_then(Value::as_object_mut)
    {
        existing.insert("value".to_string(), json!(value));
        existing.remove("valueFrom");
        return;
    }
    env.push(json!({"name": name, "value": value}));
}

fn live_pod_name(request: &RemoteComputerRunnerDryRunRequest) -> String {
    request
        .remote_computer_id
        .or(request.session_id)
        .map(|id| format!("agent-remote-computer-{}", id.simple()))
        .unwrap_or_else(|| "agent-remote-computer-live".to_string())
        .chars()
        .take(63)
        .collect()
}

fn valid_kubernetes_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 63
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && value
            .chars()
            .last()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn valid_kubernetes_label_value(value: &str) -> bool {
    let value = value.trim();
    value.len() <= 63
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
        && value
            .chars()
            .next()
            .is_none_or(|ch| ch.is_ascii_alphanumeric())
        && value
            .chars()
            .last()
            .is_none_or(|ch| ch.is_ascii_alphanumeric())
}

fn kubernetes_bearer_token_configured(config: &RemoteComputerRunnerConfig) -> bool {
    if config
        .bearer_token_path
        .as_deref()
        .is_some_and(|path| Path::new(path).exists())
    {
        return true;
    }
    config.in_cluster && Path::new(IN_CLUSTER_SERVICE_ACCOUNT_TOKEN).exists()
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Path as AxumPath,
        http::HeaderMap,
        routing::{delete, get, post},
    };
    use futures_util::{SinkExt, StreamExt};

    fn test_pod_template_path() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../deploy/k8s/agent-remote-computer.yaml")
            .to_string_lossy()
            .to_string()
    }

    fn write_test_token() -> PathBuf {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        std::fs::write(&token_path, "test-token").expect("write token");
        token_path
    }

    fn shell_runtime_metadata(command: &str) -> Value {
        json!({
            "sandbox_runtime_request": SandboxRuntimeRequest::new(
                Uuid::new_v4(),
                30,
                std::collections::BTreeMap::new(),
                crate::SandboxRuntimeOperation::Shell {
                    command: command.to_string(),
                },
            )
        })
    }

    #[test]
    fn agent_sandbox_binding_contract_supports_current_and_legacy_claim_fields() {
        assert_eq!(
            resolved_sandbox_name_from_claim(&json!({
                "status": {"sandbox": {"name": "current-sandbox"}},
                "metadata": {"annotations": {"agents.x-k8s.io/sandbox-name": "annotation-sandbox"}}
            }))
            .as_deref(),
            Some("current-sandbox")
        );
        assert_eq!(
            resolved_sandbox_name_from_claim(&json!({
                "status": {"sandbox": {"Name": "legacy-status-sandbox"}}
            }))
            .as_deref(),
            Some("legacy-status-sandbox")
        );
        assert_eq!(
            resolved_sandbox_name_from_claim(&json!({
                "metadata": {"annotations": {"agents.x-k8s.io/sandbox-name": "annotation-sandbox"}}
            }))
            .as_deref(),
            Some("annotation-sandbox")
        );
        assert_eq!(
            resolved_sandbox_name_from_claim(&json!({
                "metadata": {"labels": {"agents.x-k8s.io/sandbox-name": "legacy-label-sandbox"}}
            }))
            .as_deref(),
            Some("legacy-label-sandbox")
        );
    }

    #[test]
    fn agent_sandbox_terminal_conditions_fail_fast() {
        let failed = json!({
            "status": {"conditions": [{
                "type": "Ready",
                "status": "False",
                "reason": "TemplateInvalid",
                "message": "missing image"
            }]}
        });
        let finished = json!({
            "status": {"conditions": [{
                "type": "Finished",
                "status": "True",
                "reason": "PodFailed"
            }]}
        });

        assert!(
            sandbox_terminal_condition_error("SandboxClaim", &failed)
                .expect("terminal failure")
                .contains("TemplateInvalid")
        );
        assert!(sandbox_terminal_condition_error("Sandbox", &finished).is_some());
    }

    #[test]
    fn agent_sandbox_claim_contains_lifecycle_and_propagated_metadata() {
        let request = RemoteComputerRunnerDryRunRequest {
            operation: Some("live_create".to_string()),
            remote_computer_id: Some(Uuid::from_u128(2)),
            session_id: Some(Uuid::from_u128(1)),
            pod_name: Some("claim-1".to_string()),
            metadata: Some(json!({
                "tenant_id": Uuid::from_u128(3).to_string(),
                "sandbox_warm_pool": "pool-1",
                "cache_scope": "mandoforge",
                "workspace_seed": "mandoforge",
                "sandbox_ttl_seconds": 1200,
                "readiness_timeout_seconds": 60,
                "initial_lease_seconds": 900,
                "lifecycle_deadline": "2026-07-11T12:00:00Z"
            })),
        };

        let body = build_agent_sandbox_claim_request("tenant-ns", "claim-1", &request)
            .expect("claim request");

        assert_eq!(body["metadata"]["namespace"], "tenant-ns");
        assert_eq!(body["spec"]["warmPoolRef"]["name"], "pool-1");
        assert_eq!(
            DateTime::parse_from_rfc3339(
                body["spec"]["lifecycle"]["shutdownTime"]
                    .as_str()
                    .expect("shutdown time")
            )
            .expect("RFC3339 shutdown time")
            .timestamp(),
            DateTime::parse_from_rfc3339("2026-07-11T12:00:00Z")
                .expect("expected shutdown time")
                .timestamp()
        );
        assert_eq!(body["spec"]["lifecycle"]["ttlSecondsAfterFinished"], 1200);
        assert_eq!(body["spec"]["lifecycle"]["shutdownPolicy"], "Delete");
        assert_eq!(
            body["spec"]["additionalPodMetadata"]["labels"]["mandoforge.io/cache-scope"],
            "mandoforge"
        );
        assert_eq!(
            body["spec"]["additionalPodMetadata"]["annotations"]["mandoforge.io/workspace-seed"],
            "mandoforge"
        );
    }

    #[test]
    fn agent_sandbox_claim_rejects_invalid_deadline_and_bounds_ttl() {
        let invalid_deadline = RemoteComputerRunnerDryRunRequest {
            operation: Some("live_create".to_string()),
            remote_computer_id: None,
            session_id: None,
            pod_name: Some("claim-1".to_string()),
            metadata: Some(json!({"lifecycle_deadline": "tomorrow"})),
        };
        assert_eq!(
            build_agent_sandbox_claim_request("agent-os", "claim-1", &invalid_deadline)
                .expect_err("invalid deadline"),
            "metadata.lifecycle_deadline must be RFC3339"
        );

        let oversized_ttl = RemoteComputerRunnerDryRunRequest {
            metadata: Some(json!({"sandbox_ttl_seconds": u64::MAX})),
            ..invalid_deadline
        };
        assert_eq!(
            agent_sandbox_claim_ttl_seconds(&oversized_ttl),
            MAX_AGENT_SANDBOX_TTL_SECONDS
        );
    }

    #[test]
    fn kubernetes_mutation_idempotency_is_operation_specific() {
        assert!(mutation_is_idempotent_success(true, false, 409));
        assert!(mutation_is_idempotent_success(false, true, 404));
        assert!(mutation_is_idempotent_success(false, true, 410));
        assert!(!mutation_is_idempotent_success(true, false, 404));
        assert!(!mutation_is_idempotent_success(false, true, 409));
        assert!(!mutation_is_idempotent_success(false, true, 500));
    }

    #[test]
    fn selects_kubernetes_runner_only_when_requested() {
        let reserved = RemoteComputerRunnerConfig {
            mode: "reserved".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: None,
            bearer_token_path: None,
            in_cluster: false,
            mutation_enabled: false,
            live_mutation_enabled: false,
            execution_enabled: false,
        };
        assert_eq!(
            remote_computer_runner_for_config(&reserved)
                .readiness(&reserved)
                .status,
            "reserved"
        );

        let token_path = write_test_token();
        let kubernetes = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            kube_api_url: Some("https://kubernetes.default.svc".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            ..reserved
        };
        assert_eq!(
            remote_computer_runner_for_config(&kubernetes)
                .readiness(&kubernetes)
                .status,
            "dry_run_ready"
        );
        let _ = std::fs::remove_file(token_path);
    }

    #[tokio::test]
    async fn kubernetes_runner_dry_run_never_enables_execution() {
        let token_path = write_test_token();
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
            kube_api_url: Some("https://kubernetes.default.svc".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
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
        assert_eq!(
            response.kubernetes_api_path.as_deref(),
            Some("/api/v1/namespaces/agent-os/pods")
        );
        assert_eq!(
            response.pod_name.as_deref(),
            Some("agent-remote-computer-test")
        );
        assert!(!response.execution_enabled);
        let _ = std::fs::remove_file(token_path);
    }

    #[tokio::test]
    async fn kubernetes_runner_dry_run_exec_plans_pod_exec_without_execution() {
        let token_path = write_test_token();
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
            kube_api_url: Some("https://kubernetes.default.svc".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
        };
        let runner = KubernetesRemoteComputerRunner;
        let response = runner
            .dry_run(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(shell_runtime_metadata("pwd")),
                },
            )
            .await;
        assert_eq!(response.status, "dry_run_ready");
        assert!(!response.would_create_pod);
        assert!(!response.would_delete_pod);
        assert_eq!(
            response.kubernetes_api_path.as_deref(),
            Some("/api/v1/namespaces/agent-os/pods/agent-remote-computer-test/exec")
        );
        assert_eq!(
            response.pod_name.as_deref(),
            Some("agent-remote-computer-test")
        );
        assert!(response.message.contains("no command was executed"));
        assert!(!response.execution_enabled);
        assert_eq!(
            response.request["metadata"]["sandbox_runtime"]["operation"],
            "shell"
        );
        assert_eq!(
            response.request["metadata"]["sandbox_runtime"]["redacted"],
            true
        );
        let _ = std::fs::remove_file(token_path);
    }

    #[test]
    fn kubernetes_runner_requires_identity_for_api_server_config() {
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some("https://kubernetes.default.svc".to_string()),
            bearer_token_path: None,
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
        };
        let readiness = KubernetesRemoteComputerRunner.readiness(&config);
        assert_eq!(readiness.status, "client_missing");
        assert!(readiness.api_server_configured);
        assert!(!readiness.bearer_token_configured);
        assert!(!readiness.configured);
        assert!(readiness.dry_run_only);
    }

    #[test]
    fn kubernetes_client_access_uses_service_account_ca_for_in_cluster_tls() {
        let token_path = write_test_token();
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some("https://kubernetes.default.svc".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: true,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let access = kubernetes_client_access(&config).expect("client access");

        assert_eq!(
            access.ca_cert_path.as_deref(),
            Some(Path::new(IN_CLUSTER_SERVICE_ACCOUNT_CA))
        );
        let _ = std::fs::remove_file(token_path);
    }

    #[tokio::test]
    async fn kubernetes_runner_probe_calls_version_without_mutation() {
        async fn version(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer test-token")
            );
            Json(json!({"major": "1", "minor": "30"}))
        }

        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/version", get(version)))
                .await
                .expect("mock kube server");
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
        };
        let response = KubernetesRemoteComputerRunner
            .dry_run(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("probe".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: None,
                    metadata: None,
                },
            )
            .await;
        assert_eq!(response.status, "probe_ok");
        assert!(response.configured);
        assert!(response.live_probe_attempted);
        assert_eq!(response.live_probe_status_code, Some(200));
        assert_eq!(response.kubernetes_api_path.as_deref(), Some("/version"));
        assert!(!response.would_create_pod);
        assert!(!response.would_delete_pod);
        assert!(!response.execution_enabled);

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_mutation_blocks_without_live_gate() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some("http://127.0.0.1:1".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(response.status, "blocked");
        assert!(response.would_create_pod);
        assert!(!response.live_mutation_attempted);
        assert!(!response.execution_enabled);
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_mutations_converge_conflict_and_absent_resources() {
        async fn create_pod() -> (axum::http::StatusCode, Json<Value>) {
            (
                axum::http::StatusCode::CONFLICT,
                Json(json!({"kind": "Status", "reason": "AlreadyExists"})),
            )
        }

        async fn delete_pod() -> (axum::http::StatusCode, Json<Value>) {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({"kind": "Status", "reason": "NotFound"})),
            )
        }

        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/api/v1/namespaces/agent-os/pods", post(create_pod))
                    .route(
                        "/api/v1/namespaces/agent-os/pods/{pod_name}",
                        delete(delete_pod),
                    ),
            )
            .await
            .expect("mock kube server");
        });
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: false,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: None,
                    session_id: Some(Uuid::new_v4()),
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(json!({
                        "session_workspace_path": "/workspace/session",
                        "artifact_dir": "/workspace/session/artifacts"
                    })),
                },
            )
            .await;

        assert_eq!(response.status, "mutation_ok");
        assert!(response.would_create_pod);
        assert!(response.live_mutation_attempted);
        assert_eq!(
            response.live_mutation_status_code,
            Some(axum::http::StatusCode::CONFLICT.as_u16())
        );
        assert!(response.message.contains("existing resource"));

        let delete_response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_delete".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(delete_response.status, "mutation_ok");
        assert_eq!(
            delete_response.live_mutation_status_code,
            Some(axum::http::StatusCode::NOT_FOUND.as_u16())
        );
        assert!(delete_response.message.contains("absent resource"));

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_captures_websocket_channels() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept exec websocket");
            let mut websocket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    assert!(request.uri().path().ends_with("/pods/agent-remote-computer-test/exec"));
                    let query = request.uri().query().expect("exec query");
                    assert!(query.contains("command=%2Fusr%2Flocal%2Fbin%2Fmandoforge-sandbox-runtime"));
                    assert!(query.contains("command=execute-json"));
                    assert!(query.contains("stdin=true"));
                    assert!(!query.contains("SENSITIVE_SENTINEL"));
                    response.headers_mut().insert(
                        "sec-websocket-protocol",
                        "v4.channel.k8s.io".parse().expect("protocol header"),
                    );
                    Ok(response)
                },
            )
                .await
                .expect("accept websocket");
            let stdin = websocket
                .next()
                .await
                .expect("stdin frame")
                .expect("valid stdin frame");
            let Message::Binary(stdin) = stdin else {
                panic!("expected binary stdin frame");
            };
            assert_eq!(stdin.first(), Some(&0));
            assert!(String::from_utf8_lossy(&stdin[1..]).contains("SENSITIVE_SENTINEL"));
            let mut stdout_frame = vec![1];
            stdout_frame.extend_from_slice(b"SENSITIVE_SENTINEL\n");
            websocket
                .send(Message::Binary(stdout_frame))
                .await
                .expect("send stdout");
            let mut stderr_frame = vec![2];
            stderr_frame.extend_from_slice(b"pod warning\n");
            websocket
                .send(Message::Binary(stderr_frame))
                .await
                .expect("send stderr");
            let mut status_frame = vec![3];
            status_frame.extend_from_slice(br#"{"status":"Success","exitCode":0}"#);
            websocket
                .send(Message::Binary(status_frame))
                .await
                .expect("send status");
            let _ = websocket.close(None).await;
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(shell_runtime_metadata("echo SENSITIVE_SENTINEL")),
                },
            )
            .await;
        assert_eq!(response.status, "exec_ok", "{}", response.message);
        assert!(response.execution_enabled);
        assert_eq!(
            response.kubernetes_api_path.as_deref(),
            Some("/api/v1/namespaces/agent-os/pods/agent-remote-computer-test/exec")
        );
        let exec_result = response.exec_result.as_ref().expect("exec result");
        assert_eq!(exec_result["stdout"], "SENSITIVE_SENTINEL\n");
        assert_eq!(exec_result["stderr"], "pod warning\n");
        assert_eq!(exec_result["stdout_truncated"], false);
        assert_eq!(exec_result["stderr_truncated"], false);
        assert_eq!(exec_result["status"]["status"], "Success");
        assert!(!response.request.to_string().contains("SENSITIVE_SENTINEL"));
        assert!(
            !crate::remote_computer_runner_response_for_audit(&response)
                .to_string()
                .contains("SENSITIVE_SENTINEL")
        );

        server.await.expect("server task");
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_carries_agent_cli_arguments_only_on_stdin() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept exec websocket");
            let mut websocket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    let query = request.uri().query().expect("exec query");
                    assert!(query.contains("command=%2Fusr%2Flocal%2Fbin%2Fmandoforge-sandbox-runtime"));
                    assert!(query.contains("command=execute-json"));
                    assert!(!query.contains("python3"));
                    assert!(!query.contains("hello%20world"));
                    response.headers_mut().insert(
                        "sec-websocket-protocol",
                        "v4.channel.k8s.io".parse().expect("protocol header"),
                    );
                    Ok(response)
                },
            )
            .await
            .expect("accept websocket");
            let stdin = websocket
                .next()
                .await
                .expect("stdin frame")
                .expect("valid stdin frame");
            let Message::Binary(stdin) = stdin else {
                panic!("expected binary stdin frame");
            };
            let payload: Value =
                serde_json::from_slice(&stdin[1..]).expect("sandbox runtime payload");
            assert_eq!(payload["operation"]["type"], "agent_cli");
            assert_eq!(payload["operation"]["executable"], "python3");
            assert_eq!(
                payload["operation"]["args"],
                json!(["-c", "print('hello world')"])
            );
            let mut status_frame = vec![3];
            status_frame.extend_from_slice(br#"{"status":"Success","exitCode":0}"#);
            websocket
                .send(Message::Binary(status_frame))
                .await
                .expect("send status");
            let _ = websocket.close(None).await;
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(json!({
                        "sandbox_runtime_request": SandboxRuntimeRequest::new(
                            Uuid::new_v4(),
                            30,
                            std::collections::BTreeMap::new(),
                            crate::SandboxRuntimeOperation::AgentCli {
                                executable: "python3".to_string(),
                                args: vec!["-c".to_string(), "print('hello world')".to_string()],
                                task: "input.py".to_string(),
                                profile: "python-test".to_string(),
                            },
                        )
                    })),
                },
            )
            .await;

        assert_eq!(response.status, "exec_ok", "{}", response.message);
        assert!(response.execution_enabled);

        server.await.expect("server task");
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_rejects_invalid_runtime_envelope() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some("http://127.0.0.1:1".to_string()),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(shell_runtime_metadata("")),
                },
            )
            .await;

        assert_eq!(response.status, "exec_failed");
        assert!(!response.execution_enabled);
        assert!(
            response.message.contains("must not be empty"),
            "{}",
            response.message
        );
        assert!(response.exec_result.is_none());
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_fails_on_failure_status_frame() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept exec websocket");
            let mut websocket = tokio_tungstenite::accept_hdr_async(
                stream,
                |_request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    response.headers_mut().insert(
                        "sec-websocket-protocol",
                        "v4.channel.k8s.io".parse().expect("protocol header"),
                    );
                    Ok(response)
                },
            )
            .await
            .expect("accept websocket");
            let _ = websocket.next().await.expect("stdin frame");
            let mut stderr_frame = vec![2];
            stderr_frame.extend_from_slice(b"command failed\n");
            websocket
                .send(Message::Binary(stderr_frame))
                .await
                .expect("send stderr");
            let mut status_frame = vec![3];
            status_frame.extend_from_slice(
                br#"{"status":"Failure","reason":"NonZeroExitCode","exitCode":2,"message":"command terminated with exit code 2"}"#,
            );
            websocket
                .send(Message::Binary(status_frame))
                .await
                .expect("send status");
            let _ = websocket.close(None).await;
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(shell_runtime_metadata("exit 2")),
                },
            )
            .await;

        assert_eq!(response.status, "exec_failed");
        assert!(!response.execution_enabled);
        let exec_result = response.exec_result.expect("failure exec result");
        assert_eq!(exec_result["stderr"], "command failed\n");
        assert_eq!(exec_result["status"]["status"], "Failure");
        assert!(
            exec_result["status_failure"]
                .as_str()
                .unwrap_or_default()
                .contains("exit_code=2")
        );
        assert!(
            response.message.contains("exit_code=2"),
            "{}",
            response.message
        );

        server.await.expect("server task");
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_fails_when_status_frame_missing() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept exec websocket");
            let mut websocket = tokio_tungstenite::accept_hdr_async(
                stream,
                |_request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    response.headers_mut().insert(
                        "sec-websocket-protocol",
                        "v4.channel.k8s.io".parse().expect("protocol header"),
                    );
                    Ok(response)
                },
            )
            .await
            .expect("accept websocket");
            let _ = websocket.next().await.expect("stdin frame");
            let mut stdout_frame = vec![1];
            stdout_frame.extend_from_slice(b"partial output");
            websocket
                .send(Message::Binary(stdout_frame))
                .await
                .expect("send stdout");
            let _ = websocket.close(None).await;
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let response = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_exec".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(shell_runtime_metadata("echo partial")),
                },
            )
            .await;

        assert_eq!(response.status, "exec_failed");
        assert!(!response.execution_enabled);
        assert!(response.exec_result.is_none());
        assert!(
            response.message.contains("closed without status"),
            "{}",
            response.message
        );

        server.await.expect("server task");
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[test]
    fn kubernetes_exec_output_capture_is_bounded() {
        let mut output = Vec::new();
        let mut truncated = false;
        append_bounded_exec_output(
            &mut output,
            &vec![b'a'; MAX_KUBERNETES_EXEC_CAPTURE_BYTES + 16],
            &mut truncated,
        );
        assert_eq!(output.len(), MAX_KUBERNETES_EXEC_CAPTURE_BYTES);
        assert!(truncated);

        append_bounded_exec_output(&mut output, b"ignored", &mut truncated);
        assert_eq!(output.len(), MAX_KUBERNETES_EXEC_CAPTURE_BYTES);
        assert!(truncated);
    }

    #[tokio::test]
    async fn kubernetes_runner_live_exec_times_out_without_status_frame() {
        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept exec websocket");
            let mut websocket = tokio_tungstenite::accept_hdr_async(
                stream,
                |_request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    response.headers_mut().insert(
                        "sec-websocket-protocol",
                        "v4.channel.k8s.io".parse().expect("protocol header"),
                    );
                    Ok(response)
                },
            )
            .await
            .expect("accept websocket");
            let mut stdout_frame = vec![1];
            stdout_frame.extend_from_slice(b"partial output");
            websocket
                .send(Message::Binary(stdout_frame))
                .await
                .expect("send stdout");
            tokio::time::sleep(Duration::from_millis(250)).await;
            let _ = websocket.close(None).await;
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let result = call_kubernetes_exec_with_timeout(
            &config,
            "agent-os",
            "agent-remote-computer-test",
            b"{}\n",
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(
            result.expect_err("expected timeout"),
            "Kubernetes exec WebSocket timed out"
        );

        server.await.expect("server task");
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn kubernetes_runner_live_create_and_delete_call_pod_api_without_execution() {
        async fn create_pod(headers: HeaderMap, Json(body): Json<Value>) -> Json<Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer test-token")
            );
            assert_eq!(body["apiVersion"], "v1");
            assert_eq!(body["kind"], "Pod");
            assert_eq!(body["metadata"]["name"], "agent-remote-computer-test");
            assert_eq!(
                body["metadata"]["labels"]["mandoforge.io/session-id"],
                "00000000-0000-0000-0000-000000000000"
            );
            assert_eq!(
                body["metadata"]["labels"]["mandoforge.io/remote-computer-id"],
                "00000000-0000-0000-0000-000000000001"
            );
            assert_eq!(
                body["metadata"]["labels"]["mandoforge.io/tenant-id"],
                "00000000-0000-0000-0000-000000000002"
            );
            assert_eq!(
                body["metadata"]["labels"]["mandoforge.io/lease-id"],
                "00000000-0000-0000-0000-000000000003"
            );
            assert_eq!(
                body["metadata"]["annotations"]["mandoforge.io/lifecycle"],
                "session-bound"
            );
            assert_eq!(
                body["spec"]["serviceAccountName"],
                "mandoforge-remote-computer"
            );
            let remote_computer = &body["spec"]["containers"][0];
            assert_eq!(remote_computer["name"], "remote-computer");
            assert_eq!(remote_computer["image"], "mandoforge-adoption-api:latest");
            let remote_env = remote_computer["env"].as_array().expect("remote env");
            assert!(remote_env.iter().any(|env| {
                env["name"] == "MANDOFORGE_REMOTE_COMPUTER_MODE" && env["value"] == "session-pod"
            }));
            assert!(
                body["spec"]["containers"][0]["volumeMounts"]
                    .as_array()
                    .expect("volume mounts")
                    .iter()
                    .any(|mount| mount["name"] == "state-contract"
                        && mount["mountPath"] == "/agent-state/.mandoforge/contract"
                        && mount["readOnly"] == json!(true))
            );
            let sidecar = body["spec"]["containers"]
                .as_array()
                .expect("containers")
                .iter()
                .find(|container| container["name"] == "artifact-discovery")
                .expect("artifact discovery sidecar");
            assert_eq!(sidecar["image"], "python:3.12-alpine");
            let sidecar_env = sidecar["env"].as_array().expect("sidecar env");
            assert!(sidecar_env.iter().any(|env| {
                env["name"] == "MANDOFORGE_ARTIFACT_DISCOVERY_ENABLED" && env["value"] == "true"
            }));
            assert!(sidecar_env.iter().any(|env| {
                env["name"] == "MANDOFORGE_SESSION_ID"
                    && env["value"] == "00000000-0000-0000-0000-000000000000"
            }));
            assert!(sidecar_env.iter().any(|env| {
                env["name"] == "MANDOFORGE_REMOTE_COMPUTER_ID"
                    && env["value"] == "00000000-0000-0000-0000-000000000001"
            }));
            assert!(sidecar_env.iter().any(|env| {
                env["name"] == "MANDOFORGE_ASSIGNMENT_ID" && env["value"] == "assignment-1"
            }));
            assert!(
                body["spec"]["volumes"]
                    .as_array()
                    .expect("volumes")
                    .iter()
                    .any(|volume| volume["name"] == "state-contract"
                        && volume["configMap"]["name"]
                            == "mandoforge-remote-computer-state-contract")
            );
            Json(json!({"metadata": {"name": "agent-remote-computer-test"}}))
        }

        async fn delete_pod(
            AxumPath(pod_name): AxumPath<String>,
            headers: HeaderMap,
        ) -> Json<Value> {
            assert_eq!(pod_name, "agent-remote-computer-test");
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer test-token")
            );
            Json(json!({"status": "Success"}))
        }

        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/api/v1/namespaces/agent-os/pods", post(create_pod))
                    .route(
                        "/api/v1/namespaces/agent-os/pods/{pod_name}",
                        delete(delete_pod),
                    ),
            )
            .await
            .expect("mock kube server");
        });

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: false,
        };
        let create = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: Some(Uuid::from_u128(1)),
                    session_id: Some(Uuid::nil()),
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: Some(json!({
                        "artifact_discovery_enabled": true,
                        "assignment_id": "assignment-1",
                        "tenant_id": Uuid::from_u128(2).to_string(),
                        "lease_id": Uuid::from_u128(3).to_string()
                    })),
                },
            )
            .await;
        assert_eq!(create.status, "mutation_ok");
        assert!(create.live_mutation_attempted);
        assert_eq!(create.live_mutation_status_code, Some(200));
        assert!(create.would_create_pod);
        assert!(!create.execution_enabled);

        let delete = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_delete".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-remote-computer-test".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(delete.status, "mutation_ok");
        assert!(delete.live_mutation_attempted);
        assert_eq!(delete.live_mutation_status_code, Some(200));
        assert!(delete.would_delete_pod);
        assert!(!delete.execution_enabled);

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn agent_sandbox_runner_creates_claim_and_resolves_bound_pod() {
        async fn create_claim(headers: HeaderMap, Json(body): Json<Value>) -> Json<Value> {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer test-token")
            );
            assert_eq!(body["apiVersion"], "extensions.agents.x-k8s.io/v1beta1");
            assert_eq!(body["kind"], "SandboxClaim");
            assert_eq!(body["metadata"]["name"], "agent-as-session");
            assert_eq!(
                body["metadata"]["labels"]["mandoforge.io/runtime-substrate"],
                "agent-sandbox"
            );
            assert_eq!(
                body["spec"]["warmPoolRef"]["name"],
                "mandoforge-agent-runtime"
            );
            Json(json!({"metadata": {"name": "agent-as-session"}}))
        }

        async fn get_claim(AxumPath(claim_name): AxumPath<String>) -> Json<Value> {
            assert_eq!(claim_name, "agent-as-session");
            Json(json!({
                "status": {"sandbox": {"name": "warm-sandbox-generated"}}
            }))
        }

        async fn get_sandbox(AxumPath(sandbox_name): AxumPath<String>) -> Json<Value> {
            assert_eq!(sandbox_name, "warm-sandbox-generated");
            Json(json!({
                "metadata": {
                    "annotations": {
                        "agents.x-k8s.io/pod-name": "warm-pod-1"
                    }
                }
            }))
        }

        async fn delete_claim(AxumPath(claim_name): AxumPath<String>) -> Json<Value> {
            assert_eq!(claim_name, "agent-as-session");
            Json(json!({"status": "Success"}))
        }

        let token_path =
            std::env::temp_dir().join(format!("mandoforge-kube-token-{}.txt", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token")
            .await
            .expect("write token");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route(
                        "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/agent-os/sandboxclaims",
                        post(create_claim),
                    )
                    .route(
                        "/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/agent-os/sandboxclaims/{claim_name}",
                        get(get_claim).delete(delete_claim),
                    )
                    .route(
                        "/apis/agents.x-k8s.io/v1beta1/namespaces/agent-os/sandboxes/{sandbox_name}",
                        get(get_sandbox),
                    ),
            )
            .await
            .expect("mock kube server");
        });

        let config = RemoteComputerRunnerConfig {
            mode: "agent-sandbox".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: "missing-pod-template-is-ok-for-agent-sandbox".to_string(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: false,
        };
        let create = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_create".to_string()),
                    remote_computer_id: None,
                    session_id: Some(Uuid::nil()),
                    pod_name: Some("agent-as-session".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(create.status, "mutation_ok");
        assert!(create.live_mutation_attempted);
        assert!(create.would_create_pod);
        assert_eq!(
            create.kubernetes_api_path.as_deref(),
            Some("/apis/extensions.agents.x-k8s.io/v1beta1/namespaces/agent-os/sandboxclaims")
        );

        let binding = poll_agent_sandbox_binding(
            &config,
            "agent-os",
            "agent-as-session",
            Duration::from_secs(1),
            Duration::from_millis(10),
        )
        .await
        .expect("bound pod");
        assert_eq!(binding.sandbox_name, "warm-sandbox-generated");
        assert_eq!(binding.pod_name, "warm-pod-1");

        let delete = KubernetesRemoteComputerRunner
            .mutate(
                &config,
                RemoteComputerRunnerDryRunRequest {
                    operation: Some("live_delete".to_string()),
                    remote_computer_id: None,
                    session_id: None,
                    pod_name: Some("agent-as-session".to_string()),
                    metadata: None,
                },
            )
            .await;
        assert_eq!(delete.status, "mutation_ok");
        assert!(delete.would_delete_pod);

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_returns_err_when_api_url_missing() {
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: None,
            bearer_token_path: Some("/tmp/token".to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_millis(100),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("supported Kubernetes API client"),
            "error should mention unsupported Kubernetes client configuration"
        );
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_returns_err_when_token_path_missing() {
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some("http://127.0.0.1:19999".to_string()),
            bearer_token_path: None,
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };
        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_millis(100),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("supported Kubernetes API client"),
            "error should mention unsupported Kubernetes client configuration"
        );
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_succeeds_on_running_phase() {
        use axum::{Router, extract::Path as AxumPath, routing::get};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        async fn pod_running(_path: AxumPath<String>) -> axum::response::Json<serde_json::Value> {
            axum::response::Json(serde_json::json!({
                "status": { "phase": "Running" }
            }))
        }

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/v1/namespaces/agent-os/pods/{pod}", get(pod_running)),
            )
            .await
            .unwrap()
        });

        let token_path =
            std::env::temp_dir().join(format!("poll_test_token_running-{}", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_secs(5),
            Duration::from_millis(100),
        )
        .await;
        assert!(result.is_ok(), "should succeed when Pod phase is Running");

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_errors_on_terminal_phase() {
        use axum::{Router, extract::Path as AxumPath, routing::get};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        async fn pod_failed(_path: AxumPath<String>) -> axum::response::Json<serde_json::Value> {
            axum::response::Json(serde_json::json!({
                "status": { "phase": "Failed" }
            }))
        }

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/v1/namespaces/agent-os/pods/{pod}", get(pod_failed)),
            )
            .await
            .unwrap()
        });

        let token_path =
            std::env::temp_dir().join(format!("poll_test_token_failed-{}", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_secs(5),
            Duration::from_millis(100),
        )
        .await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("terminal phase") && msg.contains("Failed"),
            "error should mention terminal phase: {msg}"
        );

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_times_out_when_still_pending() {
        use axum::{Router, extract::Path as AxumPath, routing::get};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        async fn pod_pending(_path: AxumPath<String>) -> axum::response::Json<serde_json::Value> {
            axum::response::Json(serde_json::json!({
                "status": { "phase": "Pending" }
            }))
        }

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/v1/namespaces/agent-os/pods/{pod}", get(pod_pending)),
            )
            .await
            .unwrap()
        });

        let token_path =
            std::env::temp_dir().join(format!("poll_test_token_pending-{}", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("did not reach Running"),
            "error should mention timeout: {msg}"
        );

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_retries_past_404_then_succeeds() {
        use axum::{Router, extract::Path as AxumPath, routing::get};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handler = move |_path: AxumPath<String>| {
            let count = call_count_clone.clone();
            async move {
                let n = count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    axum::response::Response::builder()
                        .status(404)
                        .body(axum::body::Body::empty())
                        .unwrap()
                } else {
                    axum::response::Response::builder()
                        .status(200)
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(r#"{"status":{"phase":"Running"}}"#))
                        .unwrap()
                }
            }
        };

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/v1/namespaces/agent-os/pods/{pod}", get(handler)),
            )
            .await
            .unwrap()
        });

        let token_path = std::env::temp_dir().join(format!(
            "poll_test_token_404_then_running-{}",
            Uuid::new_v4()
        ));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_secs(5),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_ok(), "should succeed after retrying past 404");
        assert!(
            call_count.load(Ordering::SeqCst) >= 2,
            "should have retried at least once"
        );

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_errors_on_non_2xx_status() {
        use axum::{Router, extract::Path as AxumPath, routing::get};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/api/v1/namespaces/agent-os/pods/{pod}",
                    get(|_: AxumPath<String>| async {
                        axum::response::Response::builder()
                            .status(500)
                            .body(axum::body::Body::empty())
                            .unwrap()
                    }),
                ),
            )
            .await
            .unwrap()
        });

        let token_path =
            std::env::temp_dir().join(format!("poll_test_token_500-{}", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_secs(5),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("HTTP 500"),
            "error should mention HTTP 500: {msg}"
        );

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }

    #[tokio::test]
    async fn poll_kubernetes_pod_running_errors_on_succeeded_phase() {
        use axum::{Router, extract::Path as AxumPath, routing::get};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/api/v1/namespaces/agent-os/pods/{pod}",
                    get(|_: AxumPath<String>| async {
                        axum::response::Json(serde_json::json!({"status": {"phase": "Succeeded"}}))
                    }),
                ),
            )
            .await
            .unwrap()
        });

        let token_path =
            std::env::temp_dir().join(format!("poll_test_token_succeeded-{}", Uuid::new_v4()));
        tokio::fs::write(&token_path, "test-token").await.unwrap();

        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            kube_api_url: Some(format!("http://{addr}")),
            bearer_token_path: Some(token_path.to_string_lossy().to_string()),
            in_cluster: false,
            mutation_enabled: true,
            live_mutation_enabled: true,
            execution_enabled: true,
        };

        let result = poll_kubernetes_pod_running(
            &config,
            "agent-rc-test",
            Duration::from_secs(5),
            Duration::from_millis(50),
        )
        .await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Succeeded"),
            "error should mention Succeeded phase: {msg}"
        );

        server.abort();
        let _ = tokio::fs::remove_file(token_path).await;
    }
}
