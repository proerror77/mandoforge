use crate::api::{WorkflowDefinition, WorkflowRun};
use crate::components::{JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{FlowMeter, is_active_status, label_or, short_id, status_tone};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct WorkflowsProps {
    pub(crate) data: ConsoleData,
}

#[component]
pub(crate) fn WorkflowsView(props: &WorkflowsProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Workflow graph">
                <WorkflowGraph runs={props.data.workflow_runs.data.clone()} definitions={props.data.workflow_definitions.data.clone()} />
            </Panel>
            <Panel title="Workflow runs">
                <Rows empty="No workflow runs." rows={props.data.workflow_runs.data.iter().take(12).map(|run| {
                    (run.status.clone(), label_or(&run.title, "workflow run").to_string(), short_id(&run.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Definitions">
                <Rows empty="No workflow definitions." rows={props.data.workflow_definitions.data.iter().take(12).map(|definition| {
                    (definition.status.clone(), label_or(&definition.name, "workflow").to_string(), label_or(&definition.version, "version").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Scheduler">
                <JsonPreview value={props.data.scheduler_summary.data.clone()} />
            </Panel>
            <Panel title="Evidence surfaces">
                <KeyMetrics values={vec![
                    ("Workflow".to_string(), props.data.workflow_runs.data.len().to_string()),
                    ("Steps".to_string(), "via /api/workflow-runs/{id}/steps".to_string()),
                    ("Transitions".to_string(), "via /api/workflow-runs/{id}/transitions".to_string()),
                    ("Grants".to_string(), "via /api/workflow-runs/{id}/task-grants".to_string()),
                    ("Graph".to_string(), "via /api/workflow-runs/{id}/graph".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct WorkflowGraphProps {
    runs: Vec<WorkflowRun>,
    definitions: Vec<WorkflowDefinition>,
}

#[component]
fn WorkflowGraph(props: &WorkflowGraphProps) -> Html {
    let active_runs = props
        .runs
        .iter()
        .filter(|run| is_active_status(&run.status))
        .count();
    let failed_runs = props
        .runs
        .iter()
        .filter(|run| status_tone(&run.status) == "bad")
        .count();
    html! {
        <div class="workflow-graph">
            <div class="graph-lane">
                { for props.definitions.iter().take(6).enumerate().map(|(index, definition)| html! {
                    <div class={classes!("graph-node", status_tone(&definition.status))} key={definition.id.clone()}>
                        <span>{ index + 1 }</span>
                        <strong>{ label_or(&definition.name, "workflow") }</strong>
                    </div>
                }) }
                { if props.definitions.is_empty() {
                    html! { <p class="empty">{ "No workflow definitions." }</p> }
                } else {
                    html! {}
                }}
            </div>
            <div class="graph-stats">
                <FlowMeter label="Runs" value={props.runs.len()} max={props.runs.len().max(1)} tone="neutral" />
                <FlowMeter label="Active" value={active_runs} max={props.runs.len().max(1)} tone="info" />
                <FlowMeter label="Failed" value={failed_runs} max={props.runs.len().max(1)} tone={if failed_runs > 0 { "bad" } else { "good" }} />
            </div>
        </div>
    }
}
