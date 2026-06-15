use crate::api::{
    DEFAULT_DEPLOYMENT_TARGET, DEFAULT_ONTOLOGY_DOMAIN_SCOPE, DEFAULT_ONTOLOGY_MEMORY_SCOPE,
    DEFAULT_ONTOLOGY_OBJECTIVE, DEFAULT_ONTOLOGY_WORKFLOW_SCOPE, DEPLOYMENT_TARGET_KEY,
    ONTOLOGY_DOMAIN_SCOPE_KEY, ONTOLOGY_MEMORY_SCOPE_KEY, ONTOLOGY_OBJECTIVE_KEY,
    ONTOLOGY_WORKFLOW_SCOPE_KEY, get_admin_token, set_admin_token,
};
use crate::components::{KeyMetrics, Panel};
use crate::desktop_bridge::{
    AutostartStatus, DesktopHardeningStatus, DesktopIntegrationSnapshot, load_desktop_integration,
    set_desktop_autostart,
};
use crate::state::{ConsoleData, UiLang};
use crate::{count_errors, count_loading, storage_get, storage_set};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, SubmitEvent};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct SettingsProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
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
    let token_input = use_state(get_admin_token);
    let token_action_status = use_state(String::new);
    let ontology_domain_scope = use_state(|| {
        stored_console_default(ONTOLOGY_DOMAIN_SCOPE_KEY, DEFAULT_ONTOLOGY_DOMAIN_SCOPE)
    });
    let ontology_workflow_scope = use_state(|| {
        stored_console_default(ONTOLOGY_WORKFLOW_SCOPE_KEY, DEFAULT_ONTOLOGY_WORKFLOW_SCOPE)
    });
    let ontology_memory_scope = use_state(|| {
        stored_console_default(ONTOLOGY_MEMORY_SCOPE_KEY, DEFAULT_ONTOLOGY_MEMORY_SCOPE)
    });
    let ontology_objective = use_state(|| {
        stored_console_default(ONTOLOGY_OBJECTIVE_KEY, DEFAULT_ONTOLOGY_OBJECTIVE)
    });
    let deployment_target =
        use_state(|| stored_console_default(DEPLOYMENT_TARGET_KEY, DEFAULT_DEPLOYMENT_TARGET));
    let defaults_action_status = use_state(String::new);
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

    let save_token = {
        let token_input = token_input.clone();
        let token_action_status = token_action_status.clone();
        let lang = props.lang;
        Callback::from(move |_| {
            set_admin_token(&token_input);
            token_action_status.set(
                lang.text("Admin token saved locally.", "管理员 token 已本地保存。")
                    .to_string(),
            );
        })
    };

    let clear_token = {
        let token_input = token_input.clone();
        let token_action_status = token_action_status.clone();
        let lang = props.lang;
        Callback::from(move |_| {
            token_input.set(String::new());
            set_admin_token("");
            token_action_status.set(
                lang.text("Admin token cleared.", "管理员 token 已清除。")
                    .to_string(),
            );
        })
    };

    let save_console_defaults = {
        let ontology_domain_scope = ontology_domain_scope.clone();
        let ontology_workflow_scope = ontology_workflow_scope.clone();
        let ontology_memory_scope = ontology_memory_scope.clone();
        let ontology_objective = ontology_objective.clone();
        let deployment_target = deployment_target.clone();
        let defaults_action_status = defaults_action_status.clone();
        let lang = props.lang;
        Callback::from(move |_| {
            storage_set(ONTOLOGY_DOMAIN_SCOPE_KEY, ontology_domain_scope.trim());
            storage_set(ONTOLOGY_WORKFLOW_SCOPE_KEY, ontology_workflow_scope.trim());
            storage_set(ONTOLOGY_MEMORY_SCOPE_KEY, ontology_memory_scope.trim());
            storage_set(ONTOLOGY_OBJECTIVE_KEY, ontology_objective.trim());
            storage_set(DEPLOYMENT_TARGET_KEY, deployment_target.trim());
            defaults_action_status.set(
                lang.text("Console defaults saved locally.", "控制台默认值已本地保存。")
                    .to_string(),
            );
        })
    };

    html! {
        <div class="page-grid">
                <Panel title={props.lang.text("Notification Policy", "通知策略")}>
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ props.lang.text("Critical operator notifications", "严重操作员通知") }</span>
                            <strong>{ if props.critical_muted { props.lang.text("Muted in this browser", "此浏览器已静音") } else { props.lang.text("Enabled in this browser", "此浏览器已启用") } }</strong>
                            <p>{ props.lang.text(
                                "Applies to failed execution jobs, stalled session-loop jobs, connector blockers, ontology blockers, and enterprise readiness regressions.",
                                "适用于执行任务失败、session-loop 卡住、连接器阻塞、本体阻塞和企业就绪状态回退。"
                            ) }</p>
                        </div>
                        <button onclick={props.on_toggle_critical.clone()}>
                            { if props.critical_muted { props.lang.text("Enable critical notifications", "启用严重通知") } else { props.lang.text("Mute critical notifications", "静音严重通知") } }
                        </button>
                    </div>
                    <KeyMetrics values={vec![
                        (props.lang.text("Actionable notifications", "可处理通知").to_string(), props.notification_count.to_string()),
                        (props.lang.text("Critical notifications", "严重通知").to_string(), props.critical_notification_count.to_string()),
                        (props.lang.text("Deduplication", "去重").to_string(), props.lang.text("stable event key", "稳定事件 key").to_string()),
                        (props.lang.text("Native forwarding", "系统通知转发").to_string(), native_forwarding_label(props.lang, &desktop_snapshot).to_string()),
                    ]} />
                </div>
            </Panel>
            <Panel title={props.lang.text("Console Defaults", "控制台默认值")}>
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ props.lang.text("Ontology Builder defaults", "Ontology Builder 默认值") }</span>
                            <strong>{ props.lang.text("Used by preview proposal and deployment verify actions", "用于提案预览与部署验证动作") }</strong>
                            <p>{ props.lang.text(
                                "These values replace the old hard-coded demo scopes. They are browser-local until the backend exposes tenant configuration.",
                                "这些值取代旧的硬编码 demo scope。在后端提供租户配置前，它们保存在当前浏览器。"
                            ) }</p>
                        </div>
                    </div>
                    <div class="settings-form-grid">
                        <label>
                            <span>{ props.lang.text("Domain scope", "领域 scope") }</span>
                            <input
                                id="mandoforge-ontology-domain-scope"
                                name="mandoforge-ontology-domain-scope"
                                value={(*ontology_domain_scope).clone()}
                                placeholder={DEFAULT_ONTOLOGY_DOMAIN_SCOPE}
                                oninput={settings_input(ontology_domain_scope.clone())}
                            />
                        </label>
                        <label>
                            <span>{ props.lang.text("Workflow scope", "流程 scope") }</span>
                            <input
                                id="mandoforge-ontology-workflow-scope"
                                name="mandoforge-ontology-workflow-scope"
                                value={(*ontology_workflow_scope).clone()}
                                placeholder={DEFAULT_ONTOLOGY_WORKFLOW_SCOPE}
                                oninput={settings_input(ontology_workflow_scope.clone())}
                            />
                        </label>
                        <label>
                            <span>{ props.lang.text("Memory scope", "记忆 scope") }</span>
                            <input
                                id="mandoforge-ontology-memory-scope"
                                name="mandoforge-ontology-memory-scope"
                                value={(*ontology_memory_scope).clone()}
                                placeholder={DEFAULT_ONTOLOGY_MEMORY_SCOPE}
                                oninput={settings_input(ontology_memory_scope.clone())}
                            />
                        </label>
                        <label>
                            <span>{ props.lang.text("Deployment target", "部署目标") }</span>
                            <input
                                id="mandoforge-deployment-target"
                                name="mandoforge-deployment-target"
                                value={(*deployment_target).clone()}
                                placeholder={DEFAULT_DEPLOYMENT_TARGET}
                                oninput={settings_input(deployment_target.clone())}
                            />
                        </label>
                        <label class="settings-wide-field">
                            <span>{ props.lang.text("Ontology objective", "本体目标") }</span>
                            <input
                                id="mandoforge-ontology-objective"
                                name="mandoforge-ontology-objective"
                                value={(*ontology_objective).clone()}
                                placeholder={DEFAULT_ONTOLOGY_OBJECTIVE}
                                oninput={settings_input(ontology_objective.clone())}
                            />
                        </label>
                    </div>
                    <div class="settings-row">
                        <button type="button" onclick={save_console_defaults}>{ props.lang.text("Save defaults", "保存默认值") }</button>
                        {
                            if defaults_action_status.is_empty() {
                                html! {}
                            } else {
                                html! { <p class="empty">{ defaults_action_status.as_str() }</p> }
                            }
                        }
                    </div>
                </div>
            </Panel>
            <Panel title={props.lang.text("Console Identity", "控制台身份")}>
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ props.lang.text("API authentication", "API 认证") }</span>
                            <strong>{ if token_saved { props.lang.text("Admin token saved locally", "管理员 token 已本地保存") } else { props.lang.text("Admin token not saved", "管理员 token 未保存") } }</strong>
                            <p>{ props.lang.text(
                                "Bearer token for live gates, production evidence, and console APIs. Roles are derived by the API, not declared by the browser.",
                                "用于访问实时闸门、生产证据和控制台 API 的 Bearer token。角色由 API 推导，浏览器不再自报。"
                            ) }</p>
                        </div>
                    </div>
                    <form
                        class="settings-token-row"
                        onsubmit={Callback::from(|event: SubmitEvent| event.prevent_default())}
                    >
                        <input
                            id="mandoforge-admin-token"
                            name="mandoforge-admin-token"
                            value={(*token_input).clone()}
                            placeholder="MANDOFORGE_DEV_ADMIN_TOKEN"
                            type="password"
                            oninput={{
                                let token_input = token_input.clone();
                                Callback::from(move |event: InputEvent| {
                                    let input: HtmlInputElement = event.target_unchecked_into();
                                    token_input.set(input.value());
                                })
                            }}
                        />
                        <button type="button" onclick={save_token}>{ props.lang.text("Save token", "保存 token") }</button>
                        <button type="button" class="secondary" onclick={clear_token}>{ props.lang.text("Clear", "清除") }</button>
                    </form>
                    {
                        if token_action_status.is_empty() {
                            html! {}
                        } else {
                            html! { <p class="empty">{ token_action_status.as_str() }</p> }
                        }
                    }
                    <KeyMetrics values={vec![
                        (props.lang.text("Admin token", "管理员 token").to_string(), if token_saved { props.lang.text("saved locally", "已本地保存").to_string() } else { props.lang.text("not saved", "未保存").to_string() }),
                        (props.lang.text("API auth", "API 认证").to_string(), "Bearer + server-derived roles".to_string()),
                        (props.lang.text("API errors", "API 错误").to_string(), count_errors(&props.data).to_string()),
                        (props.lang.text("Refreshing endpoints", "刷新中的端点").to_string(), count_loading(&props.data).to_string()),
                    ]} />
                </div>
            </Panel>
            <Panel title={props.lang.text("Desktop Integration", "桌面端集成")}>
                <div class="settings-stack">
                    <div class="settings-row">
                        <div>
                            <span>{ props.lang.text("Desktop shell", "桌面外壳") }</span>
                            <strong>{ desktop_shell_label(props.lang, &desktop_snapshot) }</strong>
                            <p>{ desktop_shell_detail(props.lang, &desktop_snapshot) }</p>
                        </div>
                        <button class="secondary" onclick={refresh_desktop}>{ props.lang.text("Refresh desktop", "刷新桌面端") }</button>
                    </div>
                    <KeyMetrics values={desktop_metrics(props.lang, &desktop_snapshot)} />
                    <div class="settings-row">
                        <div>
                            <span>{ props.lang.text("Autostart", "自动启动") }</span>
                            <strong>{ autostart_label(props.lang, desktop_snapshot.autostart.as_ref()) }</strong>
                            <p>{ props.lang.text("Autostart stays disabled unless an operator explicitly changes it from this desktop shell.", "除非操作员在桌面端明确修改，否则自动启动保持关闭。") }</p>
                        </div>
                        <button
                            disabled={!desktop_snapshot.tauri_available || desktop_snapshot.autostart.is_none()}
                            onclick={toggle_autostart}
                        >
                            { if desktop_snapshot.autostart.as_ref().map(|status| status.enabled).unwrap_or(false) { props.lang.text("Disable autostart", "关闭自动启动") } else { props.lang.text("Enable autostart", "开启自动启动") } }
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
            <Panel title={props.lang.text("Desktop Boundary", "桌面端边界")}>
                <KeyMetrics values={desktop_boundary_metrics(props.lang, desktop_snapshot.hardening.as_ref())} />
            </Panel>
        </div>
    }
}

fn desktop_shell_label(lang: UiLang, snapshot: &DesktopIntegrationSnapshot) -> &'static str {
    if !snapshot.tauri_available {
        lang.text("browser console", "浏览器控制台")
    } else if snapshot.status.is_some() {
        lang.text("desktop bridge connected", "桌面桥已连接")
    } else {
        lang.text("desktop bridge incomplete", "桌面桥不完整")
    }
}

fn desktop_shell_detail(lang: UiLang, snapshot: &DesktopIntegrationSnapshot) -> String {
    snapshot.status.as_ref().map_or_else(
        || {
            lang.text(
                "Open in the Tauri desktop shell to inspect native notifications, single-instance, and autostart status.",
                "在 Tauri 桌面端打开后，可检查系统通知、单实例和自动启动状态。"
            ).to_string()
        },
        |status| {
            format!(
                "{} / {} / embedded owned: {}",
                status.api_base_url, status.connected_state, status.embedded_api_owned
            )
        },
    )
}

fn native_forwarding_label(lang: UiLang, snapshot: &DesktopIntegrationSnapshot) -> &'static str {
    if snapshot
        .hardening
        .as_ref()
        .map(|status| status.native_notifications_enabled)
        .unwrap_or(false)
    {
        lang.text("critical-only desktop bridge", "仅严重通知转发到桌面")
    } else {
        lang.text("browser center only", "仅浏览器通知中心")
    }
}

fn autostart_label(lang: UiLang, status: Option<&AutostartStatus>) -> &'static str {
    match status {
        Some(status) if status.enabled => lang.text("enabled", "已启用"),
        Some(_) => lang.text("disabled", "已关闭"),
        None => lang.text("not available in browser", "浏览器中不可用"),
    }
}

fn desktop_metrics(lang: UiLang, snapshot: &DesktopIntegrationSnapshot) -> Vec<(String, String)> {
    vec![
        (
            lang.text("Tauri bridge", "Tauri 桥").to_string(),
            if snapshot.tauri_available {
                lang.text("available", "可用").to_string()
            } else {
                lang.text("not available", "不可用").to_string()
            },
        ),
        (
            lang.text("API mode", "API 模式").to_string(),
            snapshot
                .status
                .as_ref()
                .map(|status| status.mode.clone())
                .unwrap_or_else(|| "browser".to_string()),
        ),
        (
            lang.text("Single instance", "单实例").to_string(),
            snapshot
                .hardening
                .as_ref()
                .map(|status| {
                    if status.single_instance_enabled {
                        lang.text("enabled", "已启用").to_string()
                    } else {
                        lang.text("not enabled", "未启用").to_string()
                    }
                })
                .unwrap_or_else(|| lang.text("desktop only", "仅桌面端").to_string()),
        ),
        (
            lang.text("Autostart policy", "自动启动策略").to_string(),
            snapshot
                .autostart
                .as_ref()
                .map(|status| status.policy.clone())
                .unwrap_or_else(|| lang.text("explicit opt-in only", "仅显式启用").to_string()),
        ),
    ]
}

fn desktop_boundary_metrics(
    lang: UiLang,
    status: Option<&DesktopHardeningStatus>,
) -> Vec<(String, String)> {
    vec![
        (
            lang.text("Signed distribution", "签名发布").to_string(),
            bool_label(lang, status.map(|status| status.signed_distribution_ready).unwrap_or(false)),
        ),
        (
            lang.text("Updater", "更新器").to_string(),
            bool_label(lang, status.map(|status| status.updater_enabled).unwrap_or(false)),
        ),
        (
            "CSP".to_string(),
            bool_label(lang, status.map(|status| status.csp_configured).unwrap_or(false)),
        ),
        (
            lang.text("Enterprise completion", "企业完成声明").to_string(),
            bool_label(
                lang,
                status
                    .map(|status| status.enterprise_completion_claimed)
                    .unwrap_or(false),
            ),
        ),
    ]
}

fn bool_label(lang: UiLang, value: bool) -> String {
    if value {
        lang.text("ready", "就绪").to_string()
    } else {
        lang.text("not ready", "未就绪").to_string()
    }
}

fn stored_console_default(key: &str, fallback: &str) -> String {
    storage_get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn settings_input(state: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let input: HtmlInputElement = event.target_unchecked_into();
        state.set(input.value());
    })
}
