use js_sys::{Function, Object, Promise, Reflect};
use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct DesktopStatus {
    pub(crate) api_base_url: String,
    pub(crate) mode: String,
    pub(crate) connected_state: String,
    pub(crate) embedded_api_enabled: bool,
    pub(crate) embedded_api_owned: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct AutostartStatus {
    pub(crate) control_available: bool,
    pub(crate) enabled: bool,
    pub(crate) policy: String,
    pub(crate) platform_registration: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct DesktopHardeningStatus {
    pub(crate) evidence_class: String,
    pub(crate) bundle_active: bool,
    pub(crate) signed_distribution_ready: bool,
    pub(crate) updater_enabled: bool,
    pub(crate) single_instance_control_available: bool,
    pub(crate) single_instance_enabled: bool,
    pub(crate) autostart_control_available: bool,
    pub(crate) autostart_enabled: bool,
    pub(crate) csp_configured: bool,
    pub(crate) native_notifications_enabled: bool,
    pub(crate) enterprise_completion_claimed: bool,
    pub(crate) next_actions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DesktopIntegrationSnapshot {
    pub(crate) tauri_available: bool,
    pub(crate) status: Option<DesktopStatus>,
    pub(crate) autostart: Option<AutostartStatus>,
    pub(crate) hardening: Option<DesktopHardeningStatus>,
    pub(crate) error: Option<String>,
}

pub(crate) async fn load_desktop_integration() -> DesktopIntegrationSnapshot {
    if desktop_invoke_function().is_none() {
        return DesktopIntegrationSnapshot {
            tauri_available: false,
            ..DesktopIntegrationSnapshot::default()
        };
    }

    let status = invoke_desktop::<DesktopStatus>("get_status", None).await;
    let autostart = invoke_desktop::<AutostartStatus>("get_autostart_status", None).await;
    let hardening =
        invoke_desktop::<DesktopHardeningStatus>("get_desktop_hardening_status", None).await;

    let mut error = None;
    let status = match status {
        Ok(value) => Some(value),
        Err(message) => {
            error = Some(message);
            None
        }
    };
    let autostart = match autostart {
        Ok(value) => Some(value),
        Err(message) => {
            error = Some(error.map_or(message.clone(), |current| format!("{current}; {message}")));
            None
        }
    };
    let hardening = match hardening {
        Ok(value) => Some(value),
        Err(message) => {
            error = Some(error.map_or(message.clone(), |current| format!("{current}; {message}")));
            None
        }
    };

    DesktopIntegrationSnapshot {
        tauri_available: true,
        status,
        autostart,
        hardening,
        error,
    }
}

pub(crate) async fn set_desktop_autostart(enabled: bool) -> Result<AutostartStatus, String> {
    let args = Object::new();
    let _ = Reflect::set(
        &args,
        &JsValue::from_str("enabled"),
        &JsValue::from_bool(enabled),
    );
    invoke_desktop("set_autostart_enabled", Some(args)).await
}

async fn invoke_desktop<T>(command: &str, args: Option<Object>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let Some((this_arg, invoke)) = desktop_invoke_function() else {
        return Err("Tauri desktop bridge is not available in this browser.".to_string());
    };
    let raw = if let Some(args) = args {
        invoke.call2(&this_arg, &JsValue::from_str(command), &args)
    } else {
        invoke.call1(&this_arg, &JsValue::from_str(command))
    }
    .map_err(js_error_text)?;
    let promise = raw
        .dyn_into::<Promise>()
        .map_err(|_| format!("desktop command {command} did not return a Promise"))?;
    let value = JsFuture::from(promise).await.map_err(js_error_text)?;
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| format!("desktop command {command} returned invalid payload: {error}"))
}

fn desktop_invoke_function() -> Option<(JsValue, Function)> {
    let window = web_sys::window()?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__")).ok()?;
    if tauri.is_undefined() || tauri.is_null() {
        return None;
    }
    let core = Reflect::get(&tauri, &JsValue::from_str("core"))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
        .unwrap_or(tauri);
    let invoke = Reflect::get(&core, &JsValue::from_str("invoke"))
        .ok()?
        .dyn_into::<Function>()
        .ok()?;
    Some((core, invoke))
}

fn js_error_text(value: JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {value:?}"))
}
