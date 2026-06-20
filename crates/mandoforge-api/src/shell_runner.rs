use std::path::Path as FsPath;

use tokio::process::Command;

pub(crate) fn shell_runner() -> String {
    std::env::var("MANDOFORGE_SHELL_RUNNER")
        .unwrap_or_else(|_| "host".to_string())
        .trim()
        .to_string()
}

pub(crate) fn shell_command(runner: &str, workspace: &FsPath, command: &str) -> Command {
    match runner {
        "docker" => {
            let image = std::env::var("MANDOFORGE_SHELL_DOCKER_IMAGE")
                .unwrap_or_else(|_| "alpine:3.20".to_string());
            let mut process = Command::new("docker");
            process.args(docker_shell_args(workspace, &image, command));
            process
        }
        // bubblewrap: Linux user-namespaces sandbox. Near-zero cold-start (<10ms).
        // Requires `bwrap` (bubblewrap) installed on the host.
        // Set MANDOFORGE_SHELL_RUNNER=bubblewrap to enable.
        "bubblewrap" | "bwrap" => {
            let mut process = Command::new("bwrap");
            process.args(bubblewrap_args(workspace, command));
            process
        }
        // nsjail: Google's lightweight namespace jail. Fast (<50ms), no daemon needed.
        // Requires `nsjail` installed. Set MANDOFORGE_SHELL_RUNNER=nsjail to enable.
        "nsjail" => {
            let mut process = Command::new("nsjail");
            process.args(nsjail_args(workspace, command));
            process
        }
        // host: no sandboxing, runs directly on the host shell.
        // Fastest option — use only for trusted agent workloads on your own infra.
        _ => {
            let mut process = Command::new("sh");
            process.arg("-c").arg(command).current_dir(workspace);
            process
        }
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

/// bubblewrap args: bind-mount workspace read-write, everything else read-only
/// from the host rootfs. No network. Runs as the current user.
fn bubblewrap_args(workspace: &FsPath, command: &str) -> Vec<String> {
    let workspace_str = workspace.display().to_string();
    vec![
        "--ro-bind".to_string(), "/usr".to_string(), "/usr".to_string(),
        "--ro-bind".to_string(), "/bin".to_string(), "/bin".to_string(),
        "--ro-bind".to_string(), "/lib".to_string(), "/lib".to_string(),
        "--ro-bind-try".to_string(), "/lib64".to_string(), "/lib64".to_string(),
        "--ro-bind-try".to_string(), "/etc/resolv.conf".to_string(), "/etc/resolv.conf".to_string(),
        "--ro-bind-try".to_string(), "/etc/passwd".to_string(), "/etc/passwd".to_string(),
        "--bind".to_string(), workspace_str.clone(), "/workspace".to_string(),
        "--chdir".to_string(), "/workspace".to_string(),
        "--unshare-all".to_string(),
        "--die-with-parent".to_string(),
        "--proc".to_string(), "/proc".to_string(),
        "--dev".to_string(), "/dev".to_string(),
        "--tmpfs".to_string(), "/tmp".to_string(),
        "sh".to_string(), "-c".to_string(), command.to_string(),
    ]
}

/// nsjail args: chroot-style jail with workspace bind-mounted.
fn nsjail_args(workspace: &FsPath, command: &str) -> Vec<String> {
    let workspace_str = workspace.display().to_string();
    vec![
        "--mode".to_string(), "o".to_string(),
        "--chroot".to_string(), "/".to_string(),
        "--bindmount".to_string(), format!("{workspace_str}:/workspace"),
        "--cwd".to_string(), "/workspace".to_string(),
        "--disable_proc".to_string(),
        "--iface_no_lo".to_string(),
        "--rlimit_as".to_string(), "512".to_string(),
        "--time_limit".to_string(), "0".to_string(),
        "--".to_string(),
        "/bin/sh".to_string(), "-c".to_string(), command.to_string(),
    ]
}
