use crate::api::{DynamicWorkflowPlan, WorkflowDefinition, WorkflowRun};
use crate::components::{FlowMeter, JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::{ConsoleData, UiLang};
use crate::{board_column, is_active_status, label_or, short_id, status_tone};
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunsTasksTab {
    Runs,
    Board,
    Templates,
    DynamicPlans,
    Approvals,
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct WorkflowsProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
    pub(crate) objective: String,
    pub(crate) on_objective: Callback<InputEvent>,
    pub(crate) on_compile: Callback<MouseEvent>,
}

impl RunsTasksTab {
    const ALL: [RunsTasksTab; 5] = [
        RunsTasksTab::Runs,
        RunsTasksTab::Board,
        RunsTasksTab::Templates,
        RunsTasksTab::DynamicPlans,
        RunsTasksTab::Approvals,
    ];

    fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                RunsTasksTab::Runs => "Runs",
                RunsTasksTab::Board => "Task Board",
                RunsTasksTab::Templates => "Templates",
                RunsTasksTab::DynamicPlans => "Dynamic Plans",
                RunsTasksTab::Approvals => "Approvals",
            },
            UiLang::Zh => match self {
                RunsTasksTab::Runs => "运行记录",
                RunsTasksTab::Board => "任务板",
                RunsTasksTab::Templates => "流程模板",
                RunsTasksTab::DynamicPlans => "动态计划",
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
                    "One place for runs, task board state, workflow templates, dynamic plans, and approvals.",
                    "统一查看托管智能体的运行记录、任务板、流程模板、动态计划和审批。"
                ) }</h2>
                <p>{ lang.text(
                    "This page absorbs the old Workflow, Dynamic Workflow, and Board concepts. Templates are reusable plans; dynamic plans are one-off compiled runs; the board is the execution-state view.",
                    "这里吸收原来的 Workflow、Dynamic Workflow 和 Board。固定流程是模板；动态流程是一次性计划；任务板是执行状态视图。"
                ) }</p>
            </section>

            <RunsTasksSummary data={props.data.clone()} lang={lang} />

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
                    RunsTasksTab::Runs => html! { <RunsPanel data={props.data.clone()} lang={lang} /> },
                    RunsTasksTab::Board => html! { <TaskBoardPanel data={props.data.clone()} lang={lang} /> },
                    RunsTasksTab::Templates => html! { <WorkflowTemplatesPanel data={props.data.clone()} lang={lang} /> },
                    RunsTasksTab::DynamicPlans => html! {
                        <DynamicPlansPanel
                            data={props.data.clone()}
                            lang={lang}
                            objective={props.objective.clone()}
                            on_objective={props.on_objective.clone()}
                            on_compile={props.on_compile.clone()}
                        />
                    },
                    RunsTasksTab::Approvals => html! { <ApprovalsPanel data={props.data.clone()} lang={lang} /> },
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
fn RunsPanel(props: &RunsTasksDataProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title={props.lang.text("Run Graph", "运行图")}>
                <WorkflowGraph runs={props.data.workflow_runs.data.clone()} definitions={props.data.workflow_definitions.data.clone()} lang={props.lang} />
            </Panel>
            <Panel title={props.lang.text("Run History", "运行记录")}>
                <Rows empty={props.lang.text("No workflow runs.", "没有工作流运行记录。")} rows={props.data.workflow_runs.data.iter().take(12).map(|run| {
                    (run.status.clone(), label_or(&run.title, "workflow run").to_string(), short_id(&run.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={props.lang.text("Scheduler", "调度器")}>
                <JsonPreview value={props.data.scheduler_summary.data.clone()} />
            </Panel>
            <Panel title={props.lang.text("Evidence Endpoints", "证据入口")}>
                <KeyMetrics values={vec![
                    (props.lang.text("Run steps", "运行步骤").to_string(), "/api/workflow-runs/{id}/steps".to_string()),
                    (props.lang.text("Transitions", "状态转换").to_string(), "/api/workflow-runs/{id}/transitions".to_string()),
                    (props.lang.text("Task grants", "任务授权").to_string(), "/api/workflow-runs/{id}/task-grants".to_string()),
                    (props.lang.text("Run graph", "运行图").to_string(), "/api/workflow-runs/{id}/graph".to_string()),
                ]} />
            </Panel>
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
                    (definition.status.clone(), label_or(&definition.name, "workflow").to_string(), label_or(&definition.version, "version").to_string())
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

#[derive(Properties, Clone, PartialEq)]
struct DynamicPlansPanelProps {
    data: ConsoleData,
    lang: UiLang,
    objective: String,
    on_objective: Callback<InputEvent>,
    on_compile: Callback<MouseEvent>,
}

#[component]
fn DynamicPlansPanel(props: &DynamicPlansPanelProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title={props.lang.text("Dynamic Plan Compiler", "动态计划编译器")}>
                <div class="form-stack">
                    <textarea
                        id="dynamic-plan-objective"
                        name="dynamic-plan-objective"
                        value={props.objective.clone()}
                        oninput={props.on_objective.clone()}
                    />
                    <button onclick={props.on_compile.clone()}>{ props.lang.text("Compile dynamic plan", "编译动态计划") }</button>
                </div>
            </Panel>
            <Panel title={props.lang.text("Plan Shape", "计划形态")}>
                <FleetShape plans={props.data.dynamic_workflow_plans.data.clone()} lang={props.lang} />
            </Panel>
            <Panel title={props.lang.text("Dynamic Plans", "动态计划")}>
                <Rows empty={props.lang.text("No dynamic plans.", "没有动态计划。")} rows={props.data.dynamic_workflow_plans.data.iter().take(12).map(|plan| {
                    (plan.status.clone(), label_or(&plan.objective, "dynamic plan").to_string(), label_or(&plan.runtime_adapter, "runtime").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={props.lang.text("Policy Boundaries", "策略边界")}>
                <KeyMetrics values={vec![
                    (props.lang.text("Max agents", "最大智能体数").to_string(), "1000 policy cap".to_string()),
                    (props.lang.text("Max parallel", "最大并行数").to_string(), "16 policy cap".to_string()),
                    (props.lang.text("Cross-check", "交叉复核").to_string(), "review and adjudication metadata".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[component]
fn ApprovalsPanel(props: &RunsTasksDataProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title={props.lang.text("Approval Queue", "审批队列")}>
                <Rows empty={props.lang.text("No pending approvals.", "没有待处理审批。")} rows={props.data.approvals.data.iter().take(12).map(|approval| {
                    (approval.status.clone(), label_or(&approval.kind, "approval").to_string(), label_or(&approval.reason, "reason").to_string())
                }).collect::<Vec<_>>()} />
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
                    <div class={classes!("graph-node", status_tone(&definition.status))} key={definition.id.clone()}>
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

#[derive(Properties, Clone, PartialEq)]
struct FleetShapeProps {
    plans: Vec<DynamicWorkflowPlan>,
    lang: UiLang,
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
                <FlowMeter label={props.lang.text("Compiled plans", "已编译计划")} value={props.plans.len()} max={total} tone="info" />
                <FlowMeter label={props.lang.text("Ready plans", "就绪计划")} value={props.plans.iter().filter(|plan| status_tone(&plan.status) == "good").count()} max={total} tone="good" />
            </div>
        </div>
    }
}
