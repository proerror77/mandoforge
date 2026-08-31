use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::remote_computer_runner::{
    RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
    remote_computer_runner_for_config,
};
use crate::store_remote_computers::{
    remote_computer_runtime_cleanup_claim_is_active,
    remote_computer_runtime_cleanup_outcome_matches,
};
use crate::{
    AppError, AppState, REMOTE_COMPUTER_RUNTIME_CLEANUP_CLAIM_UNTIL_MARKER,
    REMOTE_COMPUTER_RUNTIME_CLEANUP_MARKER, REMOTE_COMPUTER_RUNTIME_CLEANUP_RETRY_MARKER,
    RemoteComputer, RemoteComputerJobAssignment, RemoteComputerLease, UpdateRemoteComputerLease,
    deterministic_record_id, new_audit_log,
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
            let resource_name = pod_name.clone();
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

pub(crate) async fn delete_remote_computer_runtime_resource(
    identity: &RemoteComputerRuntimeIdentity,
    remote_computer_id: Option<uuid::Uuid>,
    session_id: Option<uuid::Uuid>,
    metadata: Value,
) -> RemoteComputerRunnerDryRunResponse {
    let delete_name = remote_computer_runtime_delete_name(identity);
    let config = remote_computer_runner_config_for_identity(
        RemoteComputerRunnerConfig::from_env(),
        identity,
    );
    remote_computer_runner_for_config(&config)
        .mutate(
            &config,
            RemoteComputerRunnerDryRunRequest {
                operation: Some("live_delete".to_string()),
                remote_computer_id,
                session_id,
                pod_name: Some(delete_name.to_string()),
                metadata: Some(merge_runtime_cleanup_metadata(
                    &metadata,
                    json!({
                        "namespace": identity.namespace,
                        "runtime_substrate": identity.substrate,
                    }),
                )),
            },
        )
        .await
}

fn remote_computer_runtime_delete_name(identity: &RemoteComputerRuntimeIdentity) -> &str {
    match identity.substrate {
        RemoteComputerSubstrate::AgentSandbox => identity
            .claim_name
            .as_deref()
            .unwrap_or(&identity.resource_name),
        RemoteComputerSubstrate::KubernetesPod => &identity.pod_name,
    }
}

fn remote_computer_runner_config_for_identity(
    mut config: RemoteComputerRunnerConfig,
    identity: &RemoteComputerRuntimeIdentity,
) -> RemoteComputerRunnerConfig {
    config.mode = match identity.substrate {
        RemoteComputerSubstrate::AgentSandbox => "agent-sandbox",
        RemoteComputerSubstrate::KubernetesPod => "kubernetes",
    }
    .to_string();
    config.namespace = identity.namespace.clone();
    config
}

pub(crate) async fn cleanup_remote_computer_lease_runtime(
    state: &AppState,
    lease: &RemoteComputerLease,
    assignment: Option<&RemoteComputerJobAssignment>,
    reason: &str,
    assignment_status: &str,
) -> Result<Value, AppError> {
    if !matches!(assignment_status, "canceled" | "released" | "failed") {
        return Err(AppError::internal(
            "Remote Computer cleanup assignment status is invalid",
        ));
    }
    if lease.status != "leased" {
        return replay_converged_remote_computer_cleanup(state, lease).await;
    }
    if let Some(assignment) = assignment
        && (assignment.lease_id != lease.id
            || assignment.remote_computer_id != lease.remote_computer_id)
    {
        return Err(AppError::internal(
            "Remote Computer cleanup assignment does not match its lease",
        ));
    }
    let lease = match state
        .claim_remote_computer_lease_runtime_cleanup(
            lease.id,
            reason,
            assignment_status,
            assignment.map(|assignment| assignment.id),
        )
        .await?
    {
        Some(lease) => lease,
        None => {
            let current = state
                .list_remote_computer_leases()
                .await?
                .into_iter()
                .find(|current| current.id == lease.id)
                .ok_or_else(|| AppError::not_found("Remote computer lease not found"))?;
            if remote_computer_runtime_cleanup_converged(&current) {
                return replay_converged_remote_computer_cleanup(state, &current).await;
            }
            if remote_computer_runtime_cleanup_claim_is_active(&current.metadata, Utc::now())
                && remote_computer_runtime_cleanup_outcome_matches(
                    &current.metadata,
                    assignment_status,
                    assignment.map(|assignment| assignment.id),
                )
            {
                return Ok(json!({
                    "status": "runtime_cleanup_in_progress",
                    "lease_id": current.id,
                    "remote_computer_id": current.remote_computer_id,
                    "lease_status": current.status,
                }));
            }
            return Err(AppError::internal(
                "Remote Computer runtime cleanup is already claimed or no longer active",
            ));
        }
    };

    let remote_computer = state
        .list_remote_computers()
        .await?
        .into_iter()
        .find(|computer| computer.id == lease.remote_computer_id)
        .ok_or_else(|| AppError::not_found("Remote computer not found"))?;
    let on_demand = remote_computer
        .metadata
        .get("on_demand")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || lease
            .metadata
            .get("on_demand")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    let mut runtime_evidence = json!({
        "delete_attempted": false,
        "delete_status": "not_required",
    });
    if on_demand {
        let identity = required_remote_computer_runtime_identity(&remote_computer)
            .map_err(AppError::internal)?;
        let response = delete_remote_computer_runtime_resource(
            &identity,
            Some(remote_computer.id),
            lease.session_id,
            json!({
                "cleanup_reason": reason,
                "lease_id": lease.id,
            }),
        )
        .await;
        runtime_evidence = json!({
            "delete_attempted": response.live_mutation_attempted,
            "delete_status": response.status,
            "delete_status_code": response.live_mutation_status_code,
            "namespace": identity.namespace,
            "resource_name": identity.resource_name,
            "substrate": identity.substrate,
        });
        if response.status != "mutation_ok" {
            let mut retry_metadata = merge_runtime_cleanup_metadata(
                &lease.metadata,
                json!({
                    "runtime_cleanup_retry_reason": reason,
                    "runtime_cleanup_retry_at": Utc::now(),
                    "runtime_cleanup_retry": runtime_evidence,
                    "runtime_cleanup_retry_assignment_status": assignment_status,
                    "runtime_cleanup_assignment_id": assignment.map(|assignment| assignment.id),
                }),
            );
            retry_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_RETRY_MARKER] = Value::Bool(true);
            retry_metadata
                .as_object_mut()
                .expect("merged cleanup metadata is an object")
                .remove(REMOTE_COMPUTER_RUNTIME_CLEANUP_CLAIM_UNTIL_MARKER);
            let retry_lease = state
                .schedule_remote_computer_lease_cleanup_retry(lease.id, retry_metadata)
                .await?;
            record_remote_computer_runtime_cleanup_evidence(
                state,
                &retry_lease,
                assignment,
                reason,
                "failed",
                &runtime_evidence,
            )
            .await?;
            return Err(AppError::internal(format!(
                "Remote Computer runtime cleanup failed: {}",
                response.message
            )));
        }
    }

    let updated_assignment = if let Some(assignment) = assignment {
        if assignment.status == "assigned" {
            Some(
                state
                    .update_remote_computer_job_assignment_status(
                        assignment.id,
                        assignment_status,
                        json!({
                            "runtime_cleanup_reason": reason,
                            "runtime_cleanup_at": Utc::now(),
                            "runtime_cleanup": runtime_evidence,
                        }),
                    )
                    .await?,
            )
        } else {
            Some(assignment.clone())
        }
    } else {
        None
    };

    let lease_status = if on_demand { "failed" } else { "released" };
    let mut lease_metadata = merge_runtime_cleanup_metadata(
        &lease.metadata,
        json!({
            "runtime_cleanup_reason": reason,
            "runtime_cleanup_at": Utc::now(),
            "runtime_cleanup": runtime_evidence,
            "runtime_cleanup_assignment_id": updated_assignment.as_ref().map(|assignment| assignment.id),
        }),
    );
    lease_metadata[REMOTE_COMPUTER_RUNTIME_CLEANUP_MARKER] = Value::Bool(true);
    let metadata = lease_metadata
        .as_object_mut()
        .expect("merged cleanup metadata is an object");
    metadata.remove(REMOTE_COMPUTER_RUNTIME_CLEANUP_RETRY_MARKER);
    metadata.remove(REMOTE_COMPUTER_RUNTIME_CLEANUP_CLAIM_UNTIL_MARKER);
    let updated_lease = state
        .transition_remote_computer_lease_after_runtime_cleanup(
            lease.id,
            lease_status,
            UpdateRemoteComputerLease {
                reason: Some(reason.to_string()),
                metadata: Some(lease_metadata),
            },
        )
        .await?;
    record_remote_computer_lease_runtime_cleaned_event(state, &updated_lease).await?;
    record_remote_computer_runtime_cleanup_evidence(
        state,
        &updated_lease,
        updated_assignment.as_ref(),
        reason,
        "completed",
        &runtime_evidence,
    )
    .await?;

    Ok(json!({
        "status": "completed",
        "lease_id": updated_lease.id,
        "lease_status": updated_lease.status,
        "assignment_id": updated_assignment.as_ref().map(|assignment| assignment.id),
        "assignment_status": updated_assignment.as_ref().map(|assignment| assignment.status.as_str()),
        "remote_computer_id": remote_computer.id,
        "remote_computer_status": if on_demand { "attention" } else { "available" },
        "runtime": runtime_evidence,
    }))
}

async fn replay_converged_remote_computer_cleanup(
    state: &AppState,
    lease: &RemoteComputerLease,
) -> Result<Value, AppError> {
    if !remote_computer_runtime_cleanup_converged(lease) {
        return Err(AppError::internal(
            "Remote Computer lease is terminal without converged runtime cleanup",
        ));
    }
    let reason = lease.metadata["runtime_cleanup_reason"]
        .as_str()
        .filter(|reason| !reason.trim().is_empty())
        .ok_or_else(|| AppError::internal("Remote Computer cleanup reason is missing"))?;
    let assignments = state.list_remote_computer_job_assignments().await?;
    let assignment = match lease.metadata.get("runtime_cleanup_assignment_id") {
        Some(Value::String(assignment_id)) => {
            let assignment_id = uuid::Uuid::parse_str(assignment_id)
                .map_err(|_| AppError::internal("Remote Computer cleanup assignment is invalid"))?;
            Some(
                assignments
                    .iter()
                    .find(|assignment| assignment.id == assignment_id)
                    .ok_or_else(|| {
                        AppError::internal("Remote Computer cleanup assignment is missing")
                    })?,
            )
        }
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(AppError::internal(
                "Remote Computer cleanup assignment is invalid",
            ));
        }
    };
    replay_remote_computer_lease_runtime_cleanup_evidence(state, lease, assignment, reason).await?;
    Ok(json!({
        "status": "already_converged",
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "lease_status": lease.status,
    }))
}

pub(crate) async fn replay_remote_computer_lease_runtime_cleanup_evidence(
    state: &AppState,
    lease: &RemoteComputerLease,
    assignment: Option<&RemoteComputerJobAssignment>,
    reason: &str,
) -> Result<(), AppError> {
    if lease.metadata["runtime_cleanup_reason"].as_str() != Some(reason)
        || !remote_computer_runtime_cleanup_converged(lease)
    {
        return Ok(());
    }
    let runtime = lease
        .metadata
        .get("runtime_cleanup")
        .cloned()
        .unwrap_or_else(|| json!({"delete_attempted": false, "delete_status": "not_required"}));
    record_remote_computer_lease_runtime_cleaned_event(state, lease).await?;
    record_remote_computer_runtime_cleanup_evidence(
        state,
        lease,
        assignment,
        reason,
        "completed",
        &runtime,
    )
    .await
}

async fn record_remote_computer_lease_runtime_cleaned_event(
    state: &AppState,
    lease: &RemoteComputerLease,
) -> Result<(), AppError> {
    let event_type = "remote_computer.lease_runtime_cleaned";
    let details = json!({
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "session_id": lease.session_id,
        "status": lease.status,
        "worker_id": lease.worker_id,
        "lease_expires_at": lease.lease_expires_at,
        "heartbeat_at": lease.heartbeat_at,
        "metadata": lease.metadata,
        "execution_enabled": false
    });
    let event_id = remote_computer_lease_runtime_cleaned_event_id(lease.id);
    if let Some(session_id) = lease.session_id {
        state
            .append_event_once(
                event_id,
                "system",
                None,
                session_id,
                event_type,
                details.clone(),
            )
            .await?;
    }
    let mut audit = new_audit_log(
        lease.session_id,
        "system",
        None,
        event_type,
        "remote_computer_lease",
        Some(lease.id),
        details,
    );
    audit.id = deterministic_record_id(event_id, "audit", &[event_type]);
    state.append_audit_log(audit).await?;
    Ok(())
}

pub(crate) async fn cleanup_remote_computer_session_runtimes(
    state: &AppState,
    session_id: uuid::Uuid,
    reason: &str,
) -> Result<Vec<Value>, AppError> {
    let leases = state.list_remote_computer_leases().await?;
    let assignments = state.list_remote_computer_job_assignments().await?;
    let mut results = Vec::new();
    for lease in leases
        .iter()
        .filter(|lease| lease.session_id == Some(session_id) && lease.status == "leased")
    {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.lease_id == lease.id && assignment.status == "assigned");
        match cleanup_remote_computer_lease_runtime(state, lease, assignment, reason, "released")
            .await
        {
            Ok(result) => results.push(result),
            Err(error) => {
                let retry = state
                    .list_remote_computer_leases()
                    .await?
                    .into_iter()
                    .find(|candidate| candidate.id == lease.id);
                let Some(retry) = retry else {
                    return Err(error);
                };
                let runtime_retry_scheduled = retry.status == "leased"
                    && retry
                        .lease_expires_at
                        .is_some_and(|expires_at| expires_at <= Utc::now())
                    && retry.metadata["runtime_cleanup_retry_reason"].as_str() == Some(reason);
                let evidence_retry_scheduled =
                    matches!(retry.status.as_str(), "released" | "failed")
                        && retry.metadata["runtime_cleanup_reason"].as_str() == Some(reason)
                        && remote_computer_runtime_cleanup_converged(&retry);
                if !runtime_retry_scheduled && !evidence_retry_scheduled {
                    return Err(error);
                }
                tracing::error!(
                    lease_id = %lease.id,
                    error = %error.message,
                    "terminal session runtime cleanup scheduled for immediate retry"
                );
                results.push(json!({
                    "status": if runtime_retry_scheduled {
                        "runtime_retry_scheduled"
                    } else {
                        "evidence_retry_scheduled"
                    },
                    "lease_id": retry.id,
                    "lease_status": retry.status,
                    "lease_expires_at": retry.lease_expires_at,
                }));
            }
        }
    }
    Ok(results)
}

fn merge_runtime_cleanup_metadata(existing: &Value, patch: Value) -> Value {
    let mut merged = existing.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

pub(crate) fn remote_computer_runtime_cleanup_converged(lease: &RemoteComputerLease) -> bool {
    lease
        .metadata
        .get(REMOTE_COMPUTER_RUNTIME_CLEANUP_MARKER)
        .and_then(Value::as_bool)
        == Some(true)
        && matches!(
            lease.metadata["runtime_cleanup"]["delete_status"].as_str(),
            Some("mutation_ok" | "not_required")
        )
}

async fn record_remote_computer_runtime_cleanup_evidence(
    state: &AppState,
    lease: &RemoteComputerLease,
    assignment: Option<&RemoteComputerJobAssignment>,
    reason: &str,
    status: &str,
    runtime: &Value,
) -> Result<(), AppError> {
    let event_type = if status == "completed" {
        "remote_computer.runtime_cleanup_completed"
    } else {
        "remote_computer.runtime_cleanup_failed"
    };
    let details = json!({
        "lease_id": lease.id,
        "remote_computer_id": lease.remote_computer_id,
        "assignment_id": assignment.map(|assignment| assignment.id),
        "session_id": lease.session_id,
        "status": status,
        "reason": reason,
        "runtime": runtime,
    });
    if let Some(session_id) = lease.session_id {
        let event_id = remote_computer_runtime_cleanup_event_id(lease.id, reason, status);
        state
            .append_event_once(
                event_id,
                "system",
                None,
                session_id,
                event_type,
                details.clone(),
            )
            .await?;
    }
    let event_id = remote_computer_runtime_cleanup_event_id(lease.id, reason, status);
    let mut audit = new_audit_log(
        lease.session_id,
        "system",
        None,
        event_type,
        "remote_computer",
        Some(lease.remote_computer_id),
        details,
    );
    audit.id = deterministic_record_id(event_id, "audit", &[event_type]);
    state.append_audit_log(audit).await?;
    Ok(())
}

fn remote_computer_lease_runtime_cleaned_event_id(lease_id: uuid::Uuid) -> uuid::Uuid {
    deterministic_record_id(
        lease_id,
        "remote-computer-lease-event",
        &["remote_computer.lease_runtime_cleaned"],
    )
}

fn remote_computer_runtime_cleanup_event_id(
    lease_id: uuid::Uuid,
    reason: &str,
    status: &str,
) -> uuid::Uuid {
    deterministic_record_id(
        lease_id,
        "remote-computer-runtime-cleanup-event",
        &[reason, status],
    )
}

pub(crate) fn remote_computer_runtime_cleanup_evidence_audit_ids(
    lease_id: uuid::Uuid,
    reason: &str,
) -> [uuid::Uuid; 2] {
    let cleaned_event_id = remote_computer_lease_runtime_cleaned_event_id(lease_id);
    let runtime_event_id = remote_computer_runtime_cleanup_event_id(lease_id, reason, "completed");
    [
        deterministic_record_id(
            cleaned_event_id,
            "audit",
            &["remote_computer.lease_runtime_cleaned"],
        ),
        deterministic_record_id(
            runtime_event_id,
            "audit",
            &["remote_computer.runtime_cleanup_completed"],
        ),
    ]
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

    #[test]
    fn cleanup_runner_config_uses_persisted_identity_after_global_mode_change() {
        let identity = RemoteComputerRuntimeIdentity::new(
            RemoteComputerSubstrate::AgentSandbox,
            "persisted-namespace".to_string(),
            "persisted-claim".to_string(),
            "persisted-pod".to_string(),
            Some("persisted-claim".to_string()),
            Some("persisted-sandbox".to_string()),
            None,
        );
        let config = remote_computer_runner_config_for_identity(
            RemoteComputerRunnerConfig {
                mode: "reserved".to_string(),
                namespace: "new-global-namespace".to_string(),
                pod_template_path: "unused".to_string(),
                service_account: "unused".to_string(),
                kubeconfig_path: None,
                kube_api_url: None,
                bearer_token_path: None,
                in_cluster: false,
                mutation_enabled: true,
                live_mutation_enabled: true,
                execution_enabled: false,
            },
            &identity,
        );

        assert_eq!(config.mode, "agent-sandbox");
        assert_eq!(config.namespace, "persisted-namespace");
        assert!(config.mutation_enabled);
        assert!(config.live_mutation_enabled);
        assert!(!config.execution_enabled);
    }

    #[test]
    fn legacy_kubernetes_pod_identity_uses_pod_name_as_delete_resource() {
        let decoded = remote_computer_runtime_identity_from_parts(
            &json!({"on_demand": true}),
            "legacy-ns",
            Some("actual-legacy-pod"),
            Some("record-display-name"),
            Some("workspace-write"),
        )
        .expect("legacy pod identity");

        assert_eq!(decoded.substrate, RemoteComputerSubstrate::KubernetesPod);
        assert_eq!(decoded.resource_name, "actual-legacy-pod");
        assert_eq!(decoded.pod_name, "actual-legacy-pod");
        assert_eq!(
            remote_computer_runtime_delete_name(&decoded),
            "actual-legacy-pod"
        );
    }
}
