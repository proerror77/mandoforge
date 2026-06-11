use crate::PackCardModel;
use crate::components::{JsonPreview, KeyMetrics, Panel, Rows, StatusLogo};
use crate::state::ConsoleData;
use crate::{
    PackMosaic, json_array_len, label_or, pack_blocker_summary, pack_card_models,
    pack_connector_rows, pack_has_external_writes, pack_lifecycle_steps, pack_metric_count,
    pack_requires_approval, pack_string_list, semantic_scope_summary, status_tone,
};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct PacksProps {
    pub(crate) data: ConsoleData,
}

#[component]
pub(crate) fn PacksView(props: &PacksProps) -> Html {
    let cards = pack_card_models(
        &props.data.workflow_pack_installations.data,
        &props.data.workflow_pack_marketplace.data,
    );
    html! {
        <div class="page-stack">
            <section class="pack-product-grid">
                { for cards.into_iter().map(|card| html! { <PackProductCard card={card} /> }) }
            </section>
            <div class="page-grid">
                <Panel title="Marketplace map">
                    <PackMosaic
                        installations={props.data.workflow_pack_installations.data.clone()}
                        marketplace={props.data.workflow_pack_marketplace.data.clone()}
                    />
                </Panel>
            <Panel title="Installations">
                <Rows empty="No pack installations." rows={props.data.workflow_pack_installations.data.iter().take(12).map(|pack| {
                    (
                        pack.status.clone(),
                        label_or(&pack.pack_id, "pack").to_string(),
                        format!("{} / {} / {}", label_or(&pack.kind, "kind"), label_or(&pack.version, "version"), semantic_scope_summary(&pack.manifest["semantic_scopes"]))
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Marketplace">
                <KeyMetrics values={vec![
                    ("Status".to_string(), props.data.workflow_pack_marketplace.data.status.clone()),
                    ("Packs".to_string(), props.data.workflow_pack_marketplace.data.packs.len().to_string()),
                    ("Bindings".to_string(), "via /api/workflow-packs/installations/{id}/bindings".to_string()),
                    ("Runtime objects".to_string(), "via /api/workflow-packs/installations/{id}/runtime-objects".to_string()),
                ]} />
                <Rows empty="No marketplace packs." rows={props.data.workflow_pack_marketplace.data.packs.iter().map(|pack| {
                    (
                        pack.status.clone(),
                        label_or(&pack.name, &pack.id).to_string(),
                        format!("{} / {} / {}", label_or(&pack.kind, "kind"), label_or(&pack.version, "version"), pack.description.clone())
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Onboarding">
                <JsonPreview value={Value::Array(props.data.capability_discovery.data.capabilities.clone())} />
            </Panel>
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PackProductCardProps {
    card: PackCardModel,
}

#[component]
fn PackProductCard(props: &PackProductCardProps) -> Html {
    let card = &props.card;
    let workflow_count = pack_metric_count(&card.manifest, "workflows", "workflow_count");
    let agent_count = pack_metric_count(&card.manifest, "agents", "agent_count");
    let connector_count = pack_metric_count(&card.manifest, "connectors", "connector_count");
    let action_count = pack_metric_count(&card.manifest, "actions", "action_count");
    let release_gate_count =
        pack_metric_count(&card.manifest, "release_gates", "required_eval_gate_count");
    let validated_file_count =
        pack_metric_count(&card.manifest, "validated_files", "validated_file_count");
    let external_writes = pack_has_external_writes(&card.manifest);
    let approval_required = pack_requires_approval(&card.manifest);
    let safe_action = if external_writes {
        "Draft + approval before external write"
    } else {
        "Read and draft surfaces only"
    };
    html! {
        <article class={classes!("pack-product-card", status_tone(&card.status))} key={card.id.clone()}>
            <header class="pack-product-header">
                <StatusLogo status={card.status.clone()} />
                <div>
                    <span>{ format!("{} / {}", label_or(&card.kind, "WorkflowPack"), label_or(&card.version, "version")) }</span>
                    <strong>{ label_or(&card.name, &card.id) }</strong>
                </div>
                <em>{ label_or(&card.status, &card.source) }</em>
            </header>
            <p>{ label_or(&card.description, "No pack description reported.") }</p>
            <div class="pack-lifecycle" aria-label="WorkflowPack lifecycle">
                { for pack_lifecycle_steps(&card.status, &card.source).into_iter().map(|(label, tone)| html! {
                    <span class={classes!("pack-lifecycle-step", tone)}>{ label }</span>
                }) }
            </div>
            <div class="pack-card-metrics">
                <PackMetric label="Workflows" value={workflow_count} />
                <PackMetric label="Agents" value={agent_count} />
                <PackMetric label="Connectors" value={connector_count} />
                <PackMetric label="Actions" value={action_count} />
                <PackMetric label="Gates" value={release_gate_count} />
                <PackMetric label="Files" value={validated_file_count} />
            </div>
            <div class="pack-capabilities">
                { for pack_string_list(&card.manifest, "capabilities", 6).into_iter().map(|capability| html! {
                    <span>{ capability }</span>
                }) }
                { if json_array_len(&card.manifest["capabilities"]) == 0 {
                    html! { <span>{ "No capabilities declared" }</span> }
                } else {
                    html! {}
                }}
            </div>
            <div class="pack-connector-list">
                { for pack_connector_rows(&card.manifest).into_iter().map(|row| html! {
                    <div class="pack-connector-row" key={row.0.clone()}>
                        <strong>{ row.0 }</strong>
                        <span>{ row.1 }</span>
                    </div>
                }) }
            </div>
            <div class="pack-gate-strip">
                <span>{ format!("{release_gate_count} release gates") }</span>
                <span>{ if approval_required { "approval required" } else { "approval not declared" } }</span>
                <span>{ safe_action }</span>
            </div>
            <div class="pack-card-footer">
                <small>{ pack_blocker_summary(card, external_writes, release_gate_count) }</small>
                <b>{ semantic_scope_summary(&card.manifest["semantic_scopes"]) }</b>
            </div>
        </article>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PackMetricProps {
    label: &'static str,
    value: usize,
}

#[component]
fn PackMetric(props: &PackMetricProps) -> Html {
    html! {
        <div class="pack-metric">
            <span>{ props.label }</span>
            <strong>{ props.value }</strong>
        </div>
    }
}
