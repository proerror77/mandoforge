use crate::components::{
    EnterpriseReadinessPanel, KeyMetrics, OverviewButton, OverviewSignal, PackMosaic, Panel, Rows,
    RuntimePipeline,
};
use crate::state::{ConsoleData, View};
use crate::{
    active_job_count, blocked_pack_count, failed_job_count, first_lane_blocker, is_active_status,
    json_status, label_or, operator_queue_rows, pack_overview_rows, pending_approval_count,
    ready_pack_count, status_tone, worker_issue_rows,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct OverviewProps {
    pub(crate) data: ConsoleData,
    pub(crate) on_view: Callback<View>,
}

#[component]
pub(crate) fn OverviewView(props: &OverviewProps) -> Html {
    let data = &props.data;
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
        .unwrap_or_else(|| "No enterprise readiness next action reported.".to_string());
    let readiness_tone = if enterprise.completion_blocked || enterprise.blocked_lane_count > 0 {
        "bad"
    } else {
        status_tone(&enterprise.status)
    };

    html! {
        <div class="overview-layout">
            <section class="overview-hero">
                <div class="overview-hero-copy">
                    <p class="eyebrow">{ "Managed Agent OS / 托管智能体操作台" }</p>
                    <h2>{ "先看系统是否健康，再进入智能体、运行、本体、能力包或系统运维。" }</h2>
                    <p>{ "MandoForge 的主对象是托管智能体。Runs & Tasks 负责执行记录和任务编排；Ontology 负责语义和工具；Capabilities 负责行业能力；System Ops 负责平台自身上线与安全。" }</p>
                </div>
                <div class="overview-hero-actions">
                    <OverviewButton label="查看托管智能体" target={View::Agents} on_view={props.on_view.clone()} />
                    <OverviewButton label="查看运行与任务" target={View::Workflows} on_view={props.on_view.clone()} />
                    <OverviewButton label="查看系统运维" target={View::Deploy} on_view={props.on_view.clone()} />
                </div>
            </section>

            <section class="operator-task-map" aria-label="Operator task navigation">
                <OperatorTaskCard
                    number="01"
                    title="托管智能体"
                    subtitle="Managed Agents"
                    description="查看每个智能体的职责、状态、session、工具调用和需要人工确认的地方。"
                    status={format!("{active_sessions} 运行 / {pending_approvals} 审批")}
                    tone={if pending_approvals > 0 { "warn" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="02"
                    title="运行与任务"
                    subtitle="Runs & Tasks"
                    description="查看运行记录、任务板、流程模板、动态计划和审批队列。"
                    status={format!("{} 运行 / {} 动态计划", data.workflow_runs.data.len(), data.dynamic_workflow_plans.data.len())}
                    tone={if active_runs > 0 { "info" } else { "neutral" }}
                    target={View::Workflows}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="03"
                    title="本体与工具"
                    subtitle="Ontology"
                    description="把数据表、字段、关系、指标和动作变成智能体可调用的治理工具。"
                    status={format!("{} 对象 / {} 关系", data.semantic_graph.data.node_count, data.semantic_graph.data.edge_count)}
                    tone={status_tone(&ontology_status)}
                    target={View::Semantic}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="04"
                    title="能力包"
                    subtitle="Capabilities"
                    description="管理行业包、连接器、模板和每个能力对托管智能体开放的边界。"
                    status={format!("{ready_packs} 就绪 / {blocked_packs} 阻塞")}
                    tone={if blocked_packs > 0 { "warn" } else { "good" }}
                    target={View::Packs}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="05"
                    title="系统运维"
                    subtitle="System Ops"
                    description="检查部署、桌面端、安全边界、审计、成本、告警和企业级上线证据。"
                    status={label_or(&enterprise.status, "unknown").to_string()}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
                <OperatorTaskCard
                    number="06"
                    title="当前阻塞"
                    subtitle="Attention"
                    description="查看严重通知、失败任务、连接器阻塞和需要人工处理的下一步。"
                    status={format!("{} 个阻塞面", enterprise.blocked_lane_count)}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
            </section>

            <section class="overview-signals">
                <OverviewSignal
                    label="运行中的任务"
                    value={active_sessions.to_string()}
                    detail={format!("共 {} 个受管 session", data.sessions.data.len())}
                    tone={if active_sessions > 0 { "info" } else { "neutral" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="待审批动作"
                    value={pending_approvals.to_string()}
                    detail={"草稿和高风险动作必须先经过人工确认。".to_string()}
                    tone={if pending_approvals > 0 { "warn" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="执行队列压力"
                    value={active_workers.to_string()}
                    detail={format!("{failed_jobs} 个失败或报错任务")}
                    tone={if failed_jobs > 0 { "bad" } else if active_workers > 0 { "info" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="工作流运行"
                    value={active_runs.to_string()}
                    detail={format!("共 {} 次工作流运行", data.workflow_runs.data.len())}
                    tone={if active_runs > 0 { "info" } else { "neutral" }}
                    target={View::Workflows}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="已发布应用包"
                    value={ready_packs.to_string()}
                    detail={format!("{blocked_packs} 个应用包安装被阻塞")}
                    tone={if blocked_packs > 0 { "warn" } else if ready_packs > 0 { "good" } else { "neutral" }}
                    target={View::Packs}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="企业上线面"
                    value={format!("{}/{}", enterprise.ready_lane_count, enterprise.lane_count.max(enterprise.lanes.len()))}
                    detail={format!("{} 个阻塞 / 证据等级 {}", enterprise.blocked_lane_count, label_or(&enterprise.required_evidence_class, "customer_grade"))}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
            </section>

            <div class="overview-grid">
                <Panel title="运行压力">
                    <RuntimePipeline
                        sessions={data.sessions.data.clone()}
                        execution_jobs={data.execution_jobs.data.clone()}
                        session_loop_jobs={data.session_loop_jobs.data.clone()}
                        approvals={data.approvals.data.clone()}
                        tool_calls={data.tool_calls.data.clone()}
                    />
                    <Rows empty="没有失败的执行任务。" rows={worker_issue_rows(&data.execution_jobs.data, &data.session_loop_jobs.data)} />
                </Panel>
                <Panel title="企业上线状态">
                    <EnterpriseReadinessPanel readiness={enterprise.clone()} />
                </Panel>
                <Panel title="应用包能力状态">
                    <PackMosaic
                        installations={data.workflow_pack_installations.data.clone()}
                        marketplace={data.workflow_pack_marketplace.data.clone()}
                    />
                    <Rows empty="还没有安装应用包。" rows={pack_overview_rows(&data.workflow_pack_installations.data, &data.workflow_pack_marketplace.data)} />
                </Panel>
                <Panel title="连接器与本体闸门">
                    <KeyMetrics values={vec![
                        ("原生连接器".to_string(), connector_status.clone()),
                        ("本体引擎".to_string(), ontology_status.clone()),
                        ("语义对象".to_string(), data.semantic_objects.data.len().to_string()),
                        ("关系边".to_string(), data.semantic_graph.data.edge_count.to_string()),
                        ("反思队列".to_string(), data.semantic_reflection_queue.data.queue.len().to_string()),
                    ]} />
                    <div class="overview-gate-actions">
                        <OverviewButton label="查看系统运维" target={View::Deploy} on_view={props.on_view.clone()} />
                        <OverviewButton label="查看本体工具" target={View::Semantic} on_view={props.on_view.clone()} />
                    </div>
                </Panel>
                <Panel title="当前需要处理">
                    <Rows empty="没有立即阻塞项。" rows={
                        if operator_queue_rows(data).is_empty() {
                            vec![(
                                readiness_tone.to_string(),
                                "下一步".to_string(),
                                primary_next_action.clone(),
                            )]
                        } else {
                            operator_queue_rows(data)
                        }
                    } />
                </Panel>
                <Panel title="证据入口">
                    <KeyMetrics values={vec![
                        ("企业产品状态".to_string(), "/api/enterprise-product/readiness".to_string()),
                        ("连接器生产状态".to_string(), "/api/native-connectors/production-readiness".to_string()),
                        ("本体就绪状态".to_string(), "/api/ontology/engine-readiness".to_string()),
                        ("应用包生命周期".to_string(), "/api/workflow-packs/installations".to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
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
