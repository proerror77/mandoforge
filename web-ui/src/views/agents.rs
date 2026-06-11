use crate::api::{Agent, Session};
use crate::components::{KeyMetrics, Panel, Rows, VersionBlock};
use crate::state::ConsoleData;
use crate::{
    FlowMeter, RuntimePipeline, effective_selected, is_active_status, label_or, orbit_point,
    position_style, session_title, short_id, status_tone,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct AgentsProps {
    pub(crate) data: ConsoleData,
    pub(crate) task_title: String,
    pub(crate) task_message: String,
    pub(crate) selected_agent_id: String,
    pub(crate) selected_environment_id: String,
    pub(crate) on_task_title: Callback<InputEvent>,
    pub(crate) on_task_message: Callback<InputEvent>,
    pub(crate) on_agent: Callback<Event>,
    pub(crate) on_environment: Callback<Event>,
    pub(crate) on_start_task: Callback<MouseEvent>,
}

#[component]
pub(crate) fn AgentsView(props: &AgentsProps) -> Html {
    let data = &props.data;
    html! {
        <div class="page-grid agents-grid">
            <Panel title="Task launcher">
                <div class="taskbar">
                    <label>
                        <span>{ "Agent" }</span>
                        <select value={effective_selected(&props.selected_agent_id, data.agents.data.first().map(|agent| agent.id.as_str()))} onchange={props.on_agent.clone()}>
                            { for data.agents.data.iter().map(|agent| html! {
                                <option value={agent.id.clone()}>{ format!("{} / {}", agent.name, label_or(&agent.agent_role, "agent")) }</option>
                            }) }
                        </select>
                    </label>
                    <label>
                        <span>{ "Environment" }</span>
                        <select value={props.selected_environment_id.clone()} onchange={props.on_environment.clone()}>
                            <option value="">{ "Default environment" }</option>
                            { for data.environments.data.iter().map(|environment| html! {
                                <option value={environment.id.clone()}>{ format!("{} / {}", environment.name, label_or(&environment.status, "status")) }</option>
                            }) }
                        </select>
                    </label>
                    <input
                        value={props.task_title.clone()}
                        placeholder="Task title"
                        oninput={props.on_task_title.clone()}
                    />
                    <textarea
                        value={props.task_message.clone()}
                        placeholder="Describe the task for the selected agent"
                        oninput={props.on_task_message.clone()}
                    />
                    <button disabled={data.agents.data.is_empty()} onclick={props.on_start_task.clone()}>{ "Start task" }</button>
                    <small>{ "Creates POST /api/sessions with an initial message; the runtime queues the session loop." }</small>
                </div>
            </Panel>
            <Panel title="Runtime topology">
                <AgentTopology agents={data.agents.data.clone()} sessions={data.sessions.data.clone()} />
            </Panel>
            <Panel title="Queue pressure">
                <RuntimePipeline
                    sessions={data.sessions.data.clone()}
                    execution_jobs={data.execution_jobs.data.clone()}
                    session_loop_jobs={data.session_loop_jobs.data.clone()}
                    approvals={data.approvals.data.clone()}
                    tool_calls={data.tool_calls.data.clone()}
                />
            </Panel>
            <Panel title="Worker state">
                <Rows empty="No worker jobs reported." rows={data.execution_jobs.data.iter().take(8).map(|job| {
                    (job.status.clone(), job.worker_id.clone().unwrap_or_else(|| job.id.clone()), job.last_error.clone().unwrap_or_else(|| job.updated_at.clone()))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Managed sessions">
                <Rows empty="No sessions yet." rows={data.sessions.data.iter().take(10).map(|session| {
                    (session.status.clone(), short_id(&session.id), session_title(session))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Workflow runs">
                <Rows empty="No workflow runs." rows={data.workflow_runs.data.iter().take(8).map(|run| {
                    (run.status.clone(), label_or(&run.title, "workflow run").to_string(), short_id(&run.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Approvals">
                <Rows empty="No approvals." rows={data.approvals.data.iter().take(8).map(|approval| {
                    (approval.status.clone(), label_or(&approval.kind, "approval").to_string(), label_or(&approval.reason, &approval.id).to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Tool calls">
                <Rows empty="No tool calls." rows={data.tool_calls.data.iter().take(8).map(|call| {
                    (call.status.clone(), label_or(&call.tool_name, "tool").to_string(), short_id(&call.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Deployment version">
                <VersionBlock version={data.deployment_version.data.clone()} />
            </Panel>
            <Panel title="Logs and artifacts">
                <KeyMetrics values={vec![
                    ("Events".to_string(), "via /api/sessions/{id}/events".to_string()),
                    ("Stream".to_string(), "via /api/sessions/{id}/stream".to_string()),
                    ("Artifacts".to_string(), "via /api/sessions/{id}/artifacts".to_string()),
                    ("Audit logs".to_string(), "via /api/sessions/{id}/audit-logs".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct AgentTopologyProps {
    agents: Vec<Agent>,
    sessions: Vec<Session>,
}

#[component]
fn AgentTopology(props: &AgentTopologyProps) -> Html {
    let total = props.agents.len().max(1);
    html! {
        <div class="agent-topology">
            <div class="topology-canvas">
                <span class="topology-hub">{ props.agents.len() }</span>
                <i class="topology-axis horizontal"></i>
                <i class="topology-axis vertical"></i>
                { for props.agents.iter().take(14).enumerate().map(|(index, agent)| {
                    let (x, y) = orbit_point(index, total, 50.0, 50.0, 41.0);
                    html! {
                        <article
                            class={classes!("topology-agent", status_tone(&agent.release_state))}
                            style={position_style(x, y)}
                            title={format!("{} / {}", agent.name, label_or(&agent.agent_role, "agent"))}
                        >
                            <span>{ agent.name.chars().next().unwrap_or('A') }</span>
                        </article>
                    }
                }) }
            </div>
            <div class="topology-side">
                <FlowMeter label="Agents" value={props.agents.len()} max={props.agents.len().max(1)} tone="good" />
                <FlowMeter label="Active sessions" value={props.sessions.iter().filter(|session| is_active_status(&session.status)).count()} max={props.sessions.len().max(1)} tone="info" />
                <FlowMeter label="Released" value={props.agents.iter().filter(|agent| status_tone(&agent.release_state) == "good").count()} max={props.agents.len().max(1)} tone="good" />
            </div>
        </div>
    }
}
