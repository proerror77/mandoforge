use crate::api::DeploymentVersion;
use crate::state::View;
use crate::{label_or, pretty_json, status_tone};
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
pub(crate) struct OverviewSignalProps {
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) detail: String,
    #[prop_or("neutral")]
    pub(crate) tone: &'static str,
    pub(crate) target: View,
    pub(crate) on_view: Callback<View>,
}

#[component]
pub(crate) fn OverviewSignal(props: &OverviewSignalProps) -> Html {
    let target = props.target;
    let on_view = props.on_view.clone();
    html! {
        <button
            class={classes!("overview-signal", props.tone)}
            onclick={Callback::from(move |_| on_view.emit(target))}
        >
            <span>{ props.label }</span>
            <strong>{ &props.value }</strong>
            <small>{ &props.detail }</small>
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
