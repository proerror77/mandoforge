use std::{
    path::{Component, Path as FsPath, PathBuf},
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use uuid::Uuid;

use crate::execution_queue::ExecutionJobRequest;
use crate::shell_runner::{shell_command, shell_runner};
use crate::{AppError, AppState, Approval, Artifact, ToolCall, new_audit_log};

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct CodexRequest {
    task: String,
    #[serde(default = "default_sandbox")]
    sandbox_mode: String,
}

#[allow(dead_code)]
fn default_sandbox() -> String {
    "workspace-write".to_string()
}

pub(crate) async fn execute_approved_tool(
    state: &AppState,
    approval: &Approval,
) -> Result<(), AppError> {
    let Some(tool_call_id) = approval.tool_call_id else {
        return Ok(());
    };
    let tool_call = state.get_tool_call(tool_call_id).await?;
    let job = state
        .execution_queue
        .enqueue(ExecutionJobRequest {
            session_id: approval.session_id,
            approval_id: approval.id,
            tool_call_id,
            tool_name: tool_call.tool_name.clone(),
        })
        .await;
    state.execution_queue.start(job.id).await?;
    let result = match tool_call.tool_name.as_str() {
        "file.write" => execute_approved_file_write(state, approval, &tool_call).await,
        "shell.exec" => execute_approved_shell(state, approval, &tool_call).await,
        "codex.exec" => execute_approved_codex(state, approval, &tool_call).await,
        _ => {
            state
                .update_tool_call_status(
                    tool_call_id,
                    "completed",
                    Some(json!({"approval": "approved"})),
                    None,
                )
                .await?;
            Ok(())
        }
    };
    if result.is_ok() {
        state.execution_queue.complete(job.id).await?;
    } else {
        state.execution_queue.fail(job.id).await?;
    }
    result
}

async fn execute_approved_file_write(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let relative_path = tool_call
        .args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("diagnostics.md");
    let content = tool_call
        .args
        .get("content")
        .or_else(|| tool_call.args.get("markdown"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let workspace = session_workspace(state, approval.session_id).await?;
    let output_path = safe_workspace_path(&workspace, relative_path)?;
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&output_path, content).await?;

    let result = json!({
        "approval": "approved",
        "path": relative_path,
        "bytes": content.len(),
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    let artifact = Artifact {
        id: Uuid::new_v4(),
        session_id: approval.session_id,
        artifact_type: "file".to_string(),
        name: FsPath::new(relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(relative_path)
            .to_string(),
        path: Some(relative_path.to_string()),
        content: json!({"text": content}),
        created_at: Utc::now(),
    };
    let artifact = state.insert_artifact(artifact).await?;
    state
        .append_event(
            "system",
            Some(artifact.id),
            approval.session_id,
            "artifact.created",
            json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
        )
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "path": relative_path, "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_shell(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let command = tool_call
        .args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad_request("shell.exec requires command"))?;
    let workspace = session_workspace(state, approval.session_id).await?;
    let runner = shell_runner();
    let mut process = shell_command(&runner, &workspace, command);
    let output = tokio::time::timeout(Duration::from_secs(30), process.output())
        .await
        .map_err(|_| AppError::bad_request("shell.exec timed out"))??;
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout), limit);
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr), limit);

    let result = json!({
        "approval": "approved",
        "command": command,
        "runner": runner,
        "workspace": workspace.display().to_string(),
        "exit_code": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated,
    });
    state
        .append_event(
            "tool",
            Some(tool_call.id),
            approval.session_id,
            "tool.result",
            json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
        )
        .await?;
    state
        .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
        .await?;
    state
        .append_audit_log(new_audit_log(
            Some(approval.session_id),
            "tool",
            Some(tool_call.id),
            "tool.completed",
            "tool_call",
            Some(tool_call.id),
            json!({"tool": tool_call.tool_name, "command": command, "runner": runner, "exit_code": output.status.code(), "resumed_after_approval": true}),
        ))
        .await?;
    Ok(())
}

async fn execute_approved_codex(
    state: &AppState,
    approval: &Approval,
    tool_call: &ToolCall,
) -> Result<(), AppError> {
    let request: CodexRequest = serde_json::from_value(tool_call.args.clone())?;
    match run_codex(state, approval.session_id, request).await {
        Ok(result) => {
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.result",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": result}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "completed", Some(result.clone()), None)
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.completed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "resumed_after_approval": true}),
                ))
                .await?;
            Ok(())
        }
        Err(error) => {
            let error_payload = json!({"error": error.message.clone()});
            state
                .append_event(
                    "tool",
                    Some(tool_call.id),
                    approval.session_id,
                    "tool.error",
                    json!({"tool_call_id": tool_call.id, "tool": tool_call.tool_name, "content": error_payload}),
                )
                .await?;
            state
                .update_tool_call_status(tool_call.id, "failed", None, Some(error_payload.clone()))
                .await?;
            state
                .append_audit_log(new_audit_log(
                    Some(approval.session_id),
                    "tool",
                    Some(tool_call.id),
                    "tool.failed",
                    "tool_call",
                    Some(tool_call.id),
                    json!({"tool": tool_call.tool_name, "error": error_payload, "resumed_after_approval": true}),
                ))
                .await?;
            Err(error)
        }
    }
}

async fn session_workspace(state: &AppState, session_id: Uuid) -> Result<PathBuf, AppError> {
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    Ok(workspace)
}

fn safe_workspace_path(workspace: &FsPath, relative_path: &str) -> Result<PathBuf, AppError> {
    let path = FsPath::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(AppError::bad_request(
            "file.write path must stay inside the session workspace",
        ));
    }
    Ok(workspace.join(path))
}

#[allow(dead_code)]
async fn run_codex(
    state: &AppState,
    session_id: Uuid,
    request: CodexRequest,
) -> Result<Value, AppError> {
    if request.sandbox_mode != "read-only" && request.sandbox_mode != "workspace-write" {
        return Err(AppError::bad_request(
            "codex sandbox mode requires approval",
        ));
    }
    let workspace = state.workspace_root.join(session_id.to_string());
    tokio::fs::create_dir_all(&workspace).await?;
    let last_message = workspace.join("last_message.md");

    state
        .append_event(
            "tool",
            None,
            session_id,
            "codex.task.started",
            json!({"task": request.task, "sandbox_mode": request.sandbox_mode, "workspace": workspace}),
        )
        .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(180),
        Command::new("codex")
            .arg("exec")
            .arg("--sandbox")
            .arg(&request.sandbox_mode)
            .arg("--json")
            .arg("--output-last-message")
            .arg(&last_message)
            .arg("--cd")
            .arg(&workspace)
            .arg(&request.task)
            .output(),
    )
    .await
    .map_err(|_| AppError::bad_request("codex exec timed out"))??;

    let stdout_full = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_full = String::from_utf8_lossy(&output.stderr).to_string();
    let final_message = tokio::fs::read_to_string(&last_message)
        .await
        .unwrap_or_default();
    for event in parse_codex_jsonl(&stdout_full) {
        state
            .append_event(
                "tool",
                None,
                session_id,
                "codex.event",
                json!({"codex_event_type": codex_jsonl_event_type(&event), "event": event}),
            )
            .await?;
    }
    let limit = execution_output_limit_bytes();
    let stdout = truncate_output(&stdout_full, limit);
    let stderr = truncate_output(&stderr_full, limit);
    let final_output = truncate_output(&final_message, limit);
    if !final_message.trim().is_empty() {
        let artifact = Artifact {
            id: Uuid::new_v4(),
            session_id,
            artifact_type: "markdown".to_string(),
            name: "codex-final-message.md".to_string(),
            path: Some("codex-final-message.md".to_string()),
            content: json!({
                "markdown": final_output.text.clone(),
                "markdown_bytes": final_output.original_bytes,
                "markdown_truncated": final_output.truncated
            }),
            created_at: Utc::now(),
        };
        let artifact = state.insert_artifact(artifact).await?;
        state
            .append_event(
                "system",
                Some(artifact.id),
                session_id,
                "artifact.created",
                json!({"artifact_id": artifact.id, "name": artifact.name, "path": artifact.path, "artifact_type": artifact.artifact_type}),
            )
            .await?;
    }
    let event_type = if output.status.success() {
        "codex.task.completed"
    } else {
        "codex.task.failed"
    };
    state
        .append_event(
            "tool",
            None,
            session_id,
            event_type,
            json!({
                "exit_code": output.status.code(),
                "stdout": stdout.text,
                "stdout_bytes": stdout.original_bytes,
                "stdout_truncated": stdout.truncated,
                "stderr": stderr.text,
                "stderr_bytes": stderr.original_bytes,
                "stderr_truncated": stderr.truncated,
                "final_message": final_output.text,
                "final_message_bytes": final_output.original_bytes,
                "final_message_truncated": final_output.truncated
            }),
        )
        .await?;
    Ok(json!({
        "status": output.status.code(),
        "stdout": stdout.text,
        "stdout_bytes": stdout.original_bytes,
        "stdout_truncated": stdout.truncated,
        "stderr": stderr.text,
        "stderr_bytes": stderr.original_bytes,
        "stderr_truncated": stderr.truncated,
        "final_message": final_output.text,
        "final_message_bytes": final_output.original_bytes,
        "final_message_truncated": final_output.truncated
    }))
}

pub(crate) fn parse_codex_jsonl(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(line).ok()
        })
        .collect()
}

pub(crate) fn codex_jsonl_event_type(event: &Value) -> String {
    event
        .get("type")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn execution_output_limit_bytes() -> usize {
    std::env::var("MANDOFORGE_EXECUTION_OUTPUT_LIMIT_BYTES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_OUTPUT_LIMIT_BYTES))
        .unwrap_or(DEFAULT_OUTPUT_LIMIT_BYTES)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TruncatedOutput {
    pub(crate) text: String,
    pub(crate) original_bytes: usize,
    pub(crate) truncated: bool,
}

pub(crate) fn truncate_output(value: &str, max_bytes: usize) -> TruncatedOutput {
    let original_bytes = value.len();
    if original_bytes <= max_bytes {
        return TruncatedOutput {
            text: value.to_string(),
            original_bytes,
            truncated: false,
        };
    }

    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    TruncatedOutput {
        text: value[..boundary].to_string(),
        original_bytes,
        truncated: true,
    }
}
