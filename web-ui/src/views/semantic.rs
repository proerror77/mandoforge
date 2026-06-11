use crate::api::{RenderedExecutionContext, SemanticGraphSnapshot, SemanticObject};
use crate::components::{JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{
    FlowMeter, compact_json, label_or, orbit_point, position_style, semantic_scope_summary,
    short_id, status_tone,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct SemanticProps {
    pub(crate) data: ConsoleData,
    pub(crate) source_text: String,
    pub(crate) context_packet_id: String,
    pub(crate) rendered_context: Option<RenderedExecutionContext>,
    pub(crate) on_source: Callback<InputEvent>,
    pub(crate) on_build: Callback<MouseEvent>,
    pub(crate) on_context_packet_id: Callback<InputEvent>,
    pub(crate) on_render_context: Callback<MouseEvent>,
}

#[component]
pub(crate) fn SemanticView(props: &SemanticProps) -> Html {
    html! {
        <div class="page-grid semantic-grid">
            <Panel title="Ontology builder">
                <div class="form-stack">
                    <textarea value={props.source_text.clone()} oninput={props.on_source.clone()} />
                    <button onclick={props.on_build.clone()}>{ "Preview ontology proposal" }</button>
                </div>
            </Panel>
            <Panel title="Ontology engine readiness">
                <JsonPreview value={props.data.ontology_engine_readiness.data.clone()} />
            </Panel>
            <Panel title="Context compiler">
                <div class="form-stack">
                    <input
                        value={props.context_packet_id.clone()}
                        placeholder="Context packet ID"
                        oninput={props.on_context_packet_id.clone()}
                    />
                    <button onclick={props.on_render_context.clone()}>{ "Render execution context" }</button>
                    <RenderedContextPreview rendered={props.rendered_context.clone()} />
                </div>
            </Panel>
            <Panel title="Semantic graph">
                <SemanticMap snapshot={props.data.semantic_graph.data.clone()} objects={props.data.semantic_objects.data.clone()} />
            </Panel>
            <Panel title="Objects">
                <Rows empty="No semantic objects." rows={props.data.semantic_objects.data.iter().take(10).map(|object| {
                    (
                        object.status.clone(),
                        label_or(&object.title, &object.object_key).to_string(),
                        format!("{} / {} / {}", object.object_type, object.trust_level, semantic_scope_summary(&object.semantic_scopes))
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Governance">
                <KeyMetrics values={vec![
                    ("Writebacks".to_string(), compact_json(&props.data.memory_writebacks.data)),
                    ("Candidates".to_string(), compact_json(&props.data.memory_writeback_candidates.data)),
                    ("Ontology".to_string(), props.data.ontology_registry.data.version.clone()),
                    ("Reflection".to_string(), props.data.semantic_reflection_queue.data.status.clone()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SemanticMapProps {
    snapshot: SemanticGraphSnapshot,
    objects: Vec<SemanticObject>,
}

#[component]
fn SemanticMap(props: &SemanticMapProps) -> Html {
    let total = props.objects.len().max(1);
    html! {
        <div class="semantic-map">
            <div class="semantic-canvas">
                <span class="semantic-core">{ props.snapshot.node_count }</span>
                { for props.objects.iter().take(16).enumerate().map(|(index, object)| {
                    let (x, y) = orbit_point(index, total, 50.0, 50.0, 42.0);
                    html! {
                        <b
                            class={classes!("semantic-node", status_tone(&object.status))}
                            title={format!("{} / {}", label_or(&object.title, &object.object_key), object.object_type)}
                            style={position_style(x, y)}
                        />
                    }
                }) }
            </div>
            <div class="semantic-stats">
                <FlowMeter label="Objects" value={props.snapshot.node_count} max={props.snapshot.node_count.max(1)} tone="info" />
                <FlowMeter label="Links" value={props.snapshot.edge_count} max={props.snapshot.edge_count.max(props.snapshot.node_count).max(1)} tone="good" />
                <FlowMeter label="Conflicts" value={props.snapshot.conflicts.len()} max={props.snapshot.conflicts.len().max(1)} tone={if props.snapshot.conflicts.is_empty() { "good" } else { "warn" }} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct RenderedContextPreviewProps {
    rendered: Option<RenderedExecutionContext>,
}

#[component]
fn RenderedContextPreview(props: &RenderedContextPreviewProps) -> Html {
    let Some(rendered) = props.rendered.as_ref() else {
        return html! { <p class="empty">{ "No execution context rendered." }</p> };
    };
    let omissions = &rendered.omitted;
    let budget = &rendered.budget;
    html! {
        <div class="context-preview">
            <div class="context-budget">
                <FlowMeter
                    label="Prompt tokens"
                    value={budget.estimated_tokens_used}
                    max={budget.max_prompt_tokens.max(1)}
                    tone={if budget.estimated_tokens_used > budget.max_prompt_tokens { "warn" } else { "good" }}
                />
                <FlowMeter
                    label="Objects"
                    value={rendered.relevant_objects.len()}
                    max={budget.max_objects.max(1)}
                    tone="info"
                />
                <FlowMeter
                    label="Fetchable"
                    value={rendered.fetchable_object_ids.len()}
                    max={rendered.fetchable_object_ids.len().max(rendered.relevant_objects.len()).max(1)}
                    tone="neutral"
                />
            </div>
            <KeyMetrics values={vec![
                ("Packet".to_string(), short_id(&rendered.context_packet_id)),
                ("Role".to_string(), label_or(&rendered.role, "agent").to_string()),
                ("Version".to_string(), rendered.context_packet_version.to_string()),
                ("Full content".to_string(), rendered.full_content_included.to_string()),
                ("Tools".to_string(), rendered.available_tools.join(", ")),
                ("Omitted".to_string(), format!(
                    "{} budget / {} object cap / {} source refs / {} content",
                    omissions.token_budget_exceeded,
                    omissions.object_limit_exceeded,
                    omissions.source_refs_not_rendered,
                    omissions.full_content_not_rendered
                )),
            ]} />
            <Rows empty="No policy reminders." rows={rendered.must_follow.iter().take(6).map(|item| {
                ("must_follow".to_string(), item.clone(), "rendered reminder".to_string())
            }).collect::<Vec<_>>()} />
            <div class="context-objects">
                { for rendered.relevant_objects.iter().map(|object| html! {
                    <article class="context-object" key={object.id.clone()}>
                        <div>
                            <span>{ format!("{} / {}", label_or(&object.object_type, "semantic_object"), short_id(&object.id)) }</span>
                            <strong>{ label_or(&object.title, &object.object_key) }</strong>
                        </div>
                        <p>{ label_or(&object.summary, "No summary.") }</p>
                        <small>{ format!("{} / {}", label_or(&object.trust_level, "trust"), label_or(&object.freshness, "freshness")) }</small>
                    </article>
                }) }
            </div>
            <JsonPreview value={rendered.ontology_scope.clone()} />
        </div>
    }
}
