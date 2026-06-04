mod api;

use api::*;
use gloo_timers::callback::Interval;
use serde_json::{Value, json};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Agents,
    Board,
    Workflows,
    Dynamic,
    Semantic,
    Packs,
    Deploy,
}

impl View {
    const ALL: [View; 7] = [
        View::Agents,
        View::Board,
        View::Workflows,
        View::Dynamic,
        View::Semantic,
        View::Packs,
        View::Deploy,
    ];

    fn id(self) -> &'static str {
        match self {
            View::Agents => "agents",
            View::Board => "board",
            View::Workflows => "workflows",
            View::Dynamic => "dynamic",
            View::Semantic => "semantic",
            View::Packs => "packs",
            View::Deploy => "deploy",
        }
    }

    fn label(self) -> &'static str {
        match self {
            View::Agents => "Agents",
            View::Board => "Board",
            View::Workflows => "Workflows",
            View::Dynamic => "Dynamic",
            View::Semantic => "Semantic",
            View::Packs => "Packs",
            View::Deploy => "Deploy",
        }
    }

    fn title(self) -> &'static str {
        match self {
            View::Agents => "Managed agent observability",
            View::Board => "Task board",
            View::Workflows => "Workflow graph console",
            View::Dynamic => "Dynamic workflow fleet",
            View::Semantic => "Semantic memory layer",
            View::Packs => "Workflow pack operations",
            View::Deploy => "Deployment truth surface",
        }
    }

    fn from_id(value: &str) -> View {
        Self::ALL
            .into_iter()
            .find(|view| view.id() == value)
            .unwrap_or(View::Agents)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ConsoleData {
    agents: ApiState<Vec<Agent>>,
    environments: ApiState<Vec<Environment>>,
    sessions: ApiState<Vec<Session>>,
    approvals: ApiState<Vec<Approval>>,
    execution_jobs: ApiState<Vec<WorkerJob>>,
    session_loop_jobs: ApiState<Vec<WorkerJob>>,
    tool_calls: ApiState<Vec<ToolCall>>,
    workflow_runs: ApiState<Vec<WorkflowRun>>,
    workflow_definitions: ApiState<Vec<WorkflowDefinition>>,
    dynamic_workflow_plans: ApiState<Vec<DynamicWorkflowPlan>>,
    task_board: ApiState<TaskBoardSnapshot>,
    work_items: ApiState<Vec<WorkItem>>,
    manager_plans: ApiState<Vec<Value>>,
    agent_handoffs: ApiState<Vec<Value>>,
    agent_handoff_assignments: ApiState<Vec<Value>>,
    workflow_pack_installations: ApiState<Vec<WorkflowPackInstallation>>,
    stage2_readiness: ApiState<Stage2Readiness>,
    observability: ApiState<ObservabilitySummary>,
    capability_discovery: ApiState<CapabilityDiscovery>,
    usage: ApiState<Value>,
    memory_governance: ApiState<Value>,
    memory_writebacks: ApiState<Value>,
    memory_writeback_candidates: ApiState<Value>,
    scheduler_summary: ApiState<Value>,
    deployment_version: ApiState<DeploymentVersion>,
    remote_computer_production_path: ApiState<Value>,
    workflow_pack_marketplace: ApiState<WorkflowPackMarketplace>,
    semantic_objects: ApiState<Vec<SemanticObject>>,
    semantic_links: ApiState<Vec<Value>>,
    semantic_search: ApiState<Value>,
    semantic_graph: ApiState<SemanticGraphSnapshot>,
    semantic_workbench: ApiState<Value>,
    semantic_reflection_queue: ApiState<SemanticReflectionQueue>,
    ontology_registry: ApiState<OntologyRegistry>,
    semantic_retrieval_backends: ApiState<Value>,
}

#[component]
fn App() -> Html {
    let active_view = use_state(|| {
        storage_get("mandoforge.activeView")
            .map(|value| View::from_id(&value))
            .unwrap_or(View::Agents)
    });
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
            "/api/semantic-workbench?domain_scope=legal",
            5_000,
        ),
        semantic_reflection_queue: use_polling::<SemanticReflectionQueue>(
            "/api/semantic-reflection/queue",
            5_000,
        ),
        ontology_registry: use_polling::<OntologyRegistry>("/api/ontology/registry", 6_000),
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
                    let path = format!("/api/sessions/{}/context-packets", session.id);
                    match api_post::<ContextPacket, _>(&path, &json!({})).await {
                        Ok(packet) => {
                            context_packet_id.set(packet.id.clone());
                            packet.id
                        }
                        Err(error) => {
                            mutation_status
                                .set(format!("Context packet creation failed: {error}"));
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

            <VisualCommandDeck data={data.clone()} view={*active_view} />

            <section class="workspace">
                {
                    match *active_view {
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
                                on_source={state_textarea(semantic_source.clone())}
                                on_build={build_ontology.clone()}
                                on_context_packet_id={state_input(context_packet_id.clone())}
                                on_render_context={render_context.clone()}
                            />
                        },
                        View::Packs => html! { <PacksView data={data.clone()} /> },
                        View::Deploy => html! { <DeployView data={data.clone()} on_verify={verify_deploy.clone()} /> },
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
struct DataProps {
    data: ConsoleData,
}

#[derive(Properties, Clone, PartialEq)]
struct AgentsProps {
    data: ConsoleData,
    task_title: String,
    task_message: String,
    selected_agent_id: String,
    selected_environment_id: String,
    on_task_title: Callback<InputEvent>,
    on_task_message: Callback<InputEvent>,
    on_agent: Callback<Event>,
    on_environment: Callback<Event>,
    on_start_task: Callback<MouseEvent>,
}

#[component]
fn AgentsView(props: &AgentsProps) -> Html {
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

#[component]
fn BoardView(props: &DataProps) -> Html {
    let items = &props.data.task_board.data.items;
    html! {
        <div class="page-stack">
            <div class="kanban">
                { for ["ready", "running", "review", "blocked", "backlog", "done"].iter().map(|column| {
                    let filtered = items.iter().filter(|item| board_column(&item.status) == *column).collect::<Vec<_>>();
                    html! {
                        <section class="board-column">
                            <header>
                                <strong>{ column.to_ascii_uppercase() }</strong>
                                <span>{ filtered.len() }</span>
                            </header>
                            { for filtered.into_iter().map(|item| html! {
                                <article class="board-card" key={item.id.clone()}>
                                    <strong>{ label_or(&item.title, item.work_item.as_ref().map(|w| w.title.as_str()).unwrap_or("Untitled work")) }</strong>
                                    <span>{ format!("{} / {}", label_or(&item.priority, "normal"), short_id(&item.id)) }</span>
                                </article>
                            }) }
                        </section>
                    }
                }) }
            </div>
            <div class="page-grid">
                <Panel title="Work items">
                    <Rows empty="No work items." rows={props.data.work_items.data.iter().take(8).map(|item| {
                        (item.status.clone(), label_or(&item.title, "work item").to_string(), item.priority.clone())
                    }).collect::<Vec<_>>()} />
                </Panel>
                <Panel title="Handoffs and reviews">
                    <KeyMetrics values={vec![
                        ("Manager plans".to_string(), props.data.manager_plans.data.len().to_string()),
                        ("Handoffs".to_string(), props.data.agent_handoffs.data.len().to_string()),
                        ("Assignments".to_string(), props.data.agent_handoff_assignments.data.len().to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
    }
}

#[component]
fn WorkflowsView(props: &DataProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Workflow graph">
                <WorkflowGraph runs={props.data.workflow_runs.data.clone()} definitions={props.data.workflow_definitions.data.clone()} />
            </Panel>
            <Panel title="Workflow runs">
                <Rows empty="No workflow runs." rows={props.data.workflow_runs.data.iter().take(12).map(|run| {
                    (run.status.clone(), label_or(&run.title, "workflow run").to_string(), short_id(&run.id))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Definitions">
                <Rows empty="No workflow definitions." rows={props.data.workflow_definitions.data.iter().take(12).map(|definition| {
                    (definition.status.clone(), label_or(&definition.name, "workflow").to_string(), label_or(&definition.version, "version").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Scheduler">
                <JsonPreview value={props.data.scheduler_summary.data.clone()} />
            </Panel>
            <Panel title="Evidence surfaces">
                <KeyMetrics values={vec![
                    ("Workflow".to_string(), props.data.workflow_runs.data.len().to_string()),
                    ("Steps".to_string(), "via /api/workflow-runs/{id}/steps".to_string()),
                    ("Transitions".to_string(), "via /api/workflow-runs/{id}/transitions".to_string()),
                    ("Grants".to_string(), "via /api/workflow-runs/{id}/task-grants".to_string()),
                    ("Graph".to_string(), "via /api/workflow-runs/{id}/graph".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct DynamicProps {
    data: ConsoleData,
    objective: String,
    on_objective: Callback<InputEvent>,
    on_compile: Callback<MouseEvent>,
}

#[component]
fn DynamicView(props: &DynamicProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Compiler">
                <div class="form-stack">
                    <textarea value={props.objective.clone()} oninput={props.on_objective.clone()} />
                    <button onclick={props.on_compile.clone()}>{ "Compile dynamic workflow" }</button>
                </div>
            </Panel>
            <Panel title="Fleet shape">
                <FleetShape plans={props.data.dynamic_workflow_plans.data.clone()} />
            </Panel>
            <Panel title="Plans">
                <Rows empty="No dynamic workflow plans." rows={props.data.dynamic_workflow_plans.data.iter().take(12).map(|plan| {
                    (plan.status.clone(), label_or(&plan.objective, "dynamic workflow").to_string(), label_or(&plan.runtime_adapter, "runtime").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Fleet policy">
                <KeyMetrics values={vec![
                    ("Max agents".to_string(), "1000 policy cap".to_string()),
                    ("Max parallel".to_string(), "16 policy cap".to_string()),
                    ("Cross-check".to_string(), "review and adjudication metadata".to_string()),
                ]} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SemanticProps {
    data: ConsoleData,
    source_text: String,
    context_packet_id: String,
    rendered_context: Option<RenderedExecutionContext>,
    on_source: Callback<InputEvent>,
    on_build: Callback<MouseEvent>,
    on_context_packet_id: Callback<InputEvent>,
    on_render_context: Callback<MouseEvent>,
}

#[component]
fn SemanticView(props: &SemanticProps) -> Html {
    html! {
        <div class="page-grid semantic-grid">
            <Panel title="Ontology builder">
                <div class="form-stack">
                    <textarea value={props.source_text.clone()} oninput={props.on_source.clone()} />
                    <button onclick={props.on_build.clone()}>{ "Preview ontology proposal" }</button>
                </div>
            </Panel>
            <Panel title="Context compiler">
                <div class="form-stack">
                    <input
                        value={props.context_packet_id.clone()}
                        placeholder="Context packet ID"
                        oninput={props.on_context_packet_id.clone()}
                    />
                    <button onclick={props.on_render_context.clone()}>{ "Render execution context" }</button>
                    <RenderedContextPreview rendered={props.rendered_context.clone()} />
                </div>
            </Panel>
            <Panel title="Semantic graph">
                <SemanticMap snapshot={props.data.semantic_graph.data.clone()} objects={props.data.semantic_objects.data.clone()} />
            </Panel>
            <Panel title="Objects">
                <Rows empty="No semantic objects." rows={props.data.semantic_objects.data.iter().take(10).map(|object| {
                    (object.status.clone(), label_or(&object.title, &object.object_key).to_string(), format!("{} / {}", object.object_type, object.trust_level))
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Governance">
                <KeyMetrics values={vec![
                    ("Writebacks".to_string(), compact_json(&props.data.memory_writebacks.data)),
                    ("Candidates".to_string(), compact_json(&props.data.memory_writeback_candidates.data)),
                    ("Ontology".to_string(), props.data.ontology_registry.data.version.clone()),
                    ("Reflection".to_string(), props.data.semantic_reflection_queue.data.status.clone()),
                ]} />
            </Panel>
        </div>
    }
}

#[component]
fn PacksView(props: &DataProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Marketplace map">
                <PackMosaic
                    installations={props.data.workflow_pack_installations.data.clone()}
                    marketplace={props.data.workflow_pack_marketplace.data.clone()}
                />
            </Panel>
            <Panel title="Installations">
                <Rows empty="No pack installations." rows={props.data.workflow_pack_installations.data.iter().take(12).map(|pack| {
                    (pack.status.clone(), label_or(&pack.pack_id, "pack").to_string(), label_or(&pack.version, "version").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title="Marketplace">
                <KeyMetrics values={vec![
                    ("Status".to_string(), props.data.workflow_pack_marketplace.data.status.clone()),
                    ("Packs".to_string(), props.data.workflow_pack_marketplace.data.packs.len().to_string()),
                    ("Bindings".to_string(), "via /api/workflow-packs/installations/{id}/bindings".to_string()),
                    ("Runtime objects".to_string(), "via /api/workflow-packs/installations/{id}/runtime-objects".to_string()),
                ]} />
            </Panel>
            <Panel title="Onboarding">
                <JsonPreview value={Value::Array(props.data.capability_discovery.data.capabilities.clone())} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct DeployProps {
    data: ConsoleData,
    on_verify: Callback<MouseEvent>,
}

#[component]
fn DeployView(props: &DeployProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Latest deployment">
                <VersionBlock version={props.data.deployment_version.data.clone()} />
                <button onclick={props.on_verify.clone()}>{ "Verify deployed version" }</button>
            </Panel>
            <Panel title="Stage 2 readiness">
                <ReadinessRadar readiness={props.data.stage2_readiness.data.clone()} observability={props.data.observability.data.clone()} />
            </Panel>
            <Panel title="Remote computer path">
                <JsonPreview value={props.data.remote_computer_production_path.data.clone()} />
            </Panel>
            <Panel title="Usage and finance">
                <JsonPreview value={props.data.usage.data.clone()} />
            </Panel>
        </div>
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
    let semantic_mass =
        data.semantic_graph.data.node_count + data.semantic_graph.data.edge_count;

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
                    <strong>{ label_or(&data.stage2_readiness.data.status, "readiness") }</strong>
                    <small>{ format!("{} semantic signals", semantic_mass) }</small>
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
        .filter(|plan| matches!(plan.status.as_str(), "approved" | "materialized" | "reviewed"))
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
        ("Native", native_plans, if native_plans > 0 { "good" } else { "neutral" }),
        ("Work", active_work, if active_work > 0 { "info" } else { "neutral" }),
        ("Runs", active_runs, if active_runs > 0 { "info" } else { "neutral" }),
        ("Gate", ready_plans, if ready_plans > 0 { "good" } else { "neutral" }),
        ("Errors", failed_jobs, if failed_jobs > 0 { "bad" } else { "good" }),
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
        .or_else(|| plan.analysis.get("execution_strategy").and_then(Value::as_str))
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

#[derive(Properties, Clone, PartialEq)]
struct FlowMeterProps {
    label: &'static str,
    value: usize,
    max: usize,
    #[prop_or("neutral")]
    tone: &'static str,
}

#[component]
fn FlowMeter(props: &FlowMeterProps) -> Html {
    html! {
        <div class={classes!("flow-meter", props.tone)}>
            <span>{ props.label }</span>
            <strong>{ props.value }</strong>
            <i><b style={width_style(props.value, props.max)}></b></i>
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

#[derive(Properties, Clone, PartialEq)]
struct RuntimePipelineProps {
    sessions: Vec<Session>,
    execution_jobs: Vec<WorkerJob>,
    session_loop_jobs: Vec<WorkerJob>,
    approvals: Vec<Approval>,
    tool_calls: Vec<ToolCall>,
}

#[component]
fn RuntimePipeline(props: &RuntimePipelineProps) -> Html {
    let stages = vec![
        ("Sessions", props.sessions.len(), "neutral"),
        (
            "Session loop",
            active_job_count(&props.session_loop_jobs),
            "info",
        ),
        ("Workers", active_job_count(&props.execution_jobs), "info"),
        ("Tools", props.tool_calls.len(), "good"),
        (
            "Approvals",
            props
                .approvals
                .iter()
                .filter(|approval| approval.status == "pending" || approval.status == "requires_action")
                .count(),
            "warn",
        ),
    ];
    let max = stages.iter().map(|(_, value, _)| *value).max().unwrap_or(1).max(1);
    html! {
        <div class="runtime-pipeline">
            { for stages.iter().enumerate().map(|(index, (label, value, tone))| html! {
                <div class={classes!("pipeline-stage", *tone)} key={(*label).to_string()}>
                    <span>{ index + 1 }</span>
                    <strong>{ label }</strong>
                    <i style={height_style(*value, max)}></i>
                    <small>{ value }</small>
                </div>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct WorkflowGraphProps {
    runs: Vec<WorkflowRun>,
    definitions: Vec<WorkflowDefinition>,
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
                    html! { <p class="empty">{ "No workflow definitions." }</p> }
                } else {
                    html! {}
                }}
            </div>
            <div class="graph-stats">
                <FlowMeter label="Runs" value={props.runs.len()} max={props.runs.len().max(1)} tone="neutral" />
                <FlowMeter label="Active" value={active_runs} max={props.runs.len().max(1)} tone="info" />
                <FlowMeter label="Failed" value={failed_runs} max={props.runs.len().max(1)} tone={if failed_runs > 0 { "bad" } else { "good" }} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct FleetShapeProps {
    plans: Vec<DynamicWorkflowPlan>,
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
                <FlowMeter label="Compiled plans" value={props.plans.len()} max={total} tone="info" />
                <FlowMeter label="Ready plans" value={props.plans.iter().filter(|plan| status_tone(&plan.status) == "good").count()} max={total} tone="good" />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SemanticMapProps {
    snapshot: SemanticGraphSnapshot,
    objects: Vec<SemanticObject>,
}

#[component]
fn SemanticMap(props: &SemanticMapProps) -> Html {
    let total = props.objects.len().max(1);
    html! {
        <div class="semantic-map">
            <div class="semantic-canvas">
                <span class="semantic-core">{ props.snapshot.node_count }</span>
                { for props.objects.iter().take(16).enumerate().map(|(index, object)| {
                    let (x, y) = orbit_point(index, total, 50.0, 50.0, 42.0);
                    html! {
                        <b
                            class={classes!("semantic-node", status_tone(&object.status))}
                            title={format!("{} / {}", label_or(&object.title, &object.object_key), object.object_type)}
                            style={position_style(x, y)}
                        />
                    }
                }) }
            </div>
            <div class="semantic-stats">
                <FlowMeter label="Objects" value={props.snapshot.node_count} max={props.snapshot.node_count.max(1)} tone="info" />
                <FlowMeter label="Links" value={props.snapshot.edge_count} max={props.snapshot.edge_count.max(props.snapshot.node_count).max(1)} tone="good" />
                <FlowMeter label="Conflicts" value={props.snapshot.conflicts.len()} max={props.snapshot.conflicts.len().max(1)} tone={if props.snapshot.conflicts.is_empty() { "good" } else { "warn" }} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct RenderedContextPreviewProps {
    rendered: Option<RenderedExecutionContext>,
}

#[component]
fn RenderedContextPreview(props: &RenderedContextPreviewProps) -> Html {
    let Some(rendered) = props.rendered.as_ref() else {
        return html! { <p class="empty">{ "No execution context rendered." }</p> };
    };
    let omissions = &rendered.omitted;
    let budget = &rendered.budget;
    html! {
        <div class="context-preview">
            <div class="context-budget">
                <FlowMeter
                    label="Prompt tokens"
                    value={budget.estimated_tokens_used}
                    max={budget.max_prompt_tokens.max(1)}
                    tone={if budget.estimated_tokens_used > budget.max_prompt_tokens { "warn" } else { "good" }}
                />
                <FlowMeter
                    label="Objects"
                    value={rendered.relevant_objects.len()}
                    max={budget.max_objects.max(1)}
                    tone="info"
                />
                <FlowMeter
                    label="Fetchable"
                    value={rendered.fetchable_object_ids.len()}
                    max={rendered.fetchable_object_ids.len().max(rendered.relevant_objects.len()).max(1)}
                    tone="neutral"
                />
            </div>
            <KeyMetrics values={vec![
                ("Packet".to_string(), short_id(&rendered.context_packet_id)),
                ("Role".to_string(), label_or(&rendered.role, "agent").to_string()),
                ("Version".to_string(), rendered.context_packet_version.to_string()),
                ("Full content".to_string(), rendered.full_content_included.to_string()),
                ("Tools".to_string(), rendered.available_tools.join(", ")),
                ("Omitted".to_string(), format!(
                    "{} budget / {} object cap / {} source refs / {} content",
                    omissions.token_budget_exceeded,
                    omissions.object_limit_exceeded,
                    omissions.source_refs_not_rendered,
                    omissions.full_content_not_rendered
                )),
            ]} />
            <Rows empty="No policy reminders." rows={rendered.must_follow.iter().take(6).map(|item| {
                ("must_follow".to_string(), item.clone(), "rendered reminder".to_string())
            }).collect::<Vec<_>>()} />
            <div class="context-objects">
                { for rendered.relevant_objects.iter().map(|object| html! {
                    <article class="context-object" key={object.id.clone()}>
                        <div>
                            <span>{ format!("{} / {}", label_or(&object.object_type, "semantic_object"), short_id(&object.id)) }</span>
                            <strong>{ label_or(&object.title, &object.object_key) }</strong>
                        </div>
                        <p>{ label_or(&object.summary, "No summary.") }</p>
                        <small>{ format!("{} / {}", label_or(&object.trust_level, "trust"), label_or(&object.freshness, "freshness")) }</small>
                    </article>
                }) }
            </div>
            <JsonPreview value={rendered.ontology_scope.clone()} />
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PackMosaicProps {
    installations: Vec<WorkflowPackInstallation>,
    marketplace: WorkflowPackMarketplace,
}

#[component]
fn PackMosaic(props: &PackMosaicProps) -> Html {
    let marketplace_count = props.marketplace.packs.len();
    let total = props.installations.len().max(marketplace_count).max(1);
    html! {
        <div class="pack-mosaic">
            <div class="mosaic-grid">
                { for props.installations.iter().take(12).enumerate().map(|(index, pack)| html! {
                    <article class={classes!("mosaic-tile", status_tone(&pack.status))} key={pack.id.clone()}>
                        <span>{ index + 1 }</span>
                        <strong>{ label_or(&pack.pack_id, "pack") }</strong>
                        <small>{ label_or(&pack.version, "version") }</small>
                    </article>
                }) }
                { if props.installations.is_empty() {
                    html! { { for (0..8).map(|index| html! { <i class="mosaic-placeholder" style={format!("--tone: {};", index)}></i> }) } }
                } else {
                    html! {}
                }}
            </div>
            <div class="mosaic-side">
                <FlowMeter label="Marketplace" value={marketplace_count} max={total} tone="neutral" />
                <FlowMeter label="Installed" value={props.installations.len()} max={total} tone="good" />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct ReadinessRadarProps {
    readiness: Stage2Readiness,
    observability: ObservabilitySummary,
}

#[component]
fn ReadinessRadar(props: &ReadinessRadarProps) -> Html {
    let score = props
        .readiness
        .readiness_score
        .unwrap_or_else(|| readiness_from_status(&props.readiness.status));
    let category_count = json_object_count(&props.readiness.categories);
    let signal_count = json_object_count(&props.observability.signals);
    html! {
        <div class="readiness-radar">
            <div class="radar-dial" style={gauge_style(score)}>
                <span>{ format!("{:.0}", score * 100.0) }</span>
                <small>{ "score" }</small>
            </div>
            <div class="radar-bars">
                <FlowMeter label="Categories" value={category_count} max={category_count.max(1)} tone="info" />
                <FlowMeter label="Signals" value={signal_count} max={signal_count.max(1)} tone="good" />
                <FlowMeter label="Observability" value={usize::from(status_tone(&props.observability.status) == "good")} max={1} tone={status_tone(&props.observability.status)} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PanelProps {
    title: &'static str,
    children: Children,
}

#[component]
fn Panel(props: &PanelProps) -> Html {
    html! {
        <section class="panel">
            <header><h2>{ props.title }</h2></header>
            { for props.children.iter() }
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct MetricProps {
    label: &'static str,
    value: String,
    #[prop_or("neutral")]
    tone: &'static str,
}

#[component]
fn Metric(props: &MetricProps) -> Html {
    html! {
        <div class={classes!("metric", props.tone)}>
            <span>{ props.label }</span>
            <strong>{ &props.value }</strong>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct KeyMetricsProps {
    values: Vec<(String, String)>,
}

#[component]
fn KeyMetrics(props: &KeyMetricsProps) -> Html {
    html! {
        <div class="key-metrics">
            { for props.values.iter().map(|(label, value)| html! {
                <div class="key-value" key={label.clone()}>
                    <span>{ label }</span>
                    <strong>{ value }</strong>
                </div>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct RowsProps {
    empty: &'static str,
    rows: Vec<(String, String, String)>,
}

#[component]
fn Rows(props: &RowsProps) -> Html {
    if props.rows.is_empty() {
        return html! { <p class="empty">{ props.empty }</p> };
    }
    html! {
        <div class="rows">
            { for props.rows.iter().map(|(status, title, detail)| html! {
                <article class="row" key={format!("{status}-{title}-{detail}")}>
                    <StatusLogo status={status.clone()} />
                    <div>
                        <strong>{ title }</strong>
                        <span>{ detail }</span>
                    </div>
                    <small>{ status }</small>
                </article>
            }) }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct JsonPreviewProps {
    value: Value,
}

#[component]
fn JsonPreview(props: &JsonPreviewProps) -> Html {
    html! { <pre class="json-preview">{ pretty_json(&props.value) }</pre> }
}

#[derive(Properties, Clone, PartialEq)]
struct StatusLogoProps {
    status: String,
}

#[component]
fn StatusLogo(props: &StatusLogoProps) -> Html {
    let tone = status_tone(&props.status);
    let letter = props
        .status
        .chars()
        .find(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "I".to_string());
    html! { <span class={classes!("status-logo", tone)}>{ letter }</span> }
}

#[derive(Properties, Clone, PartialEq)]
struct VersionBlockProps {
    version: DeploymentVersion,
}

#[component]
fn VersionBlock(props: &VersionBlockProps) -> Html {
    let version = &props.version;
    html! {
        <div class="version-block">
            <div><span>{ "Service" }</span><strong>{ label_or(&version.service, "mandoforge-api") }</strong></div>
            <div><span>{ "Image tag" }</span><strong>{ version.image_tag.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
            <div><span>{ "Git SHA" }</span><strong>{ version.git_sha.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
            <div><span>{ "Build time" }</span><strong>{ version.build_time.clone().unwrap_or_else(|| "not reported".to_string()) }</strong></div>
        </div>
    }
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
    ]
    .into_iter()
    .filter(|status| *status == LoadStatus::Error)
    .count()
}

fn active_job_count(jobs: &[WorkerJob]) -> usize {
    jobs.iter()
        .filter(|job| is_active_status(&job.status))
        .count()
}

fn is_active_status(status: &str) -> bool {
    matches!(
        status,
        "running" | "queued" | "claimed" | "in_progress" | "requires_action"
    )
}

fn board_column(status: &str) -> &'static str {
    match status {
        "ready" | "queued" | "claimable" => "ready",
        "running" | "claimed" | "in_progress" => "running",
        "review" | "requires_review" | "requires_action" | "pending_approval" => "review",
        "blocked" | "failed" | "cancelled" => "blocked",
        "completed" | "done" | "succeeded" => "done",
        _ => "backlog",
    }
}

fn status_tone(status: &str) -> &'static str {
    match status {
        "ready" | "completed" | "succeeded" | "active" | "promoted" | "released" => "good",
        "running" | "queued" | "claimed" | "in_progress" => "info",
        "pending" | "requires_action" | "review" | "warning" | "attention" => "warn",
        "failed" | "blocked" | "critical" | "cancelled" => "bad",
        _ => "neutral",
    }
}

fn label_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "unserializable".to_string())
}

fn pretty_json(value: &Value) -> String {
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

fn effective_selected(selected: &str, fallback: Option<&str>) -> String {
    if selected.trim().is_empty() {
        fallback.unwrap_or_default().to_string()
    } else {
        selected.to_string()
    }
}

fn session_title(session: &Session) -> String {
    if !session.title.trim().is_empty() {
        session.title.clone()
    } else if !session.objective.trim().is_empty() {
        session.objective.clone()
    } else {
        "Untitled session".to_string()
    }
}

fn width_style(value: usize, max: usize) -> String {
    format!("width: {:.1}%;", percent(value, max))
}

fn height_style(value: usize, max: usize) -> String {
    format!("height: {:.1}%;", percent(value, max).max(8.0))
}

fn gauge_style(score: f64) -> String {
    let pct = score.clamp(0.0, 1.0) * 100.0;
    format!("--gauge: {:.1}%;", pct)
}

fn position_style(x: f64, y: f64) -> String {
    format!("left: {:.2}%; top: {:.2}%;", x, y)
}

fn percent(value: usize, max: usize) -> f64 {
    if max == 0 {
        0.0
    } else {
        (value as f64 / max as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn orbit_point(index: usize, total: usize, cx: f64, cy: f64, radius: f64) -> (f64, f64) {
    let total = total.max(1) as f64;
    let angle = (index as f64 / total) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

fn readiness_from_status(status: &str) -> f64 {
    match status_tone(status) {
        "good" => 0.92,
        "info" => 0.72,
        "warn" => 0.48,
        "bad" => 0.2,
        _ => 0.0,
    }
}

fn json_object_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len(),
        Value::Object(map) => map.len(),
        Value::Null => 0,
        _ => 1,
    }
}

fn storage_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

fn storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
