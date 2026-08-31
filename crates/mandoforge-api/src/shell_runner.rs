use std::{
    io,
    path::Path as FsPath,
    process::{Output, Stdio},
    time::Duration,
};

use tokio::process::Command;
use uuid::Uuid;

use crate::process_control::{configure_process_group, terminate_process_group};

const DOCKER_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn shell_runner() -> String {
    std::env::var("MANDOFORGE_SHELL_RUNNER")
        .unwrap_or_else(|_| "host".to_string())
        .trim()
        .to_string()
}

#[cfg(test)]
pub(crate) fn shell_command(
    runner: &str,
    workspace: &FsPath,
    command: &str,
) -> std::io::Result<Command> {
    shell_command_with_docker_name(runner, workspace, command, None)
}

fn shell_command_with_docker_name(
    runner: &str,
    workspace: &FsPath,
    command: &str,
    docker_name: Option<&str>,
) -> std::io::Result<Command> {
    let workspace = std::path::absolute(workspace)?;
    match runner {
        "docker" => {
            let image = std::env::var("MANDOFORGE_SHELL_DOCKER_IMAGE")
                .unwrap_or_else(|_| "alpine:3.20".to_string());
            let mut process = Command::new("docker");
            process.kill_on_drop(true);
            process.args(docker_shell_args_with_name(
                &workspace,
                &image,
                command,
                docker_name,
            ));
            Ok(process)
        }
        // bubblewrap: Linux user-namespaces sandbox. Near-zero cold-start (<10ms).
        // Requires `bwrap` (bubblewrap) installed on the host.
        // Set MANDOFORGE_SHELL_RUNNER=bubblewrap to enable.
        "bubblewrap" | "bwrap" => {
            let mut process = Command::new("bwrap");
            process.kill_on_drop(true);
            process.args(bubblewrap_args(&workspace, command));
            Ok(process)
        }
        // nsjail: Google's lightweight namespace jail. Fast (<50ms), no daemon needed.
        // Requires `nsjail` installed. Set MANDOFORGE_SHELL_RUNNER=nsjail to enable.
        "nsjail" => {
            let mut process = Command::new("nsjail");
            process.kill_on_drop(true);
            process.args(nsjail_args(&workspace, command));
            Ok(process)
        }
        // host: no sandboxing, runs directly on the host shell.
        // Fastest option — use only for trusted agent workloads on your own infra.
        _ => {
            let mut process = Command::new("sh");
            process.kill_on_drop(true);
            process.arg("-c").arg(command).current_dir(&workspace);
            Ok(process)
        }
    }
}

pub(crate) async fn run_shell_command(
    runner: &str,
    workspace: &FsPath,
    command: &str,
    timeout: Duration,
) -> io::Result<Option<Output>> {
    let docker_name =
        (runner == "docker").then(|| format!("mandoforge-shell-{}", Uuid::new_v4().simple()));
    let mut process =
        shell_command_with_docker_name(runner, workspace, command, docker_name.as_deref())?;
    let host_runner = !matches!(runner, "docker" | "bubblewrap" | "bwrap" | "nsjail");
    if host_runner {
        configure_process_group(&mut process);
    }
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = process.spawn()?;
    let cleanup = if let Some(name) = docker_name {
        ShellCleanupKind::DockerContainer(name)
    } else if host_runner {
        ShellCleanupKind::ProcessGroup(
            child
                .id()
                .ok_or_else(|| io::Error::other("shell runner process id is unavailable"))?,
        )
    } else {
        ShellCleanupKind::None
    };
    let mut cleanup = ShellCleanup { kind: cleanup };
    let result = tokio::time::timeout(timeout, child.wait_with_output()).await;
    let force_docker_cleanup = !matches!(&result, Ok(Ok(_)));
    cleanup.finish(force_docker_cleanup).await?;
    match result {
        Ok(output) => output.map(Some),
        Err(_) => Ok(None),
    }
}

#[derive(Clone)]
enum ShellCleanupKind {
    None,
    ProcessGroup(u32),
    DockerContainer(String),
}

struct ShellCleanup {
    kind: ShellCleanupKind,
}

impl ShellCleanup {
    async fn finish(&mut self, force_docker_cleanup: bool) -> io::Result<()> {
        match self.kind.clone() {
            ShellCleanupKind::None => {}
            ShellCleanupKind::ProcessGroup(process_group_id) => {
                terminate_process_group(process_group_id);
            }
            ShellCleanupKind::DockerContainer(name) if force_docker_cleanup => {
                force_remove_docker_container(&name).await?;
            }
            ShellCleanupKind::DockerContainer(_) => {}
        }
        self.kind = ShellCleanupKind::None;
        Ok(())
    }
}

impl Drop for ShellCleanup {
    fn drop(&mut self) {
        match &self.kind {
            ShellCleanupKind::None => {}
            ShellCleanupKind::ProcessGroup(process_group_id) => {
                terminate_process_group(*process_group_id);
            }
            ShellCleanupKind::DockerContainer(name) => {
                let _ = std::process::Command::new("docker")
                    .args(["rm", "-f", name])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
    }
}

async fn force_remove_docker_container(name: &str) -> io::Result<()> {
    let mut process = Command::new("docker");
    process
        .kill_on_drop(true)
        .args(["rm", "-f", name])
        .stdin(Stdio::null());
    let output = tokio::time::timeout(DOCKER_CLEANUP_TIMEOUT, process.output())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Docker cleanup timed out"))??;
    if output.status.success()
        || String::from_utf8_lossy(&output.stderr).contains("No such container")
    {
        Ok(())
    } else {
        Err(io::Error::other("failed to clean up Docker shell runner"))
    }
}

#[cfg(test)]
pub(crate) fn docker_shell_args(workspace: &FsPath, image: &str, command: &str) -> Vec<String> {
    docker_shell_args_with_name(workspace, image, command, None)
}

fn docker_shell_args_with_name(
    workspace: &FsPath,
    image: &str,
    command: &str,
    name: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["run".to_string(), "--rm".to_string()];
    if let Some(name) = name {
        args.extend(["--name".to_string(), name.to_string()]);
    }
    args.extend([
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
    ]);
    args
}

/// bubblewrap args: bind-mount workspace read-write, everything else read-only
/// from the host rootfs. No network. Runs as the current user.
fn bubblewrap_args(workspace: &FsPath, command: &str) -> Vec<String> {
    let workspace_str = workspace.display().to_string();
    vec![
        "--ro-bind".to_string(),
        "/usr".to_string(),
        "/usr".to_string(),
        "--ro-bind".to_string(),
        "/bin".to_string(),
        "/bin".to_string(),
        "--ro-bind".to_string(),
        "/lib".to_string(),
        "/lib".to_string(),
        "--ro-bind-try".to_string(),
        "/lib64".to_string(),
        "/lib64".to_string(),
        "--ro-bind-try".to_string(),
        "/etc/resolv.conf".to_string(),
        "/etc/resolv.conf".to_string(),
        "--ro-bind-try".to_string(),
        "/etc/passwd".to_string(),
        "/etc/passwd".to_string(),
        "--bind".to_string(),
        workspace_str.clone(),
        "/workspace".to_string(),
        "--chdir".to_string(),
        "/workspace".to_string(),
        "--unshare-all".to_string(),
        "--die-with-parent".to_string(),
        "--proc".to_string(),
        "/proc".to_string(),
        "--dev".to_string(),
        "/dev".to_string(),
        "--tmpfs".to_string(),
        "/tmp".to_string(),
        "sh".to_string(),
        "-c".to_string(),
        command.to_string(),
    ]
}

/// nsjail args: chroot-style jail with workspace bind-mounted.
fn nsjail_args(workspace: &FsPath, command: &str) -> Vec<String> {
    let workspace_str = workspace.display().to_string();
    vec![
        "--mode".to_string(),
        "o".to_string(),
        "--chroot".to_string(),
        "/".to_string(),
        "--bindmount".to_string(),
        format!("{workspace_str}:/workspace"),
        "--cwd".to_string(),
        "/workspace".to_string(),
        "--disable_proc".to_string(),
        "--iface_no_lo".to_string(),
        "--rlimit_as".to_string(),
        "512".to_string(),
        "--time_limit".to_string(),
        "0".to_string(),
        "--".to_string(),
        "/bin/sh".to_string(),
        "-c".to_string(),
        command.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_cleanup_name_is_bound_to_the_run() {
        let args = docker_shell_args_with_name(
            FsPath::new("/tmp/mandoforge-session"),
            "alpine:3.20",
            "pwd",
            Some("mandoforge-shell-test"),
        );
        assert!(
            args.windows(2).any(|args| {
                args == ["--name".to_string(), "mandoforge-shell-test".to_string()]
            })
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn host_timeout_stops_background_descendants() {
        let workspace = std::env::temp_dir().join(format!("mandoforge-shell-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).expect("create shell test workspace");
        let marker = workspace.join("leaked");
        let command = format!(
            "(sleep 0.2; touch '{}') >/dev/null 2>&1 & sleep 5",
            marker.display()
        );

        let output = run_shell_command("host", &workspace, &command, Duration::from_millis(50))
            .await
            .expect("run host shell");
        assert!(output.is_none(), "host shell should time out");
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(!marker.exists(), "background shell descendant survived");
        std::fs::remove_dir_all(workspace).expect("remove shell test workspace");
    }
}
