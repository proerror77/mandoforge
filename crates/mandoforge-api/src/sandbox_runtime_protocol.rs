use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const SANDBOX_RUNTIME_PROTOCOL_VERSION: &str = "v1";
#[allow(dead_code)]
pub(crate) const SANDBOX_RUNTIME_EXECUTABLE: &str = "/usr/local/bin/mandoforge-sandbox-runtime";
pub(crate) const SANDBOX_RUNTIME_SUBCOMMAND: &str = "execute-json";
pub(crate) const MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENVIRONMENT_ENTRIES: usize = 128;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
#[allow(dead_code)]
const TRUSTED_AGENT_CLI_DIRECTORIES: [&str; 2] = ["/usr/bin", "/usr/local/bin"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SandboxRuntimeRequest {
    pub(crate) version: String,
    pub(crate) session_id: Uuid,
    pub(crate) workspace_path: String,
    pub(crate) timeout_seconds: u64,
    #[serde(default)]
    pub(crate) environment: BTreeMap<String, String>,
    pub(crate) operation: SandboxRuntimeOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SandboxRuntimeOperation {
    Shell {
        command: String,
    },
    FileWrite {
        path: String,
        content: String,
    },
    Codex {
        task: String,
        sandbox_mode: String,
    },
    AgentCli {
        executable: String,
        #[serde(default)]
        args: Vec<String>,
        task: String,
        profile: String,
    },
}

impl SandboxRuntimeRequest {
    #[allow(dead_code)]
    pub(crate) fn new(
        session_id: Uuid,
        timeout_seconds: u64,
        environment: BTreeMap<String, String>,
        operation: SandboxRuntimeOperation,
    ) -> Self {
        Self {
            version: SANDBOX_RUNTIME_PROTOCOL_VERSION.to_string(),
            session_id,
            workspace_path: format!("/workspace/sessions/{session_id}"),
            timeout_seconds,
            environment,
            operation,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.version != SANDBOX_RUNTIME_PROTOCOL_VERSION {
            return Err("unsupported sandbox runtime protocol version".to_string());
        }
        let expected_workspace = format!("/workspace/sessions/{}", self.session_id);
        if self.workspace_path != expected_workspace {
            return Err(format!(
                "workspace_path must equal /workspace/sessions/<session_id>: expected {expected_workspace}"
            ));
        }
        if !(1..=900).contains(&self.timeout_seconds) {
            return Err("timeout_seconds must be between 1 and 900".to_string());
        }
        validate_environment(&self.environment)?;
        match &self.operation {
            SandboxRuntimeOperation::Shell { command } => {
                validate_text("shell command", command, false)?;
            }
            SandboxRuntimeOperation::FileWrite { path, content } => {
                validate_relative_file_path(path)?;
                validate_text("file content", content, true)?;
            }
            SandboxRuntimeOperation::Codex { task, sandbox_mode } => {
                validate_text("Codex task", task, false)?;
                if !matches!(sandbox_mode.as_str(), "read-only" | "workspace-write") {
                    return Err("Codex sandbox_mode is not supported".to_string());
                }
            }
            SandboxRuntimeOperation::AgentCli {
                executable,
                args,
                task,
                profile,
            } => {
                validate_executable(executable)?;
                validate_arguments(args)?;
                validate_text("agent CLI task", task, false)?;
                if !valid_safe_name(profile) {
                    return Err("agent CLI profile must be an allowlist-safe name".to_string());
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn to_stdin_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self)
            .map_err(|error| format!("failed to encode sandbox runtime request: {error}"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES {
            return Err(format!(
                "sandbox runtime request exceeds {} bytes",
                MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES
            ));
        }
        Ok(bytes)
    }

    #[allow(dead_code)]
    pub(crate) fn operation_name(&self) -> &'static str {
        match self.operation {
            SandboxRuntimeOperation::Shell { .. } => "shell",
            SandboxRuntimeOperation::FileWrite { .. } => "file_write",
            SandboxRuntimeOperation::Codex { .. } => "codex",
            SandboxRuntimeOperation::AgentCli { .. } => "agent_cli",
        }
    }
}

#[allow(dead_code)]
pub(crate) fn parse_sandbox_runtime_request(input: &[u8]) -> Result<SandboxRuntimeRequest, String> {
    if input.is_empty() {
        return Err("sandbox runtime request is empty".to_string());
    }
    if input.len() > MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES {
        return Err(format!(
            "sandbox runtime request exceeds {} bytes",
            MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES
        ));
    }
    let request: SandboxRuntimeRequest = serde_json::from_slice(input)
        .map_err(|error| format!("invalid sandbox runtime request JSON: {error}"))?;
    request.validate()?;
    Ok(request)
}

fn validate_environment(environment: &BTreeMap<String, String>) -> Result<(), String> {
    if environment.len() > MAX_ENVIRONMENT_ENTRIES {
        return Err(format!(
            "sandbox runtime environment exceeds {MAX_ENVIRONMENT_ENTRIES} entries"
        ));
    }
    for (key, value) in environment {
        if !valid_environment_key(key) {
            return Err(format!("invalid sandbox runtime environment key: {key}"));
        }
        if launcher_owned_environment_key(key) {
            return Err(format!(
                "sandbox runtime environment key is launcher-owned: {key}"
            ));
        }
        if value.contains('\0') {
            return Err(format!(
                "sandbox runtime environment value contains a null byte: {key}"
            ));
        }
        if value.len() > MAX_TEXT_BYTES {
            return Err(format!(
                "sandbox runtime environment value is too large: {key}"
            ));
        }
    }
    Ok(())
}

fn valid_environment_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn launcher_owned_environment_key(key: &str) -> bool {
    matches!(
        key,
        "HOME" | "PWD" | "PATH" | "CARGO_TARGET_DIR" | "MANDOFORGE_SESSION_ID" | "RUSTC_WRAPPER"
    ) || key.starts_with("LD_")
        || key.starts_with("DYLD_")
}

fn validate_relative_file_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() {
        return Err("file_write path must be a non-empty relative path".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("file_write path must stay inside the session workspace".to_string());
    }
    Ok(())
}

fn validate_executable(executable: &str) -> Result<(), String> {
    let safe_basename = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
    };
    if safe_basename(executable) {
        return Ok(());
    }
    let path = Path::new(executable);
    if path
        .parent()
        .and_then(Path::to_str)
        .is_some_and(|parent| TRUSTED_AGENT_CLI_DIRECTORIES.contains(&parent))
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(safe_basename)
    {
        return Ok(());
    }
    Err("agent CLI executable must be a safe basename or trusted absolute path".to_string())
}

#[allow(dead_code)]
pub(crate) fn normalize_agent_cli_executable(executable: &str) -> Result<String, String> {
    if validate_executable(executable).is_ok() {
        let path = Path::new(executable);
        if path.is_absolute() {
            let basename = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("validated executable basename");
            return Ok(format!("/usr/local/bin/{basename}"));
        }
        return Ok(executable.to_string());
    }
    Err("agent CLI executable must be a safe basename or a trusted /usr binary path".to_string())
}

fn validate_arguments(arguments: &[String]) -> Result<(), String> {
    if arguments.len() > MAX_ARGUMENTS {
        return Err(format!(
            "agent CLI arguments exceed {MAX_ARGUMENTS} entries"
        ));
    }
    if arguments.iter().any(|argument| argument.contains('\0')) {
        return Err("agent CLI argument contains a null byte".to_string());
    }
    if arguments
        .iter()
        .any(|argument| argument.len() > MAX_ARGUMENT_BYTES)
    {
        return Err(format!(
            "agent CLI argument exceeds {MAX_ARGUMENT_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, allow_empty: bool) -> Result<(), String> {
    if !allow_empty && value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(format!("{label} exceeds {MAX_TEXT_BYTES} bytes"));
    }
    if value.contains('\0') {
        return Err(format!("{label} contains a null byte"));
    }
    Ok(())
}

fn valid_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_runtime_request_requires_exact_session_workspace() {
        let session_id = Uuid::new_v4();
        let mut request = SandboxRuntimeRequest::new(
            session_id,
            30,
            BTreeMap::new(),
            SandboxRuntimeOperation::Shell {
                command: "pwd".to_string(),
            },
        );
        request.workspace_path = format!("/workspace/sessions/{}/../other", session_id);

        assert!(request.validate().is_err());
    }

    #[test]
    fn sandbox_runtime_request_rejects_path_escape_and_launcher_env_override() {
        let session_id = Uuid::new_v4();
        let mut environment = BTreeMap::new();
        environment.insert("CARGO_TARGET_DIR".to_string(), "/shared/target".to_string());
        let request = SandboxRuntimeRequest::new(
            session_id,
            30,
            environment,
            SandboxRuntimeOperation::FileWrite {
                path: "../secret".to_string(),
                content: "secret".to_string(),
            },
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn sandbox_runtime_request_round_trips_as_one_newline_terminated_envelope() {
        let request = SandboxRuntimeRequest::new(
            Uuid::new_v4(),
            30,
            BTreeMap::new(),
            SandboxRuntimeOperation::Codex {
                task: "inspect the repository".to_string(),
                sandbox_mode: "workspace-write".to_string(),
            },
        );

        let bytes = request.to_stdin_bytes().expect("stdin envelope");
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(
            parse_sandbox_runtime_request(&bytes).expect("parsed request"),
            request
        );
    }

    #[test]
    fn sandbox_runtime_request_rejects_unknown_fields_and_launcher_owned_cache_wrapper() {
        let session_id = Uuid::new_v4();
        let unknown = format!(
            r#"{{"version":"v1","session_id":"{session_id}","workspace_path":"/workspace/sessions/{session_id}","timeout_seconds":30,"environment":{{}},"operation":{{"type":"shell","command":"pwd"}},"unexpected":true}}"#
        );
        assert!(parse_sandbox_runtime_request(unknown.as_bytes()).is_err());

        let mut environment = BTreeMap::new();
        environment.insert("RUSTC_WRAPPER".to_string(), "disabled".to_string());
        let request = SandboxRuntimeRequest::new(
            session_id,
            30,
            environment,
            SandboxRuntimeOperation::Shell {
                command: "cargo check".to_string(),
            },
        );
        assert!(request.validate().is_err());
    }

    #[test]
    fn sandbox_runtime_normalizes_only_trusted_absolute_agent_cli_paths() {
        assert_eq!(
            normalize_agent_cli_executable("/usr/bin/codex").expect("trusted path"),
            "/usr/local/bin/codex"
        );
        assert_eq!(
            normalize_agent_cli_executable("/usr/local/bin/claude").expect("trusted path"),
            "/usr/local/bin/claude"
        );
        assert_eq!(
            normalize_agent_cli_executable("opencode").expect("safe basename"),
            "opencode"
        );
        assert!(normalize_agent_cli_executable("/tmp/custom-agent").is_err());
        assert!(normalize_agent_cli_executable("../custom-agent").is_err());
    }

    #[test]
    fn sandbox_runtime_reserves_path_and_dynamic_loader_environment() {
        for key in [
            "PATH",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
        ] {
            let mut environment = BTreeMap::new();
            environment.insert(key.to_string(), "untrusted".to_string());
            let request = SandboxRuntimeRequest::new(
                Uuid::new_v4(),
                30,
                environment,
                SandboxRuntimeOperation::AgentCli {
                    executable: "/usr/local/bin/codex".to_string(),
                    args: Vec::new(),
                    task: "inspect".to_string(),
                    profile: "codex".to_string(),
                },
            );
            assert!(request.validate().is_err(), "{key} must be launcher-owned");
        }
    }
}
