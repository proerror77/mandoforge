use crate::state::{ConsoleData, UiLang, View};
use crate::{json_status, label_or, short_id, status_tone};
use js_sys::{Function, Object, Promise, Reflect};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConsoleNotification {
    pub(crate) key: String,
    pub(crate) severity: &'static str,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) target: View,
    pub(crate) target_label: &'static str,
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct NotificationCenterProps {
    pub(crate) notifications: Vec<ConsoleNotification>,
    pub(crate) lang: UiLang,
    pub(crate) critical_muted: bool,
    pub(crate) on_toggle_critical: Callback<MouseEvent>,
    pub(crate) on_view: Callback<View>,
}

#[component]
pub(crate) fn NotificationCenter(props: &NotificationCenterProps) -> Html {
    let critical_count = props
        .notifications
        .iter()
        .filter(|notification| notification.severity == "critical")
        .count();
    let visible_notifications = props
        .notifications
        .iter()
        .filter(|notification| !(props.critical_muted && notification.severity == "critical"))
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    let has_notifications = !props.notifications.is_empty();
    let on_toggle_critical = {
        let on_toggle_critical = props.on_toggle_critical.clone();
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_toggle_critical.emit(event);
        })
    };

    html! {
        <details
            class={classes!("notification-center", (!has_notifications).then_some("quiet"))}
            aria-label={props.lang.text("Operator notification center", "操作员通知中心")}
            open={critical_count > 0}
        >
            <summary class="notification-summary">
                <div>
                    <span>{ props.lang.text("Operator Notifications", "操作员通知") }</span>
                    <strong>{ if has_notifications {
                        if props.lang == UiLang::En {
                            format!("{} actionable / {} critical", props.notifications.len(), critical_count)
                        } else {
                            format!("{} 条待处理 / {} 条严重", props.notifications.len(), critical_count)
                        }
                    } else {
                        props.lang.text("No actionable notifications", "没有待处理通知").to_string()
                    } }</strong>
                </div>
            </summary>
            <div class="notification-actions">
                <button class="secondary" onclick={on_toggle_critical}>
                    { if props.critical_muted { props.lang.text("Resume critical", "恢复严重通知") } else { props.lang.text("Mute critical", "静音严重通知") } }
                </button>
            </div>
            <div class="notification-list">
                {
                    if props.critical_muted && critical_count > 0 {
                        html! {
                            <article class="notification-item muted">
                                <div>
                                    <span>{ props.lang.text("Muted", "已静音") }</span>
                                    <strong>{ if props.lang == UiLang::En { format!("{critical_count} critical notifications hidden") } else { format!("{critical_count} 条严重通知已隐藏") } }</strong>
                                    <p>{ props.lang.text("Browser and desktop forwarding stay disabled until resumed.", "重新开启前，浏览器和桌面都不会转发严重通知。") }</p>
                                </div>
                            </article>
                        }
                    } else if visible_notifications.is_empty() {
                        html! {
                            <article class="notification-item neutral">
                                <div>
                                    <span>{ props.lang.text("Normal", "正常") }</span>
                                    <strong>{ props.lang.text("Queue empty", "队列为空") }</strong>
                                    <p>{ props.lang.text("Approvals, failed jobs, connectors, ontology gates, and enterprise readiness have no immediate action.", "审批、失败任务、连接器、本体闸门和企业上线面目前都没有需要立即处理的通知。") }</p>
                                </div>
                            </article>
                        }
                    } else {
                        html! {
                            { for visible_notifications.into_iter().map(|notification| {
                                let on_view = props.on_view.clone();
                                let target = notification.target;
                                html! {
                                    <article class={classes!("notification-item", notification.severity)} key={notification.key.clone()}>
                                        <div>
                                            <span>{ severity_label(props.lang, notification.severity) }</span>
                                            <strong>{ notification.title }</strong>
                                            <p>{ notification.detail }</p>
                                        </div>
                                        <button class="secondary" onclick={Callback::from(move |_| on_view.emit(target))}>
                                            { notification.target_label }
                                        </button>
                                    </article>
                                }
                            }) }
                        }
                    }
                }
            </div>
        </details>
    }
}

fn severity_label(lang: UiLang, severity: &str) -> &'static str {
    match lang {
        UiLang::En => match severity {
            "critical" => "Critical",
            "warning" => "Warning",
            "neutral" => "Normal",
            _ => "Notice",
        },
        UiLang::Zh => match severity {
            "critical" => "严重",
            "warning" => "警告",
            "neutral" => "正常",
            _ => "通知",
        },
    }
}

pub(crate) fn console_notifications(data: &ConsoleData, lang: UiLang) -> Vec<ConsoleNotification> {
    let mut notifications = Vec::new();

    notifications.extend(data.approvals.data.iter().filter_map(|approval| {
        if approval.status == "pending" || approval.status == "requires_action" {
            Some(ConsoleNotification {
                key: format!("approval:{}", approval.id),
                severity: "warning",
                title: if lang == UiLang::En {
                    format!("Approval required: {}", label_or(&approval.kind, "runtime action"))
                } else {
                    format!("需要审批：{}", label_or(&approval.kind, "运行时动作"))
                },
                detail: label_or(&approval.reason, &approval.id).to_string(),
                target: View::Agents,
                target_label: lang.text("Open approvals", "查看审批"),
            })
        } else {
            None
        }
    }));

    notifications.extend(data.execution_jobs.data.iter().filter_map(|job| {
        if status_tone(&job.status) == "bad" || job.last_error.is_some() {
            Some(ConsoleNotification {
                key: format!("execution-job:{}", job.id),
                severity: "critical",
                title: if lang == UiLang::En {
                    format!("Execution job failed: {}", short_id(&job.id))
                } else {
                    format!("执行任务失败：{}", short_id(&job.id))
                },
                detail: job
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("status {}", label_or(&job.status, "failed"))),
                target: View::Workflows,
                target_label: lang.text("Open runs", "查看运行"),
            })
        } else {
            None
        }
    }));

    notifications.extend(data.session_loop_jobs.data.iter().filter_map(|job| {
        let stuck = matches!(job.status.as_str(), "stuck" | "timed_out" | "timeout");
        if stuck || status_tone(&job.status) == "bad" || job.last_error.is_some() {
            Some(ConsoleNotification {
                key: format!("session-loop-job:{}", job.id),
                severity: "critical",
                title: if lang == UiLang::En {
                    format!("Session loop needs attention: {}", short_id(&job.id))
                } else {
                    format!("Session 循环需要处理：{}", short_id(&job.id))
                },
                detail: job
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("status {}", label_or(&job.status, "stuck"))),
                target: View::Workflows,
                target_label: lang.text("Open runs", "查看运行"),
            })
        } else {
            None
        }
    }));

    if let Some(notification) = json_gate_notification(
        "connector:production-readiness",
        lang.text("Connector production readiness is blocked", "连接器生产就绪被阻塞"),
        &data.native_connector_production_readiness.data,
        View::Deploy,
        lang.text("Open System Ops", "查看上线"),
    ) {
        notifications.push(notification);
    }

    if let Some(notification) = json_gate_notification(
        "ontology:engine-readiness",
        lang.text("Ontology engine readiness is blocked", "本体引擎就绪被阻塞"),
        &data.ontology_engine_readiness.data,
        View::Semantic,
        lang.text("Open Ontology", "查看本体"),
    ) {
        notifications.push(notification);
    }

    let enterprise_readiness = &data.enterprise_product_readiness.data;
    let blocked_enterprise_lanes = enterprise_readiness
        .lanes
        .iter()
        .filter(|lane| status_tone(&lane.status) == "bad")
        .count();
    let warning_enterprise_lanes = enterprise_readiness
        .lanes
        .iter()
        .filter(|lane| status_tone(&lane.status) == "warn")
        .count();
    if blocked_enterprise_lanes > 0 || warning_enterprise_lanes > 0 {
        let first_lane = enterprise_readiness
            .lanes
            .iter()
            .find(|lane| status_tone(&lane.status) == "bad")
            .or_else(|| {
                enterprise_readiness
                    .lanes
                    .iter()
                    .find(|lane| status_tone(&lane.status) == "warn")
            });
        let first_action = first_lane
            .and_then(|lane| lane.blockers.first().or_else(|| lane.next_actions.first()))
            .cloned()
            .or_else(|| enterprise_readiness.next_actions.first().cloned())
            .unwrap_or_else(|| {
                label_or(
                    &enterprise_readiness.message,
                    "Enterprise readiness has open evidence requirements.",
                )
                .to_string()
            });
        let detail = if lang == UiLang::En {
            format!(
                "{blocked_enterprise_lanes} blocked lanes, {warning_enterprise_lanes} pilot/warning lanes. First action: {first_action}"
            )
        } else {
            format!(
                "{blocked_enterprise_lanes} 个阻塞面，{warning_enterprise_lanes} 个试点/警告面。第一步：{first_action}"
            )
        };
        notifications.push(ConsoleNotification {
            key: "enterprise:readiness-summary".to_string(),
            severity: "warning",
            title: lang
                .text(
                    "Enterprise readiness needs production evidence",
                    "企业上线仍需要生产证据",
                )
                .to_string(),
            detail,
            target: View::Deploy,
            target_label: lang.text("Open readiness", "查看就绪"),
        });
    }

    if data.enterprise_product_readiness.data.completion_blocked
        && data.enterprise_product_readiness.data.lanes.is_empty()
    {
        notifications.push(ConsoleNotification {
            key: "enterprise:completion-blocked".to_string(),
            severity: "critical",
            title: lang.text("Enterprise completion is blocked", "企业级完成状态被阻塞").to_string(),
            detail: data
                .enterprise_product_readiness
                .data
                .next_actions
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    label_or(
                        &data.enterprise_product_readiness.data.message,
                        "Enterprise readiness endpoint reports completion blocked.",
                    )
                    .to_string()
            }),
            target: View::Deploy,
            target_label: lang.text("Open readiness", "查看就绪"),
        });
    }

    notifications.sort_by(|left, right| {
        notification_rank(left.severity)
            .cmp(&notification_rank(right.severity))
            .then_with(|| left.key.cmp(&right.key))
    });
    notifications.dedup_by(|left, right| left.key == right.key);
    notifications
}

pub(crate) fn forward_critical_notifications_to_desktop(notifications: &[ConsoleNotification]) {
    let Some((this_arg, invoke)) = desktop_invoke_function() else {
        return;
    };
    for notification in notifications
        .iter()
        .filter(|notification| notification.severity == "critical")
    {
        if desktop_notification_forwarded(&notification.key)
            || desktop_notification_forward_pending(&notification.key)
        {
            continue;
        }
        mark_desktop_notification_forward_pending(&notification.key);
        let payload = Object::new();
        set_js_string(&payload, "key", &notification.key);
        set_js_string(&payload, "severity", notification.severity);
        set_js_string(&payload, "title", &notification.title);
        set_js_string(&payload, "detail", &notification.detail);
        set_js_string(&payload, "target_label", notification.target_label);

        let args = Object::new();
        let _ = Reflect::set(&args, &JsValue::from_str("payload"), &payload);
        let key = notification.key.clone();
        let raw = invoke.call2(
            &this_arg,
            &JsValue::from_str("forward_console_notification"),
            &args,
        );
        let Ok(raw) = raw else {
            clear_desktop_notification_forward_pending(&key);
            continue;
        };
        let Ok(promise) = raw.dyn_into::<Promise>() else {
            clear_desktop_notification_forward_pending(&key);
            continue;
        };
        spawn_local(async move {
            if JsFuture::from(promise).await.is_ok() {
                mark_desktop_notification_forwarded(&key);
            }
            clear_desktop_notification_forward_pending(&key);
        });
    }
}

fn json_gate_notification(
    key: &str,
    title: &str,
    value: &Value,
    target: View,
    target_label: &'static str,
) -> Option<ConsoleNotification> {
    if value.is_null() {
        return None;
    }
    let status = json_status(value);
    let tone = status_tone(&status);
    let blockers = json_blockers(value);
    if tone != "bad" && tone != "warn" && blockers.is_empty() {
        return None;
    }
    Some(ConsoleNotification {
        key: key.to_string(),
        severity: "warning",
        title: title.to_string(),
        detail: blockers
            .first()
            .cloned()
            .unwrap_or_else(|| format!("status {status}")),
        target,
        target_label,
    })
}

fn json_blockers(value: &Value) -> Vec<String> {
    let mut blockers = Vec::new();
    let mut visited = 0usize;
    collect_json_blockers(value, 0, &mut blockers, &mut visited);
    blockers.sort();
    blockers.dedup();
    blockers.truncate(5);
    blockers
}

fn collect_json_blockers(
    value: &Value,
    depth: usize,
    blockers: &mut Vec<String>,
    visited: &mut usize,
) {
    if depth > 3 || blockers.len() >= 12 || *visited >= 500 {
        return;
    }
    *visited += 1;
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if *visited >= 500 {
                    break;
                }
                let key_lower = key.to_ascii_lowercase();
                let relevant = key_lower.contains("blocker")
                    || key_lower.contains("blocked")
                    || key_lower.contains("failure")
                    || key_lower.contains("error")
                    || key_lower.contains("next_action")
                    || key_lower.contains("reason");
                if relevant {
                    match child {
                        Value::String(text) if !text.trim().is_empty() => {
                            blockers.push(text.to_string())
                        }
                        Value::Array(items) => {
                            for item in items {
                                if let Some(text) =
                                    item.as_str().filter(|text| !text.trim().is_empty())
                                {
                                    blockers.push(text.to_string());
                                } else {
                                    collect_json_blockers(item, depth + 1, blockers, visited);
                                }
                            }
                        }
                        Value::Object(_) => {
                            collect_json_blockers(child, depth + 1, blockers, visited)
                        }
                        _ => {}
                    }
                } else if depth < 2 {
                    collect_json_blockers(child, depth + 1, blockers, visited);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if *visited >= 500 {
                    break;
                }
                collect_json_blockers(item, depth + 1, blockers, visited);
            }
        }
        _ => {}
    }
}

fn notification_rank(severity: &str) -> usize {
    match severity {
        "critical" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 3,
    }
}

fn desktop_invoke_function() -> Option<(JsValue, Function)> {
    let window = web_sys::window()?;
    let internals =
        Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI_INTERNALS__")).ok()?;
    if internals.is_undefined() || internals.is_null() {
        return None;
    }
    let invoke = Reflect::get(&internals, &JsValue::from_str("invoke"))
        .ok()?
        .dyn_into::<Function>()
        .ok()?;
    Some((internals, invoke))
}

fn set_js_string(target: &Object, key: &str, value: &str) {
    let _ = Reflect::set(target, &JsValue::from_str(key), &JsValue::from_str(value));
}

fn desktop_notification_forwarded(key: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| {
            storage
                .get_item(&desktop_notification_storage_key(key))
                .ok()
                .flatten()
        })
        .is_some()
}

fn mark_desktop_notification_forwarded(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(&desktop_notification_storage_key(key), "1");
    }
}

fn desktop_notification_storage_key(key: &str) -> String {
    format!("mandoforge.nativeNotificationForwarded.{key}")
}

fn desktop_notification_forward_pending(key: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.session_storage().ok().flatten())
        .and_then(|storage| {
            storage
                .get_item(&desktop_notification_pending_storage_key(key))
                .ok()
                .flatten()
        })
        .is_some()
}

fn mark_desktop_notification_forward_pending(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.session_storage().ok().flatten())
    {
        let _ = storage.set_item(&desktop_notification_pending_storage_key(key), "1");
    }
}

fn clear_desktop_notification_forward_pending(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.session_storage().ok().flatten())
    {
        let _ = storage.remove_item(&desktop_notification_pending_storage_key(key));
    }
}

fn desktop_notification_pending_storage_key(key: &str) -> String {
    format!("mandoforge.nativeNotificationPending.{key}")
}
