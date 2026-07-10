use crate::api::*;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiLang {
    En,
    Zh,
}

impl UiLang {
    pub(crate) fn text(self, en: &'static str, zh: &'static str) -> &'static str {
        match self {
            UiLang::En => en,
            UiLang::Zh => zh,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            UiLang::En => "en",
            UiLang::Zh => "zh",
        }
    }
}

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

    pub(crate) const PRIMARY_NAV: [View; 6] = [
        View::Overview,
        View::Agents,
        View::Workflows,
        View::Semantic,
        View::Packs,
        View::Deploy,
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

    pub(crate) fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                View::Overview => "Overview",
                View::Wizard => "Setup",
                View::Agents => "Managed Agents",
                View::Board => "Task Board",
                View::Workflows => "Runs & Tasks",
                View::Dynamic => "Dynamic Plans",
                View::Semantic => "Ontology",
                View::Packs => "Capabilities",
                View::Deploy => "System Ops",
                View::Settings => "Settings",
            },
            UiLang::Zh => match self {
                View::Overview => "总览",
                View::Wizard => "系统设置",
                View::Agents => "托管智能体",
                View::Board => "任务板",
                View::Workflows => "运行与任务",
                View::Dynamic => "动态计划",
                View::Semantic => "本体与工具",
                View::Packs => "能力包",
                View::Deploy => "系统运维",
                View::Settings => "系统设置",
            },
        }
    }

    pub(crate) fn title(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                View::Overview => "Overview / 总览",
                View::Wizard => "Setup / 系统设置",
                View::Agents => "Managed Agents / 托管智能体",
                View::Board => "Task Board / 任务板",
                View::Workflows => "Runs & Tasks / 运行与任务",
                View::Dynamic => "Dynamic Plans / 动态计划",
                View::Semantic => "Ontology / 本体与工具",
                View::Packs => "Capabilities / 能力包",
                View::Deploy => "System Ops / 系统运维",
                View::Settings => "Settings / 系统设置",
            },
            UiLang::Zh => match self {
                View::Overview => "总览 / Overview",
                View::Wizard => "系统设置 / Setup",
                View::Agents => "托管智能体 / Managed Agents",
                View::Board => "任务板 / Task Board",
                View::Workflows => "运行与任务 / Runs & Tasks",
                View::Dynamic => "动态计划 / Dynamic Plans",
                View::Semantic => "本体与工具 / Ontology",
                View::Packs => "能力包 / Capabilities",
                View::Deploy => "系统运维 / System Ops",
                View::Settings => "系统设置 / Settings",
            },
        }
    }

    fn from_id(value: &str) -> View {
        debug_assert!(Self::ALL.contains(&View::Overview));
        match value {
            "managed-agents" | "agents" => View::Agents,
            "runs-tasks" | "runs" | "tasks" | "workflows" => View::Workflows,
            "dynamic" => View::Dynamic,
            "board" => View::Board,
            "ontology" | "semantic" => View::Semantic,
            "capabilities" | "packs" => View::Packs,
            "system-ops" | "deploy" => View::Deploy,
            "settings" => View::Settings,
            "wizard" => View::Wizard,
            "overview" => View::Overview,
            _ => View::Overview,
        }
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
    pub(crate) provider_runtime: ApiState<Value>,
    pub(crate) observability: ApiState<ObservabilitySummary>,
    pub(crate) capability_discovery: ApiState<CapabilityDiscovery>,
    pub(crate) usage: ApiState<Value>,
    pub(crate) usage_finance_operations: ApiState<Value>,
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
    pub(crate) ontology_releases: ApiState<Vec<OntologyRelease>>,
    pub(crate) semantic_retrieval_backends: ApiState<Value>,
}

impl ConsoleData {
    pub(crate) fn direct_session_launch_allowed(&self) -> bool {
        self.provider_runtime.status == LoadStatus::Ready
            && self.provider_runtime.data["agent_release_enforcement_required"].as_bool()
                != Some(true)
    }
}

pub(crate) fn storage_get(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

pub(crate) fn initial_active_view() -> View {
    if let Some(hash_view) = location_hash_view() {
        storage_set("mandoforge.activeView", hash_view.id());
        return hash_view;
    }

    let stored = storage_get("mandoforge.activeView");
    let migrated = storage_get("mandoforge.overviewDefaultMigrated").is_some();
    if stored.as_deref() == Some("agents") && !migrated {
        persist_active_view(View::Overview);
        storage_set("mandoforge.overviewDefaultMigrated", "1");
        return View::Overview;
    }
    stored
        .as_deref()
        .map(View::from_id)
        .unwrap_or(View::Overview)
}

fn location_hash_view() -> Option<View> {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    let id = hash.trim_start_matches('#').trim();
    if id.is_empty() {
        None
    } else {
        Some(View::from_id(id))
    }
}

pub(crate) fn initial_critical_notifications_muted() -> bool {
    matches!(
        storage_get("mandoforge.criticalNotificationsMuted").as_deref(),
        Some("1" | "true" | "muted")
    )
}

pub(crate) fn initial_ui_lang() -> UiLang {
    match storage_get("mandoforge.uiLang").as_deref() {
        Some("zh" | "zh-CN" | "中文") => UiLang::Zh,
        _ => UiLang::En,
    }
}

pub(crate) fn storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

pub(crate) fn persist_active_view(view: View) {
    storage_set("mandoforge.activeView", view.id());
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(view.id());
    }
}
