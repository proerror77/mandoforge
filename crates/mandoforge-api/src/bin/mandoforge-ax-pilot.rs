//! Deliberately bounded AX diagnostic, invoked through approved agent_cli.exec.
//! No arbitrary commands, repositories, models or business payloads are accepted.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    os::fd::AsRawFd,
    path::PathBuf,
    time::Duration,
};
use tokio::{
    process::Command,
    time::{Instant, sleep, timeout},
};
use uuid::Uuid;

const AX_REVISION: &str = "acb6c1709b405eeaf0507031556426bf20493d52";
const ATESPACE: &str = "mandoforge-ax-pilot";
const RESULT_PATH: &str = "/workspace/mandoforge-ax-result.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: String,
    key: Uuid,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Receipt {
    session: Uuid,
    key: Uuid,
    name: String,
    nonce: Uuid,
    server: String,
    context: String,
    image: String,
    cli_sha256: String,
    ax_revision: String,
    prompt: Option<String>,
}

struct Config {
    cli: PathBuf,
    cli_sha256: String,
    server: String,
    context: String,
    image: String,
}
impl Config {
    fn load() -> Result<Self> {
        ensure!(
            env::var("MANDOFORGE_AX_PILOT_ENABLED").as_deref() == Ok("1"),
            "AX pilot is disabled"
        );
        let cli = PathBuf::from(env::var("MANDOFORGE_AX_CLI")?);
        ensure!(cli.is_absolute(), "AX CLI must be an absolute path");
        let cli_sha256 = env::var("MANDOFORGE_AX_CLI_SHA256")?;
        ensure!(
            hex::encode(Sha256::digest(fs::read(&cli)?)) == cli_sha256,
            "AX CLI digest mismatch"
        );
        let server = env::var("MANDOFORGE_AX_SERVER")?;
        let addr: std::net::SocketAddr = server
            .parse()
            .context("AX server must be a loopback IP:port")?;
        ensure!(
            addr.ip().is_loopback() && addr.port() != 0,
            "pilot requires a loopback AX endpoint"
        );
        let context = env::var("MANDOFORGE_AX_CONTEXT")?;
        ensure!(
            context.starts_with("kind-") || context == "docker-desktop",
            "pilot requires an explicit local Kubernetes context"
        );
        let image = env::var("MANDOFORGE_AX_IMAGE")?;
        let digest = image
            .rsplit_once("@sha256:")
            .map(|(_, digest)| digest)
            .unwrap_or_default();
        ensure!(
            digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "runner image must be digest pinned"
        );
        Ok(Self {
            cli,
            cli_sha256,
            server,
            context,
            image,
        })
    }

    async fn ax(&self, args: &[&str], input: Option<&Value>) -> Result<String> {
        let mut command = Command::new(&self.cli);
        command.env("AX_SSH_FORCE_ROUTER", "1");
        command.process_group(0);
        command
            .kill_on_drop(true)
            .args([
                "--server",
                &self.server,
                "--context",
                &self.context,
                "--atespace",
                ATESPACE,
            ])
            .args(args);
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if input.is_some() {
            command.stdin(std::process::Stdio::piped());
        }
        let mut child = command.spawn()?;
        if let Some(input) = input {
            use tokio::io::AsyncWriteExt;
            let mut stdin = child.stdin.take().context("AX stdin missing")?;
            stdin
                .write_all(serde_json::to_string(input)?.as_bytes())
                .await?;
        }
        let output = bounded_output(child, Duration::from_secs(25)).await?;
        ensure!(
            output.status.success(),
            "AX command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?)
    }

    async fn get(&self, receipt: &Receipt) -> Result<Value> {
        let raw = self.ax(&["get", "task", &receipt.name], None).await?;
        let task: Value = serde_yaml::from_str(&raw)?;
        validate_task(receipt, &task)?;
        Ok(task)
    }
}

async fn bounded_output(
    child: tokio::process::Child,
    duration: Duration,
) -> Result<std::process::Output> {
    let pid = child.id().context("AX child PID missing")?;
    match timeout(duration, child.wait_with_output()).await {
        Ok(output) => Ok(output?),
        Err(_) => {
            // ax ssh can spawn kubectl port-forward; kill the complete process
            // group on timeout rather than leaving the tunnel running.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            bail!("AX command timed out; reconcile existing identity, do not resubmit")
        }
    }
}

fn manifest(receipt: &Receipt) -> Value {
    if let Some(prompt) = &receipt.prompt {
        return json!({"apiVersion":"ax.io/v1alpha1", "kind":"Task", "metadata":{"name":receipt.name,"atespace":ATESPACE},
            "spec":{"image":receipt.image,"debug":true,"command":["python3","/opt/mandoforge/ax-coding-agent.py",serde_json::to_string(&json!({"identity":expected_result(receipt),"prompt":prompt})).unwrap()]}});
    }
    let result = expected_result(receipt);
    // All interpolated values are generated UUIDs, fixed field names and literals.
    // A durable marker keeps this diagnostic from executing again after data-only resume.
    let command = format!(
        "set -eu; if [ ! -f {RESULT_PATH} ]; then printf '%s\\n' '{}' > {RESULT_PATH}.tmp; mv {RESULT_PATH}.tmp {RESULT_PATH}; fi",
        result
    );
    json!({"apiVersion":"ax.io/v1alpha1", "kind":"Task", "metadata":{"name":receipt.name,"atespace":ATESPACE},
        "spec":{"image":receipt.image,"debug":true,"command":["/bin/sh","-c",command]}})
}

fn expected_result(receipt: &Receipt) -> Value {
    if receipt.prompt.is_some() {
        return json!({"schema":"mandoforge.ax-pilot.v1", "session_id":receipt.session,"key":receipt.key,"nonce":receipt.nonce,"kind":"codex"});
    }
    json!({"schema":"mandoforge.ax-pilot.v1", "session_id":receipt.session,"key":receipt.key,"nonce":receipt.nonce,"exit_code":0,"message":"AX isolated diagnostic completed"})
}

fn validate_task(receipt: &Receipt, task: &Value) -> Result<()> {
    let expected = manifest(receipt);
    ensure!(
        task.pointer("/metadata/name") == expected.pointer("/metadata/name")
            && task.pointer("/metadata/atespace") == expected.pointer("/metadata/atespace"),
        "AX task identity mismatch"
    );
    for key in ["image", "command", "debug"] {
        ensure!(
            task["spec"][key] == expected["spec"][key],
            "AX task spec mismatch: {key}"
        );
    }
    for key in ["env", "workspaces"] {
        ensure!(
            task["spec"][key].is_null() || task["spec"][key].as_array().is_some_and(Vec::is_empty),
            "unexpected AX task {key}"
        );
    }
    Ok(())
}

fn validate_coding_result(receipt: &Receipt, result: &Value) -> Result<()> {
    ensure!(
        result["identity"] == expected_result(receipt),
        "Codex result identity mismatch"
    );
    ensure!(
        result["prompt_sha256"].as_str()
            == receipt
                .prompt
                .as_ref()
                .map(|p| hex::encode(Sha256::digest(p.as_bytes())))
                .as_deref(),
        "Codex prompt digest mismatch"
    );
    ensure!(
        result["exit_code"].as_i64().is_some(),
        "missing Codex exit status"
    );
    let events = result["events"]
        .as_array()
        .context("missing Codex events")?;
    ensure!(events.len() <= 1024, "too many Codex events");
    ensure!(
        result["events_truncated"] == false,
        "Codex event capture was truncated"
    );
    if result["exit_code"] == 0 {
        ensure!(
            events.iter().any(|e| e["type"] == "turn.completed"),
            "Codex exited without turn completion"
        );
        ensure!(
            events
                .iter()
                .any(|e| e["type"] == "item.completed" && e["item"]["type"] == "agent_message"),
            "Codex final response missing"
        );
    }
    Ok(())
}

fn event(value: Value) {
    println!("{value}");
}

// Advisory OS lock releases after a crash. Durable intent is separate and NEVER removed
// automatically: an interrupted submit can only read back its existing remote identity.
fn lock(path: &std::path::Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    ensure!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "pilot identity is already being reconciled"
    );
    Ok(file)
}

async fn run() -> Result<()> {
    let config = Config::load()?;
    let args: Vec<_> = env::args().skip(1).collect();
    ensure!(
        args.len() == 1,
        "expected one JSON task; CLI arguments are not accepted"
    );
    let request: Request = serde_json::from_str(&args[0])?;
    ensure!(
        ["run", "read", "cancel", "suspend", "resume"].contains(&request.operation.as_str()),
        "unsupported operation"
    );
    if let Some(prompt) = &request.prompt {
        ensure!(
            !prompt.trim().is_empty() && prompt.len() <= 16384,
            "Codex prompt must contain 1–16384 bytes"
        );
    }
    let cwd = env::current_dir()?;
    let session = Uuid::parse_str(
        cwd.file_name()
            .and_then(|v| v.to_str())
            .context("missing session directory")?,
    )
    .context("must run inside a MandoForge session UUID workspace")?;
    let dir = cwd.join(".ax-pilot");
    fs::create_dir_all(&dir)?;
    let _lock = lock(&dir.join(format!("{}.lock", request.key)))?;
    let path = dir.join(format!("{}.json", request.key));
    let fresh = Receipt {
        session,
        key: request.key,
        name: format!(
            "mf-{}",
            &hex::encode(Sha256::digest(format!("{session}:{}", request.key)))[..40]
        ),
        nonce: Uuid::new_v4(),
        server: config.server.clone(),
        context: config.context.clone(),
        image: config.image.clone(),
        cli_sha256: config.cli_sha256.clone(),
        ax_revision: AX_REVISION.into(),
        prompt: request.prompt.clone(),
    };
    let (receipt, submit) = match fs::read(&path) {
        Ok(bytes) => {
            let existing: Receipt = serde_json::from_slice(&bytes)?;
            let mut comparison = fresh;
            comparison.nonce = existing.nonce;
            if request.operation != "run" && request.prompt.is_none() {
                comparison.prompt = existing.prompt.clone();
            }
            ensure!(
                existing == comparison,
                "receipt identity or configuration changed; manual reconciliation required"
            );
            (existing, false)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            ensure!(
                request.operation == "run",
                "no submission receipt for this identity"
            );
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(&serde_json::to_vec(&fresh)?)?;
            file.sync_all()?;
            File::open(&dir)?.sync_all()?;
            (fresh, true)
        }
        Err(err) => return Err(err.into()),
    };
    event(
        json!({"type":"turn.started", "turn_id":receipt.name,"resume_handle":{"ax_task":receipt.name,"atespace":ATESPACE,"key":receipt.key,"session_id":session,"source":"ax_pilot"}, "ax_revision":AX_REVISION}),
    );
    if submit {
        config
            .ax(&["apply", "-f", "-"], Some(&manifest(&receipt)))
            .await?;
    }
    // Missing remote identity is never interpreted as permission to apply again.
    let initial = config.get(&receipt).await?;
    if request.operation == "cancel" {
        config.ax(&["delete", "task", &receipt.name], None).await?;
        event(
            json!({"type":"turn.completed", "turn_id":receipt.name,"status":"completed","final_message":"AX delete acknowledged; retained local receipt prevents resubmission. Substrate cleanup requires independent readback."}),
        );
        return Ok(());
    }
    if receipt.prompt.is_some() && (request.operation == "suspend" || request.operation == "resume")
    {
        bail!(
            "Codex conversation checkpoint/resume is not validated; use read or cancel. AX data restore is not CLI conversation continuation"
        );
    }
    if request.operation == "suspend" || request.operation == "resume" {
        config
            .ax(&[&request.operation, "task", &receipt.name], None)
            .await?;
    }
    if request.operation == "read" {
        event(json!({"type":"item.completed","item":{"kind":"ax_state","task":initial}}));
        event(
            json!({"type":"turn.completed","turn_id":receipt.name,"status":"completed","final_message":format!("AX state readback: {}. This is not command completion evidence.", initial["status"]["phase"])}),
        );
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut previous_phase = String::new();
    loop {
        let task = config.get(&receipt).await?;
        let phase = task["status"]["phase"].as_str().unwrap_or("Unknown");
        if phase != previous_phase {
            event(
                json!({"type":"item.completed","item":{"kind":"ax_state","phase":phase,"task_id":receipt.name}}),
            );
            previous_phase = phase.to_string();
        }
        if phase == "Suspended" && request.operation == "suspend" {
            event(
                json!({"type":"turn.completed","turn_id":receipt.name,"status":"completed","final_message":"AX reports Suspended. Data checkpoint only; process-memory resume is not claimed."}),
            );
            return Ok(());
        }
        if phase == "Running" && request.operation != "suspend" {
            // Running is not command success. Read a fixed result path through AX guest services.
            if let Ok(raw) = config
                .ax(&["ssh", &receipt.name, "--", "/bin/cat", RESULT_PATH], None)
                .await
            {
                ensure!(
                    raw.len() <= 1_048_576,
                    "sandbox result exceeds pilot evidence limit"
                );
                let result: Value =
                    serde_json::from_str(raw.trim()).context("malformed sandbox result")?;
                if receipt.prompt.is_some() {
                    validate_coding_result(&receipt, &result)?;
                    // A single nested item preserves all native events without consuming
                    // the API's per-turn event budget or replacing AX lineage.
                    event(
                        json!({"type":"item.completed","item":{"kind":"codex_result","ax_task":receipt.name,"result":result}}),
                    );
                    ensure!(
                        result["exit_code"] == 0,
                        "Codex command failed inside AX: {}",
                        result["exit_code"]
                    );
                } else {
                    ensure!(
                        result == expected_result(&receipt),
                        "sandbox result identity/content mismatch"
                    );
                }
                event(
                    json!({"type":"turn.completed","turn_id":receipt.name,"status":"completed","final_message":serde_json::to_string(&result)?,"ax_revision":AX_REVISION}),
                );
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            bail!(
                "AX result not verified before deadline (phase {phase}); retain receipt and use read/cancel, never resubmit"
            );
        }
        sleep(Duration::from_secs(2)).await;
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        event(json!({"type":"ax.pilot.error","error":format!("{err:#}")}));
        eprintln!("AX pilot failed; execution may be unresolved; reconcile the retained receipt");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> Receipt {
        Receipt {
            session: Uuid::nil(),
            key: Uuid::nil(),
            name: "mf-test".into(),
            nonce: Uuid::new_v4(),
            server: "127.0.0.1:18080".into(),
            context: "kind-test".into(),
            image: "test@sha256:abc".into(),
            cli_sha256: "test".into(),
            ax_revision: AX_REVISION.into(),
            prompt: None,
        }
    }
    #[test]
    fn rejects_remote_spec_or_identity_change() {
        let receipt = receipt();
        let task = manifest(&receipt);
        validate_task(&receipt, &task).unwrap();
        for (pointer, value) in [
            ("/metadata/name", json!("other")),
            ("/spec/image", json!("other")),
            ("/spec/command", json!(["sh"])),
            ("/spec/debug", json!(false)),
        ] {
            let mut changed = task.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(validate_task(&receipt, &changed).is_err());
        }
        let mut changed = task;
        changed["spec"]["env"] = json!([{"name":"API_KEY","value":"x"}]);
        assert!(validate_task(&receipt, &changed).is_err());
    }
    #[test]
    fn result_bound_to_nonce_session_and_key() {
        let mut receipt = receipt();
        let result = expected_result(&receipt);
        receipt.nonce = Uuid::new_v4();
        assert_ne!(expected_result(&receipt), result);
    }
    #[test]
    fn task_payload_cannot_supply_commands() {
        assert!(
            serde_json::from_value::<Request>(
                json!({"operation":"run","key":Uuid::nil(),"command":"sh"})
            )
            .is_err()
        );
    }
    #[test]
    fn codex_requires_bound_prompt_and_terminal_evidence() {
        let mut receipt = receipt();
        receipt.prompt = Some("explain Rust".into());
        let mut result = json!({"identity":expected_result(&receipt),"prompt_sha256":hex::encode(Sha256::digest(b"explain Rust")),"exit_code":0,"events_truncated":false,"events":[{"type":"turn.completed"},{"type":"item.completed","item":{"type":"agent_message","text":"answer"}}]});
        validate_coding_result(&receipt, &result).unwrap();
        result["events"] = json!([]);
        assert!(validate_coding_result(&receipt, &result).is_err());
        result["exit_code"] = json!(1);
        validate_coding_result(&receipt, &result).unwrap();
        result["prompt_sha256"] = json!("wrong");
        assert!(validate_coding_result(&receipt, &result).is_err());
    }
    #[tokio::test]
    async fn ax_timeout_stops_process_group_with_inherited_pipes() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30 & wait"])
            .process_group(0)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let started = Instant::now();
        assert!(
            bounded_output(command.spawn().unwrap(), Duration::from_millis(100))
                .await
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn concurrent_reconciliation_is_excluded() {
        let path = env::temp_dir().join(format!("ax-lock-{}", Uuid::new_v4()));
        let first = lock(&path).unwrap();
        assert!(lock(&path).is_err());
        drop(first);
        drop(lock(&path).unwrap());
        fs::remove_file(path).unwrap();
    }
}
