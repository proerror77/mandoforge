use crate::api::{
    Approval, DeploymentVersion, EnterpriseProductReadiness, Session, ToolCall, WorkerJob,
    WorkflowPackInstallation, WorkflowPackMarketplace,
};
use crate::state::{UiLang, View};
use crate::{active_job_count, label_or, pretty_json, semantic_scope_summary, status_tone};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct OverviewButtonProps {
    pub(crate) label: &'static str,
    pub(crate) target: View,
    pub(crate) on_view: Callback<View>,
}

#[component]
pub(crate) fn OverviewButton(props: &OverviewButtonProps) -> Html {
    let target = props.target;
    let on_view = props.on_view.clone();
    html! {
        <button
            class="overview-action"
            onclick={Callback::from(move |_| on_view.emit(target))}
        >
            { props.label }
        </button>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct PanelProps {
    pub(crate) title: &'static str,
    pub(crate) children: Children,
}

#[component]
pub(crate) fn Panel(props: &PanelProps) -> Html {
    html! {
        <section class="panel">
            <header><h2>{ props.title }</h2></header>
            { for props.children.iter() }
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct MetricProps {
    pub(crate) label: &'static str,
    pub(crate) value: String,
    #[prop_or("neutral")]
    pub(crate) tone: &'static str,
}

#[component]
pub(crate) fn Metric(props: &MetricProps) -> Html {
    html! {
        <div class={classes!("metric", props.tone)}>
            <span>{ props.label }</span>
            <strong>{ &props.value }</strong>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct KeyMetricsProps {
    pub(crate) values: Vec<(String, String)>,
}

#[component]
pub(crate) fn KeyMetrics(props: &KeyMetricsProps) -> Html {
    html! {
        <div class="key-metrics">
            { for props.values.iter().map(|(label, value)| html! {
                <div class="key-value" key={label.clone()}>
                    <span>{ label }</span>
                    <strong>{ value }</strong>
                </div>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct RowsProps {
    pub(crate) empty: &'static str,
    pub(crate) rows: Vec<(String, String, String)>,
}

#[component]
pub(crate) fn Rows(props: &RowsProps) -> Html {
    if props.rows.is_empty() {
        return html! { <p class="empty">{ props.empty }</p> };
    }
    html! {
        <div class="rows">
            { for props.rows.iter().map(|(status, title, detail)| html! {
                <article class="row" key={format!("{status}-{title}-{detail}")}>
                    <StatusLogo status={status.clone()} />
                    <div>
                        <strong>{ title }</strong>
                        <span>{ detail }</span>
                    </div>
                    <small>{ status }</small>
                </article>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct ApprovalRowsProps {
    pub(crate) approvals: Vec<Approval>,
    pub(crate) lang: UiLang,
    pub(crate) limit: usize,
    pub(crate) on_approve: Callback<String>,
    pub(crate) on_reject: Callback<String>,
}

#[component]
pub(crate) fn ApprovalRows(props: &ApprovalRowsProps) -> Html {
    if props.approvals.is_empty() {
        return html! { <p class="empty">{ props.lang.text("No pending approvals.", "没有待处理审批。") }</p> };
    }
    html! {
        <div class="rows">
            { for props.approvals.iter().take(props.limit).map(|approval| {
                let is_pending = approval.status == "pending" || approval.status == "requires_action";
                let on_approve = {
                    let callback = props.on_approve.clone();
                    let id = approval.id.clone();
                    Callback::from(move |_| callback.emit(id.clone()))
                };
                let on_reject = {
                    let callback = props.on_reject.clone();
                    let id = approval.id.clone();
                    Callback::from(move |_| callback.emit(id.clone()))
                };
                html! {
                    <article class="row" key={approval.id.clone()}>
                        <StatusLogo status={approval.status.clone()} />
                        <div>
                            <strong>{ label_or(&approval.kind, "approval") }</strong>
                            <span>{ label_or(&approval.reason, &approval.id) }</span>
                        </div>
                        <small>{ approval.status.clone() }</small>
                        {
                            if is_pending {
                                html! {
                                    <div class="row-actions">
                                        <button class="action-success" onclick={on_approve}>{ props.lang.text("Approve", "批准") }</button>
                                        <button class="action-danger" onclick={on_reject}>{ props.lang.text("Reject", "拒绝") }</button>
                                    </div>
                                }
                            } else {
                                html! { <span class="row-action-note">{ props.lang.text("No action needed", "无需处理") }</span> }
                            }
                        }
                    </article>
                }
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct JsonPreviewProps {
    pub(crate) value: Value,
}

#[component]
pub(crate) fn JsonPreview(props: &JsonPreviewProps) -> Html {
    html! { <pre class="json-preview">{ pretty_json(&props.value) }</pre> }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct StatusLogoProps {
    pub(crate) status: String,
}

#[component]
pub(crate) fn StatusLogo(props: &StatusLogoProps) -> Html {
    let tone = status_tone(&props.status);
    let letter = props
        .status
        .chars()
        .find(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "I".to_string());
    html! { <span class={classes!("status-logo", tone)}>{ letter }</span> }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct FlowMeterProps {
    pub(crate) label: &'static str,
    pub(crate) value: usize,
    pub(crate) max: usize,
    #[prop_or("neutral")]
    pub(crate) tone: &'static str,
}

#[component]
pub(crate) fn FlowMeter(props: &FlowMeterProps) -> Html {
    html! {
        <div class={classes!("flow-meter", props.tone)}>
            <span>{ props.label }</span>
            <strong>{ props.value }</strong>
            <i><b style={width_style(props.value, props.max)}></b></i>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct RuntimePipelineProps {
    pub(crate) sessions: Vec<Session>,
    pub(crate) execution_jobs: Vec<WorkerJob>,
    pub(crate) session_loop_jobs: Vec<WorkerJob>,
    pub(crate) approvals: Vec<Approval>,
    pub(crate) tool_calls: Vec<ToolCall>,
}

#[component]
pub(crate) fn RuntimePipeline(props: &RuntimePipelineProps) -> Html {
    let stages = vec![
        ("Sessions", props.sessions.len(), "neutral"),
        (
            "Session loop",
            active_job_count(&props.session_loop_jobs),
            "info",
        ),
        ("Workers", active_job_count(&props.execution_jobs), "info"),
        ("Tools", props.tool_calls.len(), "good"),
        (
            "Approvals",
            props
                .approvals
                .iter()
                .filter(|approval| {
                    approval.status == "pending" || approval.status == "requires_action"
                })
                .count(),
            "warn",
        ),
    ];
    let max = stages
        .iter()
        .map(|(_, value, _)| *value)
        .max()
        .unwrap_or(1)
        .max(1);
    html! {
        <div class="runtime-pipeline">
            { for stages.iter().enumerate().map(|(index, (label, value, tone))| html! {
                <div class={classes!("pipeline-stage", *tone)} key={(*label).to_string()}>
                    <span>{ index + 1 }</span>
                    <strong>{ label }</strong>
                    <i style={height_style(*value, max)}></i>
                    <small>{ value }</small>
                </div>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct PackMosaicProps {
    pub(crate) installations: Vec<WorkflowPackInstallation>,
    pub(crate) marketplace: WorkflowPackMarketplace,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PackCardModel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) manifest: Value,
}

#[component]
pub(crate) fn PackMosaic(props: &PackMosaicProps) -> Html {
    let marketplace_count = props.marketplace.packs.len();
    let total = props.installations.len().max(marketplace_count).max(1);
    html! {
        <div class="pack-mosaic">
            <div class="mosaic-grid">
                { for props.installations.iter().take(12).enumerate().map(|(index, pack)| html! {
                    <article class={classes!("mosaic-tile", status_tone(&pack.status))} key={pack.id.clone()}>
                        <span>{ index + 1 }</span>
                        <strong>{ label_or(&pack.pack_id, "pack") }</strong>
                        <small>{ semantic_scope_summary(&pack.manifest["semantic_scopes"]) }</small>
                    </article>
                }) }
                { if props.installations.is_empty() {
                    html! {
                        { for props.marketplace.packs.iter().take(8).enumerate().map(|(index, pack)| html! {
                            <article class={classes!("mosaic-tile", status_tone(&pack.status))} key={pack.id.clone()}>
                                <span>{ index + 1 }</span>
                                <strong>{ label_or(&pack.name, &pack.id) }</strong>
                                <small>{ format!("{} / {}", label_or(&pack.kind, "kind"), label_or(&pack.version, "version")) }</small>
                            </article>
                        }) }
                    }
                } else {
                    html! {}
                }}
            </div>
            <div class="mosaic-side">
                <FlowMeter label="Marketplace" value={marketplace_count} max={total} tone="neutral" />
                <FlowMeter label="Installed" value={props.installations.len()} max={total} tone="good" />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct EnterpriseReadinessPanelProps {
    pub(crate) readiness: EnterpriseProductReadiness,
}

#[component]
pub(crate) fn EnterpriseReadinessPanel(props: &EnterpriseReadinessPanelProps) -> Html {
    let readiness = &props.readiness;
    let total = readiness.lane_count.max(readiness.lanes.len()).max(1);
    let next_action = readiness
        .next_actions
        .first()
        .cloned()
        .unwrap_or_else(|| "No next action reported.".to_string());
    html! {
        <div class="enterprise-readiness">
            <div class="enterprise-summary">
                <div>
                    <span>{ label_or(&readiness.required_evidence_class, "customer_grade") }</span>
                    <strong>{ label_or(&readiness.status, "blocked") }</strong>
                    <small>{ label_or(&readiness.message, &next_action) }</small>
                </div>
                <div class="enterprise-meters">
                    <FlowMeter label="Ready" value={readiness.ready_lane_count} max={total} tone="good" />
                    <FlowMeter label="Pilot" value={readiness.pilot_ready_lane_count} max={total} tone="warn" />
                    <FlowMeter label="Blocked" value={readiness.blocked_lane_count} max={total} tone={if readiness.blocked_lane_count > 0 { "bad" } else { "good" }} />
                </div>
            </div>
            <div class="enterprise-lanes">
                { for readiness.lanes.iter().map(|lane| {
                    let blocker = lane.blockers.first().or_else(|| lane.next_actions.first());
                    let endpoint = lane.readiness_endpoints.first().cloned().unwrap_or_else(|| "no endpoint".to_string());
                    let script = lane.evidence_scripts.first().cloned().unwrap_or_else(|| "no evidence script".to_string());
                    let evidence_count = lane.required_evidence.len();
                    html! {
                        <article class={classes!("enterprise-lane", status_tone(&lane.status))} key={lane.id.clone()}>
                            <StatusLogo status={lane.status.clone()} />
                            <div>
                                <strong>{ label_or(&lane.title, &lane.id) }</strong>
                                <span>{ format!("{} -> {}", label_or(&lane.current_evidence_class, "evidence"), label_or(&lane.required_evidence_class, "customer_grade")) }</span>
                                <small>{ blocker.cloned().unwrap_or_else(|| label_or(&lane.production_target, "target").to_string()) }</small>
                                <small>{ format!("{} | {} | {} required evidence", label_or(&lane.current_boundary, "boundary"), endpoint, evidence_count) }</small>
                                <small>{ script }</small>
                            </div>
                            <em>{ label_or(&lane.status, "status") }</em>
                        </article>
                    }
                }) }
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct VersionBlockProps {
    pub(crate) version: DeploymentVersion,
}

#[component]
pub(crate) fn VersionBlock(props: &VersionBlockProps) -> Html {
    let version = &props.version;
    html! {
        <div class="version-block">
            <div><span>{ "Service" }</span><strong>{ label_or(&version.service, "mandoforge-api") }</strong></div>
            <div><span>{ "Image tag" }</span><strong>{ version.image_tag.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
            <div><span>{ "Git SHA" }</span><strong>{ version.git_sha.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
            <div><span>{ "Build time" }</span><strong>{ version.build_time.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
        </div>
    }
}

fn width_style(value: usize, max: usize) -> String {
    format!("width: {:.1}%;", percent(value, max))
}

fn height_style(value: usize, max: usize) -> String {
    format!("height: {:.1}%;", percent(value, max).max(8.0))
}

fn percent(value: usize, max: usize) -> f64 {
    if max == 0 {
        0.0
    } else {
        (value as f64 / max as f64 * 100.0).clamp(0.0, 100.0)
    }
}
