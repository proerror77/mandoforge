use serde_json::{Value, json};

use crate::remote_computer_runner::{
    RemoteComputerRunnerConfig, RemoteComputerRunnerDryRunRequest,
    RemoteComputerRunnerDryRunResponse, RemoteComputerRunnerReadiness,
    remote_computer_runner_for_config,
};

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
