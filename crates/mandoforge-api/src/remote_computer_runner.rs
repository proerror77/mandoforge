use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;

const DEFAULT_KUBERNETES_EXEC_TIMEOUT_SECONDS: u64 = 120;
const MAX_KUBERNETES_EXEC_CAPTURE_BYTES: usize = 1024 * 1024;

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
            request: json!(request),
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
            request: json!(request),
            exec_result: None,
        }
    }
}

#[async_trait]
impl RemoteComputerRunner for KubernetesRemoteComputerRunner {
    fn readiness(&self, config: &RemoteComputerRunnerConfig) -> RemoteComputerRunnerReadiness {
        let client_configured = kubernetes_client_configured(config);
        let api_server_configured = config.kube_api_url.is_some();
        let bearer_token_configured = kubernetes_bearer_token_configured(config);
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
            ],
            message: if configured && config.mutation_enabled && config.live_mutation_enabled {
                "Kubernetes Remote Computer adapter is configured for explicit live Pod create/delete; tool execution remains disabled"
            } else if configured {
                "Kubernetes Remote Computer adapter is configured for dry-run planning; Pod mutation remains disabled until both mutation gates are enabled"
            } else if !template_present {
                "Kubernetes Remote Computer adapter is selected, but the Pod template is missing"
            } else if api_server_configured && !bearer_token_configured && !config.in_cluster {
                "Kubernetes Remote Computer adapter has an API server URL, but no bearer token path or in-cluster identity is configured"
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
        let operation_is_probe = operation == "probe" || operation == "dry_run_probe";
        let operation_is_exec = operation == "exec" || operation == "dry_run_exec";
        let pod_name = request
            .pod_name
            .clone()
            .filter(|pod_name| !pod_name.trim().is_empty())
            .unwrap_or_else(|| "agent-remote-computer-dry-run".to_string());
        let kubernetes_api_path = if operation_is_create {
            Some(format!("/api/v1/namespaces/{}/pods", config.namespace))
        } else if operation_is_delete {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}",
                config.namespace, pod_name
            ))
        } else if operation_is_probe {
            Some("/version".to_string())
        } else if operation_is_exec {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}/exec",
                config.namespace, pod_name
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
            namespace: Some(config.namespace.clone()),
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
            request: json!(request),
            exec_result: None,
        }
    }

    async fn mutate(
        &self,
        config: &RemoteComputerRunnerConfig,
        request: RemoteComputerRunnerDryRunRequest,
    ) -> RemoteComputerRunnerDryRunResponse {
        let operation = request
            .operation
            .clone()
            .unwrap_or_else(|| "live_create".to_string());
        let readiness = self.readiness(config);
        let operation_is_create = operation == "create" || operation == "live_create";
        let operation_is_delete = operation == "delete" || operation == "live_delete";
        let operation_is_exec = operation == "exec" || operation == "live_exec";
        let pod_name = request
            .pod_name
            .clone()
            .filter(|pod_name| valid_kubernetes_name(pod_name))
            .unwrap_or_else(|| live_pod_name(&request));
        let kubernetes_api_path = if operation_is_create {
            Some(format!("/api/v1/namespaces/{}/pods", config.namespace))
        } else if operation_is_delete {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}",
                config.namespace, pod_name
            ))
        } else if operation_is_exec {
            Some(format!(
                "/api/v1/namespaces/{}/pods/{}/exec",
                config.namespace, pod_name
            ))
        } else {
            None
        };
        let gates_open = readiness.configured
            && config.mutation_enabled
            && config.live_mutation_enabled
            && config.kube_api_url.is_some()
            && config.bearer_token_path.is_some();
        let mutation_result = if gates_open && (operation_is_create || operation_is_delete) {
            Some(call_kubernetes_mutation(config, operation_is_create, &pod_name, &request).await)
        } else {
            None
        };
        let exec_command = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("command"))
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "true".to_string());
        let exec_gates_open = readiness.configured
            && config.execution_enabled
            && config.live_mutation_enabled
            && config.kube_api_url.is_some()
            && config.bearer_token_path.is_some();
        let exec_result = if exec_gates_open && operation_is_exec {
            Some(call_kubernetes_exec(config, &pod_name, &exec_command).await)
        } else {
            None
        };
        let live_mutation_status_code = mutation_result
            .as_ref()
            .and_then(|result| result.as_ref().ok().map(|(status_code, _)| *status_code));
        let mutation_failed_message = mutation_result
            .as_ref()
            .and_then(|result| result.as_ref().err().cloned());
        let exec_failed_message = exec_result
            .as_ref()
            .and_then(|result| result.as_ref().err().cloned());
        let exec_result_payload = exec_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(KubernetesExecResult::to_json);
        let status = if let Some(result) = &exec_result {
            if result.is_ok() {
                "exec_ok"
            } else {
                "exec_failed"
            }
        } else if let Some(result) = &mutation_result {
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
            namespace: Some(config.namespace.clone()),
            pod_name: Some(pod_name),
            pod_template_path: Some(config.pod_template_path.clone()),
            execution_enabled: exec_result.as_ref().is_some_and(|result| result.is_ok()),
            message: if let Some(message) = exec_failed_message {
                format!("Kubernetes Pod exec failed: {message}")
            } else if exec_result.is_some() {
                "Kubernetes Pod exec completed through the WebSocket transport; no execution job was created"
                    .to_string()
            } else if let Some(message) = mutation_failed_message {
                format!("Kubernetes mutation failed: {message}")
            } else if mutation_result.is_some() {
                "Kubernetes Pod mutation completed; no tool execution or execution job was started"
                    .to_string()
            } else if !readiness.configured {
                "Kubernetes mutation is blocked until template and client configuration are present"
                    .to_string()
            } else if !config.mutation_enabled || !config.live_mutation_enabled {
                "Kubernetes mutation is blocked until both mutation gates are explicitly enabled"
                    .to_string()
            } else if config.kube_api_url.is_none() || config.bearer_token_path.is_none() {
                "Kubernetes mutation requires API server URL and bearer token path; kubeconfig/in-cluster mutation is not implemented"
                    .to_string()
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
            request: json!(request),
            exec_result: exec_result_payload,
        }
    }
}

async fn probe_kubernetes_version(
    config: &RemoteComputerRunnerConfig,
) -> Result<(u16, Value), String> {
    let api_url = config
        .kube_api_url
        .as_deref()
        .ok_or_else(|| "kube API URL is not configured".to_string())?;
    let token_path = config
        .bearer_token_path
        .as_deref()
        .ok_or_else(|| "bearer token path is not configured".to_string())?;
    let token = tokio::fs::read_to_string(token_path)
        .await
        .map_err(|err| format!("failed to read bearer token: {err}"))?;
    let response = reqwest::Client::builder()
        .build()
        .map_err(|err| format!("failed to build Kubernetes HTTP client: {err}"))?
        .get(format!("{api_url}/version"))
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
    create: bool,
    pod_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Result<(u16, Value), String> {
    let api_url = config
        .kube_api_url
        .as_deref()
        .ok_or_else(|| "kube API URL is not configured".to_string())?;
    let token_path = config
        .bearer_token_path
        .as_deref()
        .ok_or_else(|| "bearer token path is not configured".to_string())?;
    let token = tokio::fs::read_to_string(token_path)
        .await
        .map_err(|err| format!("failed to read bearer token: {err}"))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("failed to build Kubernetes HTTP client: {err}"))?;
    let url = if create {
        format!("{api_url}/api/v1/namespaces/{}/pods", config.namespace)
    } else {
        format!(
            "{api_url}/api/v1/namespaces/{}/pods/{}",
            config.namespace, pod_name
        )
    };
    let request = if create {
        client
            .post(url)
            .bearer_auth(token.trim())
            .json(&build_kubernetes_pod_request(config, pod_name, request))
    } else {
        client.delete(url).bearer_auth(token.trim())
    };
    let response = request
        .send()
        .await
        .map_err(|err| format!("failed to call Kubernetes Pod API: {err}"))?;
    let status_code = response.status().as_u16();
    let body = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({"status_code": status_code}));
    if !(200..300).contains(&status_code) {
        return Err(format!("Kubernetes Pod API returned HTTP {status_code}"));
    }
    Ok((status_code, body))
}

#[derive(Debug, Clone)]
struct KubernetesExecResult {
    handshake_status_code: u16,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
    status: Option<Value>,
}

impl KubernetesExecResult {
    fn to_json(&self) -> Value {
        json!({
            "handshake_status_code": self.handshake_status_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "stdout_truncated": self.stdout_truncated,
            "stderr_truncated": self.stderr_truncated,
            "status": self.status
        })
    }
}

async fn call_kubernetes_exec(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    command: &str,
) -> Result<KubernetesExecResult, String> {
    call_kubernetes_exec_with_timeout(
        config,
        pod_name,
        command,
        Duration::from_secs(kubernetes_exec_timeout_seconds()),
    )
    .await
}

async fn call_kubernetes_exec_with_timeout(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    command: &str,
    timeout: Duration,
) -> Result<KubernetesExecResult, String> {
    let api_url = config
        .kube_api_url
        .as_deref()
        .ok_or_else(|| "kube API URL is not configured".to_string())?;
    let token_path = config
        .bearer_token_path
        .as_deref()
        .ok_or_else(|| "bearer token path is not configured".to_string())?;
    let token = tokio::fs::read_to_string(token_path)
        .await
        .map_err(|err| format!("failed to read bearer token: {err}"))?;
    let websocket_url =
        kubernetes_exec_websocket_url(api_url, &config.namespace, pod_name, command);
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
    let (mut socket, response) = connect_async(request)
        .await
        .map_err(|err| format!("failed to open Kubernetes exec WebSocket: {err}"))?;
    let handshake_status_code = response.status().as_u16();
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
    Ok(KubernetesExecResult {
        handshake_status_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        stdout_truncated,
        stderr_truncated,
        status,
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
pub(crate) async fn poll_kubernetes_pod_running(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<(), String> {
    let api_url = config
        .kube_api_url
        .as_deref()
        .ok_or_else(|| "kube API URL is not configured".to_string())?;
    let token_path = config
        .bearer_token_path
        .as_deref()
        .ok_or_else(|| "bearer token path is not configured".to_string())?;
    let poll_interval = interval.max(Duration::from_millis(100));
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(config.in_cluster)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("failed to build HTTP client: {err}"))?;
    loop {
        if Instant::now() >= deadline {
            return Err(format!(
                "Pod did not reach Running within {:.0}s",
                timeout.as_secs_f64()
            ));
        }
        let token = tokio::fs::read_to_string(token_path)
            .await
            .map_err(|err| format!("failed to read bearer token: {err}"))?;
        let url = format!(
            "{api_url}/api/v1/namespaces/{}/pods/{}",
            percent_encode(&config.namespace),
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

fn kubernetes_exec_websocket_url(
    api_url: &str,
    namespace: &str,
    pod_name: &str,
    command: &str,
) -> String {
    let base = if let Some(rest) = api_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = api_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        api_url.to_string()
    };
    format!(
        "{base}/api/v1/namespaces/{}/pods/{}/exec?container=remote-computer&stdout=true&stderr=true&stdin=false&tty=false&command=sh&command=-lc&command={}",
        percent_encode(namespace),
        percent_encode(pod_name),
        percent_encode(command)
    )
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

fn build_kubernetes_pod_request(
    config: &RemoteComputerRunnerConfig,
    pod_name: &str,
    request: &RemoteComputerRunnerDryRunRequest,
) -> Value {
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
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": pod_name,
            "namespace": config.namespace,
            "labels": {
                "app": "mandoforge-agent-remote-computer",
                "mandoforge.io/runner": "remote-computer"
            },
            "annotations": {
                "mandoforge.io/template-path": config.pod_template_path
            }
        },
        "spec": {
            "serviceAccountName": config.service_account,
            "automountServiceAccountToken": false,
            "restartPolicy": "Never",
            "securityContext": {
                "runAsNonRoot": true,
                "seccompProfile": {"type": "RuntimeDefault"}
            },
            "containers": [
                {
                    "name": "remote-computer",
                    "image": "ghcr.io/proerror77/mandoforge-remote-computer:latest",
                    "imagePullPolicy": "IfNotPresent",
                    "command": ["sleep", "infinity"],
                    "env": [
                        {"name": "MANDOFORGE_REMOTE_COMPUTER_MODE", "value": "session-pod"},
                        {"name": "MANDOFORGE_SESSION_ID", "value": session_id},
                        {"name": "MANDOFORGE_REMOTE_COMPUTER_ID", "value": remote_computer_id},
                        {"name": "MANDOFORGE_SESSION_WORKSPACE", "value": session_workspace_path},
                        {"name": "MANDOFORGE_WORKSPACE_ROOT", "value": "/workspace"}
                    ],
                    "securityContext": {
                        "allowPrivilegeEscalation": false,
                        "readOnlyRootFilesystem": false,
                        "capabilities": {"drop": ["ALL"]}
                    },
                    "volumeMounts": [
                        {"name": "state", "mountPath": "/agent-state"},
                        {"name": "state-contract", "mountPath": "/agent-state/.mandoforge/contract", "readOnly": true},
                        {"name": "workspace", "mountPath": "/workspace"}
                    ],
                    "resources": {
                        "requests": {"cpu": "100m", "memory": "256Mi"},
                        "limits": {"cpu": "1", "memory": "1Gi"}
                    }
                },
                {
                    "name": "artifact-discovery",
                    "image": "python:3.12-alpine",
                    "imagePullPolicy": "IfNotPresent",
                    "command": ["python", "/opt/mandoforge/discover-artifacts.py"],
                    "env": [
                        {"name": "MANDOFORGE_ARTIFACT_DISCOVERY_ENABLED", "value": artifact_discovery_enabled},
                        {"name": "MANDOFORGE_API_URL", "value": "http://mandoforge-api:8787"},
                        {"name": "MANDOFORGE_SESSION_ID", "value": session_id},
                        {"name": "MANDOFORGE_REMOTE_COMPUTER_ID", "value": remote_computer_id},
                        {"name": "MANDOFORGE_ASSIGNMENT_ID", "value": assignment_id},
                        {"name": "MANDOFORGE_ARTIFACT_DIR", "value": artifact_dir},
                        {"name": "MANDOFORGE_ARTIFACT_DISCOVERY_INTERVAL_SECONDS", "value": "30"},
                        {"name": "MANDOFORGE_ARTIFACT_DISCOVERY_MAX_FILES", "value": "50"}
                    ],
                    "securityContext": {
                        "allowPrivilegeEscalation": false,
                        "readOnlyRootFilesystem": true,
                        "capabilities": {"drop": ["ALL"]}
                    },
                    "volumeMounts": [
                        {"name": "workspace", "mountPath": "/workspace", "readOnly": true},
                        {"name": "artifact-discovery", "mountPath": "/opt/mandoforge", "readOnly": true}
                    ],
                    "resources": {
                        "requests": {"cpu": "25m", "memory": "64Mi"},
                        "limits": {"cpu": "100m", "memory": "128Mi"}
                    }
                }
            ],
            "volumes": [
                {"name": "state", "persistentVolumeClaim": {"claimName": "mandoforge-remote-computer-state"}},
                {"name": "state-contract", "configMap": {"name": "mandoforge-remote-computer-state-contract"}},
                {"name": "workspace", "emptyDir": {}},
                {"name": "artifact-discovery", "configMap": {"name": "mandoforge-remote-computer-artifact-discovery"}}
            ]
        }
    })
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

fn kubernetes_client_configured(config: &RemoteComputerRunnerConfig) -> bool {
    config.in_cluster
        || (config.kube_api_url.is_some() && kubernetes_bearer_token_configured(config))
        || config
            .kubeconfig_path
            .as_deref()
            .is_some_and(|path| Path::new(path).exists())
}

fn kubernetes_bearer_token_configured(config: &RemoteComputerRunnerConfig) -> bool {
    config
        .bearer_token_path
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
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Path as AxumPath,
        http::HeaderMap,
        routing::{delete, get, post},
    };
    use futures_util::SinkExt;

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
            live_mutation_enabled: false,
            execution_enabled: false,
            kube_api_url: None,
            bearer_token_path: None,
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
    }

    #[tokio::test]
    async fn kubernetes_runner_dry_run_exec_plans_pod_exec_without_execution() {
        let config = RemoteComputerRunnerConfig {
            mode: "kubernetes".to_string(),
            namespace: "agent-os".to_string(),
            pod_template_path: test_pod_template_path(),
            service_account: "mandoforge-remote-computer".to_string(),
            kubeconfig_path: None,
            in_cluster: true,
            mutation_enabled: true,
            live_mutation_enabled: false,
            execution_enabled: false,
            kube_api_url: None,
            bearer_token_path: None,
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
                    metadata: Some(json!({"command": ["sh", "-lc", "pwd"]})),
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
        assert_eq!(response.request["metadata"]["command"][0], "sh");
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
            stdout_frame.extend_from_slice(b"hello from pod\n");
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
                    metadata: Some(json!({"command": "echo hello from pod"})),
                },
            )
            .await;
        assert_eq!(response.status, "exec_ok", "{}", response.message);
        assert!(response.execution_enabled);
        assert_eq!(
            response.kubernetes_api_path.as_deref(),
            Some("/api/v1/namespaces/agent-os/pods/agent-remote-computer-test/exec")
        );
        let exec_result = response.exec_result.expect("exec result");
        assert_eq!(exec_result["stdout"], "hello from pod\n");
        assert_eq!(exec_result["stderr"], "pod warning\n");
        assert_eq!(exec_result["stdout_truncated"], false);
        assert_eq!(exec_result["stderr_truncated"], false);
        assert_eq!(exec_result["status"]["status"], "Success");

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
                    metadata: Some(json!({"command": "echo partial"})),
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
            "agent-remote-computer-test",
            "sleep 30",
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
                body["spec"]["serviceAccountName"],
                "mandoforge-remote-computer"
            );
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
                        "assignment_id": "assignment-1"
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
            result.unwrap_err().contains("kube API URL"),
            "error should mention kube API URL"
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
            result.unwrap_err().contains("bearer token"),
            "error should mention bearer token"
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
