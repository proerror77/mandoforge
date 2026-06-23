use std::path::Path;

use crate::{AgentRuntimeProfile, AgentRuntimeProfileReleaseGate};

pub(crate) fn evaluate_agent_runtime_profile_release_gate(
    profile: &AgentRuntimeProfile,
) -> AgentRuntimeProfileReleaseGate {
    let runtime_type_supported = supported_agent_runtime_profile_types()
        .iter()
        .any(|runtime_type| runtime_type == &profile.runtime_type);
    let requires_managed_profile =
        agent_runtime_profile_requires_managed_gate(&profile.runtime_type);
    let allowed_commands = agent_runtime_profile_allowed_commands(&profile.runtime_type)
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let runtime_allowlisted = if profile.runtime_type == "agent_cli" {
        true
    } else {
        runtime_type_supported
    };
    let command_allowlisted = if profile.runtime_type == "hosted" {
        false
    } else if allowed_commands.is_empty() {
        profile.runtime_type == "agent_cli"
    } else {
        agent_runtime_profile_command_allowlisted(&profile.command, &allowed_commands)
    };
    let mut blocking_reasons = Vec::new();
    if !runtime_type_supported {
        blocking_reasons.push(format!(
            "runtime type is not supported: {}",
            profile.runtime_type
        ));
    }
    if profile.status != "enabled" {
        blocking_reasons.push(format!("profile status is {}", profile.status));
    }
    if requires_managed_profile && !runtime_allowlisted {
        blocking_reasons.push(format!(
            "runtime type is not in the managed runtime allowlist: {}",
            profile.runtime_type
        ));
    }
    if requires_managed_profile && !command_allowlisted {
        blocking_reasons.push(format!(
            "command is not allowlisted for runtime type {}",
            profile.runtime_type
        ));
    }
    if profile.runtime_type == "hosted" {
        blocking_reasons.push(
            "hosted runtimes are reserved until a production hosted-runtime policy is installed"
                .to_string(),
        );
    }
    let release_state = if blocking_reasons.is_empty() {
        "passed"
    } else {
        "blocked"
    }
    .to_string();
    AgentRuntimeProfileReleaseGate {
        profile_id: profile.id,
        name: profile.name.clone(),
        runtime_type: profile.runtime_type.clone(),
        command: profile.command.clone(),
        status: profile.status.clone(),
        release_state,
        fail_closed: !blocking_reasons.is_empty(),
        requires_managed_profile,
        runtime_type_supported,
        runtime_allowlisted,
        command_allowlisted,
        remote_computer_required: profile.remote_computer_required,
        allowed_commands,
        blocking_reasons,
    }
}

fn supported_agent_runtime_profile_types() -> &'static [&'static str] {
    &[
        "agent_cli",
        "codex_cli",
        "codex_app_server",
        "claude_code",
        "gemini",
        "opencode",
        "aider",
        "hosted",
    ]
}

fn agent_runtime_profile_requires_managed_gate(runtime_type: &str) -> bool {
    matches!(
        runtime_type,
        "codex_cli"
            | "codex_app_server"
            | "claude_code"
            | "gemini"
            | "opencode"
            | "aider"
            | "hosted"
    )
}

fn agent_runtime_profile_allowed_commands(runtime_type: &str) -> Vec<&'static str> {
    match runtime_type {
        "codex_cli" => vec!["codex"],
        "codex_app_server" => vec!["codex-app-server", "codex_app_server"],
        "claude_code" => vec!["claude", "claude-code", "claude_code"],
        "gemini" => vec!["gemini", "gemini-cli"],
        "opencode" => vec!["opencode"],
        "aider" => vec!["aider"],
        _ => Vec::new(),
    }
}

fn agent_runtime_profile_command_allowlisted(command: &str, allowed_commands: &[String]) -> bool {
    let command = command.split_whitespace().next().unwrap_or_default();
    let basename = Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command);
    allowed_commands
        .iter()
        .any(|allowed| allowed == command || allowed == basename)
}
