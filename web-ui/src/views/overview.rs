use crate::components::{
    EnterpriseReadinessPanel, FlowMeter, KeyMetrics, OverviewButton, PackMosaic, Panel, Rows,
    RuntimePipeline,
};
use crate::state::{ConsoleData, UiLang, View};
use crate::{
    active_job_count, blocked_pack_count, failed_job_count, first_lane_blocker, is_active_status,
    json_status, label_or, operator_queue_rows, pack_overview_rows, pending_approval_count,
    ready_pack_count, status_tone, worker_issue_rows,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct OverviewProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
    pub(crate) on_view: Callback<View>,
}

#[component]
pub(crate) fn OverviewView(props: &OverviewProps) -> Html {
    let data = &props.data;
    let lang = props.lang;
    let active_sessions = data
        .sessions
        .data
        .iter()
        .filter(|session| is_active_status(&session.status))
        .count();
    let pending_approvals = pending_approval_count(&data.approvals.data);
    let active_workers = active_job_count(&data.execution_jobs.data)
        + active_job_count(&data.session_loop_jobs.data);
    let failed_jobs = failed_job_count(&data.execution_jobs.data)
        + failed_job_count(&data.session_loop_jobs.data);
    let active_runs = data
        .workflow_runs
        .data
        .iter()
        .filter(|run| is_active_status(&run.status))
        .count();
    let ready_packs = ready_pack_count(&data.workflow_pack_installations.data);
    let blocked_packs = blocked_pack_count(&data.workflow_pack_installations.data);
    let connector_status = json_status(&data.native_connector_production_readiness.data);
    let ontology_status = json_status(&data.ontology_engine_readiness.data);
    let enterprise = &data.enterprise_product_readiness.data;
    let primary_next_action = enterprise
        .next_actions
        .first()
        .cloned()
        .or_else(|| first_lane_blocker(enterprise))
        .unwrap_or_else(|| {
            lang.text(
                "No enterprise readiness next action reported.",
                "企业上线状态暂未上报下一步动作。",
            )
            .to_string()
        });
    let enterprise_lane_total = enterprise.lane_count.max(enterprise.lanes.len());
    let enterprise_lane_value = if enterprise_lane_total == 0 {
        lang.text("not reported", "未上报").to_string()
    } else {
        format!("{}/{}", enterprise.ready_lane_count, enterprise_lane_total)
    };
    let enterprise_status_label = if label_or(&enterprise.status, "unknown") == "unknown" {
        lang.text("unknown", "未上报").to_string()
    } else {
        label_or(&enterprise.status, "unknown").to_string()
    };
    let readiness_tone = if enterprise.completion_blocked || enterprise.blocked_lane_count > 0 {
        "bad"
    } else {
        status_tone(&enterprise.status)
    };

    html! {
        <div class="overview-layout">
            <section class="agent-os-cockpit" aria-label={lang.text("Agent OS cockpit", "Agent OS 驾驶舱")}>
                <div class="cockpit-cycle">
                    <header class="cockpit-section-title">
                        <span>{ lang.text("CYCLE", "闭环") }</span>
                        <strong>{ lang.text("Agent OS improvement loop", "Agent OS 改进循环") }</strong>
                    </header>
                    <div class="cockpit-cycle-list">
                        <CockpitCycleStep
                            number="01"
                            icon="◎"
                            title={lang.text("Managed Agents", "托管智能体")}
                            detail={lang.text("Running agents — observe fleet roles, sessions, tool calls, and approval pressure.", "Running agents — 观察智能体职责、会话、工具调用和审批压力。")}
                            value={if lang == UiLang::En { format!("{active_sessions} active") } else { format!("{active_sessions} 运行") }}
                            status={if pending_approvals > 0 { lang.text("REVIEW", "需复核") } else { lang.text("READY", "就绪") }}
                            tone={if pending_approvals > 0 { "warn" } else { "good" }}
                            target={View::Agents}
                            on_view={props.on_view.clone()}
                        />
                        <CockpitCycleStep
                            number="02"
                            icon="◇"
                            title={lang.text("Runs & Tasks", "运行与任务")}
                            detail={lang.text("Inspect runs, task board state, workflow templates, and approvals.", "检查运行记录、任务板、流程模板和审批。")}
                            value={if lang == UiLang::En { format!("{} runs", data.workflow_runs.data.len()) } else { format!("{} 次运行", data.workflow_runs.data.len()) }}
                            status={if active_runs > 0 { lang.text("RUNNING", "运行中") } else { lang.text("IDLE", "空闲") }}
                            tone={if active_runs > 0 { "info" } else { "neutral" }}
                            target={View::Workflows}
                            on_view={props.on_view.clone()}
                        />
                        <CockpitCycleFocus
                            lang={lang}
                            object_count={data.semantic_graph.data.node_count}
                            link_count={data.semantic_graph.data.edge_count}
                            reflection_count={data.semantic_reflection_queue.data.queue.len()}
                            ontology_status={ontology_status.clone()}
                            target={View::Semantic}
                            on_view={props.on_view.clone()}
                        />
                        <CockpitCycleStep
                            number="04"
                            icon="□"
                            title={lang.text("Capabilities", "能力包")}
                            detail={lang.text("Package connectors, actions, templates, and runtime boundaries.", "封装连接器、动作、模板和运行边界。")}
                            value={if lang == UiLang::En { format!("{ready_packs} ready") } else { format!("{ready_packs} 就绪") }}
                            status={if blocked_packs > 0 { lang.text("BLOCKED", "阻塞") } else { lang.text("READY", "就绪") }}
                            tone={if blocked_packs > 0 { "warn" } else { "good" }}
                            target={View::Packs}
                            on_view={props.on_view.clone()}
                        />
                        <CockpitCycleStep
                            number="05"
                            icon="△"
                            title={lang.text("System Ops", "系统运维")}
                            detail={lang.text("Validate deployment, security boundary, audit, usage, and release evidence.", "验证部署、安全边界、审计、用量和发布证据。")}
                            value={enterprise_lane_value.clone()}
                            status={enterprise_status_label.clone()}
                            tone={readiness_tone}
                            target={View::Deploy}
                            on_view={props.on_view.clone()}
                        />
                    </div>
                </div>

                <aside class="cockpit-diagnosis">
                    <header class="cockpit-section-title">
                        <span>{ lang.text("DIAGNOSIS", "诊断") }</span>
                        <strong>{ lang.text("What needs operator attention", "需要操作员注意什么") }</strong>
                    </header>
                    <CockpitDiagnosisItem
                        icon="◎"
                        label={lang.text("Pattern in focus", "当前模式")}
                        title={lang.text("Managed agents need ontology, capability, and runtime gates to agree.", "托管智能体需要本体、能力包和运行闸门保持一致。")}
                        detail={lang.text(
                            "This view summarizes the Agent OS loop before you drill into a specific page.",
                            "这个页面先汇总 Agent OS 循环，再让你进入具体页面处理。"
                        )}
                        tone="neutral"
                    />
                    <CockpitDiagnosisItem
                        icon="△"
                        label={lang.text("What goes wrong", "常见问题")}
                        title={if failed_jobs > 0 {
                            lang.text("Failed jobs are already visible in the execution queue.", "执行队列中已经出现失败任务。")
                        } else {
                            lang.text("No failed execution jobs in the current summary.", "当前摘要中没有失败执行任务。")
                        }}
                        detail={if failed_jobs > 0 {
                            if lang == UiLang::En { format!("{failed_jobs} worker or session-loop jobs need review.") } else { format!("{failed_jobs} 个 worker 或 session-loop 任务需要复核。") }
                        } else {
                            lang.text("Queue pressure is currently driven by active work rather than errors.", "当前队列压力主要来自活跃任务，而不是错误。").to_string()
                        }}
                        tone={if failed_jobs > 0 { "bad" } else { "good" }}
                    />
                    <CockpitDiagnosisItem
                        icon="?"
                        label={lang.text("Likely cause", "可能原因")}
                        title={lang.text("The next action is still controlled by enterprise readiness evidence.", "下一步仍由企业上线证据控制。")}
                        detail={primary_next_action.clone()}
                        tone={readiness_tone}
                    />
                    <CockpitDiagnosisItem
                        icon="↗"
                        label={lang.text("Impact", "影响")}
                        title={if enterprise.blocked_lane_count > 0 {
                            if lang == UiLang::En { format!("{} blocked enterprise lanes", enterprise.blocked_lane_count) } else { format!("{} 个企业上线面阻塞", enterprise.blocked_lane_count) }
                        } else {
                            lang.text("Enterprise lanes are not reporting blockers.", "企业上线面当前没有报告阻塞。").to_string()
                        }}
                        detail={lang.text(
                            "Open System Ops for production evidence, or Ontology for semantic mapping and proposal review.",
                            "进入 System Ops 查看生产证据，或进入 Ontology 审核语义映射和本体提案。"
                        ).to_string()}
                        tone={readiness_tone}
                    />
                </aside>

                <section class="cockpit-telemetry">
                    <header>
                        <span>{ lang.text("Cost, latency, and operating load", "成本、延迟和运行负载") }</span>
                        <strong>{ lang.text("live-control-plane", "实时控制平面") }</strong>
                    </header>
                    <div class="cockpit-telemetry-grid">
                        <CockpitMetricPanel
                            title={lang.text("Queue", "队列")}
                            subtitle={lang.text("Execution pressure", "执行压力")}
                            rows={vec![
                                (lang.text("Active work", "活跃任务").to_string(), active_workers, active_workers.max(1), "info"),
                                (lang.text("Failed jobs", "失败任务").to_string(), failed_jobs, failed_jobs.max(active_workers).max(1), if failed_jobs > 0 { "bad" } else { "good" }),
                                (lang.text("Approvals", "审批").to_string(), pending_approvals, pending_approvals.max(data.approvals.data.len()).max(1), if pending_approvals > 0 { "warn" } else { "good" }),
                            ]}
                        />
                        <CockpitMetricPanel
                            title={lang.text("Ontology", "本体")}
                            subtitle={lang.text("Semantic coverage", "语义覆盖")}
                            rows={vec![
                                (lang.text("Objects", "对象").to_string(), data.semantic_graph.data.node_count, data.semantic_graph.data.node_count.max(1), "good"),
                                (lang.text("Links", "关系").to_string(), data.semantic_graph.data.edge_count, data.semantic_graph.data.edge_count.max(data.semantic_graph.data.node_count).max(1), "info"),
                                (lang.text("Review queue", "审核队列").to_string(), data.semantic_reflection_queue.data.queue.len(), data.semantic_reflection_queue.data.queue.len().max(1), "warn"),
                            ]}
                        />
                        <CockpitMetricPanel
                            title={lang.text("Release", "发布")}
                            subtitle={lang.text("Evidence gates", "证据闸门")}
                            rows={vec![
                                (lang.text("Ready lanes", "就绪面").to_string(), enterprise.ready_lane_count, enterprise.lane_count.max(1), "good"),
                                (lang.text("Blocked lanes", "阻塞面").to_string(), enterprise.blocked_lane_count, enterprise.lane_count.max(1), if enterprise.blocked_lane_count > 0 { "bad" } else { "good" }),
                                (lang.text("Ready packs", "就绪能力包").to_string(), ready_packs, ready_packs.max(blocked_packs).max(1), "info"),
                            ]}
                        />
                    </div>
                </section>

                <footer class="cockpit-warning-strip">
                    <span>{ if enterprise.blocked_lane_count > 0 { "WARN" } else { "OK" } }</span>
                    <strong>{ if enterprise.blocked_lane_count > 0 {
                        if lang == UiLang::En { format!("{} candidates rejected by readiness and production gates", enterprise.blocked_lane_count) } else { format!("{} 个候选项被就绪和生产闸门拦下", enterprise.blocked_lane_count) }
                    } else {
                        lang.text("No enterprise readiness blockers in the current summary", "当前摘要中没有企业就绪阻塞").to_string()
                    } }</strong>
                </footer>
            </section>

            <details class="overview-detail-drawer">
                <summary>{ lang.text("Detailed operator map", "详细操作地图") }</summary>
                <section class="operator-task-map" aria-label="Operator task navigation">
                    <OperatorTaskCard
                    number="01"
                    title={lang.text("Managed Agents", "托管智能体")}
                    subtitle={lang.text("托管智能体", "Managed Agents")}
                    description={lang.text("Observe each agent's role, sessions, tool calls, approvals, and failure pressure.", "查看每个智能体的职责、状态、session、工具调用和需要人工确认的地方。")}
                    status={if lang == UiLang::En { format!("{active_sessions} active / {pending_approvals} approvals") } else { format!("{active_sessions} 运行 / {pending_approvals} 审批") }}
                    tone={if pending_approvals > 0 { "warn" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="02"
                    title={lang.text("Runs & Tasks", "运行与任务")}
                    subtitle={lang.text("运行与任务", "Runs & Tasks")}
                    description={lang.text("Review runs, task board state, workflow templates, and approval queues.", "查看运行记录、任务板、流程模板和审批队列。")}
                    status={if lang == UiLang::En { format!("{} workflow runs", data.workflow_runs.data.len()) } else { format!("{} 个工作流运行", data.workflow_runs.data.len()) }}
                    tone={if active_runs > 0 { "info" } else { "neutral" }}
                    target={View::Workflows}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="03"
                    title={lang.text("Ontology", "本体与工具")}
                    subtitle={lang.text("本体与工具", "Ontology")}
                    description={lang.text("Turn tables, fields, relationships, metrics, and actions into governed agent tools.", "把数据表、字段、关系、指标和动作变成智能体可调用的治理工具。")}
                    status={if lang == UiLang::En { format!("{} objects / {} links", data.semantic_graph.data.node_count, data.semantic_graph.data.edge_count) } else { format!("{} 对象 / {} 关系", data.semantic_graph.data.node_count, data.semantic_graph.data.edge_count) }}
                    tone={status_tone(&ontology_status)}
                    target={View::Semantic}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="04"
                    title={lang.text("Capabilities", "能力包")}
                    subtitle={lang.text("能力包", "Capabilities")}
                    description={lang.text("Manage industry packs, connectors, templates, and the boundaries exposed to managed agents.", "管理行业包、连接器、模板和每个能力对托管智能体开放的边界。")}
                    status={if lang == UiLang::En { format!("{ready_packs} ready / {blocked_packs} blocked") } else { format!("{ready_packs} 就绪 / {blocked_packs} 阻塞") }}
                    tone={if blocked_packs > 0 { "warn" } else { "good" }}
                    target={View::Packs}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="05"
                    title={lang.text("System Ops", "系统运维")}
                    subtitle={lang.text("系统运维", "System Ops")}
                    description={lang.text("Check deployment, desktop shell, security boundary, audit, usage, cost, alerts, and release evidence.", "检查部署、桌面端、安全边界、审计、成本、告警和企业级上线证据。")}
                    status={enterprise_status_label.clone()}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="06"
                    title={lang.text("Attention", "当前阻塞")}
                    subtitle={lang.text("当前阻塞", "Attention")}
                    description={lang.text("See critical notifications, failed work, connector blockers, and the next human action.", "查看严重通知、失败任务、连接器阻塞和需要人工处理的下一步。")}
                    status={if lang == UiLang::En { format!("{} blocked lanes", enterprise.blocked_lane_count) } else { format!("{} 个阻塞面", enterprise.blocked_lane_count) }}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
                </section>
            </details>

            <section class="overview-signals">
                <OverviewButton label={lang.text("Open Managed Agents", "查看托管智能体")} target={View::Agents} on_view={props.on_view.clone()} />
                <OverviewButton label={lang.text("Open Runs & Tasks", "查看运行与任务")} target={View::Workflows} on_view={props.on_view.clone()} />
                <OverviewButton label={lang.text("Open Ontology", "查看本体工具")} target={View::Semantic} on_view={props.on_view.clone()} />
                <OverviewButton label={lang.text("Open System Ops", "查看系统运维")} target={View::Deploy} on_view={props.on_view.clone()} />
            </section>

            <div class="overview-grid">
                <Panel title={lang.text("Runtime Pressure", "运行压力")}>
                    <RuntimePipeline
                        sessions={data.sessions.data.clone()}
                        execution_jobs={data.execution_jobs.data.clone()}
                        session_loop_jobs={data.session_loop_jobs.data.clone()}
                        approvals={data.approvals.data.clone()}
                        tool_calls={data.tool_calls.data.clone()}
                    />
                    <Rows empty={lang.text("No failed execution jobs.", "没有失败的执行任务。")} rows={worker_issue_rows(&data.execution_jobs.data, &data.session_loop_jobs.data)} />
                </Panel>
                <Panel title={lang.text("Enterprise Readiness", "企业上线状态")}>
                    <EnterpriseReadinessPanel readiness={enterprise.clone()} />
                </Panel>
                <Panel title={lang.text("Capability Pack Status", "应用包能力状态")}>
                    <PackMosaic
                        installations={data.workflow_pack_installations.data.clone()}
                        marketplace={data.workflow_pack_marketplace.data.clone()}
                    />
                    <Rows empty={lang.text("No capability packs installed yet.", "还没有安装应用包。")} rows={pack_overview_rows(&data.workflow_pack_installations.data, &data.workflow_pack_marketplace.data)} />
                </Panel>
                <Panel title={lang.text("Connector and Ontology Gates", "连接器与本体闸门")}>
                    <KeyMetrics values={vec![
                        (lang.text("Native connectors", "原生连接器").to_string(), connector_status.clone()),
                        (lang.text("Ontology engine", "本体引擎").to_string(), ontology_status.clone()),
                        (lang.text("Semantic objects", "语义对象").to_string(), data.semantic_objects.data.len().to_string()),
                        (lang.text("Relation edges", "关系边").to_string(), data.semantic_graph.data.edge_count.to_string()),
                        (lang.text("Reflection queue", "反思队列").to_string(), data.semantic_reflection_queue.data.queue.len().to_string()),
                    ]} />
                    <div class="overview-gate-actions">
                        <OverviewButton label={lang.text("Open System Ops", "查看系统运维")} target={View::Deploy} on_view={props.on_view.clone()} />
                        <OverviewButton label={lang.text("Open Ontology", "查看本体工具")} target={View::Semantic} on_view={props.on_view.clone()} />
                    </div>
                </Panel>
                <Panel title={lang.text("Needs Attention", "当前需要处理")}>
                    <Rows empty={lang.text("No immediate blockers.", "没有立即阻塞项。")} rows={
                        if operator_queue_rows(data).is_empty() {
                            vec![(
                                readiness_tone.to_string(),
                                lang.text("Next action", "下一步").to_string(),
                                primary_next_action.clone(),
                            )]
                        } else {
                            operator_queue_rows(data)
                        }
                    } />
                </Panel>
                <Panel title={lang.text("Evidence Endpoints", "证据入口")}>
                    <KeyMetrics values={vec![
                        (lang.text("Enterprise product", "企业产品状态").to_string(), "/api/enterprise-product/readiness".to_string()),
                        (lang.text("Connector production", "连接器生产状态").to_string(), "/api/native-connectors/production-readiness".to_string()),
                        (lang.text("Ontology readiness", "本体就绪状态").to_string(), "/api/ontology/engine-readiness".to_string()),
                        (lang.text("Pack lifecycle", "应用包生命周期").to_string(), "/api/workflow-packs/installations".to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct CockpitCycleStepProps {
    number: &'static str,
    icon: &'static str,
    title: &'static str,
    detail: &'static str,
    value: String,
    status: String,
    #[prop_or("neutral")]
    tone: &'static str,
    target: View,
    on_view: Callback<View>,
}

#[component]
fn CockpitCycleStep(props: &CockpitCycleStepProps) -> Html {
    let target = props.target;
    let on_view = props.on_view.clone();
    html! {
        <button
            class={classes!("cockpit-cycle-step", props.tone)}
            onclick={Callback::from(move |_| on_view.emit(target))}
        >
            <span class="cycle-icon">{ props.icon }</span>
            <span class="cycle-number">{ props.number }</span>
            <strong>{ props.title }</strong>
            <em>{ &props.value }</em>
            <small>{ props.detail }</small>
            <b>{ &props.status }</b>
        </button>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct CockpitCycleFocusProps {
    lang: UiLang,
    object_count: usize,
    link_count: usize,
    reflection_count: usize,
    ontology_status: String,
    target: View,
    on_view: Callback<View>,
}

#[component]
fn CockpitCycleFocus(props: &CockpitCycleFocusProps) -> Html {
    let target = props.target;
    let on_view = props.on_view.clone();
    let total = props.object_count + props.link_count + props.reflection_count;
    let max = total.max(1);
    html! {
        <button
            class={classes!("cockpit-cycle-focus", status_tone(&props.ontology_status))}
            onclick={Callback::from(move |_| on_view.emit(target))}
        >
            <header>
                <span class="cycle-icon">{ "◌" }</span>
                <span class="cycle-number">{ "03" }</span>
                <strong>{ props.lang.text("Ontology", "本体与工具") }</strong>
                <b>{ props.lang.text("RUNNING", "运行中") }</b>
            </header>
            <p>{ props.lang.text(
                "Builds reviewed object, link, metric, and action proposals from enterprise data.",
                "从企业数据生成可审核的对象、关系、指标和动作提案。"
            ) }</p>
            <FlowMeter label={props.lang.text("Objects", "对象")} value={props.object_count} max={max} tone="good" />
            <FlowMeter label={props.lang.text("Links", "关系")} value={props.link_count} max={max} tone="info" />
            <FlowMeter label={props.lang.text("Review queue", "审核队列")} value={props.reflection_count} max={max} tone="warn" />
            <div class="cockpit-mini-bars" aria-hidden="true">
                <span style={format!("height: {}%;", 30 + props.object_count.min(10) * 5)}></span>
                <span style={format!("height: {}%;", 30 + props.link_count.min(10) * 5)}></span>
                <span style={format!("height: {}%;", 30 + props.reflection_count.min(10) * 5)}></span>
                <span style="height: 42%;"></span>
                <span style="height: 34%;"></span>
            </div>
        </button>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct CockpitDiagnosisItemProps {
    icon: &'static str,
    label: &'static str,
    title: String,
    detail: String,
    #[prop_or("neutral")]
    tone: &'static str,
}

#[component]
fn CockpitDiagnosisItem(props: &CockpitDiagnosisItemProps) -> Html {
    html! {
        <article class={classes!("cockpit-diagnosis-item", props.tone)}>
            <span>{ props.icon }</span>
            <div>
                <small>{ props.label }</small>
                <strong>{ &props.title }</strong>
                <p>{ &props.detail }</p>
            </div>
        </article>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct CockpitMetricPanelProps {
    title: &'static str,
    subtitle: &'static str,
    rows: Vec<(String, usize, usize, &'static str)>,
}

#[component]
fn CockpitMetricPanel(props: &CockpitMetricPanelProps) -> Html {
    html! {
        <article class="cockpit-metric-panel">
            <header>
                <strong>{ props.title }</strong>
                <span>{ props.subtitle }</span>
            </header>
            { for props.rows.iter().map(|(label, value, max, tone)| html! {
                <div class="cockpit-metric-row" key={label.clone()}>
                    <span>{ label }</span>
                    <i><b class={classes!(*tone)} style={format!("width: {:.0}%;", ((*value as f64 / (*max).max(1) as f64) * 100.0).clamp(4.0, 100.0))}></b></i>
                    <strong>{ value }</strong>
                </div>
            }) }
        </article>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OperatorTaskCardProps {
    number: &'static str,
    title: &'static str,
    subtitle: &'static str,
    description: &'static str,
    status: String,
    #[prop_or("neutral")]
    tone: &'static str,
    target: View,
    on_view: Callback<View>,
}

#[component]
fn OperatorTaskCard(props: &OperatorTaskCardProps) -> Html {
    let target = props.target;
    let on_view = props.on_view.clone();
    html! {
        <button
            class={classes!("operator-task-card", props.tone)}
            onclick={Callback::from(move |_| on_view.emit(target))}
        >
            <span>{ props.number }</span>
            <strong>{ props.title }</strong>
            <b>{ props.subtitle }</b>
            <small>{ props.description }</small>
            <em>{ &props.status }</em>
        </button>
    }
}
