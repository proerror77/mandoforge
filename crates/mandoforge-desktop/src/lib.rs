mod commands;
mod tray;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    fs::OpenOptions,
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8787";
const DEFAULT_EMBEDDED_API_COMMAND: &str = "mandoforge-api";

pub struct DesktopState {
    api_base_url: String,
    mode: DesktopMode,
    started_at_ms: u128,
    embedded_api: Option<EmbeddedApiProcess>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopMode {
    ExistingApi,
    EmbeddedLocalApi,
}

#[derive(Clone, Debug, Serialize)]
pub struct DesktopStatus {
    pub api_base_url: String,
    pub mode: DesktopMode,
    pub uptime_ms: u128,
    pub connected_state: &'static str,
    pub embedded_api_enabled: bool,
    pub embedded_api_owned: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct NotificationStatus {
    pub bridge: &'static str,
    pub native_forwarding_enabled: bool,
    pub browser_permission_prompted: bool,
    pub muted_storage_key: &'static str,
}

impl DesktopState {
    fn from_env() -> Result<Self> {
        if env_bool("MANDOFORGE_DESKTOP_EMBEDDED_API") {
            let embedded = EmbeddedApiProcess::start_from_env()?;
            return Ok(Self {
                api_base_url: embedded.api_base_url.clone(),
                mode: DesktopMode::EmbeddedLocalApi,
                started_at_ms: now_ms(),
                embedded_api: Some(embedded),
            });
        }
        Ok(Self::new(
            std::env::var("MANDOFORGE_API_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string()),
        ))
    }

    fn new(api_base_url: String) -> Self {
        Self {
            api_base_url,
            mode: DesktopMode::ExistingApi,
            started_at_ms: now_ms(),
            embedded_api: None,
        }
    }

    pub fn api_base_url(&self) -> &str {
        &self.api_base_url
    }

    pub fn status(&self) -> DesktopStatus {
        DesktopStatus {
            api_base_url: self.api_base_url.clone(),
            mode: self.mode,
            uptime_ms: now_ms().saturating_sub(self.started_at_ms),
            connected_state: api_connectivity_state(&self.api_base_url),
            embedded_api_enabled: matches!(self.mode, DesktopMode::EmbeddedLocalApi),
            embedded_api_owned: self.embedded_api.is_some(),
        }
    }
}

struct EmbeddedApiProcess {
    api_base_url: String,
    child: Arc<Mutex<Child>>,
}

impl EmbeddedApiProcess {
    fn start_from_env() -> Result<Self> {
        let command = std::env::var("MANDOFORGE_DESKTOP_API_COMMAND")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_EMBEDDED_API_COMMAND.to_string());
        let args = std::env::var("MANDOFORGE_DESKTOP_API_ARGS")
            .ok()
            .map(|value| {
                value
                    .split_whitespace()
                    .filter(|part| !part.trim().is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let addr = reserve_loopback_addr()?;
        let api_base_url = format!("http://{addr}");
        let log_path = logs_dir()?.join("embedded-api.log");
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("could not open embedded API log {}", log_path.display()))?;
        let stderr = stdout.try_clone()?;
        let mut child = Command::new(&command)
            .args(args)
            .env("MANDOFORGE_ADDR", addr.to_string())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .with_context(|| format!("failed to spawn embedded API command: {command}"))?;

        if !wait_for_api_reachable(&api_base_url, Duration::from_secs(20)) {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "embedded API did not become reachable at {api_base_url}; see {}",
                log_path.display()
            );
        }

        Ok(Self {
            api_base_url,
            child: Arc::new(Mutex::new(child)),
        })
    }
}

impl Drop for EmbeddedApiProcess {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = DesktopState::from_env().expect("failed to initialize MandoForge desktop state");
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_api_base_url,
            commands::open_browser,
            commands::open_config_dir,
            commands::open_logs_dir,
            commands::get_notification_status,
        ])
        .setup(|app| {
            let state = app.state::<DesktopState>();
            if let Err(error) = open_console_window(app, state.api_base_url())
                .context("failed to open MandoForge console window")
            {
                eprintln!("mandoforge desktop setup failed: {error:#}");
                return Err(error.into());
            }
            tray::install(app.handle()).context("failed to install desktop tray")?;
            install_smoke_auto_exit(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running MandoForge desktop");
}

fn open_console_window(app: &tauri::App, api_base_url: &str) -> Result<()> {
    let url = tauri::Url::parse(api_base_url)
        .with_context(|| format!("invalid MANDOFORGE_API_BASE_URL: {api_base_url}"))?;
    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("MandoForge Control Plane")
        .inner_size(1280.0, 860.0)
        .min_inner_size(1024.0, 720.0)
        .build()
        .map_err(|error| anyhow::anyhow!("tauri window build error: {error:?}"))?;
    Ok(())
}

fn install_smoke_auto_exit(app: &tauri::AppHandle) {
    let Some(delay_ms) = std::env::var("MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
    else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(delay_ms));
        handle.exit(0);
    });
}

fn api_connectivity_state(api_base_url: &str) -> &'static str {
    let Ok(url) = tauri::Url::parse(api_base_url) else {
        return "api_url_invalid";
    };
    let Some(host) = url.host_str() else {
        return "api_url_invalid";
    };
    let Some(port) = url.port_or_known_default() else {
        return "api_url_invalid";
    };
    let Ok(mut addresses) = (host, port).to_socket_addrs() else {
        return "api_unreachable";
    };
    let Some(address) = addresses.next() else {
        return "api_unreachable";
    };
    match TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
        Ok(_) => "api_reachable",
        Err(_) => "api_unreachable",
    }
}

fn wait_for_api_reachable(api_base_url: &str, timeout: Duration) -> bool {
    let started_at = SystemTime::now();
    loop {
        if api_connectivity_state(api_base_url) == "api_reachable" {
            return true;
        }
        if started_at
            .elapsed()
            .map(|elapsed| elapsed >= timeout)
            .unwrap_or(true)
        {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn reserve_loopback_addr() -> Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(addr)
}

fn env_bool(key: &str) -> bool {
    matches!(
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
    )
}

pub fn config_dir() -> Result<std::path::PathBuf> {
    let path = dirs::config_dir()
        .context("could not resolve user config directory")?
        .join("mandoforge");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn logs_dir() -> Result<std::path::PathBuf> {
    let path = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .context("could not resolve user data directory")?
        .join("mandoforge")
        .join("logs");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_state_defaults_to_existing_api_mode_without_embedding_api() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        let status = state.status();
        assert_eq!(status.api_base_url, "http://127.0.0.1:9");
        assert!(!status.embedded_api_enabled);
        assert!(!status.embedded_api_owned);
        assert!(matches!(status.mode, DesktopMode::ExistingApi));
    }

    #[test]
    fn invalid_api_url_is_reported_without_panic() {
        assert_eq!(api_connectivity_state("not a url"), "api_url_invalid");
    }

    #[test]
    fn env_bool_accepts_explicit_truthy_values() {
        unsafe {
            std::env::set_var("MANDOFORGE_DESKTOP_TEST_BOOL", "true");
        }
        assert!(env_bool("MANDOFORGE_DESKTOP_TEST_BOOL"));
        unsafe {
            std::env::remove_var("MANDOFORGE_DESKTOP_TEST_BOOL");
        }
    }
}
