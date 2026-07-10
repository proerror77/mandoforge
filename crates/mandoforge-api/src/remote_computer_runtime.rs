use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::RemoteComputer;
use crate::remote_computer_runner::{
    RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
    remote_computer_runner_for_config,
};

pub(crate) const REMOTE_COMPUTER_RUNTIME_IDENTITY_METADATA_KEY: &str = "runtime_identity";
const REMOTE_COMPUTER_RUNTIME_IDENTITY_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RemoteComputerSubstrate {
    KubernetesPod,
    AgentSandbox,
}

impl RemoteComputerSubstrate {
    pub(crate) fn from_metadata(value: Option<&str>) -> Option<Self> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            Some("kubernetes-pod") => Some(Self::KubernetesPod),
            Some("agent-sandbox") => Some(Self::AgentSandbox),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RemoteComputerRuntimeIdentity {
    #[serde(default = "remote_computer_runtime_identity_version")]
    pub(crate) version: String,
    pub(crate) substrate: RemoteComputerSubstrate,
    pub(crate) namespace: String,
    pub(crate) resource_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) claim_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sandbox_name: Option<String>,
    pub(crate) pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lifecycle_deadline: Option<DateTime<Utc>>,
}

fn remote_computer_runtime_identity_version() -> String {
    REMOTE_COMPUTER_RUNTIME_IDENTITY_VERSION.to_string()
}

impl RemoteComputerRuntimeIdentity {
    pub(crate) fn new(
        substrate: RemoteComputerSubstrate,
        namespace: String,
        resource_name: String,
        pod_name: String,
        claim_name: Option<String>,
        sandbox_name: Option<String>,
        lifecycle_deadline: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            version: remote_computer_runtime_identity_version(),
            substrate,
            namespace,
            resource_name,
            claim_name,
            sandbox_name,
            pod_name,
            lifecycle_deadline,
        }
    }
}

pub(crate) fn remote_computer_runtime_identity(
    remote_computer: &RemoteComputer,
) -> Option<RemoteComputerRuntimeIdentity> {
    remote_computer_runtime_identity_from_parts(
        &remote_computer.metadata,
        &remote_computer.namespace,
        remote_computer.pod_name.as_deref(),
        Some(remote_computer.name.as_str()),
        Some(remote_computer.profile.as_str()),
    )
}

pub(crate) fn required_remote_computer_runtime_identity(
    remote_computer: &RemoteComputer,
) -> Result<RemoteComputerRuntimeIdentity, String> {
    let identity = remote_computer_runtime_identity(remote_computer);
    if remote_computer
        .metadata
        .get(REMOTE_COMPUTER_RUNTIME_IDENTITY_METADATA_KEY)
        .is_some()
        && identity.is_none()
    {
        return Err("Remote Computer runtime_identity metadata is invalid".to_string());
    }
    identity.ok_or_else(|| "Remote Computer has no usable runtime identity".to_string())
}

pub(crate) fn remote_computer_runtime_identity_from_parts(
    metadata: &Value,
    namespace: &str,
    pod_name: Option<&str>,
    fallback_name: Option<&str>,
    fallback_profile: Option<&str>,
) -> Option<RemoteComputerRuntimeIdentity> {
    if let Some(value) = metadata.get(REMOTE_COMPUTER_RUNTIME_IDENTITY_METADATA_KEY) {
        return serde_json::from_value::<RemoteComputerRuntimeIdentity>(value.clone()).ok();
    }

    let substrate = RemoteComputerSubstrate::from_metadata(
        metadata.get("runtime_substrate").and_then(Value::as_str),
    )
    .or_else(|| {
        metadata
            .get("sandbox_claim_name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|_| RemoteComputerSubstrate::AgentSandbox)
    })
    .or_else(|| match fallback_profile.map(str::trim) {
        Some("agent-sandbox") => Some(RemoteComputerSubstrate::AgentSandbox),
        _ => None,
    })
    .unwrap_or(RemoteComputerSubstrate::KubernetesPod);

    let pod_name = pod_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)?;
    let namespace = namespace.trim();
    if namespace.is_empty() {
        return None;
    }

    match substrate {
        RemoteComputerSubstrate::AgentSandbox => {
            let claim_name = metadata
                .get("sandbox_claim_name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .or_else(|| {
                    fallback_name
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                })?;
            Some(RemoteComputerRuntimeIdentity {
                version: remote_computer_runtime_identity_version(),
                substrate,
                namespace: namespace.to_string(),
                resource_name: claim_name.clone(),
                claim_name: Some(claim_name),
                sandbox_name: metadata
                    .get("sandbox_name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string),
                pod_name,
                lifecycle_deadline: metadata
                    .get("lifecycle_deadline")
                    .and_then(Value::as_str)
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&Utc)),
            })
        }
        RemoteComputerSubstrate::KubernetesPod => {
            let resource_name = fallback_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| pod_name.clone());
            Some(RemoteComputerRuntimeIdentity {
                version: remote_computer_runtime_identity_version(),
                substrate,
                namespace: namespace.to_string(),
                resource_name,
                claim_name: None,
                sandbox_name: None,
                pod_name,
                lifecycle_deadline: None,
            })
        }
    }
}

pub(crate) fn metadata_with_remote_computer_runtime_identity(
    metadata: &Value,
    identity: &RemoteComputerRuntimeIdentity,
) -> Value {
    let mut metadata = metadata.as_object().cloned().unwrap_or_default();
    metadata.insert(
        REMOTE_COMPUTER_RUNTIME_IDENTITY_METADATA_KEY.to_string(),
        serde_json::to_value(identity).expect("runtime identity serializes"),
    );
    metadata.insert("runtime_substrate".to_string(), json!(identity.substrate));
    match identity.claim_name.as_deref() {
        Some(claim_name) => {
            metadata.insert("sandbox_claim_name".to_string(), json!(claim_name));
        }
        None => {
            metadata.remove("sandbox_claim_name");
        }
    }
    match identity.sandbox_name.as_deref() {
        Some(sandbox_name) => {
            metadata.insert("sandbox_name".to_string(), json!(sandbox_name));
        }
        None => {
            metadata.remove("sandbox_name");
        }
    }
    match identity.lifecycle_deadline {
        Some(lifecycle_deadline) => {
            metadata.insert(
                "lifecycle_deadline".to_string(),
                json!(lifecycle_deadline.to_rfc3339()),
            );
        }
        None => {
            metadata.remove("lifecycle_deadline");
        }
    }
    Value::Object(metadata)
}

pub(crate) fn runtime_identity_metadata(identity: &RemoteComputerRuntimeIdentity) -> Value {
    json!({
        "runtime_substrate": identity.substrate,
        "namespace": identity.namespace,
        REMOTE_COMPUTER_RUNTIME_IDENTITY_METADATA_KEY: identity
    })
}

pub(crate) fn remote_computer_runner_request_is_exec(
    input: &RemoteComputerRunnerDryRunRequest,
) -> bool {
    input
        .operation
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .is_some_and(|operation| matches!(operation.as_str(), "exec" | "live_exec"))
}

pub(crate) fn remote_computer_runner_response_for_audit(
    response: &RemoteComputerRunnerDryRunResponse,
) -> Value {
    let mut value = json!(response);
    if let Some(exec_result) = response.exec_result.as_ref() {
        value["exec_result"] = json!({
            "captured": true,
            "stdout_chars": exec_result
                .get("stdout")
                .and_then(|value| value.as_str())
                .map(|value| value.chars().count())
                .unwrap_or(0),
            "stderr_chars": exec_result
                .get("stderr")
                .and_then(|value| value.as_str())
                .map(|value| value.chars().count())
                .unwrap_or(0),
            "status": exec_result.get("status").cloned().unwrap_or(Value::Null)
        });
    }
    value
}

pub(crate) fn build_remote_computer_runner_readiness() -> RemoteComputerRunnerReadiness {
    let config = RemoteComputerRunnerConfig::from_env();
    let runner = remote_computer_runner_for_config(&config);
    runner.readiness(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_identity_round_trips_through_metadata() {
        let deadline = DateTime::parse_from_rfc3339("2026-07-10T12:00:00Z")
            .expect("deadline")
            .with_timezone(&Utc);
        let identity = RemoteComputerRuntimeIdentity::new(
            RemoteComputerSubstrate::AgentSandbox,
            "agent-os".to_string(),
            "claim-1".to_string(),
            "pod-1".to_string(),
            Some("claim-1".to_string()),
            Some("sandbox-generated-1".to_string()),
            Some(deadline),
        );
        let metadata =
            metadata_with_remote_computer_runtime_identity(&json!({"on_demand": true}), &identity);

        let decoded = remote_computer_runtime_identity_from_parts(
            &metadata,
            "ignored",
            Some("ignored"),
            Some("ignored"),
            None,
        )
        .expect("runtime identity");

        assert_eq!(decoded, identity);
        assert_eq!(metadata["on_demand"], true);
        assert_eq!(metadata["sandbox_claim_name"], "claim-1");
        assert_eq!(metadata["sandbox_name"], "sandbox-generated-1");
    }

    #[test]
    fn legacy_agent_sandbox_metadata_decodes_without_versioned_identity() {
        let decoded = remote_computer_runtime_identity_from_parts(
            &json!({
                "sandbox_claim_name": "legacy-claim",
                "sandbox_name": "legacy-sandbox",
                "lifecycle_deadline": "2026-07-10T12:00:00Z"
            }),
            "legacy-ns",
            Some("legacy-pod"),
            Some("legacy-record"),
            Some("agent-sandbox"),
        )
        .expect("legacy runtime identity");

        assert_eq!(decoded.version, "v1");
        assert_eq!(decoded.substrate, RemoteComputerSubstrate::AgentSandbox);
        assert_eq!(decoded.namespace, "legacy-ns");
        assert_eq!(decoded.resource_name, "legacy-claim");
        assert_eq!(decoded.claim_name.as_deref(), Some("legacy-claim"));
        assert_eq!(decoded.sandbox_name.as_deref(), Some("legacy-sandbox"));
        assert_eq!(decoded.pod_name, "legacy-pod");
        assert!(decoded.lifecycle_deadline.is_some());
    }

    #[test]
    fn invalid_versioned_identity_does_not_fall_back_to_a_different_substrate() {
        let metadata = json!({
            "runtime_identity": {"substrate": "invalid"},
            "sandbox_claim_name": "claim-1"
        });
        assert!(
            remote_computer_runtime_identity_from_parts(
                &metadata,
                "agent-os",
                Some("pod-1"),
                Some("record-1"),
                None,
            )
            .is_none()
        );
        let remote_computer = RemoteComputer {
            id: uuid::Uuid::new_v4(),
            name: "claim-1".to_string(),
            profile: "agent-sandbox".to_string(),
            status: "attention".to_string(),
            namespace: "agent-os".to_string(),
            pod_name: Some("pod-1".to_string()),
            workspace_path: "/workspace".to_string(),
            state_mount_path: "/agent-state".to_string(),
            metadata,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(
            required_remote_computer_runtime_identity(&remote_computer)
                .expect_err("corrupt identity must fail closed"),
            "Remote Computer runtime_identity metadata is invalid"
        );
    }
}
