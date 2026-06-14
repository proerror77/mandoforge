mod api;
mod components;
mod desktop_bridge;
mod notifications;
mod state;
mod views;

use api::*;
use components::*;
use gloo_timers::callback::Interval;
use notifications::*;
use serde_json::{Value, json};
use state::*;
use views::{
    AgentsView, BoardView, DeployView, DynamicView, OverviewView, PacksView, SemanticView,
    SettingsView, WizardView, WorkflowsView,
};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[component]
fn App() -> Html {
    let active_view = use_state(initial_active_view);
    let token_input = use_state(get_admin_token);
    let mutation_status = use_state(String::new);
    let task_title = use_state(|| "Smoke run: fetch a public webpage title".to_string());
    let task_message = use_state(|| {
        "Open https://example.com, read the page title, and return a short final answer with the title and source URL.".to_string()
    });
    let task_agent_id = use_state(String::new);
    let task_environment_id = use_state(String::new);
    let dynamic_objective = use_state(|| {
        "Run a multi-agent codebase audit and produce a cross-checked report.".to_string()
    });
    let semantic_source = use_state(|| {
        "Contract review uses Contract, Party, Clause, Obligation, Risk, Jurisdiction, Template, and Approval Requirement concepts.".to_string()
    });
    let context_packet_id = use_state(String::new);
    let rendered_context = use_state(|| None::<RenderedExecutionContext>);
    let onboarding_run = use_state(|| None::<OntologyOnboardingRun>);
    let onboarding_tool_specs = use_state(Vec::<OntologyOnboardingToolSpec>::new);
    let onboarding_review_graph = use_state(|| None::<OntologyReviewGraph>);
    let onboarding_calibration = use_state(|| None::<ConfidenceCalibrationResponse>);
    let critical_notifications_muted = use_state(initial_critical_notifications_muted);

    let data = ConsoleData {
        agents: use_polling::<Vec<Agent>>("/api/agents", 5_000),
        environments: use_polling::<Vec<Environment>>("/api/environments", 6_000),
        sessions: use_polling::<Vec<Session>>("/api/sessions", 1_800),
        approvals: use_polling::<Vec<Approval>>("/api/approvals", 1_800),
        execution_jobs: use_polling::<Vec<WorkerJob>>("/api/execution-jobs", 1_500),
        session_loop_jobs: use_polling::<Vec<WorkerJob>>("/api/session-loop-jobs", 1_500),
        tool_calls: use_polling::<Vec<ToolCall>>("/api/tool-calls", 1_800),
        workflow_runs: use_polling::<Vec<WorkflowRun>>("/api/workflow-runs", 1_600),
        workflow_definitions: use_polling::<Vec<WorkflowDefinition>>(
            "/api/workflow-definitions",
            3_000,
        ),
        dynamic_workflow_plans: use_polling::<Vec<DynamicWorkflowPlan>>(
            "/api/dynamic-workflow-plans",
            2_500,
        ),
        task_board: use_polling::<TaskBoardSnapshot>("/api/task-board", 1_500),
        work_items: use_polling::<Vec<WorkItem>>("/api/work-items", 3_000),
        manager_plans: use_polling::<Vec<Value>>("/api/manager-plans", 3_000),
        agent_handoffs: use_polling::<Vec<Value>>("/api/agent-handoffs", 3_000),
        agent_handoff_assignments: use_polling::<Vec<Value>>(
            "/api/agent-handoff-assignments",
            3_000,
        ),
        workflow_pack_installations: use_polling::<Vec<WorkflowPackInstallation>>(
            "/api/workflow-packs/installations",
            4_000,
        ),
        stage2_readiness: use_polling::<Stage2Readiness>("/api/stage2/readiness", 4_000),
        enterprise_product_readiness: use_polling::<EnterpriseProductReadiness>(
            "/api/enterprise-product/readiness",
            4_000,
        ),
        native_connector_production_readiness: use_polling::<Value>(
            "/api/native-connectors/production-readiness",
            5_000,
        ),
        observability: use_polling::<ObservabilitySummary>("/api/observability", 3_000),
        capability_discovery: use_polling::<CapabilityDiscovery>(
            "/api/capability-discovery",
            4_000,
        ),
        usage: use_polling::<Value>("/api/usage", 5_000),
        memory_governance: use_polling::<Value>("/api/memory-governance/summary", 5_000),
        memory_writebacks: use_polling::<Value>(
            "/api/memory-governance/writebacks?limit=50&status=pending",
            5_000,
        ),
        memory_writeback_candidates: use_polling::<Value>(
            "/api/memory-writeback-candidates",
            5_000,
        ),
        scheduler_summary: use_polling::<Value>("/api/scheduler/summary", 4_000),
        deployment_version: use_polling::<DeploymentVersion>("/api/deployment/version", 4_000),
        remote_computer_production_path: use_polling::<Value>(
            "/api/remote-computers/production-path",
            5_000,
        ),
        workflow_pack_marketplace: use_polling::<WorkflowPackMarketplace>(
            "/api/workflow-packs/marketplace",
            6_000,
        ),
        semantic_objects: use_polling::<Vec<SemanticObject>>("/api/semantic-objects", 5_000),
        semantic_links: use_polling::<Vec<Value>>("/api/semantic-links", 5_000),
        semantic_search: use_polling::<Value>("/api/semantic-search", 5_000),
        semantic_graph: use_polling::<SemanticGraphSnapshot>("/api/semantic-graph", 5_000),
        semantic_workbench: use_polling::<Value>(
            "/api/semantic-workbench?domain_scope=ecommerce&workflow_scope=tmall",
            5_000,
        ),
        semantic_reflection_queue: use_polling::<SemanticReflectionQueue>(
            "/api/semantic-reflection/queue",
            5_000,
        ),
        ontology_registry: use_polling::<OntologyRegistry>("/api/ontology/registry", 6_000),
        ontology_engine_readiness: use_polling::<Value>("/api/ontology/engine-readiness", 6_000),
        semantic_retrieval_backends: use_polling::<Value>(
            "/api/semantic-retrieval/backends",
            6_000,
        ),
    };

    let fetching_count = count_loading(&data);
    let error_count = count_errors(&data);
    let running_agents = data
        .sessions
        .data
        .iter()
        .filter(|session| is_active_status(&session.status))
        .count();
    let pending_approvals = data
        .approvals
        .data
        .iter()
        .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
        .count();

    let save_token = {
        let token_input = token_input.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            set_admin_token(&token_input);
            mutation_status.set("Admin token saved in localStorage.".to_string());
        })
    };

    let clear_token = {
        let token_input = token_input.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            token_input.set(String::new());
            set_admin_token("");
            mutation_status.set("Admin token cleared.".to_string());
        })
    };

    let compile_dynamic = {
        let dynamic_objective = dynamic_objective.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let objective = (*dynamic_objective).clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Compiling dynamic workflow plan...".to_string());
                let body = compile_dynamic_body(&objective, 4, 2);
                match api_post::<Value, _>("/api/dynamic-workflow-plans/compile", &body).await {
                    Ok(payload) => mutation_status
                        .set(format!("Dynamic compile ready: {}", compact_json(&payload))),
                    Err(error) => mutation_status.set(format!("Dynamic compile failed: {error}")),
                }
            });
        })
    };

    let build_ontology = {
        let semantic_source = semantic_source.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let source_text = (*semantic_source).clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Building ontology proposal preview...".to_string());
                let body = ontology_builder_body(&source_text);
                match api_post::<Value, _>("/api/semantic-ontology/builder", &body).await {
                    Ok(payload) => mutation_status.set(format!(
                        "Ontology preview ready: {}",
                        compact_json(&payload)
                    )),
                    Err(error) => mutation_status.set(format!("Ontology builder failed: {error}")),
                }
            });
        })
    };

    let start_ontology_onboarding = {
        let onboarding_run = onboarding_run.clone();
        let onboarding_tool_specs = onboarding_tool_specs.clone();
        let onboarding_review_graph = onboarding_review_graph.clone();
        let onboarding_calibration = onboarding_calibration.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let onboarding_run = onboarding_run.clone();
            let onboarding_tool_specs = onboarding_tool_specs.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Starting enterprise ontology onboarding demo...".to_string());
                match api_post::<OntologyOnboardingRun, _>(
                    "/api/ontology/onboarding/demo-runs",
                    &json!({}),
                )
                .await
                {
                    Ok(run) => {
                        let graph_path =
                            format!("/api/ontology/onboarding/runs/{}/review-graph", run.id);
                        match api_get::<OntologyReviewGraph>(&graph_path).await {
                            Ok(graph) => onboarding_review_graph.set(Some(graph)),
                            Err(_) => onboarding_review_graph.set(None),
                        }
                        let calibration_path =
                            format!("/api/ontology/intelligence/runs/{}/calibration", run.id);
                        match api_get::<ConfidenceCalibrationResponse>(&calibration_path).await {
                            Ok(calibration) => onboarding_calibration.set(Some(calibration)),
                            Err(_) => onboarding_calibration.set(None),
                        }
                        mutation_status.set(format!(
                            "Onboarding run ready: {} proposals from {} datasets.",
                            run.proposal_count, run.dataset_count
                        ));
                        onboarding_tool_specs.set(Vec::new());
                        onboarding_run.set(Some(run));
                    }
                    Err(error) => {
                        mutation_status.set(format!("Onboarding start failed: {error}"));
                    }
                }
            });
        })
    };

    let approve_ontology_proposal = {
        let onboarding_run = onboarding_run.clone();
        let onboarding_review_graph = onboarding_review_graph.clone();
        let onboarding_calibration = onboarding_calibration.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |proposal_id: String| {
            let Some(current_run) = (*onboarding_run).clone() else {
                mutation_status.set("Approve failed: no onboarding run is active.".to_string());
                return;
            };
            let onboarding_run = onboarding_run.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Approving ontology proposal...".to_string());
                let path = format!("/api/ontology/onboarding/proposals/{proposal_id}/review");
                let body = json!({
                    "decision": "approve",
                    "reason": "operator approved from semantic console",
                });
                match api_post::<OntologyOnboardingProposal, _>(&path, &body).await {
                    Ok(_) => {
                        let run_path = format!("/api/ontology/onboarding/runs/{}", current_run.id);
                        match api_get::<OntologyOnboardingRun>(&run_path).await {
                            Ok(run) => {
                                let graph_path = format!(
                                    "/api/ontology/onboarding/runs/{}/review-graph",
                                    run.id
                                );
                                if let Ok(graph) = api_get::<OntologyReviewGraph>(&graph_path).await
                                {
                                    onboarding_review_graph.set(Some(graph));
                                }
                                let calibration_path = format!(
                                    "/api/ontology/intelligence/runs/{}/calibration",
                                    run.id
                                );
                                if let Ok(calibration) =
                                    api_get::<ConfidenceCalibrationResponse>(&calibration_path)
                                        .await
                                {
                                    onboarding_calibration.set(Some(calibration));
                                }
                                mutation_status.set(format!(
                                    "Proposal approved: {}/{} approved.",
                                    run.approved_count, run.proposal_count
                                ));
                                onboarding_run.set(Some(run));
                            }
                            Err(error) => mutation_status
                                .set(format!("Proposal approved; refresh failed: {error}")),
                        }
                    }
                    Err(error) => mutation_status.set(format!("Proposal approve failed: {error}")),
                }
            });
        })
    };

    let reject_ontology_proposal = {
        let onboarding_run = onboarding_run.clone();
        let onboarding_review_graph = onboarding_review_graph.clone();
        let onboarding_calibration = onboarding_calibration.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |proposal_id: String| {
            let Some(current_run) = (*onboarding_run).clone() else {
                mutation_status.set("Reject failed: no onboarding run is active.".to_string());
                return;
            };
            let onboarding_run = onboarding_run.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Rejecting ontology proposal...".to_string());
                let path = format!("/api/ontology/onboarding/proposals/{proposal_id}/review");
                let body = json!({
                    "decision": "reject",
                    "reason": "operator rejected from semantic console",
                });
                match api_post::<OntologyOnboardingProposal, _>(&path, &body).await {
                    Ok(_) => {
                        let run_path = format!("/api/ontology/onboarding/runs/{}", current_run.id);
                        match api_get::<OntologyOnboardingRun>(&run_path).await {
                            Ok(run) => {
                                let graph_path = format!(
                                    "/api/ontology/onboarding/runs/{}/review-graph",
                                    run.id
                                );
                                if let Ok(graph) = api_get::<OntologyReviewGraph>(&graph_path).await
                                {
                                    onboarding_review_graph.set(Some(graph));
                                }
                                let calibration_path = format!(
                                    "/api/ontology/intelligence/runs/{}/calibration",
                                    run.id
                                );
                                if let Ok(calibration) =
                                    api_get::<ConfidenceCalibrationResponse>(&calibration_path)
                                        .await
                                {
                                    onboarding_calibration.set(Some(calibration));
                                }
                                mutation_status.set("Proposal rejected.".to_string());
                                onboarding_run.set(Some(run));
                            }
                            Err(error) => mutation_status
                                .set(format!("Proposal rejected; refresh failed: {error}")),
                        }
                    }
                    Err(error) => mutation_status.set(format!("Proposal reject failed: {error}")),
                }
            });
        })
    };

    let materialize_ontology_onboarding = {
        let onboarding_run = onboarding_run.clone();
        let onboarding_tool_specs = onboarding_tool_specs.clone();
        let onboarding_review_graph = onboarding_review_graph.clone();
        let onboarding_calibration = onboarding_calibration.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let Some(current_run) = (*onboarding_run).clone() else {
                mutation_status.set("Materialize failed: no onboarding run is active.".to_string());
                return;
            };
            let onboarding_run = onboarding_run.clone();
            let onboarding_tool_specs = onboarding_tool_specs.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Materializing approved ontology proposals...".to_string());
                let path = format!(
                    "/api/ontology/onboarding/runs/{}/materialize",
                    current_run.id
                );
                match api_post::<Value, _>(&path, &json!({})).await {
                    Ok(_) => {
                        let run_path = format!("/api/ontology/onboarding/runs/{}", current_run.id);
                        if let Ok(run) = api_get::<OntologyOnboardingRun>(&run_path).await {
                            onboarding_run.set(Some(run));
                        }
                        let graph_path = format!(
                            "/api/ontology/onboarding/runs/{}/review-graph",
                            current_run.id
                        );
                        if let Ok(graph) = api_get::<OntologyReviewGraph>(&graph_path).await {
                            onboarding_review_graph.set(Some(graph));
                        }
                        let calibration_path = format!(
                            "/api/ontology/intelligence/runs/{}/calibration",
                            current_run.id
                        );
                        if let Ok(calibration) =
                            api_get::<ConfidenceCalibrationResponse>(&calibration_path).await
                        {
                            onboarding_calibration.set(Some(calibration));
                        }
                        let specs_path = format!(
                            "/api/ontology/onboarding/runs/{}/tool-specs",
                            current_run.id
                        );
                        match api_get::<OntologyOnboardingToolSpecResponse>(&specs_path).await {
                            Ok(response) => {
                                mutation_status.set(format!(
                                    "Ontology materialized: {} agent tools compiled.",
                                    response.tool_specs.len()
                                ));
                                onboarding_tool_specs.set(response.tool_specs);
                            }
                            Err(error) => mutation_status.set(format!(
                                "Ontology materialized; tool refresh failed: {error}"
                            )),
                        }
                    }
                    Err(error) => {
                        mutation_status.set(format!("Ontology materialize failed: {error}"))
                    }
                }
            });
        })
    };

    let render_context = {
        let sessions = data.sessions.data.clone();
        let context_packet_id = context_packet_id.clone();
        let rendered_context = rendered_context.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let requested_packet_id = (*context_packet_id).trim().to_string();
            let sessions = sessions.clone();
            let context_packet_id = context_packet_id.clone();
            let rendered_context = rendered_context.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Rendering execution context...".to_string());
                let packet_id = if requested_packet_id.is_empty() {
                    let Some(session) = sessions.first() else {
                        mutation_status
                            .set("Context render failed: no session is available.".to_string());
                        return;
                    };
                    let path = format!("/api/sessions/{}/context-packet", session.id);
                    match api_post::<ContextPacket, _>(&path, &json!({})).await {
                        Ok(packet) => {
                            context_packet_id.set(packet.id.clone());
                            packet.id
                        }
                        Err(error) => {
                            mutation_status.set(format!("Context packet creation failed: {error}"));
                            return;
                        }
                    }
                } else {
                    requested_packet_id
                };
                let body = render_context_body(1_500, 5, 280, 3);
                let path = format!("/api/context-packets/{packet_id}/render");
                match api_post::<RenderedExecutionContext, _>(&path, &body).await {
                    Ok(rendered) => {
                        mutation_status.set(format!(
                            "Context rendered: {} objects, {} fetchable.",
                            rendered.relevant_objects.len(),
                            rendered.fetchable_object_ids.len()
                        ));
                        rendered_context.set(Some(rendered));
                    }
                    Err(error) => mutation_status.set(format!("Context render failed: {error}")),
                }
            });
        })
    };

    let verify_deploy = {
        let version = data.deployment_version.data.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let version = version.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Verifying deployed version...".to_string());
                let body = json!({
                    "expected_git_sha": version.git_sha,
                    "expected_image_tag": version.image_tag,
                    "target": "whiskey",
                    "require_match": true
                });
                match api_post::<Value, _>("/api/deployment/production/verify", &body).await {
                    Ok(payload) => mutation_status
                        .set(format!("Deployment verify: {}", compact_json(&payload))),
                    Err(error) => mutation_status.set(format!("Deployment verify failed: {error}")),
                }
            });
        })
    };

    let toggle_critical_notifications = {
        let critical_notifications_muted = critical_notifications_muted.clone();
        Callback::from(move |_| {
            let next_value = !*critical_notifications_muted;
            storage_set(
                "mandoforge.criticalNotificationsMuted",
                if next_value { "1" } else { "0" },
            );
            critical_notifications_muted.set(next_value);
        })
    };

    let start_task = {
        let agents = data.agents.data.clone();
        let task_title = task_title.clone();
        let task_message = task_message.clone();
        let task_agent_id = task_agent_id.clone();
        let task_environment_id = task_environment_id.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let selected_agent_id = if task_agent_id.trim().is_empty() {
                agents
                    .first()
                    .map(|agent| agent.id.clone())
                    .unwrap_or_default()
            } else {
                (*task_agent_id).clone()
            };
            if selected_agent_id.trim().is_empty() {
                mutation_status.set("Start task failed: no agent is available.".to_string());
                return;
            }
            let environment_id = if task_environment_id.trim().is_empty() {
                None
            } else {
                Some(task_environment_id.as_str())
            };
            let body = create_session_body(
                &selected_agent_id,
                environment_id,
                &task_title,
                &task_message,
            );
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Starting task session...".to_string());
                match api_post::<Session, _>("/api/sessions", &body).await {
                    Ok(session) => mutation_status.set(format!(
                        "Started task {}: {}",
                        short_id(&session.id),
                        session_title(&session)
                    )),
                    Err(error) => mutation_status.set(format!("Start task failed: {error}")),
                }
            });
        })
    };

    let notifications = use_memo(data.clone(), |data| console_notifications(data));
    let notification_count = notifications.len();
    let critical_notification_count = notifications
        .iter()
        .filter(|notification| notification.severity == "critical")
        .count();
    {
        let notifications = (*notifications).clone();
        let critical_notifications_muted = *critical_notifications_muted;
        use_effect_with(
            (notifications, critical_notifications_muted),
            |(notifications, muted)| {
                if !*muted {
                    forward_critical_notifications_to_desktop(notifications);
                }
                || ()
            },
        );
    }
    let notifications = (*notifications).clone();

    html! {
        <main class="console-shell">
            <header class="topbar">
                <div>
                    <p class="eyebrow">{ "MandoForge Co-Work" }</p>
                    <h1>{ (*active_view).title() }</h1>
                </div>
                <div class="status-strip">
                    <Metric label="Agents running" value={running_agents.to_string()} tone="good" />
                    <Metric label="Workers active" value={active_job_count(&data.execution_jobs.data).to_string()} tone="good" />
                    <Metric label="Approvals" value={pending_approvals.to_string()} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
                    <Metric label="Refreshing" value={fetching_count.to_string()} tone="neutral" />
                    <Metric label="Errors" value={error_count.to_string()} tone={if error_count > 0 { "bad" } else { "good" }} />
                </div>
            </header>

            <section class="auth-strip">
                <div>
                    <strong>{ "API auth" }</strong>
                    <span>{ "Bearer token is used with x-mandoforge identity headers for live gates and production consoles." }</span>
                </div>
                <input
                    id="mandoforge-admin-token"
                    name="mandoforge-admin-token"
                    value={(*token_input).clone()}
                    placeholder="MANDOFORGE_DEV_ADMIN_TOKEN"
                    type="password"
                    oninput={{
                        let token_input = token_input.clone();
                        Callback::from(move |event: InputEvent| {
                            let input: HtmlInputElement = event.target_unchecked_into();
                            token_input.set(input.value());
                        })
                    }}
                />
                <button onclick={save_token}>{ "Save token" }</button>
                <button class="secondary" onclick={clear_token}>{ "Clear" }</button>
            </section>

            <nav class="tabs">
                { for View::ALL.into_iter().map(|view| {
                    let active_view = active_view.clone();
                    let is_active = *active_view == view;
                    html! {
                        <button
                            class={classes!("tab", is_active.then_some("active"))}
                            onclick={Callback::from(move |_| {
                                storage_set("mandoforge.activeView", view.id());
                                active_view.set(view);
                            })}
                        >
                            <span>{ view.label() }</span>
                        </button>
                    }
                })}
            </nav>

            <NotificationCenter
                notifications={notifications}
                critical_muted={*critical_notifications_muted}
                on_toggle_critical={toggle_critical_notifications.clone()}
                on_view={{
                    let active_view = active_view.clone();
                    Callback::from(move |view: View| {
                        storage_set("mandoforge.activeView", view.id());
                        active_view.set(view);
                    })
                }}
            />

            {
                if matches!(
                    *active_view,
                    View::Overview | View::Wizard | View::Packs | View::Settings
                ) {
                    html! {}
                } else {
                    html! { <VisualCommandDeck data={data.clone()} view={*active_view} /> }
                }
            }

            <section class="workspace">
                {
                    match *active_view {
                        View::Overview => html! {
                            <OverviewView
                                data={data.clone()}
                                on_view={{
                                    let active_view = active_view.clone();
                                    Callback::from(move |view: View| {
                                        storage_set("mandoforge.activeView", view.id());
                                        active_view.set(view);
                                    })
                                }}
                            />
                        },
                        View::Wizard => html! {
                            <WizardView
                                data={data.clone()}
                                on_status={{
                                    let mutation_status = mutation_status.clone();
                                    Callback::from(move |message: String| mutation_status.set(message))
                                }}
                                on_view={{
                                    let active_view = active_view.clone();
                                    Callback::from(move |view: View| {
                                        storage_set("mandoforge.activeView", view.id());
                                        active_view.set(view);
                                    })
                                }}
                            />
                        },
                        View::Agents => html! {
                            <AgentsView
                                data={data.clone()}
                                task_title={(*task_title).clone()}
                                task_message={(*task_message).clone()}
                                selected_agent_id={(*task_agent_id).clone()}
                                selected_environment_id={(*task_environment_id).clone()}
                                on_task_title={state_input(task_title.clone())}
                                on_task_message={state_textarea(task_message.clone())}
                                on_agent={state_select(task_agent_id.clone())}
                                on_environment={state_select(task_environment_id.clone())}
                                on_start_task={start_task.clone()}
                            />
                        },
                        View::Board => html! { <BoardView data={data.clone()} /> },
                        View::Workflows => html! { <WorkflowsView data={data.clone()} /> },
                        View::Dynamic => html! {
                            <DynamicView
                                data={data.clone()}
                                objective={(*dynamic_objective).clone()}
                                on_objective={state_input(dynamic_objective.clone())}
                                on_compile={compile_dynamic.clone()}
                            />
                        },
                        View::Semantic => html! {
                            <SemanticView
                                data={data.clone()}
                                source_text={(*semantic_source).clone()}
                                context_packet_id={(*context_packet_id).clone()}
                                rendered_context={(*rendered_context).clone()}
                                onboarding_run={(*onboarding_run).clone()}
                                onboarding_tool_specs={(*onboarding_tool_specs).clone()}
                                onboarding_review_graph={(*onboarding_review_graph).clone()}
                                onboarding_calibration={(*onboarding_calibration).clone()}
                                on_source={state_textarea(semantic_source.clone())}
                                on_build={build_ontology.clone()}
                                on_context_packet_id={state_input(context_packet_id.clone())}
                                on_render_context={render_context.clone()}
                                on_start_onboarding={start_ontology_onboarding.clone()}
                                on_approve_onboarding_proposal={approve_ontology_proposal.clone()}
                                on_reject_onboarding_proposal={reject_ontology_proposal.clone()}
                                on_materialize_onboarding={materialize_ontology_onboarding.clone()}
                            />
                        },
                        View::Packs => html! { <PacksView data={data.clone()} /> },
                        View::Deploy => html! { <DeployView data={data.clone()} on_verify={verify_deploy.clone()} /> },
                        View::Settings => html! {
                            <SettingsView
                                data={data.clone()}
                                critical_muted={*critical_notifications_muted}
                                notification_count={notification_count}
                                critical_notification_count={critical_notification_count}
                                on_toggle_critical={toggle_critical_notifications.clone()}
                            />
                        },
                    }
                }
            </section>

            <footer class="live-log">
                <strong>{ "Live log stream" }</strong>
                <span>{ if mutation_status.is_empty() { "No operator action in this browser turn." } else { mutation_status.as_str() } }</span>
            </footer>
        </main>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct VisualCommandDeckProps {
    data: ConsoleData,
    view: View,
}

#[component]
fn VisualCommandDeck(props: &VisualCommandDeckProps) -> Html {
    let data = &props.data;
    let active_jobs = active_job_count(&data.execution_jobs.data)
        + active_job_count(&data.session_loop_jobs.data);
    let pending_approvals = data
        .approvals
        .data
        .iter()
        .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
        .count();
    let readiness = data
        .stage2_readiness
        .data
        .readiness_score
        .unwrap_or_else(|| readiness_from_status(&data.stage2_readiness.data.status));
    let workflow_activity = data.workflow_runs.data.len() + data.dynamic_workflow_plans.data.len();
    let semantic_mass = data.semantic_graph.data.node_count + data.semantic_graph.data.edge_count;
    let enterprise_blocked = data.enterprise_product_readiness.data.blocked_lane_count;

    html! {
        <section class="visual-command-deck">
            <div class="deck-copy">
                <span>{ props.view.label() }</span>
                <strong>{ "Dynamic workflow map" }</strong>
                <small>{ "Live plan state, materialization strategy, active work, and gate pressure from the MandoForge control plane." }</small>
            </div>
            <DynamicWorkflowCanvas
                plans={data.dynamic_workflow_plans.data.clone()}
                workflow_runs={data.workflow_runs.data.clone()}
                execution_jobs={data.execution_jobs.data.clone()}
                session_loop_jobs={data.session_loop_jobs.data.clone()}
            />
            <div class="deck-bars">
                <FlowMeter label="Active work" value={active_jobs} max={active_jobs.max(data.sessions.data.len()).max(1)} tone="info" />
                <FlowMeter label="Approvals" value={pending_approvals} max={pending_approvals.max(data.approvals.data.len()).max(1)} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
                <FlowMeter label="Workflow graph" value={workflow_activity} max={workflow_activity.max(1)} tone="neutral" />
            </div>
            <div class="deck-gauge">
                <div class="radial-gauge" style={gauge_style(readiness)}>
                    <span>{ format!("{:.0}%", readiness * 100.0) }</span>
                </div>
                <div>
                    <strong>{ label_or(&data.enterprise_product_readiness.data.status, "readiness") }</strong>
                    <small>{ format!("{enterprise_blocked} enterprise blockers / {semantic_mass} semantic signals") }</small>
                </div>
            </div>
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct DynamicWorkflowCanvasProps {
    plans: Vec<DynamicWorkflowPlan>,
    workflow_runs: Vec<WorkflowRun>,
    execution_jobs: Vec<WorkerJob>,
    session_loop_jobs: Vec<WorkerJob>,
}

#[component]
fn DynamicWorkflowCanvas(props: &DynamicWorkflowCanvasProps) -> Html {
    let active_work =
        active_job_count(&props.execution_jobs) + active_job_count(&props.session_loop_jobs);
    let failed_jobs = props
        .execution_jobs
        .iter()
        .chain(props.session_loop_jobs.iter())
        .filter(|job| status_tone(&job.status) == "bad" || job.last_error.is_some())
        .count();
    let active_runs = props
        .workflow_runs
        .iter()
        .filter(|run| is_active_status(&run.status))
        .count();
    let native_plans = props
        .plans
        .iter()
        .filter(|plan| dynamic_plan_strategy(plan) == "native_dynamic")
        .count();
    let ready_plans = props
        .plans
        .iter()
        .filter(|plan| {
            matches!(
                plan.status.as_str(),
                "approved" | "materialized" | "reviewed"
            )
        })
        .count();
    let latest_plan = props.plans.first();
    let latest_objective = latest_plan
        .map(|plan| label_or(&plan.objective, "No dynamic workflow plan").to_string())
        .unwrap_or_else(|| "No dynamic workflow plan".to_string());
    let latest_status = latest_plan
        .map(|plan| label_or(&plan.status, "empty").to_string())
        .unwrap_or_else(|| "empty".to_string());
    let latest_strategy = latest_plan
        .map(dynamic_plan_strategy)
        .unwrap_or_else(|| "not compiled".to_string());
    let latest_phase_count = latest_plan.map(dynamic_plan_phase_count).unwrap_or(0);
    let latest_agent_count = latest_plan.map(dynamic_plan_total_agents).unwrap_or(0);
    let stages = vec![
        ("Plan", props.plans.len(), status_tone(&latest_status)),
        (
            "Native",
            native_plans,
            if native_plans > 0 { "good" } else { "neutral" },
        ),
        (
            "Work",
            active_work,
            if active_work > 0 { "info" } else { "neutral" },
        ),
        (
            "Runs",
            active_runs,
            if active_runs > 0 { "info" } else { "neutral" },
        ),
        (
            "Gate",
            ready_plans,
            if ready_plans > 0 { "good" } else { "neutral" },
        ),
        (
            "Errors",
            failed_jobs,
            if failed_jobs > 0 { "bad" } else { "good" },
        ),
    ];

    html! {
        <div class="dynamic-workflow-canvas" aria-label="Dynamic workflow status map">
            <div class="workflow-canvas-summary">
                <span>{ latest_strategy }</span>
                <strong title={latest_objective.clone()}>{ latest_objective }</strong>
                <small>{ format!("{latest_status} / {latest_phase_count} phases / {latest_agent_count} agents") }</small>
            </div>
            <div class="workflow-canvas-track">
                { for stages.iter().enumerate().map(|(index, (label, value, tone))| html! {
                    <article class={classes!("workflow-step-card", *tone)} key={(*label).to_string()}>
                        <span>{ format!("{:02}", index + 1) }</span>
                        <strong>{ *label }</strong>
                        <b>{ value }</b>
                    </article>
                }) }
            </div>
        </div>
    }
}

fn dynamic_plan_strategy(plan: &DynamicWorkflowPlan) -> String {
    plan.materialization
        .get("execution_strategy")
        .and_then(Value::as_str)
        .or_else(|| {
            plan.analysis
                .get("execution_strategy")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            if plan.runtime_adapter.is_empty() {
                None
            } else {
                Some(plan.runtime_adapter.as_str())
            }
        })
        .map(label_or_strategy)
        .unwrap_or_else(|| "unknown".to_string())
}

fn dynamic_plan_phase_count(plan: &DynamicWorkflowPlan) -> usize {
    plan.analysis
        .get("phase_count")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| plan.phases.as_array().map(Vec::len))
        .unwrap_or(0)
}

fn dynamic_plan_total_agents(plan: &DynamicWorkflowPlan) -> usize {
    plan.analysis
        .get("total_agent_count")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(0)
}

fn label_or_strategy(value: &str) -> String {
    label_or(value, "unknown").replace('_', " ")
}

#[hook]
fn use_polling<T>(path: &'static str, interval_ms: u32) -> ApiState<T>
where
    T: Clone + Default + PartialEq + for<'de> serde::Deserialize<'de> + 'static,
{
    let state = use_state(ApiState::<T>::default);
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            fetch_into_state(path, state.clone());
            let interval =
                Interval::new(interval_ms, move || fetch_into_state(path, state.clone()));
            move || drop(interval)
        });
    }
    (*state).clone()
}

fn fetch_into_state<T>(path: &'static str, state: UseStateHandle<ApiState<T>>)
where
    T: Clone + Default + PartialEq + for<'de> serde::Deserialize<'de> + 'static,
{
    state.set(ApiState {
        data: (*state).data.clone(),
        status: LoadStatus::Loading,
        error: None,
        updated_at_ms: (*state).updated_at_ms,
    });
    spawn_local(async move {
        match api_get::<T>(path).await {
            Ok(data) => state.set(ApiState {
                data,
                status: LoadStatus::Ready,
                error: None,
                updated_at_ms: now_ms(),
            }),
            Err(error) => state.set(ApiState {
                data: (*state).data.clone(),
                status: LoadStatus::Error,
                error: Some(error),
                updated_at_ms: now_ms(),
            }),
        }
    });
}

fn count_loading(data: &ConsoleData) -> usize {
    [
        data.agents.status,
        data.sessions.status,
        data.approvals.status,
        data.execution_jobs.status,
        data.workflow_runs.status,
        data.task_board.status,
        data.deployment_version.status,
        data.enterprise_product_readiness.status,
        data.native_connector_production_readiness.status,
        data.ontology_engine_readiness.status,
    ]
    .into_iter()
    .filter(|status| *status == LoadStatus::Loading)
    .count()
}

fn count_errors(data: &ConsoleData) -> usize {
    [
        data.agents.status,
        data.sessions.status,
        data.approvals.status,
        data.execution_jobs.status,
        data.workflow_runs.status,
        data.task_board.status,
        data.deployment_version.status,
        data.enterprise_product_readiness.status,
        data.native_connector_production_readiness.status,
        data.ontology_engine_readiness.status,
    ]
    .into_iter()
    .filter(|status| *status == LoadStatus::Error)
    .count()
}

pub(crate) fn active_job_count(jobs: &[WorkerJob]) -> usize {
    jobs.iter()
        .filter(|job| is_active_status(&job.status))
        .count()
}

pub(crate) fn pending_approval_count(approvals: &[Approval]) -> usize {
    approvals
        .iter()
        .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
        .count()
}

pub(crate) fn failed_job_count(jobs: &[WorkerJob]) -> usize {
    jobs.iter()
        .filter(|job| status_tone(&job.status) == "bad" || job.last_error.is_some())
        .count()
}

pub(crate) fn ready_pack_count(packs: &[WorkflowPackInstallation]) -> usize {
    packs
        .iter()
        .filter(|pack| matches!(pack.status.as_str(), "released" | "ready" | "active"))
        .count()
}

pub(crate) fn blocked_pack_count(packs: &[WorkflowPackInstallation]) -> usize {
    packs
        .iter()
        .filter(|pack| status_tone(&pack.status) == "bad" || pack.status == "blocked")
        .count()
}

pub(crate) fn json_status(value: &Value) -> String {
    value
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| value.get("readiness").and_then(Value::as_str))
        .map(|status| label_or(status, "not reported").to_string())
        .unwrap_or_else(|| {
            if value.is_null() {
                "not reported".to_string()
            } else {
                "reported".to_string()
            }
        })
}

pub(crate) fn json_gate_tone(value: &Value) -> &'static str {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| value.get("readiness").and_then(Value::as_str))
        .unwrap_or("");
    if status.trim().is_empty() {
        if value.is_null() { "warn" } else { "info" }
    } else {
        status_tone(status)
    }
}

pub(crate) fn first_lane_blocker(readiness: &EnterpriseProductReadiness) -> Option<String> {
    readiness.lanes.iter().find_map(|lane| {
        lane.blockers
            .first()
            .or_else(|| lane.next_actions.first())
            .cloned()
    })
}

pub(crate) fn worker_issue_rows(
    execution_jobs: &[WorkerJob],
    session_loop_jobs: &[WorkerJob],
) -> Vec<(String, String, String)> {
    execution_jobs
        .iter()
        .chain(session_loop_jobs.iter())
        .filter(|job| status_tone(&job.status) == "bad" || job.last_error.is_some())
        .take(6)
        .map(|job| {
            (
                job.status.clone(),
                short_id(&job.id),
                job.last_error
                    .clone()
                    .unwrap_or_else(|| label_or(&job.updated_at, "worker issue").to_string()),
            )
        })
        .collect()
}

pub(crate) fn pack_overview_rows(
    installations: &[WorkflowPackInstallation],
    marketplace: &WorkflowPackMarketplace,
) -> Vec<(String, String, String)> {
    let installation_rows = installations.iter().take(6).map(|pack| {
        (
            pack.status.clone(),
            label_or(&pack.pack_id, "pack").to_string(),
            format!(
                "{} / {} / {}",
                label_or(&pack.kind, "kind"),
                label_or(&pack.version, "version"),
                semantic_scope_summary(&pack.manifest["semantic_scopes"])
            ),
        )
    });
    let marketplace_rows = marketplace.packs.iter().take(6).map(|pack| {
        (
            pack.status.clone(),
            label_or(&pack.name, &pack.id).to_string(),
            format!(
                "{} / {} / {}",
                label_or(&pack.kind, "kind"),
                label_or(&pack.version, "version"),
                label_or(&pack.description, "marketplace pack")
            ),
        )
    });
    installation_rows.chain(marketplace_rows).take(8).collect()
}

pub(crate) fn pack_card_models(
    installations: &[WorkflowPackInstallation],
    marketplace: &WorkflowPackMarketplace,
) -> Vec<PackCardModel> {
    let mut cards = installations
        .iter()
        .map(|pack| {
            let manifest = pack.manifest.clone();
            PackCardModel {
                id: pack.pack_id.clone(),
                name: json_string(&manifest, "name")
                    .unwrap_or_else(|| label_or(&pack.pack_id, "pack").to_string()),
                kind: label_or(&pack.kind, "WorkflowPack").to_string(),
                version: label_or(&pack.version, "version").to_string(),
                description: json_string(&manifest, "description").unwrap_or_default(),
                status: label_or(&pack.status, "installed").to_string(),
                source: "installed".to_string(),
                manifest,
            }
        })
        .collect::<Vec<_>>();
    for pack in &marketplace.packs {
        if cards.iter().any(|card| card.id == pack.id) {
            continue;
        }
        let manifest = if pack.manifest_summary.is_null()
            || pack
                .manifest_summary
                .as_object()
                .is_some_and(|object| object.is_empty())
        {
            pack.validation.clone()
        } else {
            pack.manifest_summary.clone()
        };
        cards.push(PackCardModel {
            id: pack.id.clone(),
            name: label_or(&pack.name, &pack.id).to_string(),
            kind: label_or(&pack.kind, "WorkflowPack").to_string(),
            version: label_or(&pack.version, "version").to_string(),
            description: pack.description.clone(),
            status: label_or(&pack.status, "available").to_string(),
            source: "marketplace".to_string(),
            manifest,
        });
    }
    cards.sort_by(|a, b| {
        pack_sort_weight(&a.id)
            .cmp(&pack_sort_weight(&b.id))
            .then_with(|| a.name.cmp(&b.name))
    });
    cards
}

fn pack_sort_weight(id: &str) -> usize {
    match id {
        "ecommerce-tmall" => 0,
        "ecommerce-xiaohongshu" => 1,
        "ecommerce-taobao" => 2,
        "ecommerce-tiktok-shop" => 3,
        "ecommerce-amazon" => 4,
        "ecommerce-core" => 5,
        "legal" => 6,
        "ai-governance" => 7,
        _ => 20,
    }
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

pub(crate) fn json_array_len(value: &Value) -> usize {
    value.as_array().map(Vec::len).unwrap_or(0)
}

pub(crate) fn pack_metric_count(
    manifest: &Value,
    array_key: &str,
    fallback_count_key: &str,
) -> usize {
    json_array_len(&manifest[array_key]).max(
        manifest
            .get(fallback_count_key)
            .and_then(Value::as_u64)
            .map(|count| count as usize)
            .unwrap_or(0),
    )
}

pub(crate) fn pack_string_list(manifest: &Value, key: &str, limit: usize) -> Vec<String> {
    manifest
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .take(limit)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn pack_connector_rows(manifest: &Value) -> Vec<(String, String)> {
    let rows = manifest
        .get("connectors")
        .and_then(Value::as_array)
        .map(|connectors| {
            connectors
                .iter()
                .take(3)
                .filter_map(|connector| {
                    let id = connector.get("id").and_then(Value::as_str)?;
                    let kind = connector
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("connector");
                    let write_label = connector
                        .get("writes")
                        .and_then(|writes| writes.get("enabled"))
                        .and_then(Value::as_bool)
                        .map(|enabled| if enabled { "writes gated" } else { "read only" })
                        .unwrap_or("readiness gated");
                    let quality = connector
                        .get("data_quality")
                        .and_then(|quality| quality.get("min_sample_count"))
                        .and_then(Value::as_u64)
                        .map(|count| format!("{count} samples"))
                        .unwrap_or_else(|| "sample gate not declared".to_string());
                    Some((
                        id.to_string(),
                        format!("{kind} / {write_label} / {quality}"),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        vec![(
            "connector contract".to_string(),
            "No connector declared in this card payload.".to_string(),
        )]
    } else {
        rows
    }
}

pub(crate) fn pack_has_external_writes(manifest: &Value) -> bool {
    let connector_write = manifest
        .get("connectors")
        .and_then(Value::as_array)
        .map(|connectors| {
            connectors.iter().any(|connector| {
                connector
                    .get("writes")
                    .and_then(|writes| writes.get("enabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let agent_external_write = manifest
        .get("agents")
        .and_then(Value::as_array)
        .map(|agents| {
            agents.iter().any(|agent| {
                agent
                    .get("tool_scope")
                    .and_then(|scope| scope.get("external_write"))
                    .and_then(Value::as_array)
                    .is_some_and(|writes| !writes.is_empty())
                    || agent
                        .get("external_write_count")
                        .and_then(Value::as_u64)
                        .is_some_and(|count| count > 0)
            })
        })
        .unwrap_or(false);
    connector_write || agent_external_write
}

pub(crate) fn pack_requires_approval(manifest: &Value) -> bool {
    let connector_approval = manifest
        .get("connectors")
        .and_then(Value::as_array)
        .map(|connectors| {
            connectors.iter().any(|connector| {
                connector
                    .get("writes")
                    .and_then(|writes| writes.get("approval_required"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let handoff_approval = manifest
        .get("agents")
        .and_then(Value::as_array)
        .map(|agents| {
            agents.iter().any(|agent| {
                agent
                    .get("handoffs")
                    .and_then(Value::as_array)
                    .map(|handoffs| {
                        handoffs.iter().any(|handoff| {
                            handoff
                                .get("approval_required")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
                    || agent
                        .get("approval_handoff_count")
                        .and_then(Value::as_u64)
                        .is_some_and(|count| count > 0)
            })
        })
        .unwrap_or(false);
    connector_approval || handoff_approval
}

pub(crate) fn pack_lifecycle_steps(
    status: &str,
    source: &str,
) -> Vec<(&'static str, &'static str)> {
    let order = [
        ("Install", "installed"),
        ("Stage", "staged"),
        ("Onboard", "onboarding"),
        ("Quality", "quality"),
        ("Release", "released"),
        ("Rollback", "rolled_back"),
    ];
    if source == "marketplace" {
        return order
            .into_iter()
            .map(|(label, _)| {
                (
                    label,
                    if label == "Install" {
                        "warn"
                    } else {
                        "neutral"
                    },
                )
            })
            .collect();
    }
    order
        .into_iter()
        .map(|(label, step_status)| {
            let tone = if status == step_status {
                "info"
            } else if lifecycle_step_passed(status, step_status) {
                "good"
            } else if status_tone(status) == "bad" {
                "bad"
            } else {
                "neutral"
            };
            (label, tone)
        })
        .collect()
}

fn lifecycle_step_passed(status: &str, step: &str) -> bool {
    let rank = |value: &str| match value {
        "installed" => 1,
        "staged" => 2,
        "onboarding" => 3,
        "quality" => 4,
        "released" => 5,
        "rolled_back" => 6,
        "archived" => 7,
        _ => 0,
    };
    rank(status) > rank(step)
}

pub(crate) fn pack_blocker_summary(
    card: &PackCardModel,
    external_writes: bool,
    release_gate_count: usize,
) -> String {
    if card.source == "marketplace" {
        return "Available pack. Install creates a draft version before staging or release."
            .to_string();
    }
    match card.status.as_str() {
        "released" if external_writes => {
            "Released, but buyer-facing writes remain approval-gated and connector-quality-bound."
                .to_string()
        }
        "released" => "Released pack with read/draft governed runtime surfaces.".to_string(),
        "rolled_back" => {
            "Rolled back; historical release evidence remains audit-visible.".to_string()
        }
        "archived" => {
            "Archived; hidden from active tenant behavior without deleting evidence.".to_string()
        }
        status if status_tone(status) == "bad" => {
            "Blocked pack. Inspect lifecycle gate evidence before demo or release.".to_string()
        }
        _ if release_gate_count > 0 => {
            "Not released yet. Eval, policy, connector, and approval gates must pass first."
                .to_string()
        }
        _ => "Draft package shape is visible; lifecycle gates are not complete yet.".to_string(),
    }
}

pub(crate) fn operator_queue_rows(data: &ConsoleData) -> Vec<(String, String, String)> {
    let approval_rows = data.approvals.data.iter().filter_map(|approval| {
        if approval.status == "pending" || approval.status == "requires_action" {
            Some((
                approval.status.clone(),
                label_or(&approval.kind, "approval").to_string(),
                label_or(&approval.reason, &approval.id).to_string(),
            ))
        } else {
            None
        }
    });
    let job_rows = data
        .execution_jobs
        .data
        .iter()
        .chain(data.session_loop_jobs.data.iter())
        .filter_map(|job| {
            if status_tone(&job.status) == "bad" || job.last_error.is_some() {
                Some((
                    job.status.clone(),
                    short_id(&job.id),
                    job.last_error
                        .clone()
                        .unwrap_or_else(|| label_or(&job.updated_at, "worker issue").to_string()),
                ))
            } else {
                None
            }
        });
    let lane_rows = data
        .enterprise_product_readiness
        .data
        .lanes
        .iter()
        .filter_map(|lane| {
            lane.blockers.first().map(|blocker| {
                (
                    lane.status.clone(),
                    label_or(&lane.title, &lane.id).to_string(),
                    blocker.clone(),
                )
            })
        });
    approval_rows
        .chain(job_rows)
        .chain(lane_rows)
        .take(10)
        .collect()
}

pub(crate) fn is_active_status(status: &str) -> bool {
    matches!(
        status,
        "running" | "queued" | "claimed" | "in_progress" | "requires_action"
    )
}

pub(crate) fn board_column(status: &str) -> &'static str {
    match status {
        "ready" | "queued" | "claimable" => "ready",
        "running" | "claimed" | "in_progress" => "running",
        "review" | "requires_review" | "requires_action" | "pending_approval" => "review",
        "blocked" | "failed" | "cancelled" => "blocked",
        "completed" | "done" | "succeeded" => "done",
        _ => "backlog",
    }
}

pub(crate) fn status_tone(status: &str) -> &'static str {
    match status {
        "ready"
        | "completed"
        | "succeeded"
        | "active"
        | "promoted"
        | "released"
        | "enterprise_product_complete" => "good",
        "running" | "queued" | "claimed" | "in_progress" | "installed" | "staged" => "info",
        "pending" | "requires_action" | "review" | "warning" | "attention" | "pilot_ready" => {
            "warn"
        }
        "failed" | "blocked" | "critical" | "cancelled" | "rolled_back" => "bad",
        _ => "neutral",
    }
}

pub(crate) fn label_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

pub(crate) fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "unserializable".to_string())
}

pub(crate) fn semantic_scope_summary(scopes: &Value) -> String {
    let Some(object) = scopes.as_object() else {
        return "unscoped".to_string();
    };
    let parts = [
        "domain_scope",
        "workflow_scope",
        "lane_scope",
        "share_policy",
        "memory_scope",
    ]
    .into_iter()
    .filter_map(|key| {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.to_string())
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        "unscoped".to_string()
    } else {
        parts.join(" / ")
    }
}

pub(crate) fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "unserializable".to_string())
}

fn state_input(handle: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let input: HtmlInputElement = event.target_unchecked_into();
        handle.set(input.value());
    })
}

fn state_textarea(handle: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let input: HtmlTextAreaElement = event.target_unchecked_into();
        handle.set(input.value());
    })
}

fn state_select(handle: UseStateHandle<String>) -> Callback<Event> {
    Callback::from(move |event: Event| {
        let input: HtmlSelectElement = event.target_unchecked_into();
        handle.set(input.value());
    })
}

pub(crate) fn effective_selected(selected: &str, fallback: Option<&str>) -> String {
    if selected.trim().is_empty() {
        fallback.unwrap_or_default().to_string()
    } else {
        selected.to_string()
    }
}

pub(crate) fn session_title(session: &Session) -> String {
    if !session.title.trim().is_empty() {
        session.title.clone()
    } else if !session.objective.trim().is_empty() {
        session.objective.clone()
    } else {
        "Untitled session".to_string()
    }
}

pub(crate) fn gauge_style(score: f64) -> String {
    let pct = score.clamp(0.0, 1.0) * 100.0;
    format!("--gauge: {:.1}%;", pct)
}

pub(crate) fn position_style(x: f64, y: f64) -> String {
    format!("left: {:.2}%; top: {:.2}%;", x, y)
}

pub(crate) fn orbit_point(index: usize, total: usize, cx: f64, cy: f64, radius: f64) -> (f64, f64) {
    let total = total.max(1) as f64;
    let angle = (index as f64 / total) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

pub(crate) fn readiness_from_status(status: &str) -> f64 {
    match status_tone(status) {
        "good" => 0.92,
        "info" => 0.72,
        "warn" => 0.48,
        "bad" => 0.2,
        _ => 0.0,
    }
}

pub(crate) fn json_object_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len(),
        Value::Object(map) => map.len(),
        Value::Null => 0,
        _ => 1,
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
