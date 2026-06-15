use crate::api::{Agent, Session};
use crate::components::{FlowMeter, KeyMetrics, Panel, Rows, RuntimePipeline, VersionBlock};
use crate::state::{ConsoleData, UiLang};
use crate::{
    compact_json, effective_selected, is_active_status, label_or, orbit_point, position_style,
    session_title, short_id, status_tone,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct AgentsProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
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
    let lang = props.lang;
    html! {
        <div class="page-stack">
            <section class="page-purpose">
                <p class="eyebrow">{ lang.text("Managed Agents / 托管智能体", "托管智能体 / Managed Agents") }</p>
                <h2>{ lang.text("Observe and launch managed agents without turning this page into business-process modeling.", "这里观察和启动托管智能体，不承载业务流程建模。") }</h2>
                <p>{ lang.text(
                    "Manager Agent is an observer and advisor here. It summarizes pressure, handoffs, approvals, and failed work so the operator can choose the next action.",
                    "Manager Agent 在这个页面只是观察者和建议者：它汇总运行压力、交接、审批和失败任务，帮助操作者决定下一步。"
                ) }</p>
            </section>
            <div class="page-grid agents-grid">
            <Panel title={lang.text("Manager Agent Observer", "Manager Agent 观察栏")}>
                <ManagerObservationRail data={data.clone()} lang={lang} />
            </Panel>
            <Panel title={lang.text("Task Launcher", "任务启动器")}>
                <div class="taskbar">
                    <label>
                        <span>{ lang.text("Agent", "智能体") }</span>
                        <select
                            id="managed-agent-select"
                            name="managed-agent-select"
                            value={effective_selected(&props.selected_agent_id, data.agents.data.first().map(|agent| agent.id.as_str()))}
                            onchange={props.on_agent.clone()}
                        >
                            { for data.agents.data.iter().map(|agent| html! {
                                <option value={agent.id.clone()}>{ format!("{} / {}", agent.name, label_or(&agent.agent_role, "agent")) }</option>
                            }) }
                        </select>
                    </label>
                    <label>
                        <span>{ lang.text("Environment", "环境") }</span>
                        <select
                            id="managed-agent-environment"
                            name="managed-agent-environment"
                            value={props.selected_environment_id.clone()}
                            onchange={props.on_environment.clone()}
                        >
                            <option value="">{ lang.text("Default environment", "默认环境") }</option>
                            { for data.environments.data.iter().map(|environment| html! {
                                <option value={environment.id.clone()}>{ format!("{} / {}", environment.name, label_or(&environment.status, "status")) }</option>
                            }) }
                        </select>
                    </label>
                    <input
                        id="managed-agent-task-title"
                        name="managed-agent-task-title"
                        value={props.task_title.clone()}
                        placeholder={lang.text("Task title", "任务标题")}
                        oninput={props.on_task_title.clone()}
                    />
                    <textarea
                        id="managed-agent-task-message"
                        name="managed-agent-task-message"
                        value={props.task_message.clone()}
                        placeholder={lang.text("Describe the task for the selected agent", "描述要交给所选智能体的任务")}
                        oninput={props.on_task_message.clone()}
                    />
                    <button disabled={data.agents.data.is_empty()} onclick={props.on_start_task.clone()}>{ lang.text("Start task", "启动任务") }</button>
                    <small>{ lang.text(
                        "Creates POST /api/sessions with an initial message; runtime queues the session loop.",
                        "创建 POST /api/sessions 初始消息，并由运行时排入 session loop。"
                    ) }</small>
                </div>
            </Panel>
            <Panel title={lang.text("Runtime Topology", "运行拓扑")}>
                <AgentTopology agents={data.agents.data.clone()} sessions={data.sessions.data.clone()} lang={lang} />
            </Panel>
            <Panel title={lang.text("Queue Pressure", "队列压力")}>
                <RuntimePipeline
                    sessions={data.sessions.data.clone()}
                    execution_jobs={data.execution_jobs.data.clone()}
                    session_loop_jobs={data.session_loop_jobs.data.clone()}
                    approvals={data.approvals.data.clone()}
                    tool_calls={data.tool_calls.data.clone()}
                />
            </Panel>
            <Panel title={lang.text("Worker State", "Worker 状态")}>
                <Rows empty={lang.text("No worker jobs reported.", "没有 Worker 任务。")} rows={data.execution_jobs.data.iter().take(8).map(|job| {
                    (job.status.clone(), job.worker_id.clone().unwrap_or_else(|| job.id.clone()), job.last_error.clone().unwrap_or_else(|| job.updated_at.clone()))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Managed Sessions", "托管 Session")}>
                <Rows empty={lang.text("No sessions yet.", "还没有 Session。")} rows={data.sessions.data.iter().take(10).map(|session| {
                    (session.status.clone(), short_id(&session.id), session_title(session))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Workflow Runs", "工作流运行")}>
                <Rows empty={lang.text("No workflow runs.", "没有工作流运行。")} rows={data.workflow_runs.data.iter().take(8).map(|run| {
                    (run.status.clone(), label_or(&run.title, "workflow run").to_string(), short_id(&run.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Approvals", "审批")}>
                <Rows empty={lang.text("No approvals.", "没有审批。")} rows={data.approvals.data.iter().take(8).map(|approval| {
                    (approval.status.clone(), label_or(&approval.kind, "approval").to_string(), label_or(&approval.reason, &approval.id).to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Tool Calls", "工具调用")}>
                <Rows empty={lang.text("No tool calls.", "没有工具调用。")} rows={data.tool_calls.data.iter().take(8).map(|call| {
                    (call.status.clone(), label_or(&call.tool_name, "tool").to_string(), short_id(&call.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Deployment Version", "部署版本")}>
                <VersionBlock version={data.deployment_version.data.clone()} />
            </Panel>
            <Panel title={lang.text("Logs and Artifacts", "日志与产物")}>
                <KeyMetrics values={vec![
                    (lang.text("Events", "事件").to_string(), "via /api/sessions/{id}/events".to_string()),
                    (lang.text("Stream", "流").to_string(), "via /api/sessions/{id}/stream".to_string()),
                    (lang.text("Artifacts", "产物").to_string(), "via /api/sessions/{id}/artifacts".to_string()),
                    (lang.text("Audit logs", "审计日志").to_string(), "via /api/sessions/{id}/audit-logs".to_string()),
                ]} />
            </Panel>
            </div>
        </div>
    }
}

#[component]
fn ManagerObservationRail(props: &ManagerObservationRailProps) -> Html {
    let failed_jobs = props
        .data
        .execution_jobs
        .data
        .iter()
        .chain(props.data.session_loop_jobs.data.iter())
        .filter(|job| status_tone(&job.status) == "bad")
        .count();
    let pending_approvals = props
        .data
        .approvals
        .data
        .iter()
        .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
        .count();
    html! {
        <div class="manager-rail">
            <KeyMetrics values={vec![
                (props.lang.text("Manager plans", "Manager 计划").to_string(), props.data.manager_plans.data.len().to_string()),
                (props.lang.text("Handoffs", "交接").to_string(), props.data.agent_handoffs.data.len().to_string()),
                (props.lang.text("Assignments", "分派").to_string(), props.data.agent_handoff_assignments.data.len().to_string()),
                (props.lang.text("Pending approvals", "待审批").to_string(), pending_approvals.to_string()),
                (props.lang.text("Failed jobs", "失败任务").to_string(), failed_jobs.to_string()),
            ]} />
            <Rows empty={props.lang.text("No Manager Agent observations.", "没有 Manager Agent 建议。")} rows={props.data.manager_plans.data.iter().take(5).enumerate().map(|(index, plan)| {
                (
                    "info".to_string(),
                    if props.lang == UiLang::En { format!("Manager observation {}", index + 1) } else { format!("Manager 观察 {}", index + 1) },
                    compact_json(plan),
                )
            }).collect::<Vec<_>>()} />
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct ManagerObservationRailProps {
    data: ConsoleData,
    lang: UiLang,
}

#[derive(Properties, Clone, PartialEq)]
struct AgentTopologyProps {
    agents: Vec<Agent>,
    sessions: Vec<Session>,
    lang: UiLang,
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
                <FlowMeter label={props.lang.text("Agents", "智能体")} value={props.agents.len()} max={props.agents.len().max(1)} tone="good" />
                <FlowMeter label={props.lang.text("Active sessions", "运行中 Session")} value={props.sessions.iter().filter(|session| is_active_status(&session.status)).count()} max={props.sessions.len().max(1)} tone="info" />
                <FlowMeter label={props.lang.text("Released", "已发布")} value={props.agents.iter().filter(|agent| status_tone(&agent.release_state) == "good").count()} max={props.agents.len().max(1)} tone="good" />
            </div>
        </div>
    }
}
