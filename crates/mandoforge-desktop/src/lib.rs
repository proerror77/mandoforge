mod commands;
mod tray;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    fs::OpenOptions,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8787";
const DEFAULT_EMBEDDED_API_COMMAND: &str = "mandoforge-api";

pub struct DesktopState {
    api_base_url: String,
    mode: DesktopMode,
    started_at_ms: u128,
    embedded_api: Option<EmbeddedApiProcess>,
    notification_bridge: Mutex<NotificationBridgeState>,
}

#[derive(Default)]
struct NotificationBridgeState {
    forwarded_keys: VecDeque<String>,
    forwarded_key_set: HashSet<String>,
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
    pub allowed_severity: &'static str,
    pub forwarded_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct DesktopHardeningStatus {
    pub evidence_class: &'static str,
    pub bundle_active: bool,
    pub signed_distribution_ready: bool,
    pub updater_enabled: bool,
    pub single_instance_control_available: bool,
    pub single_instance_enabled: bool,
    pub autostart_control_available: bool,
    pub autostart_enabled: bool,
    pub csp_configured: bool,
    pub native_notifications_enabled: bool,
    pub enterprise_completion_claimed: bool,
    pub next_actions: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AutostartStatus {
    pub control_available: bool,
    pub enabled: bool,
    pub policy: &'static str,
    pub platform_registration: &'static str,
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
                notification_bridge: Mutex::new(NotificationBridgeState::default()),
            });
        }
        let api_base_url = std::env::var("MANDOFORGE_API_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string());
        validate_loopback_http_api_url(&api_base_url)?;
        Ok(Self::new(api_base_url))
    }

    pub(crate) fn new(api_base_url: String) -> Self {
        Self {
            api_base_url,
            mode: DesktopMode::ExistingApi,
            started_at_ms: now_ms(),
            embedded_api: None,
            notification_bridge: Mutex::new(NotificationBridgeState::default()),
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

    pub fn record_forwarded_notification_key(&self, key: &str) -> bool {
        let Ok(mut bridge) = self.notification_bridge.lock() else {
            return false;
        };
        if bridge.forwarded_key_set.contains(key) {
            return false;
        }
        bridge.forwarded_keys.push_back(key.to_string());
        bridge.forwarded_key_set.insert(key.to_string());
        if bridge.forwarded_keys.len() > 256 {
            let overflow = bridge.forwarded_keys.len() - 256;
            let evicted_keys = bridge.forwarded_keys.drain(0..overflow).collect::<Vec<_>>();
            for evicted in evicted_keys {
                bridge.forwarded_key_set.remove(&evicted);
            }
        }
        true
    }

    pub fn forget_forwarded_notification_key(&self, key: &str) {
        let Ok(mut bridge) = self.notification_bridge.lock() else {
            return;
        };
        bridge.forwarded_key_set.remove(key);
        bridge.forwarded_keys.retain(|existing| existing != key);
    }

    pub fn forwarded_notification_count(&self) -> usize {
        self.notification_bridge
            .lock()
            .map(|bridge| bridge.forwarded_keys.len())
            .unwrap_or_default()
    }
}

struct EmbeddedApiProcess {
    api_base_url: String,
    child: Child,
}

impl EmbeddedApiProcess {
    fn start_from_env() -> Result<Self> {
        let command = std::env::var("MANDOFORGE_DESKTOP_API_COMMAND")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_EMBEDDED_API_COMMAND.to_string());
        let addr = reserve_loopback_addr()?;
        let api_base_url = format!("http://{addr}");
        let health_nonce = Uuid::new_v4().to_string();
        let log_path = logs_dir()?.join("embedded-api.log");
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("could not open embedded API log {}", log_path.display()))?;
        let stderr = stdout.try_clone()?;
        let mut child = Command::new(&command)
            .env("MANDOFORGE_ADDR", addr.to_string())
            .env("MANDOFORGE_DESKTOP_HEALTH_NONCE", &health_nonce)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .with_context(|| format!("failed to spawn embedded API command: {command}"))?;

        if !wait_for_api_reachable(&api_base_url, Some(&health_nonce), Duration::from_secs(20)) {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "embedded API did not become reachable at {api_base_url}; see {}",
                log_path.display()
            );
        }

        Ok(Self {
            api_base_url,
            child,
        })
    }
}

impl Drop for EmbeddedApiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = DesktopState::from_env().expect("failed to initialize MandoForge desktop state");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--from-autostart"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_api_base_url,
            commands::open_browser,
            commands::open_config_dir,
            commands::open_logs_dir,
            commands::get_notification_status,
            commands::forward_console_notification,
            commands::get_autostart_status,
            commands::set_autostart_enabled,
            commands::get_desktop_hardening_status,
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

fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn open_console_window(app: &tauri::App, api_base_url: &str) -> Result<()> {
    let url = validate_loopback_http_api_url(api_base_url)?;
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
    let Ok(url) = validate_http_api_url(api_base_url) else {
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

fn wait_for_api_reachable(
    api_base_url: &str,
    expected_nonce: Option<&str>,
    timeout: Duration,
) -> bool {
    let started_at = SystemTime::now();
    loop {
        if api_healthz_ready(api_base_url, expected_nonce) {
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

pub(crate) fn validate_http_api_url(api_base_url: &str) -> Result<tauri::Url> {
    let url = tauri::Url::parse(api_base_url)
        .with_context(|| format!("invalid MANDOFORGE_API_BASE_URL: {api_base_url}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => bail!("MANDOFORGE_API_BASE_URL must use http or https, got {scheme}"),
    }
}

pub(crate) fn validate_loopback_http_api_url(api_base_url: &str) -> Result<tauri::Url> {
    let url = validate_http_api_url(api_base_url)?;
    let Some(host) = url.host_str() else {
        bail!("MANDOFORGE_API_BASE_URL must include a host");
    };
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    if !loopback {
        bail!("MandoForge desktop WebView only allows loopback API URLs");
    }
    Ok(url)
}

fn api_healthz_ready(api_base_url: &str, expected_nonce: Option<&str>) -> bool {
    let Ok(url) = validate_http_api_url(api_base_url) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    let Ok(mut addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    let Some(address) = addresses.next() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let request =
        format!("GET /healthz HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return false;
    }
    match expected_nonce {
        Some(nonce) => response.contains(nonce),
        None => response.contains(r#""status":"ok""#) || response.contains(r#""status": "ok""#),
    }
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
    fn desktop_api_url_rejects_non_http_schemes() {
        assert!(validate_http_api_url("file:///etc/passwd").is_err());
        assert!(validate_http_api_url("mandoforge://api").is_err());
        assert!(validate_http_api_url("http://127.0.0.1:8787").is_ok());
    }

    #[test]
    fn desktop_webview_url_requires_loopback_host() {
        assert!(validate_loopback_http_api_url("http://127.0.0.1:8787").is_ok());
        assert!(validate_loopback_http_api_url("http://localhost:8787").is_ok());
        assert!(validate_loopback_http_api_url("https://example.com").is_err());
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

    #[test]
    fn notification_bridge_deduplicates_forwarded_keys() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        assert!(state.record_forwarded_notification_key("execution-job:1"));
        assert!(!state.record_forwarded_notification_key("execution-job:1"));
        assert_eq!(state.forwarded_notification_count(), 1);
    }

    #[test]
    fn notification_bridge_evicts_oldest_keys_at_cap() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        for index in 0..257 {
            assert!(state.record_forwarded_notification_key(&format!("execution-job:{index}")));
        }
        assert_eq!(state.forwarded_notification_count(), 256);
        assert!(state.record_forwarded_notification_key("execution-job:0"));
        assert_eq!(state.forwarded_notification_count(), 256);
    }

    #[test]
    fn notification_bridge_can_forget_failed_forwarding_key() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        assert!(state.record_forwarded_notification_key("execution-job:1"));
        state.forget_forwarded_notification_key("execution-job:1");
        assert!(state.record_forwarded_notification_key("execution-job:1"));
    }
}
