use crate::api::{WorkflowDefinition, WorkflowRun, api_post, create_workflow_task_body};
use crate::components::{ApprovalRows, FlowMeter, JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::{ConsoleData, UiLang};
use crate::{board_column, is_active_status, label_or, short_id, status_tone};
use serde_json::Value;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunsTasksTab {
    Runs,
    Board,
    Templates,
    Approvals,
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct WorkflowsProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
    pub(crate) on_approve_approval: Callback<String>,
    pub(crate) on_reject_approval: Callback<String>,
}

impl RunsTasksTab {
    const ALL: [RunsTasksTab; 4] = [
        RunsTasksTab::Runs,
        RunsTasksTab::Board,
        RunsTasksTab::Templates,
        RunsTasksTab::Approvals,
    ];

    fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                RunsTasksTab::Runs => "Runs",
                RunsTasksTab::Board => "Task Board",
                RunsTasksTab::Templates => "Templates",
                RunsTasksTab::Approvals => "Approvals",
            },
            UiLang::Zh => match self {
                RunsTasksTab::Runs => "运行记录",
                RunsTasksTab::Board => "任务板",
                RunsTasksTab::Templates => "流程模板",
                RunsTasksTab::Approvals => "审批",
            },
        }
    }
}

fn board_column_label(lang: UiLang, column: &str) -> &'static str {
    match lang {
        UiLang::En => match column {
            "ready" => "Ready",
            "running" => "Running",
            "review" => "Review",
            "blocked" => "Blocked",
            "backlog" => "Backlog",
            "done" => "Done",
            _ => "Other",
        },
        UiLang::Zh => match column {
            "ready" => "待开始",
            "running" => "运行中",
            "review" => "复核",
            "blocked" => "阻塞",
            "backlog" => "待排期",
            "done" => "完成",
            _ => "其他",
        },
    }
}

#[component]
pub(crate) fn WorkflowsView(props: &WorkflowsProps) -> Html {
    let active_tab = use_state(|| RunsTasksTab::Runs);
    let lang = props.lang;
    html! {
        <div class="runs-tasks-workbench">
            <section class="page-purpose">
                <p class="eyebrow">{ lang.text("Runs & Tasks / 运行与任务", "运行与任务 / Runs & Tasks") }</p>
                <h2>{ lang.text(
                    "Submit a task, follow progress, and review the result.",
                    "提交任务、查看进展，并获取结果。"
                ) }</h2>
                <p>{ lang.text(
                    "Choose a published capability. Actions needing your approval appear with the task.",
                    "选择已发布的能力，需要你确认的动作会随任务一起显示。"
                ) }</p>
            </section>

            <RunsTasksSummary
                data={props.data.clone()}
                lang={lang}
            />

            <nav class="subnav-tabs" aria-label="Runs and tasks sections">
                { for RunsTasksTab::ALL.into_iter().map(|tab| {
                    let active_tab = active_tab.clone();
                    let is_active = *active_tab == tab;
                    html! {
                        <button
                            class={classes!("subnav-tab", is_active.then_some("active"))}
                            onclick={Callback::from(move |_| active_tab.set(tab))}
                        >
                            { tab.label(lang) }
                        </button>
                    }
                }) }
            </nav>

            {
                match *active_tab {
                    RunsTasksTab::Runs => html! {
                        <RunsPanel
                            data={props.data.clone()}
                            lang={lang}
                            on_approve_approval={props.on_approve_approval.clone()}
                            on_reject_approval={props.on_reject_approval.clone()}
                        />
                    },
                    RunsTasksTab::Board => html! {
                        <TaskBoardPanel
                            data={props.data.clone()}
                            lang={lang}
                        />
                    },
                    RunsTasksTab::Templates => html! {
                        <WorkflowTemplatesPanel
                            data={props.data.clone()}
                            lang={lang}
                        />
                    },
                    RunsTasksTab::Approvals => html! {
                        <ApprovalsPanel
                            data={props.data.clone()}
                            lang={lang}
                            on_approve_approval={props.on_approve_approval.clone()}
                            on_reject_approval={props.on_reject_approval.clone()}
                        />
                    },
                }
            }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct RunsTasksDataProps {
    data: ConsoleData,
    lang: UiLang,
}

#[component]
fn RunsTasksSummary(props: &RunsTasksDataProps) -> Html {
    let data = &props.data;
    let active_runs = data
        .workflow_runs
        .data
        .iter()
        .filter(|run| is_active_status(&run.status))
        .count();
    let failed_runs = data
        .workflow_runs
        .data
        .iter()
        .filter(|run| status_tone(&run.status) == "bad")
        .count();
    let blocked_tasks = data
        .task_board
        .data
        .items
        .iter()
        .filter(|item| board_column(&item.status) == "blocked")
        .count();
    let pending_approvals = data
        .approvals
        .data
        .iter()
        .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
        .count();

    html! {
        <section class="runs-summary">
            <FlowMeter label={props.lang.text("Runs", "运行")} value={data.workflow_runs.data.len()} max={data.workflow_runs.data.len().max(1)} tone="neutral" />
            <FlowMeter label={props.lang.text("Active", "运行中")} value={active_runs} max={data.workflow_runs.data.len().max(1)} tone="info" />
            <FlowMeter label={props.lang.text("Failed", "失败")} value={failed_runs} max={data.workflow_runs.data.len().max(1)} tone={if failed_runs > 0 { "bad" } else { "good" }} />
            <FlowMeter label={props.lang.text("Blocked tasks", "阻塞任务")} value={blocked_tasks} max={data.task_board.data.items.len().max(1)} tone={if blocked_tasks > 0 { "warn" } else { "good" }} />
            <FlowMeter label={props.lang.text("Approvals", "审批")} value={pending_approvals} max={data.approvals.data.len().max(1)} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
        </section>
    }
}

#[component]
fn RunsPanel(props: &WorkflowsProps) -> Html {
    let selected = use_state(|| None::<WorkflowRun>);
    let on_created = {
        let selected = selected.clone();
        Callback::from(move |run| selected.set(Some(run)))
    };
    let current = selected.as_ref().map(|selected| {
        props
            .data
            .workflow_runs
            .data
            .iter()
            .find(|run| run.id == selected.id)
            .unwrap_or(selected)
            .clone()
    });
    html! {
        <div class="page-stack">
            <TaskLauncher data={props.data.clone()} lang={props.lang} on_created={on_created} />
            <Panel title={props.lang.text("Tasks", "任务")}>
                if let Some(error) = &props.data.workflow_runs.error {
                    <p role="alert">{ error }</p>
                }
                <div class="task-history">
                    { for props.data.workflow_runs.data.iter().take(12).map(|run| {
                        let title = props.data.sessions.data.iter().find(|session| session.id == run.primary_session_id)
                            .map(|session| session.title.as_str()).filter(|title| !title.is_empty())
                            .unwrap_or_else(|| label_or(&run.title, props.lang.text("Task", "任务")));
                        let selected = selected.clone();
                        let value = run.clone();
                        html! { <button class="secondary task-history-item" onclick={Callback::from(move |_| selected.set(Some(value.clone())))}>
                            <strong>{ title }</strong><span>{ task_status_label(&run.status, props.lang) }</span>
                        </button> }
                    }) }
                </div>
                if props.data.workflow_runs.data.is_empty() { <p>{ props.lang.text("No tasks yet.", "还没有任务。") }</p> }
            </Panel>
            if let Some(run) = current {
                <TaskResult key={run.id.clone()} run={run.clone()} data={props.data.clone()} lang={props.lang}
                    on_approve_approval={props.on_approve_approval.clone()} on_reject_approval={props.on_reject_approval.clone()} />
            }
            <details>
                <summary>{ props.lang.text("Execution details", "执行详情") }</summary>
                <div class="page-grid">
                    <Panel title={props.lang.text("Run Graph", "运行图")}>
                        <WorkflowGraph runs={props.data.workflow_runs.data.clone()} definitions={props.data.workflow_definitions.data.clone()} lang={props.lang} />
                    </Panel>
                    <Panel title={props.lang.text("Scheduler", "调度器")}><JsonPreview value={props.data.scheduler_summary.data.clone()} /></Panel>
                    <Panel title={props.lang.text("Evidence Endpoints", "证据入口")}>
                        <KeyMetrics values={vec![
                            (props.lang.text("Run steps", "运行步骤").to_string(), "/api/workflow-runs/{id}/steps".to_string()),
                            (props.lang.text("Transitions", "状态转换").to_string(), "/api/workflow-runs/{id}/transitions".to_string()),
                            (props.lang.text("Task grants", "任务授权").to_string(), "/api/workflow-runs/{id}/task-grants".to_string()),
                        ]} />
                    </Panel>
                </div>
            </details>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct TaskLauncherProps {
    data: ConsoleData,
    lang: UiLang,
    on_created: Callback<WorkflowRun>,
}

#[component]
fn TaskLauncher(props: &TaskLauncherProps) -> Html {
    let definition_id = use_state(String::new);
    let environment_id = use_state(String::new);
    let objective = use_state(String::new);
    let submitting = use_state(|| false);
    let request_pending = use_mut_ref(|| false);
    let feedback = use_state(String::new);
    let lang = props.lang;
    let definitions: Vec<_> = props
        .data
        .workflow_definitions
        .data
        .iter()
        .filter(|definition| definition.release_state == "released")
        .collect();
    let selected = definitions
        .iter()
        .find(|definition| definition.id == *definition_id)
        .copied();
    let on_definition = {
        let definition_id = definition_id.clone();
        let environment_id = environment_id.clone();
        Callback::from(move |event: Event| {
            definition_id.set(event.target_unchecked_into::<HtmlSelectElement>().value());
            environment_id.set(String::new());
        })
    };
    let on_environment = {
        let environment_id = environment_id.clone();
        Callback::from(move |event: Event| {
            environment_id.set(event.target_unchecked_into::<HtmlSelectElement>().value())
        })
    };
    let on_objective = {
        let objective = objective.clone();
        Callback::from(move |event: InputEvent| {
            objective.set(event.target_unchecked_into::<HtmlTextAreaElement>().value())
        })
    };
    let submit = {
        let selected = selected.cloned();
        let request_pending = request_pending.clone();
        let environment_id = environment_id.clone();
        let objective = objective.clone();
        let submitting = submitting.clone();
        let feedback = feedback.clone();
        let on_created = props.on_created.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *request_pending.borrow() {
                return;
            }
            let Some(definition) = &selected else {
                return;
            };
            let env_id = if environment_id.is_empty() {
                None
            } else {
                Some(environment_id.as_str())
            };
            let body = match create_workflow_task_body(&definition.id, env_id, &objective) {
                Ok(body) => body,
                Err(error) => {
                    feedback.set(error);
                    return;
                }
            };
            *request_pending.borrow_mut() = true;
            submitting.set(true);
            feedback.set(lang.text("Submitting task…", "正在提交任务…").to_string());
            let (submitting, feedback, on_created) =
                (submitting.clone(), feedback.clone(), on_created.clone());
            let request_pending = request_pending.clone();
            spawn_local(async move {
                match api_post::<WorkflowRun, _>("/api/workflow-runs", &body).await {
                    Ok(run) => {
                        feedback.set(
                            lang.text(
                                "Task accepted. Follow its progress below.",
                                "任务已受理，请在下方查看进展。",
                            )
                            .to_string(),
                        );
                        on_created.emit(run);
                    }
                    Err(error) => feedback.set(format!(
                        "{} {error}",
                        lang.text(
                            "Submission was not confirmed; check task history before retrying.",
                            "未确认提交成功，请先检查任务记录再重试。"
                        )
                    )),
                }
                *request_pending.borrow_mut() = false;
                submitting.set(false);
            });
        })
    };
    html! {
        <Panel title={lang.text("New task", "新任务")}>
            <form class="form-stack" onsubmit={submit}>
                <label for="task-capability">{ lang.text("Published capability", "已发布能力") }</label>
                <select id="task-capability" value={(*definition_id).clone()} onchange={on_definition} disabled={*submitting} required={true}>
                    <option value="" selected={definition_id.is_empty()}>{ lang.text("Choose a capability", "选择要使用的能力") }</option>
                    { for definitions.iter().map(|definition| html! { <option value={definition.id.clone()} selected={definition.id == *definition_id}>{ &definition.name }</option> }) }
                </select>
                if let Some(error) = &props.data.workflow_definitions.error { <p role="alert">{ error }</p> }
                if definitions.is_empty() { <p>{ lang.text("No published capability is available. Publish an installed workflow before starting a task.", "还没有可用的已发布能力，请先发布已安装的业务流程。") }</p> }
                if selected.is_some_and(|definition| definition.default_environment_id.is_none()) {
                    <label for="task-environment">{ lang.text("Execution environment", "执行环境") }</label>
                    <select id="task-environment" value={(*environment_id).clone()} onchange={on_environment} disabled={*submitting}>
                        <option value="" selected={environment_id.is_empty()}>{ lang.text("Use configured default", "使用已配置的默认环境") }</option>
                        { for props.data.environments.data.iter().filter(|environment| environment.is_runnable_for_release(props.data.agent_release_environment())).map(|environment| html! {
                            <option value={environment.id.clone()} selected={environment.id == *environment_id}>{ &environment.name }</option>
                        }) }
                    </select>
                }
                <label for="task-objective">{ lang.text("What needs to be done?", "需要完成什么？") }</label>
                <textarea id="task-objective" rows="3" value={(*objective).clone()} oninput={on_objective} disabled={*submitting} required={true}
                    placeholder={lang.text("Describe the result you need and provide the relevant details.", "描述希望得到的结果，并提供相关信息。")}/>
                <div><button type="submit" disabled={*submitting || selected.is_none() || objective.trim().is_empty()}>{ lang.text("Submit task", "提交任务") }</button></div>
                <p role="status" aria-live="polite">{ &*feedback }</p>
            </form>
        </Panel>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct TaskResultProps {
    run: WorkflowRun,
    data: ConsoleData,
    lang: UiLang,
    on_approve_approval: Callback<String>,
    on_reject_approval: Callback<String>,
}

fn task_status_label(status: &str, lang: UiLang) -> &str {
    match status {
        "queued" | "initializing" | "scheduled" => lang.text("Queued", "等待执行"),
        "running" => lang.text("In progress", "执行中"),
        "requires_action" => lang.text("Waiting to continue", "等待继续"),
        "completed" => lang.text("Completed", "已完成"),
        "failed" => lang.text("Failed", "执行失败"),
        "canceled" => lang.text("Canceled", "已取消"),
        "skipped" => lang.text("Skipped", "已跳过"),
        _ => status,
    }
}

#[component]
fn TaskResult(props: &TaskResultProps) -> Html {
    let steps = crate::use_polling_dynamic::<Vec<Value>>(
        "/api/workflow-runs/{id}/steps".replace("{id}", &props.run.id),
        1_800,
        true,
    );
    let session_ids: std::collections::BTreeSet<_> = steps
        .data
        .iter()
        .filter_map(|step| step["session_id"].as_str())
        .chain(std::iter::once(props.run.primary_session_id.as_str()))
        .collect();
    let approvals = props
        .data
        .approvals
        .data
        .iter()
        .filter(|approval| {
            if matches!(
                props.run.status.as_str(),
                "completed" | "failed" | "canceled" | "skipped"
            ) && approval.status == "pending"
            {
                return false;
            }
            approval
                .session_id
                .as_deref()
                .is_some_and(|id| session_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    html! {
        <Panel title={props.lang.text("Task progress and result", "任务进展与结果")}>
            <p><strong>{ task_status_label(&props.run.status, props.lang) }</strong>{ format!(" · {}", short_id(&props.run.id)) }</p>
            if let Some(error) = steps.error.as_ref() { <p role="alert">{ error }</p> }
            { for steps.data.iter().map(|step| {
                let reason = step.pointer("/output_payload/worker_execution/error").and_then(Value::as_str)
                    .or_else(|| step.pointer("/output_payload/delegated_runtime/reason").and_then(Value::as_str));
                html! { <div class="task-step-result">
                    <strong>{ step["step_key"].as_str().unwrap_or_default() }</strong>
                    <span>{ task_status_label(step["status"].as_str().unwrap_or_default(), props.lang) }</span>
                    if let Some(reason) = reason { <p>{ reason }</p> }
                </div> }
            }) }
            if !approvals.is_empty() {
                <ApprovalRows approvals={approvals} lang={props.lang} limit={12} on_approve={props.on_approve_approval.clone()} on_reject={props.on_reject_approval.clone()} />
            }
            { for session_ids.iter().map(|session_id| html! {
                <SessionTaskOutput key={session_id.to_string()} session_id={session_id.to_string()} task_status={props.run.status.clone()} lang={props.lang} />
            }) }
        </Panel>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SessionTaskOutputProps {
    session_id: String,
    task_status: String,
    lang: UiLang,
}

#[component]
fn SessionTaskOutput(props: &SessionTaskOutputProps) -> Html {
    let events = crate::use_polling_dynamic::<Vec<Value>>(
        "/api/sessions/{session_id}/events".replace("{session_id}", &props.session_id),
        1_800,
        true,
    );
    let artifacts = crate::use_polling_dynamic::<Vec<Value>>(
        "/api/sessions/{session_id}/artifacts".replace("{session_id}", &props.session_id),
        2_500,
        true,
    );
    let summary = events
        .data
        .iter()
        .rev()
        .find(|event| {
            event["actor_type"] == "agent"
                && matches!(
                    event["event_type"].as_str(),
                    Some("session.goal.completed" | "session.goal.blocked")
                )
        })
        .and_then(|event| event["payload"]["summary"].as_str());
    html! {
        <div>
            if let Some(error) = events.error.as_ref().or(artifacts.error.as_ref()) { <p role="alert">{ error }</p> }
            if let Some(summary) = summary {
                <p><strong>{ if props.task_status == "completed" { props.lang.text("Result", "结果") } else { props.lang.text("Step update — task not complete", "阶段反馈（任务尚未完成）") } }</strong></p>
                <p class="task-result-text">{ summary }</p>
            }
            { for artifacts.data.iter().map(|artifact| html! { <details>
                <summary>{ artifact["name"].as_str().unwrap_or(props.lang.text("Result file", "结果文件")) }</summary>
                <pre class="task-result-text">{ artifact["content"].as_str().map(str::to_string).unwrap_or_else(|| artifact["content"].to_string()) }</pre>
            </details> }) }
        </div>
    }
}

#[component]
fn TaskBoardPanel(props: &RunsTasksDataProps) -> Html {
    let items = &props.data.task_board.data.items;
    html! {
        <div class="page-stack">
            <div class="kanban">
                { for ["ready", "running", "review", "blocked", "backlog", "done"].iter().map(|column| {
                    let filtered = items.iter().filter(|item| board_column(&item.status) == *column).collect::<Vec<_>>();
                    html! {
                        <section class="board-column">
                            <header>
                                <strong>{ board_column_label(props.lang, column) }</strong>
                                <span>{ filtered.len() }</span>
                            </header>
                            { for filtered.into_iter().map(|item| html! {
                                <article class="board-card" key={item.id.clone()}>
                                    <strong>{ label_or(&item.title, item.work_item.as_ref().map(|work| work.title.as_str()).unwrap_or(props.lang.text("Untitled work", "未命名工作项"))) }</strong>
                                    <span>{ format!("{} / {}", label_or(&item.priority, "normal"), short_id(&item.id)) }</span>
                                </article>
                            }) }
                        </section>
                    }
                }) }
            </div>
            <div class="page-grid">
                <Panel title={props.lang.text("Work Items", "工作项")}>
                    <Rows empty={props.lang.text("No work items.", "没有工作项。")} rows={props.data.work_items.data.iter().take(8).map(|item| {
                        (item.status.clone(), label_or(&item.title, "work item").to_string(), item.priority.clone())
                    }).collect::<Vec<_>>()} />
                </Panel>
                <Panel title={props.lang.text("Handoffs and Review", "交接与复核")}>
                    <KeyMetrics values={vec![
                        (props.lang.text("Manager plans", "Manager 计划").to_string(), props.data.manager_plans.data.len().to_string()),
                        (props.lang.text("Handoffs", "交接").to_string(), props.data.agent_handoffs.data.len().to_string()),
                        (props.lang.text("Assignments", "分派").to_string(), props.data.agent_handoff_assignments.data.len().to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
    }
}

#[component]
fn WorkflowTemplatesPanel(props: &RunsTasksDataProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title={props.lang.text("Workflow Templates", "流程模板")}>
                <Rows empty={props.lang.text("No workflow templates.", "没有流程模板。")} rows={props.data.workflow_definitions.data.iter().take(12).map(|definition| {
                    (definition.release_state.clone(), label_or(&definition.name, "workflow").to_string(), label_or(&definition.version, "version").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={props.lang.text("Template Execution Policy", "模板执行策略")}>
                <KeyMetrics values={props.data.workflow_definitions.data.iter().take(10).map(|definition| {
                    (
                        label_or(&definition.name, "workflow").to_string(),
                        label_or(&definition.execution_strategy, "strategy").to_string(),
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
        </div>
    }
}

#[component]
fn ApprovalsPanel(props: &WorkflowsProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title={props.lang.text("Approval Queue", "审批队列")}>
                <ApprovalRows
                    approvals={props.data.approvals.data.clone()}
                    lang={props.lang}
                    limit={12}
                    on_approve={props.on_approve_approval.clone()}
                    on_reject={props.on_reject_approval.clone()}
                />
            </Panel>
            <Panel title={props.lang.text("Human Review Rules", "人工确认原则")}>
                <KeyMetrics values={vec![
                    (props.lang.text("High-risk actions", "高风险动作").to_string(), props.lang.text("Human confirmation required", "必须人工确认").to_string()),
                    (props.lang.text("Business writes", "业务写入").to_string(), props.lang.text("Draft first, then approval", "先 draft，后 approval").to_string()),
                    (props.lang.text("Audit", "审计").to_string(), props.lang.text("Keep proposal, decision, actor, and reason", "保留 proposal、decision、actor、reason").to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct WorkflowGraphProps {
    runs: Vec<WorkflowRun>,
    definitions: Vec<WorkflowDefinition>,
    lang: UiLang,
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
                    <div class={classes!("graph-node", status_tone(&definition.release_state))} key={definition.id.clone()}>
                        <span>{ index + 1 }</span>
                        <strong>{ label_or(&definition.name, "workflow") }</strong>
                    </div>
                }) }
                { if props.definitions.is_empty() {
                    html! { <p class="empty">{ props.lang.text("No workflow definitions.", "没有工作流定义。") }</p> }
                } else {
                    html! {}
                }}
            </div>
            <div class="graph-stats">
                <FlowMeter label={props.lang.text("Runs", "运行")} value={props.runs.len()} max={props.runs.len().max(1)} tone="neutral" />
                <FlowMeter label={props.lang.text("Active", "运行中")} value={active_runs} max={props.runs.len().max(1)} tone="info" />
                <FlowMeter label={props.lang.text("Failed", "失败")} value={failed_runs} max={props.runs.len().max(1)} tone={if failed_runs > 0 { "bad" } else { "good" }} />
            </div>
        </div>
    }
}
