use crate::{
    DesktopHardeningStatus, DesktopState, DesktopStatus, NotificationStatus, config_dir, logs_dir,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub fn get_status(state: State<'_, DesktopState>) -> DesktopStatus {
    state.status()
}

#[tauri::command]
pub fn get_api_base_url(state: State<'_, DesktopState>) -> String {
    state.api_base_url().to_string()
}

#[tauri::command]
pub fn open_browser(state: State<'_, DesktopState>) -> Result<String, String> {
    let url = state.api_base_url().to_string();
    opener::open(&url).map_err(|error| error.to_string())?;
    Ok(url)
}

#[tauri::command]
pub fn open_config_dir() -> Result<String, String> {
    let path = config_dir().map_err(|error| error.to_string())?;
    opener::open(&path).map_err(|error| error.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn open_logs_dir() -> Result<String, String> {
    let path = logs_dir().map_err(|error| error.to_string())?;
    opener::open(&path).map_err(|error| error.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn get_notification_status(state: State<'_, DesktopState>) -> NotificationStatus {
    NotificationStatus {
        bridge: "web_console_notification_center",
        native_forwarding_enabled: true,
        browser_permission_prompted: false,
        muted_storage_key: "mandoforge.criticalNotificationsMuted",
        allowed_severity: "critical",
        forwarded_count: state.forwarded_notification_count(),
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DesktopNotificationPayload {
    pub key: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub target_label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DesktopNotificationForwardResult {
    pub key: String,
    pub status: &'static str,
    pub native_forwarded: bool,
}

#[tauri::command]
pub fn forward_console_notification(
    app: AppHandle,
    state: State<'_, DesktopState>,
    payload: DesktopNotificationPayload,
) -> Result<DesktopNotificationForwardResult, String> {
    let key = bounded_text(&payload.key, 128);
    if key.is_empty() {
        return Err("notification key is required".to_string());
    }
    if payload.severity != "critical" {
        return Ok(DesktopNotificationForwardResult {
            key,
            status: "ignored_non_critical",
            native_forwarded: false,
        });
    }
    if !state.record_forwarded_notification_key(&key) {
        return Ok(DesktopNotificationForwardResult {
            key,
            status: "duplicate_ignored",
            native_forwarded: false,
        });
    }

    let title = bounded_text(&payload.title, 96);
    let detail = bounded_text(&payload.detail, 240);
    let target = payload
        .target_label
        .as_deref()
        .map(|value| bounded_text(value, 48))
        .filter(|value| !value.is_empty());
    let body = match target {
        Some(target) if !detail.is_empty() => format!("{detail} | {target}"),
        Some(target) => target,
        None => detail,
    };

    app.notification()
        .builder()
        .title(if title.is_empty() {
            "MandoForge operator notification"
        } else {
            &title
        })
        .body(&body)
        .show()
        .map_err(|error| format!("native notification failed: {error}"))?;

    Ok(DesktopNotificationForwardResult {
        key,
        status: "forwarded",
        native_forwarded: true,
    })
}

#[tauri::command]
pub fn get_desktop_hardening_status() -> DesktopHardeningStatus {
    DesktopHardeningStatus {
        evidence_class: "mvp_local_shell",
        bundle_active: false,
        signed_distribution_ready: false,
        updater_enabled: false,
        single_instance_enabled: false,
        autostart_enabled: false,
        csp_configured: false,
        native_notifications_enabled: true,
        enterprise_completion_claimed: false,
        next_actions: vec![
            "enable signed bundle metadata before distribution",
            "add updater only with signed feed evidence",
            "add single-instance behavior before default desktop launch",
            "add explicit autostart opt-in before enabling background startup",
            "configure CSP before packaged WebView distribution",
            "capture packaged OS notification permission evidence before distribution",
        ],
    }
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::bounded_text;

    #[test]
    fn bounded_text_trims_and_limits_by_chars() {
        assert_eq!(bounded_text("  abc  ", 8), "abc");
        assert_eq!(bounded_text("abcdef", 3), "abc");
    }
}
