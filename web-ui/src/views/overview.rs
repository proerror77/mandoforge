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
                    <p class="eyebrow">{ "Enterprise Agent OS Control Plane" }</p>
                    <h2>{ "Runtime, packs, connectors, ontology, and evidence on one operator surface." }</h2>
                    <p>{ primary_next_action }</p>
                </div>
                <div class="overview-hero-actions">
                    <OverviewButton label="Start wizard" target={View::Wizard} on_view={props.on_view.clone()} />
                    <OverviewButton label="Review blockers" target={View::Deploy} on_view={props.on_view.clone()} />
                    <OverviewButton label="Open packs" target={View::Packs} on_view={props.on_view.clone()} />
                    <OverviewButton label="Check ontology" target={View::Semantic} on_view={props.on_view.clone()} />
                </div>
            </section>

            <section class="overview-signals">
                <OverviewSignal
                    label="Active sessions"
                    value={active_sessions.to_string()}
                    detail={format!("{} total managed sessions", data.sessions.data.len())}
                    tone={if active_sessions > 0 { "info" } else { "neutral" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="Pending approvals"
                    value={pending_approvals.to_string()}
                    detail={"Draft and high-risk actions stay approval-gated.".to_string()}
                    tone={if pending_approvals > 0 { "warn" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="Worker pressure"
                    value={active_workers.to_string()}
                    detail={format!("{failed_jobs} failed or errored jobs")}
                    tone={if failed_jobs > 0 { "bad" } else if active_workers > 0 { "info" } else { "good" }}
                    target={View::Agents}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="Workflow runs"
                    value={active_runs.to_string()}
                    detail={format!("{} total workflow runs", data.workflow_runs.data.len())}
                    tone={if active_runs > 0 { "info" } else { "neutral" }}
                    target={View::Workflows}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="Released packs"
                    value={ready_packs.to_string()}
                    detail={format!("{blocked_packs} blocked pack installations")}
                    tone={if blocked_packs > 0 { "warn" } else if ready_packs > 0 { "good" } else { "neutral" }}
                    target={View::Packs}
                    on_view={props.on_view.clone()}
                />
                <OverviewSignal
                    label="Enterprise lanes"
                    value={format!("{}/{}", enterprise.ready_lane_count, enterprise.lane_count.max(enterprise.lanes.len()))}
                    detail={format!("{} blocked / evidence class {}", enterprise.blocked_lane_count, label_or(&enterprise.required_evidence_class, "customer_grade"))}
                    tone={readiness_tone}
                    target={View::Deploy}
                    on_view={props.on_view.clone()}
                />
            </section>

            <div class="overview-grid">
                <Panel title="Runtime pressure">
                    <RuntimePipeline
                        sessions={data.sessions.data.clone()}
                        execution_jobs={data.execution_jobs.data.clone()}
                        session_loop_jobs={data.session_loop_jobs.data.clone()}
                        approvals={data.approvals.data.clone()}
                        tool_calls={data.tool_calls.data.clone()}
                    />
                    <Rows empty="No failed worker jobs." rows={worker_issue_rows(&data.execution_jobs.data, &data.session_loop_jobs.data)} />
                </Panel>
                <Panel title="Enterprise readiness">
                    <EnterpriseReadinessPanel readiness={enterprise.clone()} />
                </Panel>
                <Panel title="Pack capability state">
                    <PackMosaic
                        installations={data.workflow_pack_installations.data.clone()}
                        marketplace={data.workflow_pack_marketplace.data.clone()}
                    />
                    <Rows empty="No installed packs." rows={pack_overview_rows(&data.workflow_pack_installations.data, &data.workflow_pack_marketplace.data)} />
                </Panel>
                <Panel title="Connector and ontology gates">
                    <KeyMetrics values={vec![
                        ("Native connectors".to_string(), connector_status.clone()),
                        ("Ontology engine".to_string(), ontology_status.clone()),
                        ("Semantic objects".to_string(), data.semantic_objects.data.len().to_string()),
                        ("Graph edges".to_string(), data.semantic_graph.data.edge_count.to_string()),
                        ("Reflection queue".to_string(), data.semantic_reflection_queue.data.queue.len().to_string()),
                    ]} />
                    <div class="overview-gate-actions">
                        <OverviewButton label="Connector details" target={View::Deploy} on_view={props.on_view.clone()} />
                        <OverviewButton label="Ontology details" target={View::Semantic} on_view={props.on_view.clone()} />
                    </div>
                </Panel>
                <Panel title="Immediate operator queue">
                    <Rows empty="No immediate blockers reported." rows={operator_queue_rows(data)} />
                </Panel>
                <Panel title="Evidence surfaces">
                    <KeyMetrics values={vec![
                        ("Enterprise contract".to_string(), "/api/enterprise-product/readiness".to_string()),
                        ("Connector production".to_string(), "/api/native-connectors/production-readiness".to_string()),
                        ("Ontology readiness".to_string(), "/api/ontology/engine-readiness".to_string()),
                        ("Pack lifecycle".to_string(), "/api/workflow-packs/installations".to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
    }
}
