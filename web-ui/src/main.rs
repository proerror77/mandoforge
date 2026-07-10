mod api;
mod components;
mod desktop_bridge;
mod graph_island;
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
    AgentsView, DeployView, OverviewView, PacksView, SemanticView, SettingsView, WizardView,
    WorkflowsView,
};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[component]
fn App() -> Html {
    let active_view = use_state(initial_active_view);
    let ui_lang = use_state(initial_ui_lang);
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
        "合同审核会用到合同、当事方、条款、义务、风险、司法辖区、模板和审批要求等业务概念。"
            .to_string()
    });
    let context_packet_id = use_state(String::new);
    let rendered_context = use_state(|| None::<RenderedExecutionContext>);
    let onboarding_run = use_state(|| None::<OntologyOnboardingRun>);
    let onboarding_tool_specs = use_state(Vec::<OntologyOnboardingToolSpec>::new);
    let onboarding_review_graph = use_state(|| None::<OntologyReviewGraph>);
    let onboarding_calibration = use_state(|| None::<ConfidenceCalibrationResponse>);
    let ontology_release_version = use_state(String::new);
    let critical_notifications_muted = use_state(initial_critical_notifications_muted);
    let current_view = *active_view;
    let poll_agent_detail = matches!(current_view, View::Wizard | View::Agents);
    let poll_agent_activity = matches!(current_view, View::Overview | View::Agents);
    let poll_runs_tasks = matches!(
        current_view,
        View::Overview | View::Agents | View::Board | View::Workflows | View::Dynamic
    );
    let poll_runs_detail = matches!(current_view, View::Board | View::Workflows | View::Dynamic);
    let poll_capabilities = matches!(current_view, View::Overview | View::Packs);
    let poll_capability_detail = matches!(current_view, View::Packs);
    let poll_ontology_summary = matches!(current_view, View::Overview | View::Semantic);
    let poll_ontology_detail = matches!(current_view, View::Semantic);
    let poll_system_ops_detail = matches!(current_view, View::Deploy);

    let data = ConsoleData {
        agents: use_polling::<Vec<Agent>>("/api/agents", 5_000, true),
        environments: use_polling::<Vec<Environment>>(
            "/api/environments",
            6_000,
            poll_agent_detail,
        ),
        sessions: use_polling::<Vec<Session>>("/api/sessions", 1_800, true),
        approvals: use_polling::<Vec<Approval>>("/api/approvals", 1_800, true),
        execution_jobs: use_polling::<Vec<WorkerJob>>("/api/execution-jobs", 1_500, true),
        session_loop_jobs: use_polling::<Vec<WorkerJob>>("/api/session-loop-jobs", 1_500, true),
        tool_calls: use_polling::<Vec<ToolCall>>("/api/tool-calls", 1_800, poll_agent_activity),
        workflow_runs: use_polling::<Vec<WorkflowRun>>(
            "/api/workflow-runs",
            1_600,
            poll_runs_tasks,
        ),
        workflow_definitions: use_polling::<Vec<WorkflowDefinition>>(
            "/api/workflow-definitions",
            3_000,
            poll_runs_detail,
        ),
        dynamic_workflow_plans: use_polling::<Vec<DynamicWorkflowPlan>>(
            "/api/dynamic-workflow-plans",
            2_500,
            poll_runs_tasks,
        ),
        task_board: use_polling::<TaskBoardSnapshot>("/api/task-board", 1_500, poll_runs_tasks),
        work_items: use_polling::<Vec<WorkItem>>("/api/work-items", 3_000, poll_runs_detail),
        manager_plans: use_polling::<Vec<Value>>(
            "/api/manager-plans",
            3_000,
            matches!(
                current_view,
                View::Agents | View::Workflows | View::Board | View::Dynamic
            ),
        ),
        agent_handoffs: use_polling::<Vec<Value>>(
            "/api/agent-handoffs",
            3_000,
            matches!(
                current_view,
                View::Agents | View::Workflows | View::Board | View::Dynamic
            ),
        ),
        agent_handoff_assignments: use_polling::<Vec<Value>>(
            "/api/agent-handoff-assignments",
            3_000,
            matches!(
                current_view,
                View::Agents | View::Workflows | View::Board | View::Dynamic
            ),
        ),
        workflow_pack_installations: use_polling::<Vec<WorkflowPackInstallation>>(
            "/api/workflow-packs/installations",
            4_000,
            poll_capabilities,
        ),
        stage2_readiness: use_polling::<Stage2Readiness>(
            "/api/stage2/readiness",
            4_000,
            matches!(current_view, View::Workflows | View::Deploy),
        ),
        enterprise_product_readiness: use_polling::<EnterpriseProductReadiness>(
            "/api/enterprise-product/readiness",
            4_000,
            true,
        ),
        native_connector_production_readiness: use_polling::<Value>(
            "/api/native-connectors/production-readiness",
            5_000,
            true,
        ),
        provider_runtime: use_polling::<Value>(
            "/api/providers/runtime",
            5_000,
            poll_system_ops_detail || poll_agent_detail,
        ),
        observability: use_polling::<ObservabilitySummary>(
            "/api/observability",
            3_000,
            poll_system_ops_detail,
        ),
        capability_discovery: use_polling::<CapabilityDiscovery>(
            "/api/capability-discovery",
            4_000,
            poll_capability_detail,
        ),
        usage: use_polling::<Value>("/api/usage", 5_000, poll_system_ops_detail),
        usage_finance_operations: use_polling::<Value>(
            "/api/usage/finance-operations/summary",
            5_000,
            poll_system_ops_detail,
        ),
        memory_governance: use_polling::<Value>(
            "/api/memory-governance/summary",
            5_000,
            poll_ontology_detail,
        ),
        memory_writebacks: use_polling::<Value>(
            "/api/memory-governance/writebacks?limit=50&status=pending",
            5_000,
            poll_ontology_detail,
        ),
        memory_writeback_candidates: use_polling::<Value>(
            "/api/memory-writeback-candidates",
            5_000,
            poll_ontology_detail,
        ),
        scheduler_summary: use_polling::<Value>("/api/scheduler/summary", 4_000, poll_runs_detail),
        deployment_version: use_polling::<DeploymentVersion>(
            "/api/deployment/version",
            4_000,
            matches!(current_view, View::Agents | View::Deploy),
        ),
        remote_computer_production_path: use_polling::<Value>(
            "/api/remote-computers/production-path",
            5_000,
            poll_system_ops_detail,
        ),
        workflow_pack_marketplace: use_polling::<WorkflowPackMarketplace>(
            "/api/workflow-packs/marketplace",
            6_000,
            poll_capabilities,
        ),
        semantic_objects: use_polling::<Vec<SemanticObject>>(
            "/api/semantic-objects",
            5_000,
            poll_ontology_summary,
        ),
        semantic_links: use_polling::<Vec<Value>>(
            "/api/semantic-links",
            5_000,
            poll_ontology_detail,
        ),
        semantic_search: use_polling::<Value>("/api/semantic-search", 5_000, poll_ontology_detail),
        semantic_graph: use_polling::<SemanticGraphSnapshot>(
            "/api/semantic-graph",
            5_000,
            poll_ontology_summary,
        ),
        semantic_workbench: use_polling_dynamic::<Value>(
            semantic_workbench_path_from_storage(),
            5_000,
            poll_ontology_detail,
        ),
        semantic_reflection_queue: use_polling::<SemanticReflectionQueue>(
            "/api/semantic-reflection/queue",
            5_000,
            poll_ontology_summary,
        ),
        ontology_registry: use_polling::<OntologyRegistry>(
            "/api/ontology/registry",
            6_000,
            poll_ontology_detail,
        ),
        ontology_engine_readiness: use_polling::<Value>(
            "/api/ontology/engine-readiness",
            6_000,
            true,
        ),
        ontology_releases: use_polling::<Vec<OntologyRelease>>(
            "/api/ontology/releases",
            6_000,
            poll_ontology_detail,
        ),
        semantic_retrieval_backends: use_polling::<Value>(
            "/api/semantic-retrieval/backends",
            6_000,
            poll_ontology_detail,
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
                mutation_status.set("正在生成本体提案预览...".to_string());
                let body =
                    ontology_builder_body(&source_text, &ontology_builder_config_from_storage());
                match api_post::<Value, _>("/api/semantic-ontology/builder", &body).await {
                    Ok(payload) => {
                        mutation_status.set(format!("本体预览已生成：{}", compact_json(&payload)))
                    }
                    Err(error) => mutation_status.set(format!("本体构建失败：{error}")),
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
            if get_admin_token().trim().is_empty() {
                mutation_status.set(missing_ontology_admin_token_message("本体入职启动失败"));
                return;
            }
            let onboarding_run = onboarding_run.clone();
            let onboarding_tool_specs = onboarding_tool_specs.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("正在启动企业本体入职示例...".to_string());
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
                            "本体入职运行已就绪：{} 个提案，来自 {} 个数据集。",
                            run.proposal_count, run.dataset_count
                        ));
                        onboarding_tool_specs.set(Vec::new());
                        onboarding_run.set(Some(run));
                    }
                    Err(error) => {
                        mutation_status
                            .set(ontology_write_error_message("本体入职启动失败", &error));
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
                mutation_status.set("正在批准本体提案...".to_string());
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
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Proposal approve failed",
                        &error,
                    )),
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
                mutation_status.set("正在拒绝本体提案...".to_string());
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
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Proposal reject failed",
                        &error,
                    )),
                }
            });
        })
    };

    let approve_ontology_proposals = {
        let onboarding_run = onboarding_run.clone();
        let onboarding_review_graph = onboarding_review_graph.clone();
        let onboarding_calibration = onboarding_calibration.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |proposal_ids: Vec<String>| {
            let Some(current_run) = (*onboarding_run).clone() else {
                mutation_status
                    .set("Batch approve failed: no onboarding run is active.".to_string());
                return;
            };
            if proposal_ids.is_empty() {
                mutation_status.set("没有可批量批准的本体提案。".to_string());
                return;
            }
            let onboarding_run = onboarding_run.clone();
            let onboarding_review_graph = onboarding_review_graph.clone();
            let onboarding_calibration = onboarding_calibration.clone();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                let total = proposal_ids.len();
                mutation_status.set(format!("正在批量批准 {total} 个本体提案..."));
                let body = json!({
                    "decision": "approve",
                    "reason": "operator batch-approved from semantic console",
                });
                let mut approved = 0usize;
                let mut first_error = None::<String>;
                for proposal_id in proposal_ids {
                    let path = format!("/api/ontology/onboarding/proposals/{proposal_id}/review");
                    match api_post::<OntologyOnboardingProposal, _>(&path, &body).await {
                        Ok(_) => approved += 1,
                        Err(error) => {
                            if first_error.is_none() {
                                first_error = Some(error);
                            }
                        }
                    }
                }
                if approved == 0 {
                    if let Some(error) = first_error {
                        mutation_status
                            .set(ontology_write_error_message("Batch approve failed", &error));
                        return;
                    }
                }
                let run_path = format!("/api/ontology/onboarding/runs/{}", current_run.id);
                match api_get::<OntologyOnboardingRun>(&run_path).await {
                    Ok(run) => {
                        let graph_path =
                            format!("/api/ontology/onboarding/runs/{}/review-graph", run.id);
                        if let Ok(graph) = api_get::<OntologyReviewGraph>(&graph_path).await {
                            onboarding_review_graph.set(Some(graph));
                        }
                        let calibration_path =
                            format!("/api/ontology/intelligence/runs/{}/calibration", run.id);
                        if let Ok(calibration) =
                            api_get::<ConfidenceCalibrationResponse>(&calibration_path).await
                        {
                            onboarding_calibration.set(Some(calibration));
                        }
                        if let Some(error) = first_error {
                            let failed = total.saturating_sub(approved);
                            mutation_status.set(format!(
                                "Batch partially approved {approved}/{total} proposals; {failed} failed. First error: {}",
                                ontology_write_error_message("Batch approve failed", &error)
                            ));
                        } else {
                            mutation_status.set(format!(
                                "Batch approved {approved}/{total} proposals: {}/{} approved.",
                                run.approved_count, run.proposal_count
                            ));
                        }
                        onboarding_run.set(Some(run));
                    }
                    Err(error) => mutation_status.set(format!(
                        "Batch approved {approved}/{total}; refresh failed: {error}"
                    )),
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
                mutation_status.set("正在发布已批准的本体提案...".to_string());
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
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Ontology materialize failed",
                        &error,
                    )),
                }
            });
        })
    };

    let create_ontology_release_candidate = {
        let onboarding_run = onboarding_run.clone();
        let ontology_release_version = ontology_release_version.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            let Some(current_run) = (*onboarding_run).clone() else {
                mutation_status
                    .set("Release candidate failed: no onboarding run is active.".to_string());
                return;
            };
            if current_run.materialized_count == 0 {
                mutation_status.set(
                    "Release candidate failed: materialize approved ontology proposals first."
                        .to_string(),
                );
                return;
            }
            let version = (*ontology_release_version).trim().to_string();
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Creating ontology release candidate...".to_string());
                let path = format!(
                    "/api/ontology/onboarding/runs/{}/release-candidate",
                    current_run.id
                );
                let body = if version.is_empty() {
                    json!({})
                } else {
                    json!({"version": version})
                };
                match api_post::<OntologyRelease, _>(&path, &body).await {
                    Ok(release) => mutation_status.set(format!(
                        "Release candidate created: {} ({})",
                        release.version,
                        short_id(&release.id)
                    )),
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Release candidate failed",
                        &error,
                    )),
                }
            });
        })
    };

    let gate_ontology_release = {
        let mutation_status = mutation_status.clone();
        Callback::from(move |release_id: String| {
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Running ontology release gate...".to_string());
                let path = format!("/api/ontology/releases/{release_id}/gate");
                match api_post::<OntologyRelease, _>(&path, &json!({})).await {
                    Ok(release) => {
                        let gate_status = release
                            .gate_result
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        mutation_status
                            .set(format!("Release gate {gate_status}: {}", release.version));
                    }
                    Err(error) => mutation_status
                        .set(ontology_write_error_message("Release gate failed", &error)),
                }
            });
        })
    };

    let promote_ontology_release = {
        let mutation_status = mutation_status.clone();
        Callback::from(move |release_id: String| {
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Promoting ontology release...".to_string());
                let path = format!("/api/ontology/releases/{release_id}/promote");
                match api_post::<OntologyRelease, _>(&path, &json!({})).await {
                    Ok(release) => mutation_status.set(format!(
                        "Release promoted: {} is active for {}.",
                        release.version, release.domain_scope
                    )),
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Release promote failed",
                        &error,
                    )),
                }
            });
        })
    };

    let rollback_ontology_release = {
        let mutation_status = mutation_status.clone();
        Callback::from(move |release_id: String| {
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Rolling back ontology release...".to_string());
                let path = format!("/api/ontology/releases/{release_id}/rollback");
                match api_post::<OntologyRelease, _>(&path, &json!({})).await {
                    Ok(release) => mutation_status.set(format!(
                        "Rollback restored active release: {} ({})",
                        release.version,
                        short_id(&release.id)
                    )),
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Release rollback failed",
                        &error,
                    )),
                }
            });
        })
    };

    let archive_ontology_release = {
        let mutation_status = mutation_status.clone();
        Callback::from(move |release_id: String| {
            let mutation_status = mutation_status.clone();
            spawn_local(async move {
                mutation_status.set("Archiving ontology release...".to_string());
                let path = format!("/api/ontology/releases/{release_id}/archive");
                match api_post::<OntologyRelease, _>(&path, &json!({})).await {
                    Ok(release) => mutation_status.set(format!(
                        "Release archived: {} ({})",
                        release.version,
                        short_id(&release.id)
                    )),
                    Err(error) => mutation_status.set(ontology_write_error_message(
                        "Release archive failed",
                        &error,
                    )),
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
                let target = deployment_target_from_storage();
                let body = json!({
                    "expected_git_sha": version.git_sha,
                    "expected_image_tag": version.image_tag,
                    "target": target,
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

    let set_lang_en = {
        let ui_lang = ui_lang.clone();
        Callback::from(move |_| {
            storage_set("mandoforge.uiLang", UiLang::En.id());
            ui_lang.set(UiLang::En);
        })
    };
    let set_lang_zh = {
        let ui_lang = ui_lang.clone();
        Callback::from(move |_| {
            storage_set("mandoforge.uiLang", UiLang::Zh.id());
            ui_lang.set(UiLang::Zh);
        })
    };

    let start_task = {
        let agents = data.agents.data.clone();
        let environments = data.environments.data.clone();
        let direct_session_launch_allowed = data.direct_session_launch_allowed();
        let task_title = task_title.clone();
        let task_message = task_message.clone();
        let task_agent_id = task_agent_id.clone();
        let task_environment_id = task_environment_id.clone();
        let mutation_status = mutation_status.clone();
        Callback::from(move |_| {
            if !direct_session_launch_allowed {
                mutation_status.set(
                    "Start task blocked: production requires a WorkflowRun-issued TaskGrant."
                        .to_string(),
                );
                return;
            }
            let selected_agent_id = agents
                .iter()
                .find(|agent| agent.is_runnable() && agent.id == *task_agent_id)
                .or_else(|| agents.iter().find(|agent| agent.is_runnable()))
                .map(|agent| agent.id.clone())
                .unwrap_or_default();
            if selected_agent_id.trim().is_empty() {
                mutation_status.set("Start task failed: no agent is available.".to_string());
                return;
            }
            let environment_id = environments
                .iter()
                .find(|environment| {
                    environment.is_runnable() && environment.id == *task_environment_id
                })
                .map(|environment| environment.id.as_str());
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

    let notifications = use_memo((data.clone(), *ui_lang), |(data, lang)| {
        console_notifications(data, *lang)
    });
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
            <aside class="command-rail" aria-label={(*ui_lang).text("Control plane navigation", "控制平面导航")}>
                <div class="rail-brand">
                    <span>{ "MF" }</span>
                    <div>
                        <strong>{ "MandoForge Co-Work" }</strong>
                        <small>{ (*ui_lang).text("Agent OS Kernel", "Agent OS Kernel") }</small>
                    </div>
                </div>

                <nav class="tabs">
                    { for View::PRIMARY_NAV.into_iter().map(|view| {
                        let active_view = active_view.clone();
                        let is_active = *active_view == view;
                        html! {
                            <button
                                class={classes!("tab", is_active.then_some("active"))}
                                onclick={Callback::from(move |_| {
                                    persist_active_view(view);
                                    active_view.set(view);
                                })}
                            >
                                <span class="tab-glyph">{ view_nav_glyph(view) }</span>
                                <span>{ view.label(*ui_lang) }</span>
                                <small>{ view_nav_hint(view, *ui_lang) }</small>
                            </button>
                        }
                    })}
                </nav>

                <div class="rail-tools">
                    <button
                        class={classes!("utility-nav-button", (*active_view == View::Wizard).then_some("active"))}
                        onclick={{
                            let active_view = active_view.clone();
                            Callback::from(move |_| {
                                persist_active_view(View::Wizard);
                                active_view.set(View::Wizard);
                            })
                        }}
                    >
                        { (*ui_lang).text("Setup", "设置向导") }
                    </button>
                    <button
                        class={classes!("utility-nav-button", (*active_view == View::Settings).then_some("active"))}
                        onclick={{
                            let active_view = active_view.clone();
                            Callback::from(move |_| {
                                persist_active_view(View::Settings);
                                active_view.set(View::Settings);
                            })
                        }}
                    >
                        { (*ui_lang).text("Settings", "系统设置") }
                    </button>
                </div>

                <div class="rail-status">
                    <Metric label={(*ui_lang).text("Agents", "智能体")} value={running_agents.to_string()} tone="good" />
                    <Metric label={(*ui_lang).text("Queue", "队列")} value={active_job_count(&data.execution_jobs.data).to_string()} tone="good" />
                    <Metric label={(*ui_lang).text("Approvals", "审批")} value={pending_approvals.to_string()} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
                    <Metric label={(*ui_lang).text("Errors", "错误")} value={error_count.to_string()} tone={if error_count > 0 { "bad" } else { "good" }} />
                </div>

                <div class="language-toggle" aria-label="Console language">
                    <button class={classes!((*ui_lang == UiLang::En).then_some("active"))} onclick={set_lang_en.clone()}>{ "EN" }</button>
                    <button class={classes!((*ui_lang == UiLang::Zh).then_some("active"))} onclick={set_lang_zh.clone()}>{ "中文" }</button>
                </div>
            </aside>

            <section class="control-plane">
                <header class="topbar">
                    <div>
                        <p class="eyebrow">{ (*ui_lang).text("Managed agent command surface", "托管智能体指挥面") }</p>
                        <h1>{ (*active_view).title(*ui_lang) }</h1>
                    </div>
                    <div class="status-strip">
                        <Metric label={(*ui_lang).text("Refreshing", "刷新中")} value={fetching_count.to_string()} tone="neutral" />
                        <Metric label={(*ui_lang).text("Active runs", "活跃运行")} value={running_agents.to_string()} tone="good" />
                        <Metric label={(*ui_lang).text("Human gates", "人工闸门")} value={pending_approvals.to_string()} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
                    </div>
                </header>

                <NotificationCenter
                    notifications={notifications}
                    lang={*ui_lang}
                    critical_muted={*critical_notifications_muted}
                    on_toggle_critical={toggle_critical_notifications.clone()}
                    on_view={{
                        let active_view = active_view.clone();
                        Callback::from(move |view: View| {
                            persist_active_view(view);
                            active_view.set(view);
                        })
                    }}
                />

                {
                    if matches!(*active_view, View::Workflows) {
                        html! { <VisualCommandDeck data={data.clone()} view={*active_view} lang={*ui_lang} /> }
                    } else {
                        html! {}
                    }
                }

                <section class="workspace">
                {
                    match *active_view {
                        View::Overview => html! {
                            <OverviewView
                                data={data.clone()}
                                lang={*ui_lang}
                                on_view={{
                                    let active_view = active_view.clone();
                                    Callback::from(move |view: View| {
                                        persist_active_view(view);
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
                                        persist_active_view(view);
                                        active_view.set(view);
                                    })
                                }}
                            />
                        },
                        View::Agents => html! {
                            <AgentsView
                                data={data.clone()}
                                lang={*ui_lang}
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
                        View::Board | View::Workflows | View::Dynamic => html! {
                            <WorkflowsView
                                data={data.clone()}
                                lang={*ui_lang}
                                objective={(*dynamic_objective).clone()}
                                on_objective={state_input(dynamic_objective.clone())}
                                on_compile={compile_dynamic.clone()}
                            />
                        },
                        View::Semantic => html! {
                            <SemanticView
                                data={data.clone()}
                                lang={*ui_lang}
                                source_text={(*semantic_source).clone()}
                                context_packet_id={(*context_packet_id).clone()}
                                rendered_context={(*rendered_context).clone()}
                                onboarding_run={(*onboarding_run).clone()}
                                onboarding_tool_specs={(*onboarding_tool_specs).clone()}
                                onboarding_review_graph={(*onboarding_review_graph).clone()}
                                onboarding_calibration={(*onboarding_calibration).clone()}
                                ontology_release_version={(*ontology_release_version).clone()}
                                on_source={state_textarea(semantic_source.clone())}
                                on_build={build_ontology.clone()}
                                on_context_packet_id={state_input(context_packet_id.clone())}
                                on_render_context={render_context.clone()}
                                on_start_onboarding={start_ontology_onboarding.clone()}
                                on_approve_onboarding_proposal={approve_ontology_proposal.clone()}
                                on_approve_onboarding_proposals={approve_ontology_proposals.clone()}
                                on_reject_onboarding_proposal={reject_ontology_proposal.clone()}
                                on_materialize_onboarding={materialize_ontology_onboarding.clone()}
                                on_release_version={state_input(ontology_release_version.clone())}
                                on_create_release_candidate={create_ontology_release_candidate.clone()}
                                on_gate_release={gate_ontology_release.clone()}
                                on_promote_release={promote_ontology_release.clone()}
                                on_rollback_release={rollback_ontology_release.clone()}
                                on_archive_release={archive_ontology_release.clone()}
                            />
                        },
                        View::Packs => html! { <PacksView data={data.clone()} lang={*ui_lang} /> },
                        View::Deploy => html! { <DeployView data={data.clone()} lang={*ui_lang} on_verify={verify_deploy.clone()} /> },
                        View::Settings => html! {
                            <SettingsView
                                data={data.clone()}
                                lang={*ui_lang}
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
                    <strong>{ (*ui_lang).text("Live operation log", "实时操作日志") }</strong>
                    <span>{ if mutation_status.is_empty() { (*ui_lang).text("No backend action has been triggered in this browser session.", "本轮浏览器操作还没有触发后台动作。") } else { mutation_status.as_str() } }</span>
                </footer>
            </section>
        </main>
    }
}

fn view_nav_glyph(view: View) -> &'static str {
    match view {
        View::Overview => "⌂",
        View::Agents => "◎",
        View::Workflows => "↔",
        View::Semantic => "◌",
        View::Packs => "□",
        View::Deploy => "△",
        _ => "•",
    }
}

fn view_nav_hint(view: View, lang: UiLang) -> &'static str {
    match view {
        View::Overview => lang.text("health and attention", "健康与注意项"),
        View::Agents => lang.text("fleet and approvals", "队列与审批"),
        View::Workflows => lang.text("runs, plans, board", "运行、计划、任务板"),
        View::Semantic => lang.text("objects to tools", "对象到工具"),
        View::Packs => lang.text("packs and connectors", "包与连接器"),
        View::Deploy => lang.text("evidence gates", "证据闸门"),
        _ => "",
    }
}

#[derive(Properties, Clone, PartialEq)]
struct VisualCommandDeckProps {
    data: ConsoleData,
    view: View,
    lang: UiLang,
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
                <span>{ props.view.label(props.lang) }</span>
                <strong>{ props.lang.text("Dynamic workflow map", "动态工作流地图") }</strong>
                <small>{ props.lang.text(
                    "Live plan state, materialization strategy, active work, and gate pressure from the MandoForge control plane.",
                    "展示实时计划状态、发布策略、活跃任务和控制平面的闸门压力。"
                ) }</small>
            </div>
            <DynamicWorkflowCanvas
                plans={data.dynamic_workflow_plans.data.clone()}
                workflow_runs={data.workflow_runs.data.clone()}
                execution_jobs={data.execution_jobs.data.clone()}
                session_loop_jobs={data.session_loop_jobs.data.clone()}
            />
            <div class="deck-bars">
                <FlowMeter label={props.lang.text("Active work", "活跃任务")} value={active_jobs} max={active_jobs.max(data.sessions.data.len()).max(1)} tone="info" />
                <FlowMeter label={props.lang.text("Approvals", "审批")} value={pending_approvals} max={pending_approvals.max(data.approvals.data.len()).max(1)} tone={if pending_approvals > 0 { "warn" } else { "good" }} />
                <FlowMeter label={props.lang.text("Workflow graph", "运行图")} value={workflow_activity} max={workflow_activity.max(1)} tone="neutral" />
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

fn storage_value_or(key: &str, default: &str) -> String {
    storage_get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn ontology_write_error_message(action: &str, error: &str) -> String {
    if error.contains("403") && error.contains("demo-operator") {
        if get_admin_token().trim().is_empty() {
            return missing_ontology_admin_token_message(action);
        }
        return format!(
            "{action}: 当前 mandoforge.adminToken 没有通过 API 授权。请确认它与 API 启动参数 MANDOFORGE_DEV_ADMIN_TOKEN 完全一致。"
        );
    }
    format!("{action}: {error}")
}

fn missing_ontology_admin_token_message(action: &str) -> String {
    format!(
        "{action}: 当前浏览器没有 mandoforge.adminToken。请到 Settings 填入与 API 启动参数 MANDOFORGE_DEV_ADMIN_TOKEN 相同的 token。"
    )
}

fn ontology_builder_config_from_storage() -> OntologyBuilderConfig {
    OntologyBuilderConfig {
        domain_scope: storage_value_or(ONTOLOGY_DOMAIN_SCOPE_KEY, DEFAULT_ONTOLOGY_DOMAIN_SCOPE),
        workflow_scope: storage_value_or(
            ONTOLOGY_WORKFLOW_SCOPE_KEY,
            DEFAULT_ONTOLOGY_WORKFLOW_SCOPE,
        ),
        memory_scope: storage_value_or(ONTOLOGY_MEMORY_SCOPE_KEY, DEFAULT_ONTOLOGY_MEMORY_SCOPE),
        objective: storage_value_or(ONTOLOGY_OBJECTIVE_KEY, DEFAULT_ONTOLOGY_OBJECTIVE),
    }
}

fn deployment_target_from_storage() -> String {
    storage_value_or(DEPLOYMENT_TARGET_KEY, DEFAULT_DEPLOYMENT_TARGET)
}

fn semantic_workbench_path_from_storage() -> String {
    let config = ontology_builder_config_from_storage();
    format!(
        "/api/semantic-workbench?domain_scope={}&workflow_scope={}",
        url_component(&config.domain_scope),
        url_component(&config.workflow_scope)
    )
}

fn url_component(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_default()
}

#[hook]
fn use_polling<T>(path: &'static str, interval_ms: u32, enabled: bool) -> ApiState<T>
where
    T: Clone + Default + PartialEq + for<'de> serde::Deserialize<'de> + 'static,
{
    let state = use_state(ApiState::<T>::default);
    {
        let state = state.clone();
        use_effect_with(enabled, move |enabled| {
            let interval = if *enabled {
                fetch_into_state(path.to_string(), state.clone());
                Some(Interval::new(interval_ms, move || {
                    fetch_into_state(path.to_string(), state.clone())
                }))
            } else {
                None
            };
            move || drop(interval)
        });
    }
    (*state).clone()
}

#[hook]
fn use_polling_dynamic<T>(path: String, interval_ms: u32, enabled: bool) -> ApiState<T>
where
    T: Clone + Default + PartialEq + for<'de> serde::Deserialize<'de> + 'static,
{
    let state = use_state(ApiState::<T>::default);
    {
        let state = state.clone();
        use_effect_with((path, enabled), move |(path, enabled)| {
            let interval = if *enabled {
                fetch_into_state(path.clone(), state.clone());
                let path = path.clone();
                Some(Interval::new(interval_ms, move || {
                    fetch_into_state(path.clone(), state.clone())
                }))
            } else {
                None
            };
            move || drop(interval)
        });
    }
    (*state).clone()
}

fn fetch_into_state<T>(path: String, state: UseStateHandle<ApiState<T>>)
where
    T: Clone + Default + PartialEq + for<'de> serde::Deserialize<'de> + 'static,
{
    if (*state).in_flight {
        return;
    }
    state.set(ApiState {
        data: (*state).data.clone(),
        status: LoadStatus::Loading,
        error: None,
        updated_at_ms: (*state).updated_at_ms,
        in_flight: true,
    });
    spawn_local(async move {
        match api_get::<T>(&path).await {
            Ok(data) => state.set(ApiState {
                data,
                status: LoadStatus::Ready,
                error: None,
                updated_at_ms: now_ms(),
                in_flight: false,
            }),
            Err(error) => state.set(ApiState {
                data: (*state).data.clone(),
                status: LoadStatus::Error,
                error: Some(error),
                updated_at_ms: now_ms(),
                in_flight: false,
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
        data.provider_runtime.status,
        data.usage_finance_operations.status,
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
        data.provider_runtime.status,
        data.usage_finance_operations.status,
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
        "active_trigger_failed" => "warn",
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
