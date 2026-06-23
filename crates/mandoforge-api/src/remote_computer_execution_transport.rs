use crate::{AppError, AppState, RemoteComputerExecutionTransportReadiness, env_bool};

pub(crate) async fn build_remote_computer_execution_transport_readiness(
    state: &AppState,
) -> Result<RemoteComputerExecutionTransportReadiness, AppError> {
    let assignments = state.list_remote_computer_job_assignments().await?;
    let assignment_count = assignments.len();
    let active_assignment_count = assignments
        .iter()
        .filter(|assignment| assignment.status == "assigned")
        .count();
    let mode = std::env::var("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "reserved".to_string());
    let requested_execution_enabled = env_bool("MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED");
    let (execution_enabled, status) =
        remote_computer_execution_transport_state(&mode, requested_execution_enabled);
    let required_implementation = if execution_enabled {
        Vec::new()
    } else {
        vec![
            "automatic artifact discovery sidecar ID injection and production supervision"
                .to_string(),
        ]
    };
    let message = if execution_enabled {
        "Remote Computer assigned-Pod execution is enabled for approved file.write, shell.exec, and codex.exec jobs through the worker, policy, approval, event, and audit paths"
    } else {
        "Remote Computer runner can plan assigned Pod execution fail-closed; enable Kubernetes transport gates only after runner, state sync, and sidecar evidence are ready"
    };
    Ok(RemoteComputerExecutionTransportReadiness {
        mode,
        requested_execution_enabled,
        execution_enabled,
        status: status.to_string(),
        assignment_count,
        active_assignment_count,
        supported_operations: vec![
            "plan_pod_exec_intent".to_string(),
            "runner_live_exec_websocket".to_string(),
            "assigned_file_write".to_string(),
            "assigned_shell_exec".to_string(),
            "assigned_codex_exec".to_string(),
            "cancel_assigned_pod_exec".to_string(),
            "push_remote_artifacts_to_artifact_store".to_string(),
            "discover_remote_artifacts_from_shared_workspace".to_string(),
            "audit_handoff".to_string(),
            "fail_closed".to_string(),
        ],
        required_implementation,
        message: message.to_string(),
    })
}

pub(crate) fn remote_computer_execution_transport_state(
    mode: &str,
    requested_execution_enabled: bool,
) -> (bool, &'static str) {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    if requested_execution_enabled && matches!(normalized_mode.as_str(), "kubernetes" | "k8s") {
        (true, "enabled")
    } else if matches!(normalized_mode.as_str(), "kubernetes" | "k8s") {
        (false, "blocked")
    } else {
        (false, "reserved")
    }
}
