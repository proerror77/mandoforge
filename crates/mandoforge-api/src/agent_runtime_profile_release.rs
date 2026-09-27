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
        "grok_cli",
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
            | "grok_cli"
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
        "grok_cli" => vec!["grok"],
        _ => Vec::new(),
    }
}

pub(crate) fn validate_grok_cli_profile_contract(
    profile: &AgentRuntimeProfile,
) -> Result<(), crate::AppError> {
    if profile.status != "enabled" || profile.runtime_type != "grok_cli" {
        return Ok(());
    }
    validate_grok_cli_default_args(&profile.default_args)
}

pub(crate) fn validate_grok_cli_default_args(args: &[String]) -> Result<(), crate::AppError> {
    reject_grok_cli_permission_bypass(args)?;
    if grok_output_format(args) != Some("streaming-messages-json") {
        return Err(crate::AppError::bad_request(
            "grok_cli default_args must set --output-format streaming-messages-json",
        ));
    }
    if args.iter().any(|arg| arg == "--include-partial-messages") {
        return Err(crate::AppError::bad_request(
            "grok_cli default_args must not include --include-partial-messages",
        ));
    }
    match args.last().map(String::as_str) {
        Some("-p" | "--single") => Ok(()),
        _ => Err(crate::AppError::bad_request(
            "grok_cli default_args must end with -p or --single so the task is the headless prompt",
        )),
    }
}

pub(crate) fn grok_cli_launch_args(
    profile_args: &[String],
    request_args: &[String],
) -> Result<Vec<String>, crate::AppError> {
    validate_grok_cli_default_args(profile_args)?;
    reject_grok_cli_permission_bypass(request_args)?;
    if request_args.iter().any(grok_prompt_flag) {
        return Err(crate::AppError::bad_request(
            "grok_cli request args cannot set another prompt; the task is the headless prompt",
        ));
    }
    let prompt_flag = profile_args.last().cloned().ok_or_else(|| {
        crate::AppError::bad_request(
            "grok_cli default_args must end with -p or --single so the task is the headless prompt",
        )
    })?;
    let mut args = profile_args[..profile_args.len() - 1].to_vec();
    args.extend(request_args.iter().cloned());
    args.push(prompt_flag);
    Ok(args)
}

fn grok_prompt_flag(arg: &String) -> bool {
    matches!(
        arg.as_str(),
        "-p" | "--single" | "--prompt-file" | "--prompt-json"
    ) || arg.starts_with("--prompt-file=")
        || arg.starts_with("--prompt-json=")
        || arg.starts_with("--single=")
}

fn reject_grok_cli_permission_bypass(args: &[String]) -> Result<(), crate::AppError> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--yolo" || arg == "--always-approve" {
            return Err(crate::AppError::bad_request(
                "grok_cli cannot enable --yolo or --always-approve; business actions stay on the MandoForge approval path",
            ));
        }
        let mode = if arg == "--permission-mode" {
            args.next().map(String::as_str)
        } else {
            arg.strip_prefix("--permission-mode=")
        };
        if mode == Some("bypassPermissions") {
            return Err(crate::AppError::bad_request(
                "grok_cli cannot set --permission-mode bypassPermissions; business actions stay on the MandoForge approval path",
            ));
        }
    }
    Ok(())
}

fn grok_output_format(args: &[String]) -> Option<&str> {
    let mut args = args.iter();
    let mut format = None;
    while let Some(arg) = args.next() {
        if arg == "--output-format" {
            format = args.next().map(String::as_str);
        } else if let Some(value) = arg.strip_prefix("--output-format=") {
            format = Some(value);
        }
    }
    format
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_cli_launch_keeps_resume_before_the_headless_prompt_flag() {
        let args = grok_cli_launch_args(
            &[
                "--output-format".to_string(),
                "streaming-messages-json".to_string(),
                "--permission-mode".to_string(),
                "acceptEdits".to_string(),
                "-p".to_string(),
            ],
            &["--resume".to_string(), "session-1".to_string()],
        )
        .expect("launch args");

        assert_eq!(
            args,
            vec![
                "--output-format",
                "streaming-messages-json",
                "--permission-mode",
                "acceptEdits",
                "--resume",
                "session-1",
                "-p",
            ]
        );
    }

    #[test]
    fn grok_cli_rejects_permission_bypass_and_partial_stream() {
        let bypass = validate_grok_cli_default_args(&[
            "--output-format".to_string(),
            "streaming-messages-json".to_string(),
            "--yolo".to_string(),
            "-p".to_string(),
        ]);
        assert!(bypass.is_err());

        let partial = validate_grok_cli_default_args(&[
            "--output-format".to_string(),
            "streaming-messages-json".to_string(),
            "--include-partial-messages".to_string(),
            "-p".to_string(),
        ]);
        assert!(partial.is_err());
    }
}
