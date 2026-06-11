use crate::components::{KeyMetrics, Panel};
use crate::desktop_bridge::{
    AutostartStatus, DesktopHardeningStatus, DesktopIntegrationSnapshot, load_desktop_integration,
    set_desktop_autostart,
};
use crate::state::ConsoleData;
use crate::{count_errors, count_loading, storage_get};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct SettingsProps {
    pub(crate) data: ConsoleData,
    pub(crate) critical_muted: bool,
    pub(crate) notification_count: usize,
    pub(crate) critical_notification_count: usize,
    pub(crate) on_toggle_critical: Callback<MouseEvent>,
}

#[component]
pub(crate) fn SettingsView(props: &SettingsProps) -> Html {
    let token_saved = storage_get("mandoforge.adminToken")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let desktop_snapshot = use_state(DesktopIntegrationSnapshot::default);
    let desktop_action_status = use_state(String::new);

    {
        let desktop_snapshot = desktop_snapshot.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                desktop_snapshot.set(load_desktop_integration().await);
            });
            || ()
        });
    }

    let refresh_desktop = {
        let desktop_snapshot = desktop_snapshot.clone();
        let desktop_action_status = desktop_action_status.clone();
        Callback::from(move |_| {
            let desktop_snapshot = desktop_snapshot.clone();
            let desktop_action_status = desktop_action_status.clone();
            spawn_local(async move {
                desktop_action_status.set("Refreshing desktop integration...".to_string());
                desktop_snapshot.set(load_desktop_integration().await);
                desktop_action_status.set("Desktop integration refreshed.".to_string());
            });
        })
    };

    let toggle_autostart = {
        let desktop_snapshot = desktop_snapshot.clone();
        let desktop_action_status = desktop_action_status.clone();
        let next_enabled = !desktop_snapshot
            .autostart
            .as_ref()
            .map(|status| status.enabled)
            .unwrap_or(false);
        Callback::from(move |_| {
            let desktop_snapshot = desktop_snapshot.clone();
            let desktop_action_status = desktop_action_status.clone();
            spawn_local(async move {
                desktop_action_status.set(if next_enabled {
                    "Enabling desktop autostart...".to_string()
                } else {
                    "Disabling desktop autostart...".to_string()
                });
                match set_desktop_autostart(next_enabled).await {
                    Ok(status) => {
                        let mut next_snapshot = load_desktop_integration().await;
                        if next_snapshot.autostart.is_none() {
                            next_snapshot.autostart = Some(status);
                        }
                        desktop_snapshot.set(next_snapshot);
                        desktop_action_status.set(if next_enabled {
                            "Desktop autostart enabled by explicit operator action.".to_string()
                        } else {
                            "Desktop autostart disabled.".to_string()
                        });
                    }
                    Err(error) => desktop_action_status
                        .set(format!("Desktop autostart update failed: {error}")),
                }
            });
        })
    };

    html! {
        <div class="page-grid">
            <Panel title="Notification policy">
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ "Critical operator notifications" }</span>
                            <strong>{ if props.critical_muted { "Muted in this browser" } else { "Enabled in this browser" } }</strong>
                            <p>{ "Applies to failed execution jobs, stalled session-loop jobs, connector blockers, ontology blockers, and enterprise readiness regressions." }</p>
                        </div>
                        <button onclick={props.on_toggle_critical.clone()}>
                            { if props.critical_muted { "Enable critical notifications" } else { "Mute critical notifications" } }
                        </button>
                    </div>
                    <KeyMetrics values={vec![
                        ("Actionable notifications".to_string(), props.notification_count.to_string()),
                        ("Critical notifications".to_string(), props.critical_notification_count.to_string()),
                        ("Deduplication".to_string(), "stable event key".to_string()),
                        ("Native forwarding".to_string(), native_forwarding_label(&desktop_snapshot).to_string()),
                    ]} />
                </div>
            </Panel>
            <Panel title="Console identity">
                <KeyMetrics values={vec![
                    ("Admin token".to_string(), if token_saved { "saved locally".to_string() } else { "not saved".to_string() }),
                    ("API auth".to_string(), "Bearer + x-mandoforge identity headers".to_string()),
                    ("API errors".to_string(), count_errors(&props.data).to_string()),
                    ("Refreshing endpoints".to_string(), count_loading(&props.data).to_string()),
                ]} />
            </Panel>
            <Panel title="Desktop integration">
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ "Desktop shell" }</span>
                            <strong>{ desktop_shell_label(&desktop_snapshot) }</strong>
                            <p>{ desktop_shell_detail(&desktop_snapshot) }</p>
                        </div>
                        <button class="secondary" onclick={refresh_desktop}>{ "Refresh desktop" }</button>
                    </div>
                    <KeyMetrics values={desktop_metrics(&desktop_snapshot)} />
                    <div class="settings-row">
                        <div>
                            <span>{ "Autostart" }</span>
                            <strong>{ autostart_label(desktop_snapshot.autostart.as_ref()) }</strong>
                            <p>{ "Autostart stays disabled unless an operator explicitly changes it from this desktop shell." }</p>
                        </div>
                        <button
                            disabled={!desktop_snapshot.tauri_available || desktop_snapshot.autostart.is_none()}
                            onclick={toggle_autostart}
                        >
                            { if desktop_snapshot.autostart.as_ref().map(|status| status.enabled).unwrap_or(false) { "Disable autostart" } else { "Enable autostart" } }
                        </button>
                    </div>
                    {
                        if desktop_action_status.is_empty() && desktop_snapshot.error.is_none() {
                            html! {}
                        } else {
                            html! {
                                <p class="empty">
                                    { desktop_snapshot.error.clone().unwrap_or_else(|| (*desktop_action_status).clone()) }
                                </p>
                            }
                        }
                    }
                </div>
            </Panel>
            <Panel title="Desktop boundary">
                <KeyMetrics values={desktop_boundary_metrics(desktop_snapshot.hardening.as_ref())} />
            </Panel>
        </div>
    }
}

fn desktop_shell_label(snapshot: &DesktopIntegrationSnapshot) -> &'static str {
    if !snapshot.tauri_available {
        "browser console"
    } else if snapshot.status.is_some() {
        "desktop bridge connected"
    } else {
        "desktop bridge incomplete"
    }
}

fn desktop_shell_detail(snapshot: &DesktopIntegrationSnapshot) -> String {
    snapshot.status.as_ref().map_or_else(
        || {
            "Open in the Tauri desktop shell to inspect native notifications, single-instance, and autostart status.".to_string()
        },
        |status| {
            format!(
                "{} / {} / embedded owned: {}",
                status.api_base_url, status.connected_state, status.embedded_api_owned
            )
        },
    )
}

fn native_forwarding_label(snapshot: &DesktopIntegrationSnapshot) -> &'static str {
    if snapshot
        .hardening
        .as_ref()
        .map(|status| status.native_notifications_enabled)
        .unwrap_or(false)
    {
        "critical-only desktop bridge"
    } else {
        "browser center only"
    }
}

fn autostart_label(status: Option<&AutostartStatus>) -> &'static str {
    match status {
        Some(status) if status.enabled => "enabled",
        Some(_) => "disabled",
        None => "not available in browser",
    }
}

fn desktop_metrics(snapshot: &DesktopIntegrationSnapshot) -> Vec<(String, String)> {
    vec![
        (
            "Tauri bridge".to_string(),
            if snapshot.tauri_available {
                "available".to_string()
            } else {
                "not available".to_string()
            },
        ),
        (
            "API mode".to_string(),
            snapshot
                .status
                .as_ref()
                .map(|status| status.mode.clone())
                .unwrap_or_else(|| "browser".to_string()),
        ),
        (
            "Single instance".to_string(),
            snapshot
                .hardening
                .as_ref()
                .map(|status| {
                    if status.single_instance_enabled {
                        "enabled".to_string()
                    } else {
                        "not enabled".to_string()
                    }
                })
                .unwrap_or_else(|| "desktop only".to_string()),
        ),
        (
            "Autostart policy".to_string(),
            snapshot
                .autostart
                .as_ref()
                .map(|status| status.policy.clone())
                .unwrap_or_else(|| "explicit opt-in only".to_string()),
        ),
    ]
}

fn desktop_boundary_metrics(status: Option<&DesktopHardeningStatus>) -> Vec<(String, String)> {
    vec![
        (
            "Signed distribution".to_string(),
            bool_label(status.map(|status| status.signed_distribution_ready).unwrap_or(false)),
        ),
        (
            "Updater".to_string(),
            bool_label(status.map(|status| status.updater_enabled).unwrap_or(false)),
        ),
        (
            "CSP".to_string(),
            bool_label(status.map(|status| status.csp_configured).unwrap_or(false)),
        ),
        (
            "Enterprise completion".to_string(),
            bool_label(
                status
                    .map(|status| status.enterprise_completion_claimed)
                    .unwrap_or(false),
            ),
        ),
    ]
}

fn bool_label(value: bool) -> String {
    if value {
        "ready".to_string()
    } else {
        "not ready".to_string()
    }
}
