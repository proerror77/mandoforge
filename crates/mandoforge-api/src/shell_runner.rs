use std::path::Path as FsPath;

use tokio::process::Command;

pub(crate) fn shell_runner() -> String {
    std::env::var("MANDOFORGE_SHELL_RUNNER")
        .unwrap_or_else(|_| "host".to_string())
        .trim()
        .to_string()
}

pub(crate) fn shell_command(runner: &str, workspace: &FsPath, command: &str) -> Command {
    if runner == "docker" {
        let image = std::env::var("MANDOFORGE_SHELL_DOCKER_IMAGE")
            .unwrap_or_else(|_| "alpine:3.20".to_string());
        let mut process = Command::new("docker");
        process.args(docker_shell_args(workspace, &image, command));
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command).current_dir(workspace);
        process
    }
}

pub(crate) fn docker_shell_args(workspace: &FsPath, image: &str, command: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "--rm".to_string(),
        "--network".to_string(),
        "none".to_string(),
        "--cpus".to_string(),
        "1".to_string(),
        "--memory".to_string(),
        "512m".to_string(),
        "-v".to_string(),
        format!("{}:/workspace", workspace.display()),
        "-w".to_string(),
        "/workspace".to_string(),
        image.to_string(),
        "sh".to_string(),
        "-lc".to_string(),
        command.to_string(),
    ]
}
