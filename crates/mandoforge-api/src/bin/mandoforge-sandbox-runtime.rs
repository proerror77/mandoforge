use std::fs;
use std::io::{BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::Duration;

#[path = "../sandbox_runtime_protocol.rs"]
mod sandbox_runtime_protocol;

use sandbox_runtime_protocol::{
    MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES, SANDBOX_RUNTIME_SUBCOMMAND, SandboxRuntimeOperation,
    SandboxRuntimeRequest, parse_sandbox_runtime_request,
};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use uuid::Uuid;

const DEFAULT_SOURCE_DIR: &str = "/opt/mandoforge-source";
const CODEX_FINAL_BEGIN: &str = "__MANDOFORGE_CODEX_FINAL_BEGIN__";
const CODEX_FINAL_END: &str = "__MANDOFORGE_CODEX_FINAL_END__";
const MAX_LAUNCHER_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_CODEX_JSONL_OUTPUT_BYTES: usize = MAX_LAUNCHER_OUTPUT_BYTES / 2;
const CODEX_FINAL_FRAMING_BYTES: usize = CODEX_FINAL_BEGIN.len() + CODEX_FINAL_END.len() + 4;
const MAX_CODEX_FINAL_MESSAGE_BYTES: usize =
    MAX_LAUNCHER_OUTPUT_BYTES - MAX_CODEX_JSONL_OUTPUT_BYTES - CODEX_FINAL_FRAMING_BYTES;
const PROCESS_OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

struct ProcessOutput {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1)),
        Err(error) => {
            eprintln!("mandoforge sandbox runtime failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<i32, String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 || args[1] != SANDBOX_RUNTIME_SUBCOMMAND {
        return Err(format!("usage: {} {SANDBOX_RUNTIME_SUBCOMMAND}", args[0]));
    }
    let request = read_request_from_stdin()?;
    let source_dir = std::env::var("MANDOFORGE_SANDBOX_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOURCE_DIR));
    ensure_session_workspace(&source_dir, Path::new(&request.workspace_path))?;
    execute_request(&request).await
}

fn read_request_from_stdin() -> Result<SandboxRuntimeRequest, String> {
    let stdin = std::io::stdin();
    let mut input = Vec::new();
    stdin
        .lock()
        .take((MAX_SANDBOX_RUNTIME_ENVELOPE_BYTES + 1) as u64)
        .read_until(b'\n', &mut input)
        .map_err(|error| format!("failed to read sandbox runtime request: {error}"))?;
    if input.last() != Some(&b'\n') {
        return Err("sandbox runtime request must be newline terminated".to_string());
    }
    parse_sandbox_runtime_request(&input)
}

async fn execute_request(request: &SandboxRuntimeRequest) -> Result<i32, String> {
    let workspace = Path::new(&request.workspace_path);
    match &request.operation {
        SandboxRuntimeOperation::FileWrite { path, content } => {
            write_workspace_file(workspace, Path::new(path), content.as_bytes())?;
            println!("wrote file {path}");
            Ok(0)
        }
        SandboxRuntimeOperation::Shell { command } => {
            let mut process = Command::new("sh");
            process.arg("-lc").arg(command);
            let output =
                run_process(process, request, workspace, MAX_LAUNCHER_OUTPUT_BYTES).await?;
            emit_process_output(&output)?;
            Ok(output.code)
        }
        SandboxRuntimeOperation::Codex { task, sandbox_mode } => {
            let state_dir = ensure_session_directory(workspace, ".mandoforge")?;
            let final_path = state_dir.join("codex-final-message.md");
            if final_path.exists()
                && fs::symlink_metadata(&final_path)
                    .map_err(|error| {
                        format!("failed to inspect Codex final message path: {error}")
                    })?
                    .file_type()
                    .is_symlink()
            {
                return Err("Codex final message path must not be a symlink".to_string());
            }
            let mut process = Command::new("codex");
            process
                .arg("exec")
                .arg("--sandbox")
                .arg(sandbox_mode)
                .arg("--json")
                .arg("--output-last-message")
                .arg(&final_path)
                .arg("--cd")
                .arg(workspace)
                .arg(task);
            let output =
                run_process(process, request, workspace, MAX_CODEX_JSONL_OUTPUT_BYTES).await?;
            emit_process_output(&output)?;
            println!("\n{CODEX_FINAL_BEGIN}");
            let (final_message, truncated) =
                read_bounded_regular_file(&final_path, &state_dir, MAX_CODEX_FINAL_MESSAGE_BYTES)?;
            std::io::stdout()
                .write_all(&final_message)
                .map_err(|error| format!("failed to write Codex final message: {error}"))?;
            if !final_message.ends_with(b"\n") {
                println!();
            }
            if truncated {
                eprintln!("mandoforge sandbox runtime Codex final message truncated");
            }
            println!("{CODEX_FINAL_END}");
            Ok(output.code)
        }
        SandboxRuntimeOperation::AgentCli {
            executable,
            args,
            task,
            profile,
        } => {
            let mut process = Command::new(executable);
            process.args(args).arg(task);
            process
                .env("MANDOFORGE_AGENT_CLI_PROFILE", profile)
                .env("MANDOFORGE_AGENT_TASK", task);
            let output =
                run_process(process, request, workspace, MAX_LAUNCHER_OUTPUT_BYTES).await?;
            emit_process_output(&output)?;
            Ok(output.code)
        }
    }
}

async fn run_process(
    mut process: Command,
    request: &SandboxRuntimeRequest,
    workspace: &Path,
    stdout_limit: usize,
) -> Result<ProcessOutput, String> {
    let home = ensure_session_directory(workspace, ".home")?;
    let cargo_target = ensure_session_directory(workspace, "target")?;
    process
        .current_dir(workspace)
        .kill_on_drop(true)
        .env("HOME", &home)
        .env("CARGO_TARGET_DIR", cargo_target)
        .env("MANDOFORGE_SESSION_ID", request.session_id.to_string())
        .env("RUSTC_WRAPPER", "sccache")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &request.environment {
        process.env(key, value);
    }
    configure_process_group(&mut process);
    let mut child = process
        .spawn()
        .map_err(|error| format!("failed to execute sandbox runtime process: {error}"))?;
    let process_group_id = child
        .id()
        .ok_or_else(|| "sandbox runtime process id is unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "sandbox runtime process stdout is unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "sandbox runtime process stderr is unavailable".to_string())?;
    let mut stdout_task = tokio::spawn(read_bounded_output(stdout, stdout_limit));
    let mut stderr_task = tokio::spawn(read_bounded_output(stderr, MAX_LAUNCHER_OUTPUT_BYTES));
    let status = match tokio::time::timeout(
        Duration::from_secs(request.timeout_seconds),
        child.wait(),
    )
    .await
    {
        Ok(status) => status
            .map_err(|error| format!("failed to wait for sandbox runtime process: {error}"))?,
        Err(_) => {
            terminate_process_group(process_group_id);
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err("sandbox runtime process timed out".to_string());
        }
    };
    terminate_process_group(process_group_id);
    let captures = tokio::time::timeout(PROCESS_OUTPUT_DRAIN_TIMEOUT, async {
        let stdout = (&mut stdout_task)
            .await
            .map_err(|error| format!("failed to join stdout capture: {error}"))??;
        let stderr = (&mut stderr_task)
            .await
            .map_err(|error| format!("failed to join stderr capture: {error}"))??;
        Ok::<_, String>((stdout, stderr))
    })
    .await;
    let ((stdout, stdout_truncated), (stderr, stderr_truncated)) = match captures {
        Ok(captures) => captures?,
        Err(_) => {
            stdout_task.abort();
            stderr_task.abort();
            return Err("sandbox runtime process output drain timed out".to_string());
        }
    };
    Ok(ProcessOutput {
        code: status.code().unwrap_or(1),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

async fn read_bounded_output<R>(mut reader: R, max_bytes: usize) -> Result<(Vec<u8>, bool), String>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to read sandbox runtime process output: {error}"))?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(output.len());
        let take = remaining.min(read);
        output.extend_from_slice(&buffer[..take]);
        truncated |= take < read;
    }
    Ok((output, truncated))
}

fn read_bounded_regular_file(
    path: &Path,
    expected_parent: &Path,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), String> {
    if !is_real_directory(expected_parent) {
        return Err("bounded output parent must be a real directory".to_string());
    }
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect bounded output file: {error}"))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err("bounded output path must be a regular non-symlink file".to_string());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(|error| {
        format!("failed to open bounded output file without following links: {error}")
    })?;
    if !file
        .metadata()
        .map_err(|error| format!("failed to inspect opened bounded output file: {error}"))?
        .is_file()
    {
        return Err("opened bounded output must be a regular file".to_string());
    }
    let canonical_parent = fs::canonicalize(expected_parent)
        .map_err(|error| format!("failed to resolve bounded output parent: {error}"))?;
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve bounded output path: {error}"))?;
    if canonical_path.parent() != Some(canonical_parent.as_path()) {
        return Err("bounded output path escaped its session-private parent".to_string());
    }
    let mut output = Vec::new();
    file.take((max_bytes + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| format!("failed to read bounded output file: {error}"))?;
    let truncated = output.len() > max_bytes;
    output.truncate(max_bytes);
    Ok((output, truncated))
}

#[cfg(unix)]
fn configure_process_group(process: &mut Command) {
    use std::os::unix::process::CommandExt;

    process.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_process: &mut Command) {}

#[cfg(unix)]
fn terminate_process_group(process_group_id: u32) {
    let Ok(process_group_id) = i32::try_from(process_group_id) else {
        return;
    };
    // SAFETY: a negative PID targets only the child-created process group.
    unsafe {
        libc::kill(-process_group_id, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_process_group_id: u32) {}

fn emit_process_output(output: &ProcessOutput) -> Result<(), String> {
    std::io::stdout()
        .write_all(&output.stdout)
        .map_err(|error| format!("failed to write sandbox runtime stdout: {error}"))?;
    std::io::stderr()
        .write_all(&output.stderr)
        .map_err(|error| format!("failed to write sandbox runtime stderr: {error}"))?;
    if output.stdout_truncated {
        eprintln!("mandoforge sandbox runtime stdout truncated");
    }
    if output.stderr_truncated {
        eprintln!("mandoforge sandbox runtime stderr truncated");
    }
    Ok(())
}

fn ensure_session_workspace(source: &Path, workspace: &Path) -> Result<(), String> {
    if workspace.exists() {
        let metadata = fs::symlink_metadata(workspace)
            .map_err(|error| format!("failed to inspect session workspace: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("session workspace must be a real directory".to_string());
        }
        return Ok(());
    }
    let parent = workspace
        .parent()
        .ok_or_else(|| "session workspace has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create session workspace parent: {error}"))?;
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|error| format!("failed to inspect session workspace parent: {error}"))?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err("session workspace parent must be a real directory".to_string());
    }
    let temp = parent.join(format!(
        ".{}.seed-{}",
        workspace
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "session workspace name is invalid".to_string())?,
        Uuid::new_v4()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp)
            .map_err(|error| format!("failed to remove stale seed directory: {error}"))?;
    }
    if let Err(error) = copy_source_tree(source, &temp) {
        let _ = fs::remove_dir_all(&temp);
        return Err(error);
    }
    match fs::rename(&temp, workspace) {
        Ok(()) => Ok(()),
        Err(_error) if is_real_directory(workspace) => {
            let _ = fs::remove_dir_all(&temp);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            Err(format!("failed to publish session workspace: {error}"))
        }
    }
}

fn ensure_session_directory(workspace: &Path, name: &str) -> Result<PathBuf, String> {
    if !matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    ) {
        return Err("session directory name must be one normal path component".to_string());
    }
    let path = workspace.join(name);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(format!(
                "failed to create session directory {name}: {error}"
            ));
        }
    }
    if !is_real_directory(&path) {
        return Err(format!("session directory {name} must be a real directory"));
    }
    Ok(path)
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn copy_source_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("failed to inspect source seed: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("source seed must be a real directory".to_string());
    }
    fs::create_dir(destination)
        .map_err(|error| format!("failed to create seed destination: {error}"))?;
    for entry in
        fs::read_dir(source).map_err(|error| format!("failed to read source seed: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to read source seed entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect source seed entry: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!(
                "source seed must not contain symlinks: {}",
                entry.path().display()
            ));
        }
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_source_tree(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &destination_path)
                .map_err(|error| format!("failed to copy source seed file: {error}"))?;
        } else {
            return Err(format!(
                "source seed contains an unsupported entry: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn write_workspace_file(
    workspace: &Path,
    relative_path: &Path,
    content: &[u8],
) -> Result<(), String> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("file_write path must stay inside the session workspace".to_string());
    }
    let mut current = workspace.to_path_buf();
    let components = relative_path.components().collect::<Vec<_>>();
    let (file_name, parents) = components
        .split_last()
        .ok_or_else(|| "file_write path is empty".to_string())?;
    for component in parents {
        let Component::Normal(component) = component else {
            return Err("file_write path must stay inside the session workspace".to_string());
        };
        current.push(component);
        if current.exists() {
            let metadata = fs::symlink_metadata(&current)
                .map_err(|error| format!("failed to inspect file_write parent: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("file_write parent must be a real directory".to_string());
            }
        } else {
            fs::create_dir(&current)
                .map_err(|error| format!("failed to create file_write parent: {error}"))?;
        }
    }
    let Component::Normal(file_name) = file_name else {
        return Err("file_write path must end in a normal file name".to_string());
    };
    let destination = current.join(file_name);
    if destination.exists()
        && fs::symlink_metadata(&destination)
            .map_err(|error| format!("failed to inspect file_write target: {error}"))?
            .file_type()
            .is_symlink()
    {
        return Err("file_write target must not be a symlink".to_string());
    }
    let temp = current.join(format!(
        ".{}.tmp-{}",
        Path::new(file_name)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "file_write file name is invalid".to_string())?,
        Uuid::new_v4()
    ));
    let publish_result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("failed to create file_write temp file: {error}"))?;
        file.write_all(content)
            .map_err(|error| format!("failed to write file content: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync file content: {error}"))?;
        fs::rename(&temp, &destination)
            .map_err(|error| format!("failed to publish file_write target: {error}"))
    })();
    if publish_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    publish_result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mandoforge-sandbox-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn sandbox_runtime_workspace_seed_is_atomic_and_reused() {
        let root = temp_dir("seed");
        let source = root.join("source");
        let workspace = root.join("sessions/session");
        fs::create_dir_all(&source).expect("source directory");
        fs::write(source.join("seed.txt"), "v1").expect("seed file");

        ensure_session_workspace(&source, &workspace).expect("seed workspace");
        assert_eq!(
            fs::read_to_string(workspace.join("seed.txt")).expect("seeded file"),
            "v1"
        );
        fs::write(source.join("seed.txt"), "v2").expect("update source");
        ensure_session_workspace(&source, &workspace).expect("reuse workspace");
        assert_eq!(
            fs::read_to_string(workspace.join("seed.txt")).expect("reused file"),
            "v1"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_runtime_file_write_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink");
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).expect("workspace");
        fs::create_dir_all(&outside).expect("outside");
        symlink(&outside, workspace.join("escape")).expect("symlink");

        let error = write_workspace_file(
            &workspace,
            Path::new("escape/secret.txt"),
            b"must stay inside",
        )
        .expect_err("symlink escape must fail");
        assert!(error.contains("real directory"));
        assert!(!outside.join("secret.txt").exists());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_runtime_session_state_rejects_symlink_directory() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("state-symlink");
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        fs::create_dir_all(&workspace).expect("workspace");
        fs::create_dir_all(&outside).expect("outside");
        symlink(&outside, workspace.join(".home")).expect("symlink");

        let error = ensure_session_directory(&workspace, ".home")
            .expect_err("session state symlink must fail");
        assert!(error.contains("real directory"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[tokio::test]
    async fn sandbox_runtime_process_output_is_bounded() {
        let root = temp_dir("output");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let session_id = Uuid::new_v4();
        let mut request = SandboxRuntimeRequest::new(
            session_id,
            30,
            std::collections::BTreeMap::new(),
            SandboxRuntimeOperation::Shell {
                command: "unused".to_string(),
            },
        );
        request.workspace_path = workspace.to_string_lossy().to_string();
        let mut process = Command::new("sh");
        process.arg("-c").arg(format!(
            "yes x | head -c {}",
            MAX_LAUNCHER_OUTPUT_BYTES + 32
        ));

        let output = run_process(process, &request, &workspace, MAX_LAUNCHER_OUTPUT_BYTES)
            .await
            .expect("bounded process output");
        assert_eq!(output.stdout.len(), MAX_LAUNCHER_OUTPUT_BYTES);
        assert!(output.stdout_truncated);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn sandbox_runtime_codex_final_message_is_bounded() {
        let root = temp_dir("codex-final");
        fs::create_dir_all(&root).expect("test directory");
        let final_path = root.join("final.md");
        fs::write(&final_path, vec![b'x'; MAX_CODEX_FINAL_MESSAGE_BYTES + 32])
            .expect("oversized final message");

        let (output, truncated) =
            read_bounded_regular_file(&final_path, &root, MAX_CODEX_FINAL_MESSAGE_BYTES)
                .expect("bounded final message");
        assert_eq!(output.len(), MAX_CODEX_FINAL_MESSAGE_BYTES);
        assert!(truncated);
        assert_eq!(
            MAX_CODEX_JSONL_OUTPUT_BYTES + output.len() + CODEX_FINAL_FRAMING_BYTES,
            MAX_LAUNCHER_OUTPUT_BYTES
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_runtime_bounded_output_rejects_post_exec_symlink() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("codex-final-symlink");
        let state_dir = root.join("state");
        fs::create_dir_all(&state_dir).expect("state directory");
        let outside = root.join("outside.txt");
        fs::write(&outside, "must not be emitted").expect("outside file");
        let final_path = state_dir.join("final.md");
        symlink(&outside, &final_path).expect("final symlink");

        let error =
            read_bounded_regular_file(&final_path, &state_dir, MAX_CODEX_FINAL_MESSAGE_BYTES)
                .expect_err("final symlink must fail closed");
        assert!(error.contains("non-symlink"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sandbox_runtime_reaps_descendants_that_hold_output_pipes() {
        let root = temp_dir("descendant");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let mut request = SandboxRuntimeRequest::new(
            Uuid::new_v4(),
            5,
            std::collections::BTreeMap::new(),
            SandboxRuntimeOperation::Shell {
                command: "unused".to_string(),
            },
        );
        request.workspace_path = workspace.to_string_lossy().to_string();
        let mut process = Command::new("sh");
        process.arg("-c").arg("sleep 30 & printf ready");

        let started = std::time::Instant::now();
        let output = run_process(process, &request, &workspace, MAX_LAUNCHER_OUTPUT_BYTES)
            .await
            .expect("background descendant is reaped");
        assert_eq!(output.stdout, b"ready");
        assert!(started.elapsed() < Duration::from_secs(3));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
