use crate::api::*;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    Overview,
    Wizard,
    Agents,
    Board,
    Workflows,
    Dynamic,
    Semantic,
    Packs,
    Deploy,
    Settings,
}

impl View {
    pub(crate) const ALL: [View; 10] = [
        View::Overview,
        View::Wizard,
        View::Agents,
        View::Board,
        View::Workflows,
        View::Dynamic,
        View::Semantic,
        View::Packs,
        View::Deploy,
        View::Settings,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            View::Overview => "overview",
            View::Wizard => "wizard",
            View::Agents => "agents",
            View::Board => "board",
            View::Workflows => "workflows",
            View::Dynamic => "dynamic",
            View::Semantic => "semantic",
            View::Packs => "packs",
            View::Deploy => "deploy",
            View::Settings => "settings",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Wizard => "Wizard",
            View::Agents => "Agents",
            View::Board => "Board",
            View::Workflows => "Workflows",
            View::Dynamic => "Dynamic",
            View::Semantic => "Semantic",
            View::Packs => "Packs",
            View::Deploy => "Deploy",
            View::Settings => "Settings",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            View::Overview => "Enterprise control overview",
            View::Wizard => "First-run enterprise wizard",
            View::Agents => "Managed agent observability",
            View::Board => "Task board",
            View::Workflows => "Workflow graph console",
            View::Dynamic => "Dynamic workflow fleet",
            View::Semantic => "Semantic memory layer",
            View::Packs => "Workflow pack operations",
            View::Deploy => "Deployment truth surface",
            View::Settings => "Operator settings",
        }
    }

    fn from_id(value: &str) -> View {
        Self::ALL
            .into_iter()
            .find(|view| view.id() == value)
            .unwrap_or(View::Overview)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConsoleData {
    pub(crate) agents: ApiState<Vec<Agent>>,
    pub(crate) environments: ApiState<Vec<Environment>>,
    pub(crate) sessions: ApiState<Vec<Session>>,
    pub(crate) approvals: ApiState<Vec<Approval>>,
    pub(crate) execution_jobs: ApiState<Vec<WorkerJob>>,
    pub(crate) session_loop_jobs: ApiState<Vec<WorkerJob>>,
    pub(crate) tool_calls: ApiState<Vec<ToolCall>>,
    pub(crate) workflow_runs: ApiState<Vec<WorkflowRun>>,
    pub(crate) workflow_definitions: ApiState<Vec<WorkflowDefinition>>,
    pub(crate) dynamic_workflow_plans: ApiState<Vec<DynamicWorkflowPlan>>,
    pub(crate) task_board: ApiState<TaskBoardSnapshot>,
    pub(crate) work_items: ApiState<Vec<WorkItem>>,
    pub(crate) manager_plans: ApiState<Vec<Value>>,
    pub(crate) agent_handoffs: ApiState<Vec<Value>>,
    pub(crate) agent_handoff_assignments: ApiState<Vec<Value>>,
    pub(crate) workflow_pack_installations: ApiState<Vec<WorkflowPackInstallation>>,
    pub(crate) stage2_readiness: ApiState<Stage2Readiness>,
    pub(crate) enterprise_product_readiness: ApiState<EnterpriseProductReadiness>,
    pub(crate) native_connector_production_readiness: ApiState<Value>,
    pub(crate) observability: ApiState<ObservabilitySummary>,
    pub(crate) capability_discovery: ApiState<CapabilityDiscovery>,
    pub(crate) usage: ApiState<Value>,
    pub(crate) memory_governance: ApiState<Value>,
    pub(crate) memory_writebacks: ApiState<Value>,
    pub(crate) memory_writeback_candidates: ApiState<Value>,
    pub(crate) scheduler_summary: ApiState<Value>,
    pub(crate) deployment_version: ApiState<DeploymentVersion>,
    pub(crate) remote_computer_production_path: ApiState<Value>,
    pub(crate) workflow_pack_marketplace: ApiState<WorkflowPackMarketplace>,
    pub(crate) semantic_objects: ApiState<Vec<SemanticObject>>,
    pub(crate) semantic_links: ApiState<Vec<Value>>,
    pub(crate) semantic_search: ApiState<Value>,
    pub(crate) semantic_graph: ApiState<SemanticGraphSnapshot>,
    pub(crate) semantic_workbench: ApiState<Value>,
    pub(crate) semantic_reflection_queue: ApiState<SemanticReflectionQueue>,
    pub(crate) ontology_registry: ApiState<OntologyRegistry>,
    pub(crate) ontology_engine_readiness: ApiState<Value>,
    pub(crate) semantic_retrieval_backends: ApiState<Value>,
}

pub(crate) fn storage_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

pub(crate) fn initial_active_view() -> View {
    let stored = storage_get("mandoforge.activeView");
    let migrated = storage_get("mandoforge.overviewDefaultMigrated").is_some();
    if stored.as_deref() == Some("agents") && !migrated {
        storage_set("mandoforge.activeView", View::Overview.id());
        storage_set("mandoforge.overviewDefaultMigrated", "1");
        return View::Overview;
    }
    stored
        .as_deref()
        .map(View::from_id)
        .unwrap_or(View::Overview)
}

pub(crate) fn initial_critical_notifications_muted() -> bool {
    matches!(
        storage_get("mandoforge.criticalNotificationsMuted").as_deref(),
        Some("1" | "true" | "muted")
    )
}

pub(crate) fn storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}
