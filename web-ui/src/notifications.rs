use crate::state::{ConsoleData, View};
use crate::{json_status, label_or, short_id, status_tone};
use serde_json::Value;
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

    html! {
        <section class={classes!("notification-center", (!has_notifications).then_some("quiet"))} aria-label="Operator notification center">
            <div class="notification-summary">
                <div>
                    <span>{ "Operator notifications" }</span>
                    <strong>{ if has_notifications {
                        format!("{} actionable / {} critical", props.notifications.len(), critical_count)
                    } else {
                        "No actionable notifications".to_string()
                    } }</strong>
                </div>
                <button class="secondary" onclick={props.on_toggle_critical.clone()}>
                    { if props.critical_muted { "Enable critical" } else { "Mute critical" } }
                </button>
            </div>
            <div class="notification-list">
                {
                    if props.critical_muted && critical_count > 0 {
                        html! {
                            <article class="notification-item muted">
                                <div>
                                    <span>{ "muted" }</span>
                                    <strong>{ format!("{critical_count} critical notifications hidden") }</strong>
                                    <p>{ "Critical browser and desktop forwarding stays disabled until re-enabled in Settings." }</p>
                                </div>
                            </article>
                        }
                    } else if visible_notifications.is_empty() {
                        html! {
                            <article class="notification-item neutral">
                                <div>
                                    <span>{ "clear" }</span>
                                    <strong>{ "Queue is clear" }</strong>
                                    <p>{ "Approvals, failed jobs, connector gates, ontology gates, and enterprise lanes report no current actionable notification." }</p>
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
                                            <span>{ notification.severity }</span>
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
        </section>
    }
}

pub(crate) fn console_notifications(data: &ConsoleData) -> Vec<ConsoleNotification> {
    let mut notifications = Vec::new();

    notifications.extend(data.approvals.data.iter().filter_map(|approval| {
        if approval.status == "pending" || approval.status == "requires_action" {
            Some(ConsoleNotification {
                key: format!("approval:{}", approval.id),
                severity: "warning",
                title: format!("Approval required: {}", label_or(&approval.kind, "runtime action")),
                detail: label_or(&approval.reason, &approval.id).to_string(),
                target: View::Agents,
                target_label: "Open approvals",
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
                title: format!("Execution job failed: {}", short_id(&job.id)),
                detail: job
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("status {}", label_or(&job.status, "failed"))),
                target: View::Workflows,
                target_label: "Open runs",
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
                title: format!("Session loop attention: {}", short_id(&job.id)),
                detail: job
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("status {}", label_or(&job.status, "stuck"))),
                target: View::Workflows,
                target_label: "Open runs",
            })
        } else {
            None
        }
    }));

    if let Some(notification) = json_gate_notification(
        "connector:production-readiness",
        "Connector production readiness blocked",
        &data.native_connector_production_readiness.data,
        View::Deploy,
        "Open deploy",
    ) {
        notifications.push(notification);
    }

    if let Some(notification) = json_gate_notification(
        "ontology:engine-readiness",
        "Ontology engine readiness blocked",
        &data.ontology_engine_readiness.data,
        View::Semantic,
        "Open ontology",
    ) {
        notifications.push(notification);
    }

    notifications.extend(
        data.enterprise_product_readiness
            .data
            .lanes
            .iter()
            .filter_map(|lane| {
                let blocker = lane.blockers.first().or_else(|| lane.next_actions.first());
                let tone = status_tone(&lane.status);
                if tone == "bad" || (tone == "warn" && blocker.is_some()) {
                    Some(ConsoleNotification {
                        key: format!("enterprise:{}", label_or(&lane.id, &lane.title)),
                        severity: if tone == "bad" { "critical" } else { "warning" },
                        title: format!(
                            "Enterprise lane regression: {}",
                            label_or(&lane.title, &lane.id)
                        ),
                        detail: blocker.cloned().unwrap_or_else(|| {
                            format!(
                                "{} -> {}",
                                label_or(&lane.current_evidence_class, "current evidence"),
                                label_or(&lane.required_evidence_class, "required evidence")
                            )
                        }),
                        target: View::Deploy,
                        target_label: "Open readiness",
                    })
                } else {
                    None
                }
            }),
    );

    if data.enterprise_product_readiness.data.completion_blocked
        && data.enterprise_product_readiness.data.lanes.is_empty()
    {
        notifications.push(ConsoleNotification {
            key: "enterprise:completion-blocked".to_string(),
            severity: "critical",
            title: "Enterprise completion blocked".to_string(),
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
            target_label: "Open readiness",
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
        severity: if tone == "bad" || !blockers.is_empty() {
            "critical"
        } else {
            "warning"
        },
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
    collect_json_blockers(value, 0, &mut blockers);
    blockers.sort();
    blockers.dedup();
    blockers.truncate(5);
    blockers
}

fn collect_json_blockers(value: &Value, depth: usize, blockers: &mut Vec<String>) {
    if depth > 3 || blockers.len() >= 12 {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
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
                                    collect_json_blockers(item, depth + 1, blockers);
                                }
                            }
                        }
                        Value::Object(_) => collect_json_blockers(child, depth + 1, blockers),
                        _ => {}
                    }
                } else if depth < 2 {
                    collect_json_blockers(child, depth + 1, blockers);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_json_blockers(item, depth + 1, blockers);
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
