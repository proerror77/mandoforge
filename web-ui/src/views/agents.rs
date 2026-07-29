use crate::api::{Agent, Session, api_get};
use crate::components::{
    ApprovalRows, FlowMeter, JsonPreview, KeyMetrics, Panel, Rows, RuntimePipeline, VersionBlock,
};
use crate::state::{ConsoleData, UiLang};
use crate::{
    compact_json, is_active_status, label_or, orbit_point, position_style, session_title, short_id,
    status_tone,
};
use serde_json::Value;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

const SESSION_DETAIL_ROUTE: &str = "/api/sessions/{session_id}";
const SESSION_EVENTS_ROUTE: &str = "/api/sessions/{session_id}/events";
const SESSION_ARTIFACTS_ROUTE: &str = "/api/sessions/{session_id}/artifacts";
const SESSION_AUDIT_LOGS_ROUTE: &str = "/api/sessions/{session_id}/audit-logs";

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct AgentsProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
    pub(crate) on_approve_approval: Callback<String>,
    pub(crate) on_reject_approval: Callback<String>,
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
    let runnable_agents = data
        .agents
        .data
        .iter()
        .filter(|agent| agent.is_runnable())
        .collect::<Vec<_>>();
    let runnable_environments = data
        .environments
        .data
        .iter()
        .filter(|environment| environment.is_runnable_for_release(data.agent_release_environment()))
        .collect::<Vec<_>>();
    let selected_agent_id = runnable_agents
        .iter()
        .find(|agent| agent.id == props.selected_agent_id)
        .or_else(|| runnable_agents.first())
        .map(|agent| agent.id.clone())
        .unwrap_or_default();
    let selected_environment_id = runnable_environments
        .iter()
        .find(|environment| environment.id == props.selected_environment_id)
        .map(|environment| environment.id.clone())
        .unwrap_or_default();
    let direct_session_launch_allowed = data.direct_session_launch_allowed();
    let selected_session_id = use_state(|| {
        data.sessions
            .data
            .first()
            .map(|session| session.id.clone())
            .unwrap_or_default()
    });
    let selected_session_detail = use_state(|| None::<Value>);
    let selected_session_events = use_state(|| None::<Value>);
    let selected_session_artifacts = use_state(|| None::<Value>);
    let selected_session_audit_logs = use_state(|| None::<Value>);
    let session_evidence_status = use_state(String::new);

    {
        let selected_session_id = selected_session_id.clone();
        let sessions = data.sessions.data.clone();
        use_effect_with(sessions, move |sessions| {
            if let Some(first_session) = sessions.first() {
                if selected_session_id.is_empty()
                    || !sessions
                        .iter()
                        .any(|session| session.id == *selected_session_id)
                {
                    selected_session_id.set(first_session.id.clone());
                }
            } else {
                selected_session_id.set(String::new());
            }
            || ()
        });
    }
    {
        let selected_session_id_handle = selected_session_id.clone();
        let selected_session_detail = selected_session_detail.clone();
        let selected_session_events = selected_session_events.clone();
        let selected_session_artifacts = selected_session_artifacts.clone();
        let selected_session_audit_logs = selected_session_audit_logs.clone();
        let session_evidence_status = session_evidence_status.clone();
        use_effect_with(
            (*selected_session_id_handle).clone(),
            move |selected_session_id| {
                if selected_session_id.is_empty() {
                    selected_session_detail.set(None);
                    selected_session_events.set(None);
                    selected_session_artifacts.set(None);
                    selected_session_audit_logs.set(None);
                    session_evidence_status.set(String::new());
                } else {
                    session_evidence_status.set("Loading session evidence...".to_string());
                    let detail_handle = selected_session_detail.clone();
                    let events_handle = selected_session_events.clone();
                    let artifacts_handle = selected_session_artifacts.clone();
                    let audit_handle = selected_session_audit_logs.clone();
                    let status_handle = session_evidence_status.clone();
                    let selected_session_id_handle = selected_session_id_handle.clone();
                    let session_id = selected_session_id.clone();
                    spawn_local(async move {
                        let detail_path = SESSION_DETAIL_ROUTE.replace("{session_id}", &session_id);
                        let events_path = SESSION_EVENTS_ROUTE.replace("{session_id}", &session_id);
                        let artifacts_path =
                            SESSION_ARTIFACTS_ROUTE.replace("{session_id}", &session_id);
                        let audit_path =
                            SESSION_AUDIT_LOGS_ROUTE.replace("{session_id}", &session_id);
                        let detail = api_get::<Value>(&detail_path).await;
                        let events = api_get::<Value>(&events_path).await;
                        let artifacts = api_get::<Value>(&artifacts_path).await;
                        let audit_logs = api_get::<Value>(&audit_path).await;
                        if *selected_session_id_handle != session_id {
                            return;
                        }
                        let mut failed = Vec::new();
                        if detail.is_err() {
                            failed.push("session");
                        }
                        if events.is_err() {
                            failed.push("events");
                        }
                        if artifacts.is_err() {
                            failed.push("artifacts");
                        }
                        if audit_logs.is_err() {
                            failed.push("audit logs");
                        }
                        detail_handle.set(detail.ok());
                        events_handle.set(events.ok());
                        artifacts_handle.set(artifacts.ok());
                        audit_handle.set(audit_logs.ok());
                        status_handle.set(if failed.is_empty() {
                            "Session evidence loaded.".to_string()
                        } else {
                            format!("Session evidence incomplete: {} failed.", failed.join(", "))
                        });
                    });
                }
                || ()
            },
        );
    }

    let on_session_select = {
        let selected_session_id = selected_session_id.clone();
        Callback::from(move |event: Event| {
            let value = event.target_unchecked_into::<HtmlSelectElement>().value();
            selected_session_id.set(value);
        })
    };
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
                            value={selected_agent_id}
                            onchange={props.on_agent.clone()}
                        >
                            { for runnable_agents.iter().map(|agent| html! {
                                <option value={agent.id.clone()}>{ format!("{} / {}", agent.name, label_or(&agent.agent_role, "agent")) }</option>
                            }) }
                        </select>
                    </label>
                    <label>
                        <span>{ lang.text("Environment", "环境") }</span>
                        <select
                            id="managed-agent-environment"
                            name="managed-agent-environment"
                            value={selected_environment_id}
                            onchange={props.on_environment.clone()}
                        >
                            <option value="">{ lang.text("Default environment", "默认环境") }</option>
                            { for runnable_environments.iter().map(|environment| html! {
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
                    <button disabled={runnable_agents.is_empty() || !direct_session_launch_allowed} onclick={props.on_start_task.clone()}>{ lang.text("Start task", "启动任务") }</button>
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
                <ApprovalRows
                    approvals={data.approvals.data.clone()}
                    lang={lang}
                    limit={8}
                    on_approve={props.on_approve_approval.clone()}
                    on_reject={props.on_reject_approval.clone()}
                />
            </Panel>
            <Panel title={lang.text("Tool Calls", "工具调用")}>
                <Rows empty={lang.text("No tool calls.", "没有工具调用。")} rows={data.tool_calls.data.iter().take(8).map(|call| {
                    (call.status.clone(), label_or(&call.tool_name, "tool").to_string(), short_id(&call.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Deployment Version", "部署版本")}>
                <VersionBlock version={data.deployment_version.data.clone()} />
            </Panel>
            <Panel title={lang.text("Session Evidence", "会话证据")}>
                <label>
                    <span>{ lang.text("Session", "会话") }</span>
                    <select value={(*selected_session_id).clone()} onchange={on_session_select}>
                        { for data.sessions.data.iter().map(|session| {
                            html! { <option value={session.id.clone()}>{ format!("{} / {}", short_id(&session.id), session_title(session)) }</option> }
                        }) }
                    </select>
                </label>
                <h3>{ lang.text("Session Detail", "会话详情") }</h3>
                <JsonPreview value={selected_session_detail.as_ref().cloned().unwrap_or(Value::Null)} />
                <h3>{ lang.text("Session Events", "会话事件") }</h3>
                <JsonPreview value={selected_session_events.as_ref().cloned().unwrap_or(Value::Null)} />
                <h3>{ lang.text("Session Artifacts", "会话产物") }</h3>
                <JsonPreview value={selected_session_artifacts.as_ref().cloned().unwrap_or(Value::Null)} />
                <h3>{ lang.text("Session Audit Logs", "审计日志") }</h3>
                <JsonPreview value={selected_session_audit_logs.as_ref().cloned().unwrap_or(Value::Null)} />
                <small>{ if selected_session_id.is_empty() {
                    lang.text("No active session selected for evidence inspection.", "未选择用于检查证据的会话。").to_string()
                } else {
                    (*session_evidence_status).clone()
                } }</small>
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
