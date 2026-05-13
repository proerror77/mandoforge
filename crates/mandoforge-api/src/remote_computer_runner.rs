use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

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
                "dry_run_probe".to_string(),
                "live_create".to_string(),
                "live_delete".to_string(),
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
            } else if readiness.configured {
                "Kubernetes adapter dry-run calculated Pod intent only; no Kubernetes API mutation or tool execution was performed"
                    .to_string()
            } else {
                "Kubernetes adapter dry-run is blocked until template and client configuration are present"
                    .to_string()
            },
            request: json!(request),
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
        } else {
            None
        };
        let gates_open = readiness.configured
            && config.mutation_enabled
            && config.live_mutation_enabled
            && config.kube_api_url.is_some()
            && config.bearer_token_path.is_some();
        let mutation_result = if gates_open && (operation_is_create || operation_is_delete) {
            Some(call_kubernetes_mutation(config, operation_is_create, &pod_name).await)
        } else {
            None
        };
        let live_mutation_status_code = mutation_result
            .as_ref()
            .and_then(|result| result.as_ref().ok().map(|(status_code, _)| *status_code));
        let mutation_failed_message = mutation_result
            .as_ref()
            .and_then(|result| result.as_ref().err().cloned());
        let status = if let Some(result) = &mutation_result {
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
            execution_enabled: false,
            message: if let Some(message) = mutation_failed_message {
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
            } else if !(operation_is_create || operation_is_delete) {
                "Kubernetes mutation only supports live_create and live_delete".to_string()
            } else {
                "Kubernetes mutation was blocked by the runner policy".to_string()
            },
            request: json!(request),
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
            .json(&build_kubernetes_pod_request(config, pod_name))
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

fn build_kubernetes_pod_request(config: &RemoteComputerRunnerConfig, pod_name: &str) -> Value {
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
            "containers": [{
                "name": "remote-computer",
                "image": "ghcr.io/proerror77/mandoforge-remote-computer:latest",
                "imagePullPolicy": "IfNotPresent",
                "command": ["sleep", "infinity"],
                "env": [{"name": "MANDOFORGE_REMOTE_COMPUTER_MODE", "value": "skeleton"}],
                "securityContext": {
                    "allowPrivilegeEscalation": false,
                    "readOnlyRootFilesystem": false,
                    "capabilities": {"drop": ["ALL"]}
                },
                "volumeMounts": [
                    {"name": "state", "mountPath": "/agent-state"},
                    {"name": "workspace", "mountPath": "/workspace"}
                ],
                "resources": {
                    "requests": {"cpu": "100m", "memory": "256Mi"},
                    "limits": {"cpu": "1", "memory": "1Gi"}
                }
            }],
            "volumes": [
                {"name": "state", "persistentVolumeClaim": {"claimName": "mandoforge-remote-computer-state"}},
                {"name": "workspace", "emptyDir": {}}
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
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Path as AxumPath,
        http::HeaderMap,
        routing::{delete, get, post},
    };

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
        };
        let create = KubernetesRemoteComputerRunner
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
}
