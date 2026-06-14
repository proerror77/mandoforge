use crate::api::{
    OntologyOnboardingProposal, OntologyOnboardingRun, OntologyOnboardingToolSpec,
    OntologyReviewGraph, OntologyReviewGraphNode, RenderedExecutionContext, SemanticGraphSnapshot,
    SemanticObject,
};
use crate::components::{FlowMeter, JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{
    compact_json, label_or, orbit_point, position_style, semantic_scope_summary, short_id,
    status_tone,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct SemanticProps {
    pub(crate) data: ConsoleData,
    pub(crate) source_text: String,
    pub(crate) context_packet_id: String,
    pub(crate) rendered_context: Option<RenderedExecutionContext>,
    pub(crate) onboarding_run: Option<OntologyOnboardingRun>,
    pub(crate) onboarding_tool_specs: Vec<OntologyOnboardingToolSpec>,
    pub(crate) onboarding_review_graph: Option<OntologyReviewGraph>,
    pub(crate) on_source: Callback<InputEvent>,
    pub(crate) on_build: Callback<MouseEvent>,
    pub(crate) on_context_packet_id: Callback<InputEvent>,
    pub(crate) on_render_context: Callback<MouseEvent>,
    pub(crate) on_start_onboarding: Callback<MouseEvent>,
    pub(crate) on_approve_onboarding_proposal: Callback<String>,
    pub(crate) on_reject_onboarding_proposal: Callback<String>,
    pub(crate) on_materialize_onboarding: Callback<MouseEvent>,
}

#[component]
pub(crate) fn SemanticView(props: &SemanticProps) -> Html {
    html! {
        <div class="page-grid semantic-grid">
            <Panel title="Enterprise ontology fast-onboarding">
                <OnboardingPanel
                    run={props.onboarding_run.clone()}
                    tool_specs={props.onboarding_tool_specs.clone()}
                    review_graph={props.onboarding_review_graph.clone()}
                    on_start={props.on_start_onboarding.clone()}
                    on_approve={props.on_approve_onboarding_proposal.clone()}
                    on_reject={props.on_reject_onboarding_proposal.clone()}
                    on_materialize={props.on_materialize_onboarding.clone()}
                />
            </Panel>
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
struct OnboardingPanelProps {
    run: Option<OntologyOnboardingRun>,
    tool_specs: Vec<OntologyOnboardingToolSpec>,
    review_graph: Option<OntologyReviewGraph>,
    on_start: Callback<MouseEvent>,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
    on_materialize: Callback<MouseEvent>,
}

#[component]
fn OnboardingPanel(props: &OnboardingPanelProps) -> Html {
    let Some(run) = props.run.as_ref() else {
        return html! {
            <div class="ontology-onboarding">
                <div class="ontology-onboarding-actions">
                    <button onclick={props.on_start.clone()}>{ "Start ecommerce demo run" }</button>
                </div>
                <p class="empty">{ "No onboarding run started." }</p>
            </div>
        };
    };
    html! {
        <div class="ontology-onboarding">
            <div class="ontology-onboarding-actions">
                <button onclick={props.on_start.clone()}>{ "Start new demo run" }</button>
                <button onclick={props.on_materialize.clone()}>{ "Materialize approved" }</button>
            </div>
            <KeyMetrics values={vec![
                ("Run".to_string(), short_id(&run.id)),
                ("Status".to_string(), label_or(&run.status, "pending").to_string()),
                ("Source".to_string(), label_or(&run.source_mode, "demo").to_string()),
                ("Datasets".to_string(), run.dataset_count.to_string()),
                ("Profiles".to_string(), run.profile_count.to_string()),
                ("Proposals".to_string(), run.proposal_count.to_string()),
                ("Approved".to_string(), run.approved_count.to_string()),
                ("Materialized".to_string(), run.materialized_count.to_string()),
            ]} />
            <OntologyReviewGraphPanel graph={props.review_graph.clone()} />
            <div class="ontology-proposal-list">
                { for ["object", "relation", "metric", "logic", "action"].iter().map(|proposal_type| html! {
                    <section class="ontology-proposal-group" key={proposal_type.to_string()}>
                        <h4>{ proposal_type.to_ascii_uppercase() }</h4>
                        { for run.proposals.iter().filter(|proposal| proposal.proposal_type == *proposal_type).map(|proposal| {
                            html! {
                                <OnboardingProposalRow
                                    key={proposal.id.clone()}
                                    proposal={proposal.clone()}
                                    on_approve={props.on_approve.clone()}
                                    on_reject={props.on_reject.clone()}
                                />
                            }
                        }) }
                    </section>
                }) }
            </div>
            <div class="ontology-tool-specs">
                <h4>{ "Compiled agent tools" }</h4>
                <Rows empty="No tool specs compiled." rows={props.tool_specs.iter().map(|spec| {
                    (
                        if spec.approval_required { "approval" } else { "ready" }.to_string(),
                        spec.name.clone(),
                        format!("{} / {} / {}", spec.target_object, label_or(&spec.read_write_risk, "risk_unset"), label_or(&spec.description, "ontology action"))
                    )
                }).collect::<Vec<_>>()} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyReviewGraphPanelProps {
    graph: Option<OntologyReviewGraph>,
}

#[component]
fn OntologyReviewGraphPanel(props: &OntologyReviewGraphPanelProps) -> Html {
    let Some(graph) = props.graph.as_ref() else {
        return html! {
            <div class="ontology-review-graph empty">{ "No review graph loaded." }</div>
        };
    };
    let node_types = ["dataset", "object", "metric", "logic", "action", "tool"];
    html! {
        <div class="ontology-review-graph">
            <div class="ontology-review-graph-head">
                <strong>{ "Ontology review graph" }</strong>
                <span>{ format!("{} nodes / {} edges", graph.nodes.len(), graph.edges.len()) }</span>
                {
                    if graph.truncated {
                        html! { <span>{ format!("truncated: +{} nodes / +{} edges", graph.omitted_node_count, graph.omitted_edge_count) }</span> }
                    } else {
                        html! {}
                    }
                }
            </div>
            <div class="ontology-review-node-groups">
                { for node_types.iter().map(|node_type| {
                    let nodes = graph.nodes.iter().filter(|node| node.node_type == *node_type).collect::<Vec<_>>();
                    html! {
                        <section class="ontology-review-node-group" key={node_type.to_string()}>
                            <h4>{ format!("{} ({})", node_type.to_ascii_uppercase(), nodes.len()) }</h4>
                            <div class="ontology-review-node-list">
                                { for nodes.iter().take(8).map(|node| html! {
                                    <OntologyReviewGraphNodeChip key={node.id.clone()} node={(*node).clone()} />
                                }) }
                            </div>
                        </section>
                    }
                }) }
            </div>
            <div class="ontology-review-edge-list">
                <h4>{ "Business logic edges" }</h4>
                <Rows empty="No graph edges." rows={graph.edges.iter().take(12).map(|edge| {
                    (
                        edge.edge_type.clone(),
                        format!("{} -> {}", edge.from, edge.to),
                        format!(
                            "{} / {:.0}% / {} / {}",
                            label_or(&edge.status, "pending"),
                            edge.confidence * 100.0,
                            label_or(&edge.risk, "low"),
                            edge.source_proposal_id.as_deref().map(short_id).unwrap_or_else(|| "source".to_string())
                        )
                    )
                }).collect::<Vec<_>>()} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyReviewGraphNodeChipProps {
    node: OntologyReviewGraphNode,
}

#[component]
fn OntologyReviewGraphNodeChip(props: &OntologyReviewGraphNodeChipProps) -> Html {
    html! {
        <div class={classes!("ontology-review-node", status_tone(&props.node.status))}>
            <strong>{ props.node.label.clone() }</strong>
            <span>{ format!("{:.0}% / {}", props.node.confidence * 100.0, label_or(&props.node.risk, "low")) }</span>
            <small>{ format!("{} / {}", label_or(&props.node.status, "pending"), props.node.source_proposal_id.as_deref().map(short_id).unwrap_or_else(|| "source".to_string())) }</small>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OnboardingProposalRowProps {
    proposal: OntologyOnboardingProposal,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
}

#[component]
fn OnboardingProposalRow(props: &OnboardingProposalRowProps) -> Html {
    let approve = {
        let id = props.proposal.id.clone();
        let on_approve = props.on_approve.clone();
        Callback::from(move |_| on_approve.emit(id.clone()))
    };
    let reject = {
        let id = props.proposal.id.clone();
        let on_reject = props.on_reject.clone();
        Callback::from(move |_| on_reject.emit(id.clone()))
    };
    html! {
        <article class="ontology-proposal-row">
            <div class="ontology-proposal-head">
                <div>
                    <span>{ format!("{} / {:.0}%", props.proposal.review_status, props.proposal.confidence * 100.0) }</span>
                    <strong>{ props.proposal.name.clone() }</strong>
                    <small>{ props.proposal.source_mapping.clone() }</small>
                </div>
                <div class="ontology-onboarding-actions">
                    <button onclick={approve} disabled={props.proposal.review_status == "approved"}>{ "Approve" }</button>
                    <button onclick={reject} disabled={props.proposal.review_status == "rejected"}>{ "Reject" }</button>
                </div>
            </div>
            <JsonPreview value={props.proposal.evidence.clone()} />
        </article>
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
