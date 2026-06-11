use crate::{DesktopState, DesktopStatus, NotificationStatus, config_dir, logs_dir};
use tauri::State;

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
pub fn get_notification_status() -> NotificationStatus {
    NotificationStatus {
        bridge: "web_console_notification_center",
        native_forwarding_enabled: false,
        browser_permission_prompted: false,
        muted_storage_key: "mandoforge.criticalNotificationsMuted",
    }
}
