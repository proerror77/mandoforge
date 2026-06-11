use crate::{
    AutostartStatus, DesktopHardeningStatus, DesktopState, DesktopStatus, NotificationStatus,
    config_dir, logs_dir, validate_http_api_url,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
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
    validate_http_api_url(&url).map_err(|error| error.to_string())?;
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
    let prepared = prepare_desktop_notification_forward(&state, payload)?;
    let PreparedDesktopNotification::Forward { key, title, body } = prepared else {
        return Ok(prepared.into_result());
    };

    if let Err(error) = app
        .notification()
        .builder()
        .title(if title.is_empty() {
            "MandoForge operator notification"
        } else {
            &title
        })
        .body(&body)
        .show()
    {
        state.forget_forwarded_notification_key(&key);
        return Err(format!("native notification failed: {error}"));
    }

    Ok(DesktopNotificationForwardResult {
        key,
        status: "forwarded",
        native_forwarded: true,
    })
}

enum PreparedDesktopNotification {
    Forward {
        key: String,
        title: String,
        body: String,
    },
    Return(DesktopNotificationForwardResult),
}

impl PreparedDesktopNotification {
    fn into_result(self) -> DesktopNotificationForwardResult {
        match self {
            Self::Forward { key, .. } => DesktopNotificationForwardResult {
                key,
                status: "forwarded",
                native_forwarded: true,
            },
            Self::Return(result) => result,
        }
    }
}

fn prepare_desktop_notification_forward(
    state: &DesktopState,
    payload: DesktopNotificationPayload,
) -> Result<PreparedDesktopNotification, String> {
    let key = bounded_text(&payload.key, 128);
    if key.is_empty() {
        return Err("notification key is required".to_string());
    }
    if payload.severity != "critical" {
        return Ok(PreparedDesktopNotification::Return(
            DesktopNotificationForwardResult {
                key,
                status: "ignored_non_critical",
                native_forwarded: false,
            },
        ));
    }
    if !state.record_forwarded_notification_key(&key) {
        return Ok(PreparedDesktopNotification::Return(
            DesktopNotificationForwardResult {
                key,
                status: "duplicate_ignored",
                native_forwarded: false,
            },
        ));
    }

    let title = notification_text(&payload.title, 96);
    let detail = notification_text(&payload.detail, 240);
    let target = payload
        .target_label
        .as_deref()
        .map(|value| notification_text(value, 48))
        .filter(|value| !value.is_empty());
    let body = match target {
        Some(target) if !detail.is_empty() => format!("{detail} | {target}"),
        Some(target) => target,
        None => detail,
    };

    Ok(PreparedDesktopNotification::Forward { key, title, body })
}

#[tauri::command]
pub fn get_autostart_status(app: AppHandle) -> AutostartStatus {
    AutostartStatus {
        control_available: true,
        enabled: autostart_enabled(&app),
        policy: "explicit_operator_opt_in_only",
        platform_registration: "tauri_plugin_autostart",
    }
}

#[tauri::command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<AutostartStatus, String> {
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|error| format!("autostart enable failed: {error}"))?;
    } else {
        manager
            .disable()
            .map_err(|error| format!("autostart disable failed: {error}"))?;
    }
    Ok(get_autostart_status(app))
}

#[tauri::command]
pub fn get_desktop_hardening_status(app: AppHandle) -> DesktopHardeningStatus {
    DesktopHardeningStatus {
        evidence_class: "mvp_local_shell",
        bundle_active: false,
        signed_distribution_ready: false,
        updater_enabled: false,
        single_instance_control_available: true,
        single_instance_enabled: true,
        autostart_control_available: true,
        autostart_enabled: autostart_enabled(&app),
        csp_configured: true,
        native_notifications_enabled: true,
        enterprise_completion_claimed: false,
        next_actions: vec![
            "enable signed bundle metadata before distribution",
            "add updater only with signed feed evidence",
            "keep autostart disabled by default and require explicit operator opt-in",
            "replace the minimal IPC wrapper with bundled @tauri-apps/api modules if the frontend adopts an npm bundler",
            "capture packaged OS notification permission evidence before distribution",
        ],
    }
}

fn autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
}

fn notification_text(value: &str, max_chars: usize) -> String {
    bounded_text(value, max_chars)
        .chars()
        .filter_map(|character| {
            if character == '\n' || character == '\r' || character == '\t' {
                Some(' ')
            } else if character.is_control() {
                None
            } else {
                Some(character)
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopNotificationPayload, PreparedDesktopNotification, bounded_text, notification_text,
        prepare_desktop_notification_forward,
    };
    use crate::DesktopState;

    #[test]
    fn bounded_text_trims_and_limits_by_chars() {
        assert_eq!(bounded_text("  abc  ", 8), "abc");
        assert_eq!(bounded_text("abcdef", 3), "abc");
    }

    #[test]
    fn notification_text_removes_multiline_and_control_content() {
        assert_eq!(
            notification_text("  first\nsecond\r\n\u{0007}third  ", 64),
            "first second third"
        );
    }

    #[test]
    fn notification_forward_rejects_empty_key() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        let result = prepare_desktop_notification_forward(
            &state,
            DesktopNotificationPayload {
                key: "   ".to_string(),
                severity: "critical".to_string(),
                title: "title".to_string(),
                detail: "detail".to_string(),
                target_label: None,
            },
        );
        assert!(result.is_err());
        assert_eq!(state.forwarded_notification_count(), 0);
    }

    #[test]
    fn notification_forward_ignores_non_critical_without_recording_key() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        let result = prepare_desktop_notification_forward(
            &state,
            DesktopNotificationPayload {
                key: "approval:1".to_string(),
                severity: "warning".to_string(),
                title: "title".to_string(),
                detail: "detail".to_string(),
                target_label: None,
            },
        )
        .expect("non-critical payload should be accepted");
        assert!(matches!(
            result,
            PreparedDesktopNotification::Return(result)
                if result.status == "ignored_non_critical" && !result.native_forwarded
        ));
        assert_eq!(state.forwarded_notification_count(), 0);
    }

    #[test]
    fn notification_forward_ignores_duplicate_key() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        assert!(state.record_forwarded_notification_key("execution-job:1"));
        let result = prepare_desktop_notification_forward(
            &state,
            DesktopNotificationPayload {
                key: "execution-job:1".to_string(),
                severity: "critical".to_string(),
                title: "title".to_string(),
                detail: "detail".to_string(),
                target_label: None,
            },
        )
        .expect("duplicate payload should be accepted");
        assert!(matches!(
            result,
            PreparedDesktopNotification::Return(result)
                if result.status == "duplicate_ignored" && !result.native_forwarded
        ));
        assert_eq!(state.forwarded_notification_count(), 1);
    }

    #[test]
    fn notification_forward_prepares_sanitized_native_payload() {
        let state = DesktopState::new("http://127.0.0.1:9".to_string());
        let result = prepare_desktop_notification_forward(
            &state,
            DesktopNotificationPayload {
                key: "execution-job:1".to_string(),
                severity: "critical".to_string(),
                title: "failed\nrun".to_string(),
                detail: "line one\nline two".to_string(),
                target_label: Some("Open\nruns".to_string()),
            },
        )
        .expect("critical payload should be prepared");
        assert!(matches!(
            result,
            PreparedDesktopNotification::Forward { title, body, .. }
                if title == "failed run" && body == "line one line two | Open runs"
        ));
        assert_eq!(state.forwarded_notification_count(), 1);
    }
}
