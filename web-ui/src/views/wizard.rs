use crate::api::{Session, api_post, create_session_body};
use crate::components::{KeyMetrics, OverviewButton, Panel};
use crate::state::{ConsoleData, View, storage_get, storage_set};
use crate::{
    first_lane_blocker, json_gate_tone, json_status, label_or, pack_card_models,
    pack_has_external_writes, pack_metric_count, pack_requires_approval, session_title, short_id,
};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct WizardProps {
    pub(crate) data: ConsoleData,
    pub(crate) on_status: Callback<String>,
    pub(crate) on_view: Callback<View>,
}

#[derive(Properties, Clone, PartialEq)]
struct WizardStepProps {
    number: AttrValue,
    title: AttrValue,
    status: AttrValue,
    detail: AttrValue,
    tone: AttrValue,
}

#[component]
fn WizardStep(props: &WizardStepProps) -> Html {
    html! {
        <article class={classes!("wizard-step", props.tone.clone())}>
            <div class="wizard-step-number">{ props.number.clone() }</div>
            <div>
                <span>{ props.title.clone() }</span>
                <strong>{ props.status.clone() }</strong>
                <p>{ props.detail.clone() }</p>
            </div>
        </article>
    }
}

#[component]
pub(crate) fn WizardView(props: &WizardProps) -> Html {
    let data = &props.data;
    let cards = pack_card_models(
        &data.workflow_pack_installations.data,
        &data.workflow_pack_marketplace.data,
    );
    let access_mode = use_state(|| {
        storage_get("mandoforge.firstRun.accessMode")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "local_dev".to_string())
    });
    let selected_pack = {
        let cards = cards.clone();
        use_state(move || {
            storage_get("mandoforge.firstRun.packId")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    cards
                        .iter()
                        .find(|card| card.id.starts_with("ecommerce-"))
                        .or_else(|| cards.first())
                        .map(|card| card.id.clone())
                        .unwrap_or_default()
                })
        })
    };
    let token_saved = storage_get("mandoforge.adminToken")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_runtime = data.agents.data.iter().any(|agent| agent.is_runnable())
        && data
            .environments
            .data
            .iter()
            .any(|environment| environment.is_runnable());
    let fallback_pack_id = cards
        .iter()
        .find(|card| card.id.starts_with("ecommerce-"))
        .or_else(|| cards.first())
        .map(|card| card.id.clone())
        .unwrap_or_default();
    let effective_selected_pack = if cards
        .iter()
        .any(|card| card.id == (*selected_pack).as_str())
    {
        (*selected_pack).clone()
    } else {
        fallback_pack_id
    };
    let selected_card = cards
        .iter()
        .find(|card| card.id == effective_selected_pack.as_str());
    let selected_pack_name = selected_card
        .map(|card| card.name.clone())
        .unwrap_or_else(|| "No pack selected".to_string());
    let selected_pack_summary = selected_card
        .map(|card| {
            format!(
                "{} / {} / {} workflows / {} connectors",
                card.kind,
                card.version,
                pack_metric_count(&card.manifest, "workflows", "workflow_count"),
                pack_metric_count(&card.manifest, "connectors", "connector_count")
            )
        })
        .unwrap_or_else(|| "Install or select a WorkflowPack before starting a pilot.".to_string());
    let connector_status = json_status(&data.native_connector_production_readiness.data);
    let connector_tone = json_gate_tone(&data.native_connector_production_readiness.data);
    let ontology_status = json_status(&data.ontology_engine_readiness.data);
    let ontology_tone = json_gate_tone(&data.ontology_engine_readiness.data);
    let enterprise = &data.enterprise_product_readiness.data;
    let evidence_summary = format!(
        "{} ready / {} pilot / {} blocked lanes, evidence target {}",
        enterprise.ready_lane_count,
        enterprise.pilot_ready_lane_count,
        enterprise.blocked_lane_count,
        label_or(&enterprise.required_evidence_class, "customer_grade")
    );
    let first_enterprise_action = enterprise
        .next_actions
        .first()
        .cloned()
        .or_else(|| first_lane_blocker(enterprise))
        .unwrap_or_else(|| "No enterprise readiness next action reported.".to_string());
    let can_start_session = has_runtime && data.direct_session_launch_allowed();

    let choose_mode = |mode: &'static str, access_mode: UseStateHandle<String>| {
        Callback::from(move |_| {
            storage_set("mandoforge.firstRun.accessMode", mode);
            access_mode.set(mode.to_string());
        })
    };

    let on_pack = {
        let selected_pack = selected_pack.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            let value = select.value();
            storage_set("mandoforge.firstRun.packId", &value);
            selected_pack.set(value);
        })
    };

    let start_session = {
        let data = data.clone();
        let on_status = props.on_status.clone();
        Callback::from(move |_| {
            let Some(agent) = data
                .agents
                .data
                .iter()
                .find(|agent| agent.is_runnable())
                .cloned()
            else {
                on_status
                    .emit("Wizard cannot start a pilot session: no agent is registered.".to_string());
                return;
            };
            let environment_id = data
                .environments
                .data
                .iter()
                .find(|environment| environment.is_runnable())
                .map(|environment| environment.id.as_str());
            let body = create_session_body(
                &agent.id,
                environment_id,
                "First-run governed pilot session",
                "Run a first-run pilot smoke task inside MandoForge policy, approval, audit, and evidence boundaries. Do not perform external writes.",
            );
            let on_status = on_status.clone();
            spawn_local(async move {
                on_status.emit("Wizard is starting a governed pilot session...".to_string());
                match api_post::<Session, _>("/api/sessions", &body).await {
                    Ok(session) => on_status.emit(format!(
                        "Wizard started session {}: {}",
                        short_id(&session.id),
                        session_title(&session)
                    )),
                    Err(error) => on_status.emit(format!("Wizard start session failed: {error}")),
                }
            });
        })
    };

    html! {
        <div class="wizard-layout">
            <section class="wizard-hero">
                <div>
                    <p class="eyebrow">{ "First-run enterprise onboarding" }</p>
                    <h2>{ "Turn a fresh console into a governed pilot without implying production completion." }</h2>
                    <p>{ "The wizard stores local progress, points every setup task at API-backed readiness surfaces, and keeps external business actions draft/approval-only." }</p>
                </div>
                <div class="wizard-mode-switch" aria-label="Access mode">
                    <button class={classes!("secondary", ((*access_mode).as_str() == "local_dev").then_some("active"))} onclick={choose_mode("local_dev", access_mode.clone())}>{ "Local" }</button>
                    <button class={classes!("secondary", ((*access_mode).as_str() == "repo_pilot").then_some("active"))} onclick={choose_mode("repo_pilot", access_mode.clone())}>{ "Pilot" }</button>
                    <button class={classes!("secondary", ((*access_mode).as_str() == "customer_grade").then_some("active"))} onclick={choose_mode("customer_grade", access_mode.clone())}>{ "Customer-grade" }</button>
                </div>
            </section>

            <section class="wizard-steps">
                <WizardStep
                    number="1"
                    title="Access mode"
                    status={access_mode_label((*access_mode).as_str())}
                    detail={access_mode_detail((*access_mode).as_str())}
                    tone="info"
                />
                <WizardStep
                    number="2"
                    title="Identity posture"
                    status={if token_saved { "token saved" } else { "token missing" }}
                    detail={if token_saved { "Bearer auth and x-mandoforge identity headers will be sent by the console." } else { "Save an admin token in the top auth strip before using protected live gates." }}
                    tone={if token_saved { "good" } else { "warn" }}
                />
                <WizardStep
                    number="3"
                    title="Runtime profile"
                    status={if has_runtime { "agent and environment ready" } else { "runtime inventory missing" }}
                    detail={format!("{} agents / {} environments visible through the API", data.agents.data.len(), data.environments.data.len())}
                    tone={if has_runtime { "good" } else { "warn" }}
                />
                <WizardStep
                    number="4"
                    title="Choose first pack"
                    status={selected_pack_name}
                    detail={selected_pack_summary}
                    tone={if selected_card.is_some() { "good" } else { "warn" }}
                />
                <WizardStep
                    number="5"
                    title="Connector readiness"
                    status={connector_status}
                    detail={"Live connector writes remain blocked unless production evidence and approval binding pass."}
                    tone={connector_tone}
                />
                <WizardStep
                    number="6"
                    title="Ontology readiness"
                    status={ontology_status}
                    detail={format!("{} ontology object types / {} semantic objects", data.ontology_registry.data.object_types.len(), data.semantic_objects.data.len())}
                    tone={ontology_tone}
                />
                <WizardStep
                    number="7"
                    title="Governed pilot"
                    status={if can_start_session { "ready to start" } else if has_runtime { "workflow required" } else { "blocked by runtime inventory" }}
                    detail={"Creates a normal managed session through /api/sessions; it does not bypass policy, approval, audit, or connector gates."}
                    tone={if can_start_session { "info" } else { "warn" }}
                />
                <WizardStep
                    number="8"
                    title="Evidence checklist"
                    status={evidence_summary}
                    detail={first_enterprise_action}
                    tone={if enterprise.blocked_lane_count > 0 || enterprise.completion_blocked { "bad" } else { "good" }}
                />
            </section>

            <section class="wizard-actions">
                <Panel title="Pack selection">
                    <div class="form-stack">
                        <label>
                            <span>{ "WorkflowPack" }</span>
                            <select value={effective_selected_pack.clone()} onchange={on_pack}>
                                {
                                    if cards.is_empty() {
                                        html! { <option value="">{ "No marketplace packs reported" }</option> }
                                    } else {
                                        html! {
                                            { for cards.iter().map(|card| html! {
                                                <option
                                                    value={card.id.clone()}
                                                    selected={card.id == effective_selected_pack.as_str()}
                                                >
                                                    { format!("{} - {} ({})", card.name, card.version, card.status) }
                                                </option>
                                            }) }
                                        }
                                    }
                                }
                            </select>
                        </label>
                        <KeyMetrics values={vec![
                            ("Selected pack".to_string(), label_or(&effective_selected_pack, "none").to_string()),
                            ("Access mode".to_string(), access_mode_label((*access_mode).as_str()).to_string()),
                            ("External writes".to_string(), selected_card.map(|card| if pack_has_external_writes(&card.manifest) { "approval-gated" } else { "not declared" }).unwrap_or("none").to_string()),
                            ("Approval posture".to_string(), selected_card.map(|card| if pack_requires_approval(&card.manifest) { "required" } else { "not declared" }).unwrap_or("unknown").to_string()),
                        ]} />
                    </div>
                </Panel>
                <Panel title="Next governed action">
                    <div class="settings-stack">
                        <div class="settings-row">
                            <div>
                                <span>{ "Start pilot session" }</span>
                                <strong>{ if can_start_session { "available" } else if has_runtime { "workflow required" } else { "needs agent and environment" } }</strong>
                                <p>{ "The first-run session is an internal smoke task. It creates no external writes and remains visible in session, worker, approval, and audit surfaces." }</p>
                            </div>
                            <button disabled={!can_start_session} onclick={start_session}>{ "Start session" }</button>
                        </div>
                        <div class="overview-gate-actions">
                            <OverviewButton label="Open packs" target={View::Packs} on_view={props.on_view.clone()} />
                            <OverviewButton label="Connector gates" target={View::Deploy} on_view={props.on_view.clone()} />
                            <OverviewButton label="Ontology gates" target={View::Semantic} on_view={props.on_view.clone()} />
                            <OverviewButton label="Task console" target={View::Agents} on_view={props.on_view.clone()} />
                        </div>
                    </div>
                </Panel>
            </section>
        </div>
    }
}

fn access_mode_label(mode: &str) -> &'static str {
    match mode {
        "repo_pilot" => "repo-controlled pilot",
        "customer_grade" => "customer-grade target",
        _ => "local development",
    }
}

fn access_mode_detail(mode: &str) -> &'static str {
    match mode {
        "repo_pilot" => {
            "Use repo-controlled evidence, dry-run connectors, and approval-gated pilot actions."
        }
        "customer_grade" => {
            "Keep completion blocked until customer-grade credentials, reconciliation, support, and archived deployment evidence exist."
        }
        _ => "Use local memory/dev auth and no external business writes while learning the console.",
    }
}
