use crate::api::DynamicWorkflowPlan;
use crate::components::{FlowMeter, KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{label_or, status_tone};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct DynamicProps {
    pub(crate) data: ConsoleData,
    pub(crate) objective: String,
    pub(crate) on_objective: Callback<InputEvent>,
    pub(crate) on_compile: Callback<MouseEvent>,
}

#[component]
pub(crate) fn DynamicView(props: &DynamicProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Compiler">
                <div class="form-stack">
                    <textarea value={props.objective.clone()} oninput={props.on_objective.clone()} />
                    <button onclick={props.on_compile.clone()}>{ "Compile dynamic workflow" }</button>
                </div>
            </Panel>
            <Panel title="Fleet shape">
                <FleetShape plans={props.data.dynamic_workflow_plans.data.clone()} />
            </Panel>
            <Panel title="Plans">
                <Rows empty="No dynamic workflow plans." rows={props.data.dynamic_workflow_plans.data.iter().take(12).map(|plan| {
                    (plan.status.clone(), label_or(&plan.objective, "dynamic workflow").to_string(), label_or(&plan.runtime_adapter, "runtime").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Fleet policy">
                <KeyMetrics values={vec![
                    ("Max agents".to_string(), "1000 policy cap".to_string()),
                    ("Max parallel".to_string(), "16 policy cap".to_string()),
                    ("Cross-check".to_string(), "review and adjudication metadata".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct FleetShapeProps {
    plans: Vec<DynamicWorkflowPlan>,
}

#[component]
fn FleetShape(props: &FleetShapeProps) -> Html {
    let total = props.plans.len().max(1);
    html! {
        <div class="fleet-shape">
            { for props.plans.iter().take(18).enumerate().map(|(index, plan)| {
                let size = 26 + ((index % 5) * 8);
                html! {
                    <article
                        class={classes!("fleet-cell", status_tone(&plan.status))}
                        key={plan.id.clone()}
                        style={format!("--cell-size: {}px;", size)}
                    >
                        <strong>{ index + 1 }</strong>
                        <span>{ label_or(&plan.runtime_adapter, "runtime") }</span>
                    </article>
                }
            }) }
            { if props.plans.is_empty() {
                html! {
                    <div class="fleet-empty">
                        { for (0..9).map(|index| html! { <i style={format!("--delay: {}ms;", index * 80)}></i> }) }
                    </div>
                }
            } else {
                html! {}
            }}
            <div class="fleet-summary">
                <FlowMeter label="Compiled plans" value={props.plans.len()} max={total} tone="info" />
                <FlowMeter label="Ready plans" value={props.plans.iter().filter(|plan| status_tone(&plan.status) == "good").count()} max={total} tone="good" />
            </div>
        </div>
    }
}
