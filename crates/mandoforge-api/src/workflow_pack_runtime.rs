use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path as FsPath, PathBuf},
};

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::*;

#[derive(Clone)]
pub(crate) struct WorkflowPackAgentRuntimeTarget {
    pub(crate) agent: Agent,
    pub(crate) version: AgentVersion,
}

pub(crate) struct WorkflowPackRuntimeMaterialization {
    pub(crate) bindings: Vec<WorkflowPackBinding>,
    pub(crate) agents: Vec<(Agent, AgentVersion)>,
    pub(crate) workflow_definitions: Vec<WorkflowDefinition>,
}

pub(crate) fn load_and_validate_workflow_pack(
    manifest_path: &str,
) -> Result<
    (
        PathBuf,
        workflow_pack::WorkflowPackManifest,
        workflow_pack::WorkflowPackValidationReport,
    ),
    AppError,
> {
    let manifest_path = resolve_workflow_pack_manifest_path(manifest_path)?;
    let input = std::fs::read_to_string(&manifest_path)?;
    let manifest = workflow_pack::WorkflowPackManifest::from_yaml_str(&input)
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    let package_dir = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let report = manifest
        .validate_package_dir(package_dir)
        .map_err(|error| AppError::bad_request(error.to_string()))?;
    Ok((manifest_path, manifest, report))
}

pub(crate) fn assess_workflow_pack_onboarding(
    installation: &WorkflowPackInstallation,
    persisted_profiles: &[WorkflowPackProfileAsset],
    input: WorkflowPackOnboardingAssessmentRequest,
) -> Result<WorkflowPackOnboardingAssessment, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let onboarding = manifest.onboarding.as_ref().ok_or_else(|| {
        AppError::bad_request("workflow pack manifest missing onboarding contract")
    })?;

    let mut input_profiles = HashMap::new();
    for profile in input.profiles {
        let profile_id = profile.id.trim().to_string();
        if profile_id.is_empty() {
            return Err(AppError::bad_request("onboarding profile id is required"));
        }
        if input_profiles.insert(profile_id.clone(), profile).is_some() {
            return Err(AppError::bad_request(format!(
                "duplicate onboarding profile {}",
                profile_id
            )));
        }
    }

    let mut merged_profiles: HashMap<String, WorkflowPackOnboardingProfileInput> =
        persisted_profiles
            .iter()
            .map(|profile| {
                (
                    profile.profile_id.clone(),
                    WorkflowPackOnboardingProfileInput {
                        id: profile.profile_id.clone(),
                        content: profile.content.clone(),
                    },
                )
            })
            .collect();
    for (profile_id, profile) in &input_profiles {
        merged_profiles.insert(profile_id.clone(), profile.clone());
    }

    let mut input_connectors = HashMap::new();
    for connector in input.connectors {
        let connector_id = connector.id.trim().to_string();
        if connector_id.is_empty() {
            return Err(AppError::bad_request("onboarding connector id is required"));
        }
        if input_connectors
            .insert(connector_id.clone(), connector)
            .is_some()
        {
            return Err(AppError::bad_request(format!(
                "duplicate onboarding connector {}",
                connector_id
            )));
        }
    }

    let profile_refs: HashMap<_, _> = manifest
        .profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect();
    let mut missing_profiles = Vec::new();
    let mut placeholder_profiles = Vec::new();
    for profile_id in &onboarding.required_profiles {
        match merged_profiles.get(profile_id) {
            Some(profile) if !profile.content.trim().is_empty() => {
                let declared = profile_refs.get(profile_id.as_str()).ok_or_else(|| {
                    AppError::bad_request(format!(
                        "workflow pack onboarding profile {} is not declared in manifest",
                        profile_id
                    ))
                })?;
                let default_profile_content =
                    std::fs::read_to_string(package_dir.join(&declared.path)).with_context(
                        || format!("read workflow pack profile template {}", declared.path),
                    )?;
                if profile.content.trim() == default_profile_content.trim() {
                    placeholder_profiles.push(profile_id.clone());
                }
            }
            _ => missing_profiles.push(profile_id.clone()),
        }
    }

    let mut connector_blockers = Vec::new();
    let mut ready_connector_count = 0usize;
    for connector in &manifest.connectors {
        let mut blockers = Vec::new();
        match input_connectors.get(connector.id.as_str()) {
            Some(candidate) => {
                let permissions: HashSet<_> = candidate
                    .available_permissions
                    .iter()
                    .map(|permission| permission.trim().to_string())
                    .collect();
                for permission in &connector.required_permissions {
                    if !permissions.contains(permission) {
                        blockers.push(format!("missing permission {}", permission));
                    }
                }
                if connector.provenance.required && !candidate.provenance_attested {
                    blockers.push("connector provenance is not attested".to_string());
                }
                if candidate
                    .tenant_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    blockers.push("tenant scope tenant_id is missing".to_string());
                }
                if candidate
                    .workspace_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    blockers.push("tenant scope workspace_id is missing".to_string());
                }
                if connector.prompt_injection_boundary.treat_results_as_data
                    && !candidate.treats_results_as_data
                {
                    blockers.push("connector results are not treated as data".to_string());
                }
                if connector.writes.enabled && !candidate.writes_enabled {
                    blockers.push("connector write capability is not enabled".to_string());
                }
                if connector.writes.enabled
                    && connector.writes.approval_required
                    && !candidate.write_approval_required
                {
                    blockers.push("connector write approval gate is missing".to_string());
                }
            }
            None => blockers.push("connector assessment is missing".to_string()),
        }

        blockers.sort();
        if blockers.is_empty() {
            ready_connector_count += 1;
        }
        connector_blockers.push(WorkflowPackConnectorAssessment {
            id: connector.id.clone(),
            status: if blockers.is_empty() {
                "ready".to_string()
            } else {
                "blocked".to_string()
            },
            blockers,
        });
    }
    connector_blockers.sort_by(|left, right| left.id.cmp(&right.id));

    missing_profiles.sort();
    placeholder_profiles.sort();

    let mut blockers = Vec::new();
    blockers.extend(
        missing_profiles
            .iter()
            .map(|profile| format!("missing required profile {}", profile)),
    );
    blockers.extend(
        placeholder_profiles
            .iter()
            .map(|profile| format!("profile {} still matches package default", profile)),
    );
    for connector in &connector_blockers {
        for blocker in &connector.blockers {
            blockers.push(format!("connector {}: {}", connector.id, blocker));
        }
    }

    let checked_at = Utc::now();
    Ok(WorkflowPackOnboardingAssessment {
        installation_id: installation.id,
        pack_id: installation.pack_id.clone(),
        version: installation.version.clone(),
        status: if blockers.is_empty() {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        onboarding_workflow: onboarding.workflow.clone(),
        onboarding_eval: onboarding.eval.clone(),
        required_profile_count: onboarding.required_profiles.len(),
        profile_schema_count: onboarding.profile_schemas.len(),
        inline_profile_count: input_profiles.len(),
        persisted_profile_count: persisted_profiles.len(),
        provided_profile_count: merged_profiles.len(),
        placeholder_profile_count: placeholder_profiles.len(),
        connector_requirement_count: manifest.connectors.len(),
        ready_connector_count,
        missing_profiles,
        placeholder_profiles,
        connector_blockers,
        blockers,
        checked_at,
    })
}

pub(crate) fn workflow_pack_manifest_and_dir_from_installation(
    installation: &WorkflowPackInstallation,
) -> Result<(workflow_pack::WorkflowPackManifest, PathBuf), AppError> {
    let manifest = serde_json::from_value::<workflow_pack::WorkflowPackManifest>(
        installation.manifest.clone(),
    )?;
    let manifest_path = PathBuf::from(&installation.manifest_path);
    let package_dir = manifest_path.parent().ok_or_else(|| {
        AppError::bad_request("workflow pack manifest path has no parent package directory")
    })?;
    Ok((manifest, package_dir.to_path_buf()))
}

pub(crate) fn workflow_pack_default_profile_assets(
    manifest: &workflow_pack::WorkflowPackManifest,
    manifest_path: &std::path::Path,
) -> Result<Vec<(String, String)>, AppError> {
    let onboarding = manifest.onboarding.as_ref().ok_or_else(|| {
        AppError::bad_request("workflow pack manifest missing onboarding contract")
    })?;
    let package_dir = manifest_path.parent().ok_or_else(|| {
        AppError::bad_request("workflow pack manifest path has no parent package directory")
    })?;
    let profile_refs: HashMap<_, _> = manifest
        .profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect();
    onboarding
        .required_profiles
        .iter()
        .map(|profile_id| {
            let declared = profile_refs.get(profile_id.as_str()).ok_or_else(|| {
                AppError::bad_request(format!(
                    "workflow pack onboarding profile {} is not declared in manifest",
                    profile_id
                ))
            })?;
            let content =
                std::fs::read_to_string(package_dir.join(&declared.path)).with_context(|| {
                    format!("read workflow pack profile template {}", declared.path)
                })?;
            Ok((profile_id.clone(), content))
        })
        .collect()
}

pub(crate) async fn workflow_pack_materialized_bindings_with_runtime_targets(
    state: &AppState,
    installation: &WorkflowPackInstallation,
    profile_assets: &[WorkflowPackProfileAsset],
) -> Result<WorkflowPackRuntimeMaterialization, AppError> {
    let mut bindings = workflow_pack_materialized_bindings(installation, profile_assets, "staged")?;
    let agent_targets = workflow_pack_materialize_agents(state, installation).await?;
    let workflow_definitions =
        workflow_pack_materialize_workflow_definitions(installation, &agent_targets)?;
    for binding in &mut bindings {
        match binding.binding_type.as_str() {
            "agent" => {
                let Some(target) = agent_targets.get(&binding.binding_key) else {
                    continue;
                };
                binding.target_id = Some(target.version.id);
                let Value::Object(payload) = &mut binding.materialized_payload else {
                    continue;
                };
                payload.insert("agent_id".to_string(), json!(target.agent.id));
                payload.insert("agent_version_id".to_string(), json!(target.version.id));
                payload.insert("version".to_string(), json!(target.version.version));
                payload.insert(
                    "release_state".to_string(),
                    json!(target.agent.release_state),
                );
            }
            "workflow" => {
                let Some(definition) = workflow_definitions.get(&binding.binding_key) else {
                    continue;
                };
                binding.target_id = Some(definition.id);
                let Value::Object(payload) = &mut binding.materialized_payload else {
                    continue;
                };
                payload.insert("workflow_definition_id".to_string(), json!(definition.id));
                payload.insert("release_state".to_string(), json!(definition.release_state));
                payload.insert(
                    "execution_strategy".to_string(),
                    json!(definition.execution_strategy),
                );
                payload.insert(
                    "runtime_adapter".to_string(),
                    json!(definition.runtime_adapter),
                );
                payload.insert("runtime_mode".to_string(), json!(definition.runtime_mode));
                payload.insert(
                    "step_count".to_string(),
                    json!(
                        definition
                            .step_graph
                            .get("steps")
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len)
                    ),
                );
            }
            _ => {}
        }
    }
    Ok(WorkflowPackRuntimeMaterialization {
        bindings,
        agents: agent_targets
            .into_values()
            .map(|target| (target.agent, target.version))
            .collect(),
        workflow_definitions: workflow_definitions.into_values().collect(),
    })
}

pub(crate) async fn workflow_pack_materialize_agents(
    state: &AppState,
    installation: &WorkflowPackInstallation,
) -> Result<BTreeMap<String, WorkflowPackAgentRuntimeTarget>, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let default_agent = workflow_pack_materialization_default_agent(state).await?;
    let base_version = state.current_agent_version(default_agent.id).await?;
    let semantic_scopes = merge_semantic_scopes(
        &json!(manifest.semantic_scopes),
        &json!({"pack_id": manifest.id}),
    );
    let agent_ids = manifest
        .agents
        .iter()
        .map(|agent| (agent.id.clone(), Uuid::new_v4()))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::new();

    for agent_ref in &manifest.agents {
        let contract = workflow_pack_load_agent_file(&package_dir, agent_ref)?;
        let tools = workflow_pack_agent_tools(agent_ref);
        let tool_policy = workflow_pack_agent_tool_policy(agent_ref);
        let allowed_targets = agent_ref
            .handoffs
            .iter()
            .map(|handoff| {
                let target_agent_id = agent_ids.get(&handoff.target_agent).ok_or_else(|| {
                    AppError::bad_request(format!(
                        "workflow pack agent {} handoff target {} was not declared",
                        agent_ref.id, handoff.target_agent
                    ))
                })?;
                Ok(json!({
                    "target_agent_id": target_agent_id,
                    "target_agent_ref": handoff.target_agent,
                    "intents": handoff.intents,
                    "risk_levels": [workflow_pack_risk_level_slug(&handoff.risk_level)],
                    "approval_required": handoff.approval_required,
                    "schema_ref": handoff.schema,
                }))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        let runtime_config = workflow_pack_agent_runtime_config(
            &base_version.runtime_config,
            installation,
            agent_ref,
            allowed_targets,
        )?;
        let agent_id = *agent_ids.get(&agent_ref.id).ok_or_else(|| {
            AppError::bad_request(format!(
                "workflow pack agent {} has no materialization id",
                agent_ref.id
            ))
        })?;
        let (agent, version) = state
            .prepare_agent_with_id(
                agent_id,
                CreateAgent {
                    name: format!("{} / {}", manifest.name, agent_ref.id),
                    kind: agent_ref.role.as_slug().to_string(),
                    provider: base_version.provider.clone(),
                    model: base_version.model.clone(),
                    team_id: default_agent.team_id,
                    project_id: default_agent.project_id,
                    runtime_profile_id: base_version.runtime_profile_id,
                    agent_role: workflow_pack_runtime_agent_role(agent_ref.role).to_string(),
                    system_prompt: contract.instructions,
                    runtime_config,
                    tools,
                    tool_policy,
                    mcp_server_ids: Vec::new(),
                    skill_ids: Vec::new(),
                    workflow_pack_ids: vec![manifest.id.clone()],
                    remote_computer_profile: base_version.remote_computer_profile.clone(),
                    semantic_scopes: semantic_scopes.clone(),
                    release_state: "draft".to_string(),
                },
            )
            .await?;
        targets.insert(
            agent_ref.id.clone(),
            WorkflowPackAgentRuntimeTarget { agent, version },
        );
    }
    Ok(targets)
}

pub(crate) fn workflow_pack_load_agent_file(
    package_dir: &FsPath,
    agent: &workflow_pack::AgentRef,
) -> Result<workflow_pack::AgentFileContract, AppError> {
    let content = std::fs::read_to_string(package_dir.join(&agent.path))
        .with_context(|| format!("read workflow pack agent {}", agent.path))?;
    let contract =
        serde_yaml::from_str::<workflow_pack::AgentFileContract>(&content).map_err(|error| {
            AppError::bad_request(format!(
                "workflow pack agent {} is invalid YAML: {error}",
                agent.path
            ))
        })?;
    if contract.id != agent.id || contract.role != agent.role {
        return Err(AppError::bad_request(format!(
            "workflow pack agent {} does not match its manifest declaration",
            agent.id
        )));
    }
    Ok(contract)
}

pub(crate) fn workflow_pack_agent_tools(agent: &workflow_pack::AgentRef) -> Vec<String> {
    agent
        .tool_scope
        .read
        .iter()
        .chain(&agent.tool_scope.write)
        .chain(&agent.tool_scope.external_write)
        .flat_map(|scope| workflow_pack_runtime_tools_for_scope(scope))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn workflow_pack_agent_tool_policy(agent: &workflow_pack::AgentRef) -> Value {
    json!({
        "source": "workflow_pack",
        "tool_scope": agent.tool_scope,
        "runtime_tool_scope": workflow_pack_runtime_tool_scope(&agent.tool_scope),
        "approval_required_for_external_write": !agent.tool_scope.external_write.is_empty(),
    })
}

pub(crate) fn workflow_pack_runtime_tools_for_scope(scope: &str) -> Vec<String> {
    match scope {
        "connector.read" | "profile.read" => ["semantic_object.fetch", "semantic_object.search"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        "schema.read" => [
            "ontology_type.lookup",
            "semantic_object.fetch",
            "semantic_object.search",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        "artifact.write" => vec!["artifact.create".to_string()],
        runtime_tool => vec![runtime_tool.to_string()],
    }
}

pub(crate) fn workflow_pack_runtime_tool_scope(scope: &workflow_pack::ToolScope) -> Value {
    let map_scopes = |scopes: &[String]| {
        scopes
            .iter()
            .flat_map(|scope| workflow_pack_runtime_tools_for_scope(scope))
            .collect::<BTreeSet<_>>()
    };
    json!({
        "read": map_scopes(&scope.read),
        "write": map_scopes(&scope.write),
        "external_write": map_scopes(&scope.external_write),
    })
}

pub(crate) fn workflow_pack_agent_runtime_config(
    base: &Value,
    installation: &WorkflowPackInstallation,
    agent: &workflow_pack::AgentRef,
    allowed_targets: Vec<Value>,
) -> Result<Value, AppError> {
    let mut runtime_config = base.clone();
    let object = runtime_config.as_object_mut().ok_or_else(|| {
        AppError::bad_request("workflow pack base agent runtime_config must be an object")
    })?;
    object.insert(
        "workflow_pack".to_string(),
        json!({
            "installation_id": installation.id,
            "pack_id": installation.pack_id,
            "pack_version": installation.version,
            "agent_ref": agent.id,
            "role": agent.role,
        }),
    );
    object.insert(
        "handoffs".to_string(),
        json!({"allowed_targets": allowed_targets}),
    );
    Ok(runtime_config)
}

pub(crate) fn workflow_pack_runtime_agent_role(role: workflow_pack::AgentRole) -> &'static str {
    match role {
        workflow_pack::AgentRole::Manager | workflow_pack::AgentRole::Orchestrator => "manager",
        _ => "specialist",
    }
}

pub(crate) fn workflow_pack_risk_level_slug(risk_level: &workflow_pack::RiskLevel) -> &'static str {
    match risk_level {
        workflow_pack::RiskLevel::Low => "low",
        workflow_pack::RiskLevel::Medium => "medium",
        workflow_pack::RiskLevel::High => "high",
    }
}

pub(crate) fn workflow_pack_materialize_workflow_definitions(
    installation: &WorkflowPackInstallation,
    agent_targets: &BTreeMap<String, WorkflowPackAgentRuntimeTarget>,
) -> Result<BTreeMap<String, WorkflowDefinition>, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let mut definitions = BTreeMap::new();
    for workflow in &manifest.workflows {
        let workflow_file = workflow_pack_load_workflow_file(&package_dir, workflow)?;
        let declared_id = workflow_file
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if declared_id.is_some_and(|declared_id| declared_id != workflow.id) {
            return Err(AppError::bad_request(format!(
                "workflow pack workflow {} file declares mismatched id",
                workflow.id
            )));
        }
        let execution_strategy = workflow_pack_workflow_execution_strategy(&workflow_file)?;
        let runtime_adapter = workflow_pack_workflow_runtime_adapter(&workflow_file)?;
        let runtime_mode = workflow_pack_workflow_runtime_mode(&workflow_file)?;
        let runtime_capability_contract =
            workflow_pack_workflow_runtime_capability_contract(&workflow_file)?;
        let event_ingestion_policy = workflow_pack_workflow_event_ingestion_policy(&workflow_file)?;
        let semantic_scopes = workflow_pack_workflow_semantic_scopes(&manifest, &workflow_file)?;
        validate_workflow_execution_binding(
            &execution_strategy,
            runtime_adapter.as_deref(),
            &runtime_capability_contract,
        )?;
        let mut step_graph = workflow_definition_step_graph_for_execution(
            &execution_strategy,
            &workflow_pack_workflow_step_graph(&manifest, workflow, &workflow_file)?,
        );
        workflow_pack_bind_step_agents(&mut step_graph, workflow, agent_targets)?;
        workflow_graph_start_steps(&step_graph)?;
        let default_agent_id = agent_targets
            .get(&workflow.entry_agent)
            .map(|target| target.agent.id)
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "workflow pack workflow {} entry agent {} was not materialized",
                    workflow.id, workflow.entry_agent
                ))
            })?;
        let trigger_type = normalize_workflow_trigger_type(
            workflow_file.trigger_type.as_deref().unwrap_or("manual"),
        )?;
        let eval_gate_refs = if workflow_file.eval_gate_refs.is_empty() {
            manifest
                .evals
                .iter()
                .filter(|eval| eval.gate.required)
                .map(|eval| eval.id.clone())
                .collect::<Vec<_>>()
        } else {
            workflow_file.eval_gate_refs.clone()
        };
        let now = Utc::now();
        let definition = WorkflowDefinition {
            id: Uuid::new_v4(),
            pack_installation_id: Some(installation.id),
            pack_id: Some(installation.pack_id.clone()),
            pack_version: Some(installation.version.clone()),
            name: workflow_file
                .name
                .clone()
                .unwrap_or_else(|| workflow.id.clone()),
            entrypoint: workflow.id.clone(),
            trigger_type,
            default_agent_id,
            default_environment_id: None,
            input_schema_ref: workflow_file.input_schema_ref.clone(),
            output_schema_ref: workflow_pack_workflow_output_schema_ref(&workflow_file),
            step_graph,
            handoff_rules: workflow_pack_workflow_handoff_rules(
                &manifest,
                workflow,
                &workflow_file,
                &semantic_scopes,
                &package_dir,
            )?,
            execution_strategy,
            runtime_adapter,
            runtime_mode,
            runtime_capability_contract,
            event_ingestion_policy,
            approval_policy_ref: workflow_file.approval_policy_ref.clone(),
            eval_gate_refs,
            release_state: "staged".to_string(),
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        definitions.insert(workflow.id.clone(), definition);
    }
    Ok(definitions)
}

pub(crate) async fn workflow_pack_materialization_default_agent(
    state: &AppState,
) -> Result<Agent, AppError> {
    let mut agents = state.list_agents().await?;
    agents.sort_by_key(|agent| agent.created_at);
    agents
        .iter()
        .find(|agent| agent.release_state == "active" && agent.agent_role == "manager")
        .cloned()
        .or_else(|| {
            agents
                .iter()
                .find(|agent| agent.release_state == "active")
                .cloned()
        })
        .or_else(|| agents.first().cloned())
        .ok_or_else(|| {
            AppError::bad_request("workflow pack staging requires at least one runtime agent")
        })
}

pub(crate) fn workflow_pack_load_workflow_file(
    package_dir: &FsPath,
    workflow: &workflow_pack::WorkflowRef,
) -> Result<WorkflowPackWorkflowFile, AppError> {
    let path = package_dir.join(&workflow.path);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read workflow pack workflow {}", workflow.path))?;
    serde_yaml::from_str::<WorkflowPackWorkflowFile>(&content).map_err(|error| {
        AppError::bad_request(format!(
            "workflow pack workflow {} is invalid YAML: {error}",
            workflow.path
        ))
    })
}

pub(crate) fn workflow_pack_load_action_type_file(
    package_dir: &FsPath,
    action: &workflow_pack::PackFileRef,
) -> Result<Value, AppError> {
    let path = package_dir.join(&action.path);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read workflow pack action {}", action.path))?;
    let value = serde_yaml::from_str::<Value>(&content).map_err(|error| {
        AppError::bad_request(format!(
            "workflow pack action {} is invalid YAML: {error}",
            action.path
        ))
    })?;
    if !value.is_object() {
        return Err(AppError::bad_request(format!(
            "workflow pack action {} must be a YAML object",
            action.path
        )));
    }
    if value.get("id").and_then(Value::as_str) != Some(action.id.as_str()) {
        return Err(AppError::bad_request(format!(
            "workflow pack action {} id must match manifest action {}",
            action.path, action.id
        )));
    }
    Ok(value)
}

pub(crate) fn workflow_pack_load_yaml_value(
    package_dir: &FsPath,
    relative_path: &str,
    label: &str,
) -> Result<Value, AppError> {
    let content = std::fs::read_to_string(package_dir.join(relative_path))
        .with_context(|| format!("read workflow pack {label} {}", relative_path))?;
    serde_yaml::from_str::<Value>(&content).map_err(|error| {
        AppError::bad_request(format!(
            "workflow pack {label} {} is invalid YAML: {error}",
            relative_path
        ))
    })
}

pub(crate) fn workflow_pack_value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn workflow_pack_value_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(crate) fn workflow_pack_value_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn workflow_pack_workflow_step_graph(
    manifest: &workflow_pack::WorkflowPackManifest,
    workflow: &workflow_pack::WorkflowRef,
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<Value, AppError> {
    let mut graph_steps = Vec::new();
    let mut used_keys = BTreeSet::new();
    let mut previous_key: Option<String> = None;
    let mut previous_agent_ref: Option<String> = None;
    for (index, step) in workflow_file.steps.iter().enumerate() {
        let step_key = workflow_pack_workflow_step_key(step, index, &mut used_keys);
        let mut graph_step = serde_json::Map::new();
        graph_step.insert("key".to_string(), json!(step_key));
        graph_step.insert("type".to_string(), json!("agent"));
        let agent_ref = workflow_pack_workflow_step_string(step, "agent")
            .unwrap_or_else(|| workflow.entry_agent.clone());
        graph_step.insert("workflow_agent_ref".to_string(), json!(agent_ref));
        if index == 0 {
            graph_step.insert("start".to_string(), json!(true));
        } else if let Some(previous_key) = &previous_key {
            graph_step.insert("depends_on".to_string(), json!([previous_key]));
        }
        for key in [
            "task",
            "output_schema",
            "handoff_intent",
            "required_profiles",
            "required_schemas",
            "skills",
            "risk_level",
        ] {
            if let Some(value) = step.get(key) {
                graph_step.insert(key.to_string(), value.clone());
            }
        }
        if let Some(source_agent_ref) = &previous_agent_ref
            && (source_agent_ref != &agent_ref || graph_step.contains_key("handoff_intent"))
        {
            let intent = graph_step.get("handoff_intent").and_then(Value::as_str);
            let source_agent = manifest
                .agents
                .iter()
                .find(|agent| agent.id == *source_agent_ref)
                .ok_or_else(|| {
                    AppError::bad_request(format!(
                        "workflow pack workflow {} references unknown source agent {}",
                        workflow.id, source_agent_ref
                    ))
                })?;
            let candidates = source_agent
                .handoffs
                .iter()
                .filter(|handoff| handoff.target_agent == agent_ref)
                .collect::<Vec<_>>();
            let handoff = match intent {
                Some(intent) => {
                    let mut matches = candidates.iter().copied().filter(|handoff| {
                        handoff.intents.iter().any(|candidate| candidate == intent)
                    });
                    let handoff = matches.next().ok_or_else(|| {
                        AppError::bad_request(format!(
                            "workflow pack workflow {} handoff {} -> {} does not allow intent {}",
                            workflow.id, source_agent_ref, agent_ref, intent
                        ))
                    })?;
                    if matches.next().is_some() {
                        return Err(AppError::bad_request(format!(
                            "workflow pack workflow {} handoff {} -> {} intent {} is ambiguous",
                            workflow.id, source_agent_ref, agent_ref, intent
                        )));
                    }
                    handoff
                }
                None if candidates.len() == 1 => candidates[0],
                None => {
                    return Err(AppError::bad_request(format!(
                        "workflow pack workflow {} handoff {} -> {} requires handoff_intent",
                        workflow.id, source_agent_ref, agent_ref
                    )));
                }
            };
            graph_step.insert(
                "handoff_source_agent_ref".to_string(),
                json!(source_agent_ref),
            );
            graph_step.insert(
                "risk_level".to_string(),
                json!(workflow_pack_risk_level_slug(&handoff.risk_level)),
            );
            graph_step.insert(
                "approval_required".to_string(),
                json!(handoff.approval_required),
            );
            graph_step.insert("handoff_schema_ref".to_string(), json!(handoff.schema));
        }
        previous_agent_ref = Some(agent_ref);
        previous_key = Some(step_key);
        graph_steps.push(Value::Object(graph_step));
    }
    if graph_steps.is_empty() {
        graph_steps.push(json!({
            "key": workflow.entry_agent,
            "type": "agent",
            "workflow_agent_ref": workflow.entry_agent,
            "start": true
        }));
    }
    let generated_step_graph = json!({
        "source": "workflow_pack_file",
        "workflow_id": workflow.id,
        "steps": graph_steps
    });
    match &workflow_file.step_graph {
        Some(step_graph) => workflow_pack_apply_manifest_governance_to_step_graph(
            workflow,
            step_graph,
            &generated_step_graph,
        ),
        None => Ok(generated_step_graph),
    }
}

fn workflow_pack_apply_manifest_governance_to_step_graph(
    workflow: &workflow_pack::WorkflowRef,
    step_graph: &Value,
    generated_step_graph: &Value,
) -> Result<Value, AppError> {
    let explicit_start_keys = workflow_graph_start_steps(step_graph)?
        .into_iter()
        .map(workflow_graph_step_key)
        .collect::<Result<Vec<_>, _>>()?;
    let mut step_graph = step_graph.clone();
    let explicit_steps = step_graph
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            AppError::bad_request(format!(
                "workflow pack workflow {} step_graph must be a JSON object with steps",
                workflow.id
            ))
        })?;
    let generated_steps = generated_step_graph
        .get("steps")
        .and_then(Value::as_array)
        .expect("generated workflow pack graph always contains steps");
    let explicit_agent_step_indexes = explicit_steps
        .iter()
        .enumerate()
        .filter(|(_, step)| !workflow_graph_step_is_adapter_owned_compensation(step))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if explicit_agent_step_indexes.len() != generated_steps.len() {
        return Err(AppError::bad_request(format!(
            "workflow pack workflow {} step_graph must match its declared steps",
            workflow.id
        )));
    }
    let explicit_agent_step_keys = explicit_agent_step_indexes
        .iter()
        .map(|index| workflow_graph_step_key(&explicit_steps[*index]))
        .collect::<Result<Vec<_>, _>>()?;
    if explicit_start_keys != explicit_agent_step_keys[..1] {
        return Err(AppError::bad_request(format!(
            "workflow pack workflow {} step_graph must start with its declared entry agent",
            workflow.id
        )));
    }
    for (position, explicit_index) in explicit_agent_step_indexes.iter().enumerate() {
        let step = &explicit_steps[*explicit_index];
        let dependencies = workflow_graph_step_dependencies(step)?;
        let expected_dependencies = position
            .checked_sub(1)
            .map(|previous| vec![explicit_agent_step_keys[previous].clone()])
            .unwrap_or_default();
        let explicitly_starts = step
            .get("start")
            .or_else(|| step.get("entrypoint"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if dependencies != expected_dependencies || (position > 0 && explicitly_starts) {
            return Err(AppError::bad_request(format!(
                "workflow pack workflow {} step_graph must preserve its declared linear handoff topology",
                workflow.id
            )));
        }
    }

    for (generated_step, explicit_index) in generated_steps.iter().zip(explicit_agent_step_indexes)
    {
        let explicit_step = &mut explicit_steps[explicit_index];
        let expected_agent_ref =
            workflow_pack_workflow_step_string(generated_step, "workflow_agent_ref")
                .expect("generated workflow pack agent step always declares workflow_agent_ref");
        let actual_agent_ref =
            workflow_pack_workflow_step_string(explicit_step, "workflow_agent_ref")
                .or_else(|| workflow_pack_workflow_step_string(explicit_step, "agent_ref"))
                .or_else(|| workflow_pack_workflow_step_string(explicit_step, "agent"))
                .unwrap_or_else(|| workflow.entry_agent.clone());
        if actual_agent_ref != expected_agent_ref {
            return Err(AppError::bad_request(format!(
                "workflow pack workflow {} step_graph agent {} does not match declared agent {}",
                workflow.id, actual_agent_ref, expected_agent_ref
            )));
        }
        let explicit_step = explicit_step.as_object_mut().ok_or_else(|| {
            AppError::bad_request("workflow pack step_graph steps must be JSON objects")
        })?;
        explicit_step.insert("workflow_agent_ref".to_string(), json!(expected_agent_ref));
        for key in [
            "handoff_source_agent_ref",
            "handoff_intent",
            "risk_level",
            "approval_required",
            "handoff_schema_ref",
        ] {
            match generated_step.get(key) {
                Some(value) => {
                    explicit_step.insert(key.to_string(), value.clone());
                }
                None => {
                    explicit_step.remove(key);
                }
            }
        }
    }
    Ok(step_graph)
}

pub(crate) fn workflow_pack_bind_step_agents(
    step_graph: &mut Value,
    workflow: &workflow_pack::WorkflowRef,
    agent_targets: &BTreeMap<String, WorkflowPackAgentRuntimeTarget>,
) -> Result<(), AppError> {
    let steps = step_graph
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| AppError::bad_request("workflow pack step graph must declare steps"))?;
    for step in steps {
        if workflow_graph_step_is_adapter_owned_compensation(step) {
            continue;
        }
        let agent_ref = workflow_pack_workflow_step_string(step, "workflow_agent_ref")
            .or_else(|| workflow_pack_workflow_step_string(step, "agent_ref"))
            .or_else(|| workflow_pack_workflow_step_string(step, "agent"))
            .unwrap_or_else(|| workflow.entry_agent.clone());
        let target = agent_targets.get(&agent_ref).ok_or_else(|| {
            AppError::bad_request(format!(
                "workflow pack workflow {} references unmaterialized agent {}",
                workflow.id, agent_ref
            ))
        })?;
        let object = step.as_object_mut().ok_or_else(|| {
            AppError::bad_request("workflow pack graph steps must be JSON objects")
        })?;
        object.insert("workflow_agent_ref".to_string(), json!(agent_ref));
        object.insert("agent_id".to_string(), json!(target.agent.id));
        object.insert("agent_version_id".to_string(), json!(target.version.id));
    }
    Ok(())
}

pub(crate) fn workflow_pack_workflow_step_key(
    step: &Value,
    index: usize,
    used_keys: &mut BTreeSet<String>,
) -> String {
    let base = workflow_pack_workflow_step_string(step, "key")
        .or_else(|| workflow_pack_workflow_step_string(step, "handoff_intent"))
        .or_else(|| workflow_pack_workflow_step_string(step, "agent"))
        .unwrap_or_else(|| format!("step-{}", index + 1));
    let base = base
        .chars()
        .map(|item| {
            if item.is_ascii_alphanumeric() || item == '-' || item == '_' {
                item.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() {
        format!("step-{}", index + 1)
    } else {
        base
    };
    if used_keys.insert(base.clone()) {
        return base;
    }
    for suffix in 2usize.. {
        let candidate = format!("{base}-{suffix}");
        if used_keys.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("workflow step key suffix search is unbounded")
}

pub(crate) fn workflow_pack_workflow_step_string(step: &Value, key: &str) -> Option<String> {
    step.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn workflow_pack_workflow_output_schema_ref(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Option<String> {
    workflow_file.output_schema_ref.clone().or_else(|| {
        workflow_file
            .steps
            .iter()
            .find_map(|step| workflow_pack_workflow_step_string(step, "output_schema"))
    })
}

pub(crate) fn workflow_pack_workflow_execution_strategy(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<String, AppError> {
    let value = workflow_file
        .execution
        .get("strategy")
        .or_else(|| workflow_file.execution.get("execution_strategy"))
        .and_then(Value::as_str)
        .unwrap_or(&workflow_file.execution_strategy);
    normalize_workflow_execution_strategy(value)
}

pub(crate) fn workflow_pack_workflow_runtime_adapter(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<Option<String>, AppError> {
    let value = workflow_file
        .execution
        .get("runtime_adapter")
        .or_else(|| workflow_file.execution.get("adapter"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| workflow_file.runtime_adapter.clone());
    normalize_optional_runtime_adapter(value)
}

pub(crate) fn workflow_pack_workflow_runtime_mode(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<Option<String>, AppError> {
    let value = workflow_file
        .execution
        .get("runtime_mode")
        .or_else(|| workflow_file.execution.get("mode"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| workflow_file.runtime_mode.clone());
    normalize_optional_runtime_mode(value)
}

pub(crate) fn workflow_pack_workflow_runtime_capability_contract(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<Value, AppError> {
    let contract = workflow_file
        .execution
        .get("runtime_capability_contract")
        .or_else(|| workflow_file.execution.get("capability_contract"))
        .cloned()
        .unwrap_or_else(|| workflow_file.runtime_capability_contract.clone());
    if !contract.is_object() {
        return Err(AppError::bad_request(
            "workflow pack workflow runtime_capability_contract must be a JSON object",
        ));
    }
    Ok(contract)
}

pub(crate) fn workflow_pack_workflow_event_ingestion_policy(
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<String, AppError> {
    let value = workflow_file
        .execution
        .get("event_ingestion_policy")
        .or_else(|| workflow_file.execution.get("event_ingestion"))
        .and_then(Value::as_str)
        .unwrap_or(&workflow_file.event_ingestion_policy);
    normalize_event_ingestion_policy(value)
}

pub(crate) fn workflow_pack_workflow_semantic_scopes(
    manifest: &workflow_pack::WorkflowPackManifest,
    workflow_file: &WorkflowPackWorkflowFile,
) -> Result<Value, AppError> {
    if !workflow_file.semantic_scopes.is_object() {
        return Err(AppError::bad_request(
            "workflow pack workflow semantic_scopes must be a JSON object",
        ));
    }
    let pack_scopes = json!(manifest.semantic_scopes);
    Ok(merge_semantic_scopes(
        &pack_scopes,
        &workflow_file.semantic_scopes,
    ))
}

pub(crate) fn workflow_pack_workflow_handoff_rules(
    manifest: &workflow_pack::WorkflowPackManifest,
    workflow: &workflow_pack::WorkflowRef,
    workflow_file: &WorkflowPackWorkflowFile,
    semantic_scopes: &Value,
    package_dir: &FsPath,
) -> Result<Value, AppError> {
    let tool_scope = workflow_pack_workflow_tool_scope(manifest, workflow, workflow_file);
    let external_connector_grant = tool_scope["external_write"]
        .as_array()
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.as_str() == Some("native.connector.call"))
        })
        .then(|| workflow_pack_external_connector_grant(manifest, package_dir))
        .transpose()?;
    let mut rules = if workflow_file.handoff_rules.is_object() {
        workflow_file.handoff_rules.clone()
    } else {
        json!({
        "source": "workflow_pack_file",
        "workflow_id": workflow.id,
        "entry_agent": workflow.entry_agent,
        "steps": workflow_file.steps.clone(),
        "approval": workflow_file.approval.clone(),
        "output": workflow_file.output.clone(),
        "outputs": workflow_file.outputs.clone()
        })
    };
    if let Value::Object(rules_object) = &mut rules {
        let root_task_grant = rules_object
            .entry("root_task_grant".to_string())
            .or_insert_with(empty_json_object);
        if !root_task_grant.is_object() {
            *root_task_grant = empty_json_object();
        }
        if let Some(root_task_grant) = root_task_grant.as_object_mut() {
            root_task_grant
                .entry("semantic_scopes".to_string())
                .or_insert_with(|| semantic_scopes.clone());
            root_task_grant
                .entry("memory_scope".to_string())
                .or_insert_with(workflow_pack_root_task_grant_memory_scope);
            root_task_grant
                .entry("tool_scope".to_string())
                .or_insert_with(|| tool_scope.clone());
            if let Some((connector_scope, external_effects)) = &external_connector_grant {
                root_task_grant
                    .entry("connector_scope".to_string())
                    .or_insert_with(|| connector_scope.clone());
                root_task_grant
                    .entry("external_effects".to_string())
                    .or_insert_with(|| external_effects.clone());
            }
        }
    }
    Ok(rules)
}

fn workflow_pack_external_connector_grant(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
) -> Result<(Value, Value), AppError> {
    let connector_ids = manifest
        .connectors
        .iter()
        .filter(|connector| {
            connector.writes.enabled
                && matches!(&connector.kind, workflow_pack::ConnectorKind::Native)
        })
        .map(|connector| connector.id.clone())
        .collect::<BTreeSet<_>>();
    if connector_ids.is_empty() {
        return Err(AppError::bad_request(
            "workflow pack external-write agents require a writable native connector",
        ));
    }

    let mut operation_ids = BTreeSet::new();
    let mut side_effect_classes = BTreeSet::new();
    for action in &manifest.actions {
        let action_type = workflow_pack_load_action_type_file(package_dir, action)?;
        if workflow_pack_value_string(&action_type, "connector_id")
            .is_some_and(|connector_id| connector_ids.contains(&connector_id))
        {
            if let Some(operation_id) = workflow_pack_value_string(&action_type, "operation_id") {
                operation_ids.insert(operation_id);
            }
            if let Some(side_effect_class) =
                workflow_pack_value_string(&action_type, "side_effect_class")
            {
                side_effect_classes.insert(side_effect_class);
            }
        }
    }
    if operation_ids.is_empty() || side_effect_classes.is_empty() {
        return Err(AppError::bad_request(
            "workflow pack external-write agents require governed action side effects",
        ));
    }
    let external_effects = Value::Object(
        side_effect_classes
            .iter()
            .map(|side_effect_class| (side_effect_class.clone(), json!(true)))
            .collect(),
    );
    Ok((
        json!({
            "mode": "commit_write",
            "allowed_connector_ids": connector_ids,
            "allowed_tool_names": operation_ids,
            "tenant_scope": {},
            "side_effect_classes": side_effect_classes,
        }),
        external_effects,
    ))
}

pub(crate) fn workflow_pack_workflow_tool_scope(
    manifest: &workflow_pack::WorkflowPackManifest,
    workflow: &workflow_pack::WorkflowRef,
    workflow_file: &WorkflowPackWorkflowFile,
) -> Value {
    let mut agent_refs = BTreeSet::from([workflow.entry_agent.clone()]);
    for step in &workflow_file.steps {
        if let Some(agent_ref) = workflow_pack_workflow_step_string(step, "agent") {
            agent_refs.insert(agent_ref);
        }
    }
    if let Some(steps) = workflow_file
        .step_graph
        .as_ref()
        .and_then(|graph| graph.get("steps"))
        .and_then(Value::as_array)
    {
        for step in steps {
            if let Some(agent_ref) = workflow_pack_workflow_step_string(step, "workflow_agent_ref")
                .or_else(|| workflow_pack_workflow_step_string(step, "agent_ref"))
                .or_else(|| workflow_pack_workflow_step_string(step, "agent"))
            {
                agent_refs.insert(agent_ref);
            }
        }
    }
    loop {
        let reachable_targets = manifest
            .agents
            .iter()
            .filter(|agent| agent_refs.contains(&agent.id))
            .flat_map(|agent| {
                agent
                    .handoffs
                    .iter()
                    .map(|handoff| handoff.target_agent.clone())
            })
            .collect::<BTreeSet<_>>();
        let previous_count = agent_refs.len();
        agent_refs.extend(reachable_targets);
        if agent_refs.len() == previous_count {
            break;
        }
    }
    let mut read = BTreeSet::new();
    let mut write = BTreeSet::new();
    let mut external_write = BTreeSet::new();
    for agent in manifest
        .agents
        .iter()
        .filter(|agent| agent_refs.contains(&agent.id))
    {
        read.extend(
            agent
                .tool_scope
                .read
                .iter()
                .flat_map(|scope| workflow_pack_runtime_tools_for_scope(scope)),
        );
        write.extend(
            agent
                .tool_scope
                .write
                .iter()
                .flat_map(|scope| workflow_pack_runtime_tools_for_scope(scope)),
        );
        external_write.extend(
            agent
                .tool_scope
                .external_write
                .iter()
                .flat_map(|scope| workflow_pack_runtime_tools_for_scope(scope)),
        );
    }
    json!({
        "read": read,
        "write": write,
        "external_write": external_write,
    })
}

pub(crate) fn workflow_pack_root_task_grant_memory_scope() -> Value {
    json!({
        "mode": "snapshot_only",
        "allowed_scope_keys": ["domain_scope", "workflow_scope", "lane_scope", "share_policy", "pack_id"],
        "allowed_object_types": [
            "pack",
            "workflow",
            "agent",
            "connector",
            "profile",
            "schema",
            "skill",
            "policy",
            "eval",
            "release_gate",
            "memory"
        ],
        "allowed_source_types": ["workflow_pack", "memory"],
        "allowed_object_ids": [],
        "minimum_trust_level": "source_attested",
        "max_objects": 12,
        "approval_memory_allowed": false,
        "handoff_memory_allowed": false,
        "writeback_allowed": false
    })
}

pub(crate) fn workflow_pack_materialized_bindings(
    installation: &WorkflowPackInstallation,
    profile_assets: &[WorkflowPackProfileAsset],
    status: &str,
) -> Result<Vec<WorkflowPackBinding>, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let now = Utc::now();
    let mut bindings = Vec::new();

    for extension in &manifest.extends {
        bindings.push(new_workflow_pack_binding(
            installation,
            "pack_extension",
            &extension.id,
            None,
            "workflow_pack_dependency",
            status,
            json!({
                "id": extension.id,
                "version": extension.version,
                "required": extension.required,
                "semantic_scopes": extension.semantic_scopes,
            }),
            now,
        ));
    }

    for workflow in &manifest.workflows {
        let workflow_file = workflow_pack_load_workflow_file(&package_dir, workflow)?;
        let semantic_scopes = workflow_pack_workflow_semantic_scopes(&manifest, &workflow_file)?;
        let mut materialized_payload = json!({
            "entry_agent": workflow.entry_agent,
            "semantic_scopes": semantic_scopes,
            "source_digest": workflow_pack_source_digest(&package_dir, &workflow.path)?,
        });
        if workflow_file
            .semantic_synthesis_schedule
            .as_object()
            .is_some_and(|schedule| !schedule.is_empty())
        {
            materialized_payload["semantic_synthesis_schedule"] =
                workflow_file.semantic_synthesis_schedule.clone();
        }
        bindings.push(new_workflow_pack_binding(
            installation,
            "workflow",
            &workflow.id,
            Some(&workflow.path),
            "workflow_definition",
            status,
            materialized_payload,
            now,
        ));
    }
    for agent in &manifest.agents {
        bindings.push(new_workflow_pack_binding(
            installation,
            "agent",
            &agent.id,
            Some(&agent.path),
            "agent_version",
            status,
            json!({
                "role": &agent.role,
                "tool_scope": &agent.tool_scope,
                "handoffs": &agent.handoffs,
                "source_digest": workflow_pack_source_digest(&package_dir, &agent.path)?,
            }),
            now,
        ));
    }
    for connector in &manifest.connectors {
        bindings.push(new_workflow_pack_binding(
            installation,
            "connector",
            &connector.id,
            Some(&connector.path),
            "connector_definition",
            status,
            json!({
                "kind": &connector.kind,
                "required_permissions": &connector.required_permissions,
                "writes_enabled": connector.writes.enabled,
                "write_approval_required": connector.writes.approval_required,
                "provenance_required": connector.provenance.required,
                "tenant_scope": &connector.tenant_scope,
                "prompt_injection_boundary": &connector.prompt_injection_boundary,
                "data_quality": &connector.data_quality,
                "source_digest": workflow_pack_source_digest(&package_dir, &connector.path)?,
            }),
            now,
        ));
    }
    for action in &manifest.actions {
        let action_type = workflow_pack_load_action_type_file(&package_dir, action)?;
        bindings.push(new_workflow_pack_binding(
            installation,
            "action",
            &action.id,
            Some(&action.path),
            "ontology_action_type",
            status,
            json!({
                "action_type": action_type,
                "object_type": workflow_pack_value_string(&action_type, "object_type"),
                "connector_id": workflow_pack_value_string(&action_type, "connector_id"),
                "operation_id": workflow_pack_value_string(&action_type, "operation_id"),
                "side_effect_class": workflow_pack_value_string(&action_type, "side_effect_class"),
                "approval": action_type.get("approval").cloned(),
                "effects": action_type.get("effects").cloned(),
                "source_digest": workflow_pack_source_digest(&package_dir, &action.path)?,
            }),
            now,
        ));
    }
    for policy in &manifest.policies {
        bindings.push(new_workflow_pack_binding(
            installation,
            "policy",
            &policy.id,
            Some(&policy.path),
            "policy_revision",
            status,
            json!({
                "required": policy.required,
                "source_digest": workflow_pack_source_digest(&package_dir, &policy.path)?,
            }),
            now,
        ));
    }
    for eval in &manifest.evals {
        bindings.push(new_workflow_pack_binding(
            installation,
            "eval",
            &eval.id,
            Some(&eval.path),
            "eval_suite",
            status,
            json!({
                "gate": &eval.gate,
                "source_digest": workflow_pack_source_digest(&package_dir, &eval.path)?,
            }),
            now,
        ));
    }
    for schema in &manifest.schemas {
        bindings.push(new_workflow_pack_binding(
            installation,
            "schema",
            &schema.id,
            Some(&schema.path),
            "schema_contract",
            status,
            json!({
                "required": schema.required,
                "source_digest": workflow_pack_source_digest(&package_dir, &schema.path)?,
            }),
            now,
        ));
    }
    for skill in &manifest.skills {
        bindings.push(new_workflow_pack_binding(
            installation,
            "skill",
            &skill.id,
            Some(&skill.path),
            "skill_package",
            status,
            json!({
                "required": skill.required,
                "source_digest": workflow_pack_source_digest(&package_dir, &skill.path)?,
            }),
            now,
        ));
    }
    for profile in &manifest.profiles {
        bindings.push(new_workflow_pack_binding(
            installation,
            "profile",
            &profile.id,
            Some(&profile.path),
            "profile_template",
            status,
            json!({
                "required": profile.required,
                "source_digest": workflow_pack_source_digest(&package_dir, &profile.path)?,
            }),
            now,
        ));
    }
    if let Some(onboarding) = &manifest.onboarding {
        let profiles_by_id = manifest
            .profiles
            .iter()
            .map(|profile| (profile.id.as_str(), profile))
            .collect::<HashMap<_, _>>();
        let assets_by_id = profile_assets
            .iter()
            .map(|asset| (asset.profile_id.as_str(), asset))
            .collect::<HashMap<_, _>>();
        for profile_id in &onboarding.required_profiles {
            let profile_ref = profiles_by_id.get(profile_id.as_str());
            let asset = assets_by_id.get(profile_id.as_str());
            let source_path = profile_ref.map(|profile| profile.path.as_str());
            let source_digest = source_path
                .map(|path| workflow_pack_source_digest(&package_dir, path))
                .transpose()?;
            bindings.push(new_workflow_pack_binding(
                installation,
                "profile_requirement",
                profile_id,
                source_path,
                "workflow_pack_profile_asset",
                status,
                json!({
                    "onboarding_required": true,
                    "workflow": onboarding.workflow,
                    "eval": onboarding.eval,
                    "profile_asset_id": asset.map(|asset| asset.id),
                    "profile_asset_version": asset.map(|asset| asset.version),
                    "source_digest": source_digest,
                }),
                now,
            ));
        }
        for schema in &onboarding.profile_schemas {
            bindings.push(new_workflow_pack_binding(
                installation,
                "onboarding_schema",
                &schema.id,
                Some(&schema.path),
                "schema_contract",
                status,
                json!({
                    "required": schema.required,
                    "workflow": onboarding.workflow,
                    "source_digest": workflow_pack_source_digest(&package_dir, &schema.path)?,
                }),
                now,
            ));
        }
    }
    for gate in &manifest.release_gates {
        bindings.push(new_workflow_pack_binding(
            installation,
            "release_gate",
            &gate.id,
            None,
            "release_gate",
            status,
            json!({
                "gate_type": gate.gate_type,
                "required": gate.required,
            }),
            now,
        ));
    }

    bindings.sort_by(|left, right| {
        left.binding_type
            .cmp(&right.binding_type)
            .then(left.binding_key.cmp(&right.binding_key))
    });
    Ok(bindings)
}

pub(crate) fn workflow_pack_runtime_objects_from_bindings(
    installation: &WorkflowPackInstallation,
    bindings: &[WorkflowPackBinding],
    status: &str,
) -> Result<Vec<WorkflowPackRuntimeObject>, AppError> {
    let now = Utc::now();
    let mut objects = Vec::new();
    for binding in bindings {
        match binding.binding_type.as_str() {
            "workflow" => {
                objects.push(new_workflow_pack_runtime_object(
                    installation,
                    binding,
                    "schedule",
                    &format!("workflow:{}:schedule", binding.binding_key),
                    "workflow_schedule",
                    status,
                    json!({
                        "binding_id": binding.id,
                        "workflow_id": binding.binding_key.clone(),
                        "workflow_definition_id": binding.target_id,
                        "entry_agent": binding.materialized_payload.get("entry_agent").cloned(),
                        "semantic_scopes": binding.materialized_payload.get("semantic_scopes").cloned(),
                        "source_path": binding.source_path.clone(),
                        "source_digest": binding.materialized_payload.get("source_digest").cloned(),
                        "schedule_policy": {
                            "mode": "manual_or_scheduler",
                            "source": "workflow_pack_binding"
                        },
                        "provider_specific_validation": "not_required"
                    }),
                    now,
                ));
                if let Some(spec) = workflow_pack_semantic_synthesis_schedule_spec(binding)? {
                    objects.push(new_workflow_pack_runtime_object(
                        installation,
                        binding,
                        "schedule",
                        &format!("workflow:{}:semantic-synthesis", binding.binding_key),
                        "semantic_synthesis_schedule",
                        status,
                        spec,
                        now,
                    ));
                }
            }
            "connector" => objects.push(new_workflow_pack_runtime_object(
                installation,
                binding,
                "connector_account",
                &format!("connector:{}:account", binding.binding_key),
                "generic_connector_account",
                status,
                json!({
                    "binding_id": binding.id,
                    "connector_id": binding.binding_key.clone(),
                    "connector_kind": binding.materialized_payload.get("kind").cloned(),
                    "required_permissions": binding.materialized_payload.get("required_permissions").cloned(),
                    "writes_enabled": binding.materialized_payload.get("writes_enabled").cloned(),
                    "write_approval_required": binding.materialized_payload.get("write_approval_required").cloned(),
                    "tenant_scope": binding.materialized_payload.get("tenant_scope").cloned(),
                    "prompt_injection_boundary": binding.materialized_payload.get("prompt_injection_boundary").cloned(),
                    "data_quality": binding.materialized_payload.get("data_quality").cloned(),
                    "source_path": binding.source_path.clone(),
                    "source_digest": binding.materialized_payload.get("source_digest").cloned(),
                    "provider_specific_validation": "deferred_to_connector_adapter"
                }),
                now,
            )),
            "pack_extension" => objects.push(new_workflow_pack_runtime_object(
                installation,
                binding,
                "pack_dependency",
                &format!("pack-extension:{}:dependency", binding.binding_key),
                "workflow_pack_dependency",
                status,
                json!({
                    "binding_id": binding.id,
                    "extends_pack_id": binding.binding_key.clone(),
                    "version": binding.materialized_payload.get("version").cloned(),
                    "required": binding.materialized_payload.get("required").cloned(),
                    "semantic_scopes": binding.materialized_payload.get("semantic_scopes").cloned(),
                    "provider_specific_validation": "resolved_by_workflow_pack_validator"
                }),
                now,
            )),
            "agent" => objects.push(new_workflow_pack_runtime_object(
                installation,
                binding,
                "provider_deployment_handle",
                &format!("agent:{}:provider-deployment", binding.binding_key),
                "generic_provider_deployment",
                status,
                json!({
                    "binding_id": binding.id,
                    "agent_ref": binding.binding_key.clone(),
                    "agent_id": binding.materialized_payload.get("agent_id").cloned(),
                    "agent_version_id": binding.materialized_payload.get("agent_version_id").cloned(),
                    "role": binding.materialized_payload.get("role").cloned(),
                    "tool_scope": binding.materialized_payload.get("tool_scope").cloned(),
                    "handoffs": binding.materialized_payload.get("handoffs").cloned(),
                    "source_path": binding.source_path.clone(),
                    "source_digest": binding.materialized_payload.get("source_digest").cloned(),
                    "provider_specific_validation": "deferred_to_provider_adapter"
                }),
                now,
            )),
            "action" => objects.push(new_workflow_pack_runtime_object(
                installation,
                binding,
                "action_type",
                &format!("action:{}:contract", binding.binding_key),
                "ontology_action_type",
                status,
                json!({
                    "binding_id": binding.id,
                    "action_id": binding.binding_key.clone(),
                    "object_type": binding.materialized_payload.get("object_type").cloned(),
                    "connector_id": binding.materialized_payload.get("connector_id").cloned(),
                    "operation_id": binding.materialized_payload.get("operation_id").cloned(),
                    "side_effect_class": binding.materialized_payload.get("side_effect_class").cloned(),
                    "approval": binding.materialized_payload.get("approval").cloned(),
                    "effects": binding.materialized_payload.get("effects").cloned(),
                    "source_path": binding.source_path.clone(),
                    "source_digest": binding.materialized_payload.get("source_digest").cloned(),
                    "provider_specific_validation": "deferred_to_action_runtime"
                }),
                now,
            )),
            _ => {}
        }
    }
    objects.sort_by(|left, right| {
        left.object_type
            .cmp(&right.object_type)
            .then(left.object_key.cmp(&right.object_key))
    });
    Ok(objects)
}

pub(crate) async fn project_workflow_pack_semantic_layer(
    state: &AppState,
    installation: &WorkflowPackInstallation,
    bindings: &[WorkflowPackBinding],
    runtime_objects: &[WorkflowPackRuntimeObject],
) -> Result<Value, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let source_uri = format!("mandoforge://workflow-packs/{}", installation.id);
    let source = workflow_pack_get_or_create_semantic_source(
        state,
        CreateSemanticSource {
            source_type: "workflow_pack".to_string(),
            source_uri: source_uri.clone(),
            display_name: format!("{} {}", manifest.name, installation.version),
            owner_type: Some("workflow_pack_installation".to_string()),
            owner_id: Some(installation.id),
            metadata: json!({
                "pack_id": installation.pack_id,
                "version": installation.version,
                "kind": workflow_pack_kind_label(&manifest.kind),
                "status": installation.status,
            }),
            provenance: json!({
                "source": "workflow_pack.staged",
                "installation_id": installation.id,
                "projected_at": Utc::now(),
            }),
            freshness: json!({"status": "current"}),
            status: "active".to_string(),
            last_ingested_at: Some(Utc::now()),
        },
    )
    .await?;
    let pack_scopes = json!(manifest.semantic_scopes);
    let pack_object = workflow_pack_get_or_create_semantic_object(
        state,
        CreateSemanticObject {
            source_id: Some(source.id),
            object_type: "pack".to_string(),
            object_key: format!("workflow_pack:{}:pack", installation.id),
            title: manifest.name.clone(),
            summary: manifest.description.clone(),
            content: json!({
                "installation_id": installation.id,
                "pack_id": manifest.id,
                "version": manifest.version,
                "kind": workflow_pack_kind_label(&manifest.kind),
                "extends": manifest.extends,
                "capabilities": manifest.capabilities,
                "workflow_count": manifest.workflows.len(),
                "agent_count": manifest.agents.len(),
                "connector_count": manifest.connectors.len(),
            }),
            semantic_scopes: pack_scopes.clone(),
            source_uri: Some(source_uri.clone()),
            provenance: json!({"source": "workflow_pack_manifest", "path": "package.yaml"}),
            trust_level: "source_attested".to_string(),
            freshness: "current".to_string(),
            status: "active".to_string(),
        },
    )
    .await?;

    let mut object_count = 1usize;
    let mut link_count = 0usize;
    let mut component_objects = BTreeMap::<String, SemanticObject>::new();
    for (object_type, id, path, title) in workflow_pack_semantic_component_refs(&manifest) {
        let object = workflow_pack_get_or_create_semantic_object(
            state,
            CreateSemanticObject {
                source_id: Some(source.id),
                object_type: object_type.clone(),
                object_key: format!("workflow_pack:{}:{}:{}", installation.id, object_type, id),
                title,
                summary: format!(
                    "{} {} declared by workflow pack {}.",
                    object_type, id, manifest.id
                ),
                content: json!({
                    "installation_id": installation.id,
                    "pack_id": manifest.id,
                    "component_type": object_type,
                    "component_id": id,
                    "path": path,
                }),
                semantic_scopes: pack_scopes.clone(),
                source_uri: Some(format!(
                    "mandoforge://workflow-packs/{}/{}",
                    installation.id, path
                )),
                provenance: json!({"source": "workflow_pack_manifest", "path": path}),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            },
        )
        .await?;
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            &pack_object,
            "contains",
            &object,
            json!({"source": "workflow_pack_manifest", "component_type": object_type}),
        )
        .await?;
        component_objects.insert(
            format!(
                "{}:{}",
                object.object_type.clone(),
                object
                    .content
                    .get("component_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ),
            object,
        );
        object_count += 1;
    }

    let ontology_projection = workflow_pack_project_ontology_seed(
        state,
        &source,
        &pack_object,
        installation,
        &manifest,
        &package_dir,
        &pack_scopes,
    )
    .await?;
    object_count += ontology_projection.object_count;
    link_count += ontology_projection.link_count;

    let action_projection = workflow_pack_project_action_types(
        state,
        &source,
        &pack_object,
        installation,
        &manifest,
        &package_dir,
        &pack_scopes,
        &component_objects,
        &ontology_projection.object_type_objects,
    )
    .await?;
    object_count += action_projection.object_count;
    link_count += action_projection.link_count;

    let bindings_by_key = bindings
        .iter()
        .filter(|binding| binding.binding_type == "workflow")
        .map(|binding| (binding.binding_key.as_str(), binding))
        .collect::<HashMap<_, _>>();
    let runtime_objects_by_key = runtime_objects
        .iter()
        .filter(|object| object.object_type == "workflow_schedule")
        .map(|object| (object.object_key.as_str(), object))
        .collect::<HashMap<_, _>>();

    for workflow in &manifest.workflows {
        let workflow_file = workflow_pack_load_workflow_file(&package_dir, workflow)?;
        let workflow_scopes = workflow_pack_workflow_semantic_scopes(&manifest, &workflow_file)?;
        let binding = bindings_by_key.get(workflow.id.as_str()).copied();
        let runtime_object = runtime_objects_by_key
            .get(format!("workflow:{}:schedule", workflow.id).as_str())
            .copied();
        let workflow_object = workflow_pack_get_or_create_semantic_object(
            state,
            CreateSemanticObject {
                source_id: Some(source.id),
                object_type: "workflow".to_string(),
                object_key: format!("workflow_pack:{}:workflow:{}", installation.id, workflow.id),
                title: workflow_file
                    .name
                    .clone()
                    .unwrap_or_else(|| workflow.id.clone()),
                summary: format!(
                    "Workflow {} enters through {} and runs {} declared step(s).",
                    workflow.id,
                    workflow.entry_agent,
                    workflow_file.steps.len()
                ),
                content: json!({
                    "installation_id": installation.id,
                    "pack_id": manifest.id,
                    "workflow_id": workflow.id,
                    "entry_agent": workflow.entry_agent,
                    "path": workflow.path,
                    "binding_id": binding.map(|binding| binding.id),
                    "workflow_definition_id": binding.and_then(|binding| binding.target_id),
                    "runtime_object_id": runtime_object.map(|object| object.id),
                    "steps": workflow_file.steps,
                }),
                semantic_scopes: workflow_scopes,
                source_uri: Some(format!(
                    "mandoforge://workflow-packs/{}/{}",
                    installation.id, workflow.path
                )),
                provenance: json!({"source": "workflow_pack_workflow_file", "path": workflow.path}),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            },
        )
        .await?;
        object_count += 1;
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            &pack_object,
            "contains_workflow",
            &workflow_object,
            json!({"source": "workflow_pack_manifest", "workflow_id": workflow.id}),
        )
        .await?;
        link_count += workflow_pack_link_workflow_dependencies(
            state,
            &workflow_object,
            workflow,
            &workflow_file,
            &component_objects,
            &manifest.connectors,
        )
        .await?;
    }

    state
        .append_audit_log(new_audit_log(
            None,
            "system",
            Some(installation.id),
            "workflow_pack.semantic_layer_projected",
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "installation_id": installation.id,
                "pack_id": installation.pack_id,
                "source_id": source.id,
                "object_count": object_count,
                "link_count": link_count,
                "ontology_object_type_count": ontology_projection.object_type_objects.len(),
                "ontology_relation_type_count": ontology_projection.relation_type_count,
                "action_type_count": action_projection.action_type_count,
            }),
        ))
        .await?;
    Ok(json!({
        "source_id": source.id,
        "object_count": object_count,
        "link_count": link_count,
        "ontology_object_type_count": ontology_projection.object_type_objects.len(),
        "ontology_relation_type_count": ontology_projection.relation_type_count,
        "action_type_count": action_projection.action_type_count,
    }))
}

pub(crate) async fn workflow_pack_project_ontology_seed(
    state: &AppState,
    source: &SemanticSource,
    pack_object: &SemanticObject,
    installation: &WorkflowPackInstallation,
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    pack_scopes: &Value,
) -> Result<WorkflowPackOntologyProjection, AppError> {
    let Some(profile) = manifest
        .profiles
        .iter()
        .find(|profile| profile.id == "ontology-seed")
    else {
        return Ok(WorkflowPackOntologyProjection {
            object_count: 0,
            link_count: 0,
            relation_type_count: 0,
            object_type_objects: BTreeMap::new(),
        });
    };
    let seed = workflow_pack_load_yaml_value(package_dir, &profile.path, "ontology seed")?;
    let ontology = seed.get("ontology").unwrap_or(&seed);
    let mut object_count = 0usize;
    let mut link_count = 0usize;
    let mut object_type_objects = BTreeMap::new();
    for object_type in workflow_pack_value_array(ontology, "object_types") {
        let Some(id) = workflow_pack_value_string(object_type, "id") else {
            continue;
        };
        let summary = workflow_pack_value_string(object_type, "summary")
            .unwrap_or_else(|| format!("Ontology object type {id}."));
        let object = workflow_pack_get_or_create_semantic_object(
            state,
            CreateSemanticObject {
                source_id: Some(source.id),
                object_type: "ontology_object_type".to_string(),
                object_key: format!(
                    "workflow_pack:{}:ontology-object-type:{}",
                    installation.id, id
                ),
                title: id.clone(),
                summary,
                content: json!({
                    "installation_id": installation.id,
                    "pack_id": manifest.id,
                    "ontology_object_type_id": id,
                    "source": object_type.get("source").cloned(),
                    "definition": object_type,
                }),
                semantic_scopes: pack_scopes.clone(),
                source_uri: Some(format!(
                    "mandoforge://workflow-packs/{}/{}#object_types/{}",
                    installation.id, profile.path, id
                )),
                provenance: json!({"source": "ontology_seed", "path": profile.path}),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            },
        )
        .await?;
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            pack_object,
            "declares_ontology_object_type",
            &object,
            json!({"source": "ontology_seed", "object_type_id": id}),
        )
        .await?;
        object_type_objects.insert(id, object);
        object_count += 1;
    }

    let mut relation_type_count = 0usize;
    for relation_type in workflow_pack_value_array(ontology, "relation_types") {
        let Some(id) = workflow_pack_value_string(relation_type, "id") else {
            continue;
        };
        let object = workflow_pack_get_or_create_semantic_object(
            state,
            CreateSemanticObject {
                source_id: Some(source.id),
                object_type: "ontology_relation_type".to_string(),
                object_key: format!(
                    "workflow_pack:{}:ontology-relation-type:{}",
                    installation.id, id
                ),
                title: id.clone(),
                summary: format!("Ontology relation type {id}."),
                content: json!({
                    "installation_id": installation.id,
                    "pack_id": manifest.id,
                    "ontology_relation_type_id": id,
                    "from": relation_type.get("from").cloned(),
                    "to": relation_type.get("to").cloned(),
                    "definition": relation_type,
                }),
                semantic_scopes: pack_scopes.clone(),
                source_uri: Some(format!(
                    "mandoforge://workflow-packs/{}/{}#relation_types/{}",
                    installation.id, profile.path, id
                )),
                provenance: json!({"source": "ontology_seed", "path": profile.path}),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            },
        )
        .await?;
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            pack_object,
            "declares_ontology_relation_type",
            &object,
            json!({"source": "ontology_seed", "relation_type_id": id}),
        )
        .await?;
        for from_id in workflow_pack_value_string_array(relation_type.get("from")) {
            if let Some(from_object) = object_type_objects.get(&from_id) {
                link_count += workflow_pack_create_semantic_link_if_absent(
                    state,
                    &object,
                    "relation_from_object_type",
                    from_object,
                    json!({"source": "ontology_seed", "relation_type_id": id}),
                )
                .await?;
            }
        }
        for to_id in workflow_pack_value_string_array(relation_type.get("to")) {
            if let Some(to_object) = object_type_objects.get(&to_id) {
                link_count += workflow_pack_create_semantic_link_if_absent(
                    state,
                    &object,
                    "relation_to_object_type",
                    to_object,
                    json!({"source": "ontology_seed", "relation_type_id": id}),
                )
                .await?;
            }
        }
        relation_type_count += 1;
        object_count += 1;
    }

    Ok(WorkflowPackOntologyProjection {
        object_count,
        link_count,
        relation_type_count,
        object_type_objects,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn workflow_pack_project_action_types(
    state: &AppState,
    source: &SemanticSource,
    pack_object: &SemanticObject,
    installation: &WorkflowPackInstallation,
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    pack_scopes: &Value,
    component_objects: &BTreeMap<String, SemanticObject>,
    ontology_object_types: &BTreeMap<String, SemanticObject>,
) -> Result<WorkflowPackActionProjection, AppError> {
    let mut object_count = 0usize;
    let mut link_count = 0usize;
    let mut action_type_count = 0usize;
    for action in &manifest.actions {
        let action_type = workflow_pack_load_action_type_file(package_dir, action)?;
        let action_object = workflow_pack_get_or_create_semantic_object(
            state,
            CreateSemanticObject {
                source_id: Some(source.id),
                object_type: "ontology_action_type".to_string(),
                object_key: format!(
                    "workflow_pack:{}:action-type:{}",
                    installation.id, action.id
                ),
                title: action.id.clone(),
                summary: format!(
                    "ActionType {} acts on {} through connector operation {}.",
                    action.id,
                    workflow_pack_value_string(&action_type, "object_type")
                        .unwrap_or_else(|| "unknown".to_string()),
                    workflow_pack_value_string(&action_type, "operation_id")
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                content: json!({
                    "installation_id": installation.id,
                    "pack_id": manifest.id,
                    "action_id": action.id,
                    "path": action.path,
                    "definition": action_type,
                }),
                semantic_scopes: pack_scopes.clone(),
                source_uri: Some(format!(
                    "mandoforge://workflow-packs/{}/{}",
                    installation.id, action.path
                )),
                provenance: json!({"source": "workflow_pack_action_type", "path": action.path}),
                trust_level: "source_attested".to_string(),
                freshness: "current".to_string(),
                status: "active".to_string(),
            },
        )
        .await?;
        object_count += 1;
        action_type_count += 1;
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            pack_object,
            "declares_action_type",
            &action_object,
            json!({"source": "workflow_pack_action_type", "action_id": action.id}),
        )
        .await?;
        if let Some(component) = component_objects.get(&format!("action:{}", action.id)) {
            link_count += workflow_pack_create_semantic_link_if_absent(
                state,
                component,
                "materializes_action_type",
                &action_object,
                json!({"source": "workflow_pack_action_type", "action_id": action.id}),
            )
            .await?;
        }
        if let Some(object_type_id) = workflow_pack_value_string(&action_type, "object_type")
            && let Some(object_type) = ontology_object_types.get(&object_type_id)
        {
            link_count += workflow_pack_create_semantic_link_if_absent(
                state,
                &action_object,
                "acts_on_object_type",
                object_type,
                json!({"source": "workflow_pack_action_type", "action_id": action.id}),
            )
            .await?;
        }
        if let Some(connector_id) = workflow_pack_value_string(&action_type, "connector_id")
            && let Some(connector) = component_objects.get(&format!("connector:{connector_id}"))
        {
            link_count += workflow_pack_create_semantic_link_if_absent(
                state,
                &action_object,
                "uses_connector",
                connector,
                json!({
                    "source": "workflow_pack_action_type",
                    "action_id": action.id,
                    "operation_id": workflow_pack_value_string(&action_type, "operation_id"),
                }),
            )
            .await?;
        }
    }
    Ok(WorkflowPackActionProjection {
        object_count,
        link_count,
        action_type_count,
    })
}

pub(crate) async fn workflow_pack_get_or_create_semantic_source(
    state: &AppState,
    input: CreateSemanticSource,
) -> Result<SemanticSource, AppError> {
    let source_uri = input.source_uri.clone();
    if let Some(source) = state
        .list_semantic_sources()
        .await?
        .into_iter()
        .find(|source| source.source_uri.eq_ignore_ascii_case(&source_uri))
    {
        return Ok(source);
    }
    match state.create_semantic_source(input).await {
        Ok(source) => Ok(source),
        Err(error) if error.message.contains("already exists") => state
            .list_semantic_sources()
            .await?
            .into_iter()
            .find(|source| source.source_uri.eq_ignore_ascii_case(&source_uri))
            .ok_or(error),
        Err(error) => Err(error),
    }
}

pub(crate) async fn workflow_pack_get_or_create_semantic_object(
    state: &AppState,
    input: CreateSemanticObject,
) -> Result<SemanticObject, AppError> {
    let object_key = input.object_key.clone();
    if let Some(object) = state
        .list_semantic_objects()
        .await?
        .into_iter()
        .find(|object| object.object_key.eq_ignore_ascii_case(&object_key))
    {
        return Ok(object);
    }
    match state.create_semantic_object(input).await {
        Ok(object) => Ok(object),
        Err(error) if error.message.contains("already exists") => state
            .list_semantic_objects()
            .await?
            .into_iter()
            .find(|object| object.object_key.eq_ignore_ascii_case(&object_key))
            .ok_or(error),
        Err(error) => Err(error),
    }
}

pub(crate) async fn workflow_pack_create_semantic_link_if_absent(
    state: &AppState,
    from: &SemanticObject,
    relation_type: &str,
    to: &SemanticObject,
    metadata: Value,
) -> Result<usize, AppError> {
    let from_id = from.id.to_string();
    let to_id = to.id.to_string();
    if state.list_semantic_links().await?.into_iter().any(|link| {
        link.status == "active"
            && link.from_entity_id == from_id
            && link.to_entity_id == to_id
            && link.relation_type == relation_type
    }) {
        return Ok(0);
    }
    match state
        .create_semantic_link(CreateSemanticLink {
            from_entity_type: from.object_type.clone(),
            from_entity_id: from_id,
            relation_type: relation_type.to_string(),
            to_entity_type: to.object_type.clone(),
            to_entity_id: to_id,
            metadata,
            provenance: json!({
                "source": "workflow_pack.semantic_layer_projected",
                "projected_at": Utc::now(),
            }),
            confidence: 1.0,
            status: "active".to_string(),
        })
        .await
    {
        Ok(_) => Ok(1),
        Err(error) if error.message.contains("already exists") => Ok(0),
        Err(error) => Err(error),
    }
}

pub(crate) fn workflow_pack_semantic_component_refs(
    manifest: &workflow_pack::WorkflowPackManifest,
) -> Vec<(String, String, String, String)> {
    let mut refs = Vec::new();
    refs.extend(manifest.agents.iter().map(|agent| {
        (
            "agent".to_string(),
            agent.id.clone(),
            agent.path.clone(),
            format!("{} agent", agent.id),
        )
    }));
    refs.extend(manifest.connectors.iter().map(|connector| {
        (
            "connector".to_string(),
            connector.id.clone(),
            connector.path.clone(),
            format!("{} connector", connector.id),
        )
    }));
    refs.extend(manifest.actions.iter().map(|action| {
        (
            "action".to_string(),
            action.id.clone(),
            action.path.clone(),
            format!("{} action type", action.id),
        )
    }));
    refs.extend(manifest.profiles.iter().map(|profile| {
        (
            "profile".to_string(),
            profile.id.clone(),
            profile.path.clone(),
            format!("{} profile", profile.id),
        )
    }));
    refs.extend(manifest.schemas.iter().map(|schema| {
        (
            "schema".to_string(),
            schema.id.clone(),
            schema.path.clone(),
            format!("{} schema", schema.id),
        )
    }));
    refs.extend(manifest.skills.iter().map(|skill| {
        (
            "skill".to_string(),
            skill.id.clone(),
            skill.path.clone(),
            format!("{} skill", skill.id),
        )
    }));
    refs.extend(manifest.policies.iter().map(|policy| {
        (
            "policy".to_string(),
            policy.id.clone(),
            policy.path.clone(),
            format!("{} policy", policy.id),
        )
    }));
    refs.extend(manifest.evals.iter().map(|eval| {
        (
            "eval".to_string(),
            eval.id.clone(),
            eval.path.clone(),
            format!("{} eval", eval.id),
        )
    }));
    refs.extend(manifest.release_gates.iter().map(|gate| {
        (
            "release_gate".to_string(),
            gate.id.clone(),
            "package.yaml".to_string(),
            format!("{} release gate", gate.id),
        )
    }));
    refs
}

pub(crate) async fn workflow_pack_link_workflow_dependencies(
    state: &AppState,
    workflow_object: &SemanticObject,
    workflow: &workflow_pack::WorkflowRef,
    workflow_file: &WorkflowPackWorkflowFile,
    component_objects: &BTreeMap<String, SemanticObject>,
    connectors: &[workflow_pack::ConnectorRef],
) -> Result<usize, AppError> {
    let mut link_count = 0usize;
    let mut linked = BTreeSet::<String>::new();
    linked.insert(format!("agent:{}", workflow.entry_agent));
    for step in &workflow_file.steps {
        if let Some(agent) = workflow_pack_workflow_step_string(step, "agent") {
            linked.insert(format!("agent:{agent}"));
        }
        for profile in workflow_pack_workflow_step_string_array(step, "required_profiles") {
            linked.insert(format!("profile:{profile}"));
        }
        for schema in workflow_pack_workflow_step_string_array(step, "required_schemas") {
            linked.insert(format!("schema:{schema}"));
        }
        if let Some(schema) = workflow_pack_workflow_step_string(step, "output_schema") {
            linked.insert(format!("schema:{schema}"));
        }
        for skill in workflow_pack_workflow_step_string_array(step, "skills") {
            linked.insert(format!("skill:{skill}"));
        }
    }
    for connector in connectors {
        linked.insert(format!("connector:{}", connector.id));
    }
    for dependency_key in linked {
        let Some(dependency) = component_objects.get(&dependency_key) else {
            continue;
        };
        let relation_type = dependency_key
            .split_once(':')
            .map(|(kind, _)| match kind {
                "agent" => "uses_agent",
                "connector" => "uses_connector",
                "profile" => "requires_profile",
                "schema" => "requires_schema",
                "skill" => "uses_skill",
                _ => "references",
            })
            .unwrap_or("references");
        link_count += workflow_pack_create_semantic_link_if_absent(
            state,
            workflow_object,
            relation_type,
            dependency,
            json!({
                "source": "workflow_pack_workflow_file",
                "workflow_id": workflow.id,
                "dependency_key": dependency_key,
            }),
        )
        .await?;
    }
    Ok(link_count)
}

pub(crate) fn workflow_pack_workflow_step_string_array(step: &Value, key: &str) -> Vec<String> {
    step.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn workflow_pack_semantic_synthesis_schedule_spec(
    binding: &WorkflowPackBinding,
) -> Result<Option<Value>, AppError> {
    let Some(schedule) = binding
        .materialized_payload
        .get("semantic_synthesis_schedule")
    else {
        return Ok(None);
    };
    if !schedule.is_object() {
        return Err(AppError::bad_request(
            "workflow pack semantic_synthesis_schedule must be a JSON object",
        ));
    }
    if schedule
        .get("enabled")
        .and_then(Value::as_bool)
        .is_some_and(|enabled| !enabled)
    {
        return Ok(None);
    }
    let mut spec = schedule.as_object().cloned().unwrap_or_default();
    spec.insert("binding_id".to_string(), json!(binding.id));
    spec.insert(
        "workflow_id".to_string(),
        json!(binding.binding_key.clone()),
    );
    spec.insert(
        "workflow_definition_id".to_string(),
        json!(binding.target_id),
    );
    spec.insert(
        "source_path".to_string(),
        json!(binding.source_path.clone()),
    );
    spec.insert(
        "source_digest".to_string(),
        binding
            .materialized_payload
            .get("source_digest")
            .cloned()
            .unwrap_or(Value::Null),
    );
    spec.entry("schedule_policy".to_string())
        .or_insert_with(|| {
            json!({
                "mode": "scheduler",
                "source": "workflow_pack_binding"
            })
        });
    spec.entry("session_selector".to_string())
        .or_insert_with(|| {
            json!({
                "source": "completed_workflow_runs",
                "status": "completed"
            })
        });
    spec.entry("metadata".to_string())
        .or_insert_with(|| json!({}));
    Ok(Some(Value::Object(spec)))
}

pub(crate) fn new_workflow_pack_runtime_object(
    installation: &WorkflowPackInstallation,
    binding: &WorkflowPackBinding,
    object_type: &str,
    object_key: &str,
    runtime_kind: &str,
    status: &str,
    spec: Value,
    now: DateTime<Utc>,
) -> WorkflowPackRuntimeObject {
    WorkflowPackRuntimeObject {
        id: Uuid::new_v4(),
        installation_id: installation.id,
        binding_id: binding.id,
        pack_id: installation.pack_id.clone(),
        pack_version: installation.version.clone(),
        object_type: object_type.to_string(),
        object_key: object_key.to_string(),
        runtime_kind: runtime_kind.to_string(),
        status: status.to_string(),
        spec,
        created_at: now,
        updated_at: now,
    }
}

pub(crate) fn new_workflow_pack_binding(
    installation: &WorkflowPackInstallation,
    binding_type: &str,
    binding_key: &str,
    source_path: Option<&str>,
    target_kind: &str,
    status: &str,
    materialized_payload: Value,
    now: DateTime<Utc>,
) -> WorkflowPackBinding {
    WorkflowPackBinding {
        id: Uuid::new_v4(),
        installation_id: installation.id,
        pack_id: installation.pack_id.clone(),
        pack_version: installation.version.clone(),
        binding_type: binding_type.to_string(),
        binding_key: binding_key.to_string(),
        source_path: source_path.map(ToString::to_string),
        target_kind: target_kind.to_string(),
        target_id: None,
        status: status.to_string(),
        materialized_payload,
        created_at: now,
        updated_at: now,
    }
}

pub(crate) fn workflow_pack_source_digest(
    package_dir: &FsPath,
    source_path: &str,
) -> Result<String, AppError> {
    let content = std::fs::read_to_string(package_dir.join(source_path))
        .with_context(|| format!("read workflow pack source {}", source_path))?;
    Ok(normalized_json_sha256(&json!(content)))
}

pub(crate) fn validate_workflow_pack_profile_assets_input(
    installation: &WorkflowPackInstallation,
    profiles: Vec<WorkflowPackOnboardingProfileInput>,
) -> Result<Vec<WorkflowPackOnboardingProfileInput>, AppError> {
    if profiles.is_empty() {
        return Err(AppError::bad_request(
            "at least one workflow pack onboarding profile is required",
        ));
    }
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let onboarding = manifest.onboarding.as_ref().ok_or_else(|| {
        AppError::bad_request("workflow pack manifest missing onboarding contract")
    })?;
    let profile_refs: HashMap<_, _> = manifest
        .profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect();
    let allowed_profiles: HashSet<_> = onboarding.required_profiles.iter().cloned().collect();

    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let profile_id = profile.id.trim().to_string();
        if profile_id.is_empty() {
            return Err(AppError::bad_request(
                "workflow pack profile id is required",
            ));
        }
        if !seen.insert(profile_id.clone()) {
            return Err(AppError::bad_request(format!(
                "duplicate workflow pack profile asset {}",
                profile_id
            )));
        }
        if !allowed_profiles.contains(&profile_id) {
            return Err(AppError::bad_request(format!(
                "workflow pack profile {} is not part of the onboarding contract",
                profile_id
            )));
        }
        let content = profile.content.trim().to_string();
        if content.is_empty() {
            return Err(AppError::bad_request(format!(
                "workflow pack profile {} content is required",
                profile_id
            )));
        }
        let declared = profile_refs.get(profile_id.as_str()).ok_or_else(|| {
            AppError::bad_request(format!(
                "workflow pack onboarding profile {} is not declared in manifest",
                profile_id
            ))
        })?;
        let default_profile_content = std::fs::read_to_string(package_dir.join(&declared.path))
            .with_context(|| format!("read workflow pack profile template {}", declared.path))?;
        if content == default_profile_content.trim() {
            return Err(AppError::bad_request(format!(
                "workflow pack profile {} still matches the packaged default template",
                profile_id
            )));
        }
        validated.push(WorkflowPackOnboardingProfileInput {
            id: profile_id,
            content,
        });
    }
    Ok(validated)
}

pub(crate) async fn assess_workflow_pack_connector_quality(
    state: &AppState,
    installation: &WorkflowPackInstallation,
    input: WorkflowPackConnectorQualityAssessmentRequest,
) -> Result<WorkflowPackConnectorQualityAssessment, AppError> {
    let (manifest, package_dir) = workflow_pack_manifest_and_dir_from_installation(installation)?;
    let secret_records = state.list_secret_records().await?;
    let checked_at = Utc::now();
    let mut input_connectors = HashMap::new();
    for connector in input.connectors {
        let connector_id = connector.id.trim().to_string();
        if connector_id.is_empty() {
            return Err(AppError::bad_request(
                "workflow pack connector id is required",
            ));
        }
        if input_connectors
            .insert(connector_id.clone(), connector)
            .is_some()
        {
            return Err(AppError::bad_request(format!(
                "duplicate workflow pack connector quality input {}",
                connector_id
            )));
        }
    }

    let mut connector_results = Vec::new();
    let mut ready_connector_count = 0usize;
    let mut blockers = Vec::new();
    for connector in &manifest.connectors {
        let Some(contract) = connector.data_quality.as_ref() else {
            let blocker = "connector data_quality contract is missing".to_string();
            connector_results.push(WorkflowPackConnectorQualityResult {
                id: connector.id.clone(),
                status: "blocked".to_string(),
                sample_count: 0,
                passing_sample_count: 0,
                bound_team_id: None,
                bound_server_id: None,
                bound_server_name: None,
                bound_server_health_status: None,
                bound_tool_name: None,
                tenant_binding_status: None,
                credential_status: None,
                secret_ref_statuses: Vec::new(),
                operation_statuses: Vec::new(),
                lane_impacts: BTreeMap::new(),
                blockers: vec![blocker.clone()],
            });
            blockers.push(format!("connector {}: {}", connector.id, blocker));
            continue;
        };
        let Some(input_connector) = input_connectors.get(connector.id.as_str()) else {
            let blocker = "connector quality assessment is missing".to_string();
            connector_results.push(WorkflowPackConnectorQualityResult {
                id: connector.id.clone(),
                status: "blocked".to_string(),
                sample_count: 0,
                passing_sample_count: 0,
                bound_team_id: None,
                bound_server_id: None,
                bound_server_name: None,
                bound_server_health_status: None,
                bound_tool_name: None,
                tenant_binding_status: None,
                credential_status: None,
                secret_ref_statuses: Vec::new(),
                operation_statuses: Vec::new(),
                lane_impacts: BTreeMap::new(),
                blockers: vec![blocker.clone()],
            });
            blockers.push(format!("connector {}: {}", connector.id, blocker));
            continue;
        };

        let mut bound_server_id = None;
        let mut bound_server_name = None;
        let mut bound_server_health_status = None;
        let bound_tool_name = input_connector
            .tool_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let mut passing_sample_count = 0usize;
        let mut connector_blockers = Vec::new();
        let mut connector_warnings = Vec::new();
        let required_tenant_fields =
            workflow_pack_connector_required_tenant_fields(&manifest, &package_dir, connector)?;
        let tenant_binding_status = if required_tenant_fields.is_empty() {
            None
        } else {
            let mut missing_fields = Vec::new();
            for field in &required_tenant_fields {
                if !workflow_pack_connector_tenant_field_present(
                    input_connector.tenant_binding.as_ref(),
                    field,
                ) {
                    missing_fields.push(field.clone());
                }
            }
            if missing_fields.is_empty() {
                Some("ready".to_string())
            } else {
                for field in missing_fields {
                    connector_blockers.push(format!("tenant binding missing {}", field));
                }
                Some("blocked".to_string())
            }
        };

        let mut required_secret_refs =
            workflow_pack_connector_required_secret_refs(&manifest, &package_dir, connector)?;
        for (alias, reference) in &input_connector.secret_refs {
            let trimmed_alias = alias.trim();
            let trimmed_reference = reference.trim();
            if trimmed_alias.is_empty() || trimmed_reference.is_empty() {
                return Err(AppError::bad_request(format!(
                    "connector {} secret_refs must use non-empty aliases and references",
                    connector.id
                )));
            }
            required_secret_refs.insert(trimmed_alias.to_string(), trimmed_reference.to_string());
        }
        let secret_ref_statuses = workflow_pack_connector_secret_ref_statuses(
            &connector.id,
            &required_secret_refs,
            &secret_records,
        );
        for secret_ref in &secret_ref_statuses {
            if secret_ref.status != "ready" {
                connector_blockers.extend(secret_ref.blockers.iter().cloned());
            }
        }
        let credential_status = if secret_ref_statuses.is_empty() {
            None
        } else if secret_ref_statuses
            .iter()
            .all(|secret_ref| secret_ref.status == "ready")
        {
            Some("ready".to_string())
        } else {
            Some("blocked".to_string())
        };

        let operation_contracts =
            workflow_pack_connector_operation_contracts(&package_dir, connector)?;
        let lane_requirements =
            workflow_pack_connector_lane_requirements(&manifest, &package_dir, connector)?;
        let operation_statuses = workflow_pack_connector_operation_statuses(
            connector,
            &operation_contracts,
            &lane_requirements,
            &input_connector.operation_statuses,
        )?;
        for operation in &operation_statuses {
            connector_blockers.extend(operation.blockers.iter().cloned());
            if operation.status == "degraded" {
                connector_warnings
                    .push(format!("operation {} is degraded", operation.operation_id));
            }
        }
        let mut lane_impacts =
            workflow_pack_connector_lane_impacts(&lane_requirements, &operation_statuses);
        for (lane_id, impact) in &input_connector.lane_impacts {
            let lane_id = lane_id.trim();
            if lane_id.is_empty() {
                return Err(AppError::bad_request(format!(
                    "connector {} lane_impacts must use non-empty lane ids",
                    connector.id
                )));
            }
            match lane_impacts.get(lane_id) {
                Some(computed) if computed.status != impact.status => {
                    connector_blockers.push(format!(
                        "lane {} impact status {} does not match computed status {}",
                        lane_id, impact.status, computed.status
                    ))
                }
                Some(_) => {}
                None => {
                    lane_impacts.insert(
                        lane_id.to_string(),
                        WorkflowPackConnectorLaneImpact {
                            status: impact.status.trim().to_string(),
                            enabled_workflows: impact.enabled_workflows.clone(),
                            blocked_workflows: impact.blocked_workflows.clone(),
                            degraded_reason: impact
                                .degraded_reason
                                .clone()
                                .unwrap_or_default()
                                .trim()
                                .to_string(),
                        },
                    );
                }
            }
        }
        for (lane_id, impact) in &lane_impacts {
            match impact.status.as_str() {
                "blocked" => connector_blockers.push(format!("lane {} is blocked", lane_id)),
                "degraded" => connector_warnings.push(format!("lane {} is degraded", lane_id)),
                _ => {}
            }
        }
        if let (Some(team_id), Some(server_id)) =
            (input_connector.team_id, input_connector.server_id)
        {
            let server = state
                .list_mcp_servers(team_id)
                .await?
                .into_iter()
                .find(|candidate| candidate.id == server_id)
                .ok_or_else(|| {
                    AppError::bad_request(format!(
                        "workflow pack connector {} bound mcp server {} was not found in team {}",
                        connector.id, server_id, team_id
                    ))
                })?;
            bound_server_id = Some(server.id);
            bound_server_name = Some(server.name.clone());
            let server_health_status = server
                .config
                .pointer("/health_check/last_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            bound_server_health_status = Some(server_health_status.clone());
            if server.status != "active" {
                connector_blockers.push(format!("bound MCP server {} is not active", server.name));
            }
            if server
                .config
                .pointer("/health_check/last_healthy")
                .and_then(Value::as_bool)
                != Some(true)
            {
                connector_blockers.push(format!("bound MCP server {} is not healthy", server.name));
            }
            let last_checked_at = server
                .config
                .pointer("/health_check/last_checked_at")
                .and_then(Value::as_str)
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc));
            match last_checked_at {
                Some(last_checked_at)
                    if checked_at.signed_duration_since(last_checked_at)
                        <= chrono::Duration::hours(24) => {}
                Some(_) => connector_blockers.push(format!(
                    "bound MCP server {} health evidence is stale",
                    server.name
                )),
                None => connector_blockers.push(format!(
                    "bound MCP server {} has no health evidence",
                    server.name
                )),
            }
            if let Some(tool_name) = bound_tool_name.as_deref()
                && !server.tool_allowlist.iter().any(|tool| tool == tool_name)
            {
                connector_blockers.push(format!(
                    "bound MCP server {} does not allow tool {}",
                    server.name, tool_name
                ));
            }
        }

        for (index, sample) in input_connector.samples.iter().enumerate() {
            let sample_label = if sample.object_id.trim().is_empty() {
                format!("sample {}", index + 1)
            } else {
                format!("sample {}", sample.object_id.trim())
            };
            let mut sample_ok = true;
            if sample.object_id.trim().is_empty() {
                connector_blockers.push(format!("{sample_label} missing object_id"));
                sample_ok = false;
            }
            if checked_at
                .signed_duration_since(sample.retrieved_at)
                .num_hours()
                > contract.max_age_hours
            {
                connector_blockers.push(format!(
                    "{sample_label} is older than {} hours",
                    contract.max_age_hours
                ));
                sample_ok = false;
            }
            if contract.citation_required
                && sample
                    .citation_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
            {
                connector_blockers.push(format!("{sample_label} missing citation_url"));
                sample_ok = false;
            }

            let metadata = sample.metadata.as_object().ok_or_else(|| {
                AppError::bad_request(format!(
                    "connector {} sample metadata must be a JSON object",
                    connector.id
                ))
            })?;
            for field in &contract.required_metadata_fields {
                if !json_field_present(metadata.get(field)) {
                    connector_blockers
                        .push(format!("{sample_label} missing metadata field {}", field));
                    sample_ok = false;
                }
            }

            let content = sample.content.as_object().ok_or_else(|| {
                AppError::bad_request(format!(
                    "connector {} sample content must be a JSON object",
                    connector.id
                ))
            })?;
            for field in &contract.required_content_fields {
                if !json_field_present(content.get(field)) {
                    connector_blockers
                        .push(format!("{sample_label} missing content field {}", field));
                    sample_ok = false;
                }
            }

            if sample_ok {
                passing_sample_count += 1;
            }
        }
        if passing_sample_count < contract.min_sample_count {
            connector_blockers.push(format!(
                "requires at least {} passing sample(s)",
                contract.min_sample_count
            ));
        }
        connector_blockers.sort();
        connector_blockers.dedup();
        let status = if connector_blockers.is_empty() && connector_warnings.is_empty() {
            ready_connector_count += 1;
            "ready".to_string()
        } else if connector_blockers.is_empty() {
            "degraded".to_string()
        } else {
            blockers.extend(
                connector_blockers
                    .iter()
                    .map(|blocker| format!("connector {}: {}", connector.id, blocker)),
            );
            "blocked".to_string()
        };
        connector_results.push(WorkflowPackConnectorQualityResult {
            id: connector.id.clone(),
            status,
            sample_count: input_connector.samples.len(),
            passing_sample_count,
            bound_team_id: input_connector.team_id,
            bound_server_id,
            bound_server_name,
            bound_server_health_status,
            bound_tool_name,
            tenant_binding_status,
            credential_status,
            secret_ref_statuses,
            operation_statuses,
            lane_impacts,
            blockers: connector_blockers,
        });
    }
    connector_results.sort_by(|left, right| left.id.cmp(&right.id));
    blockers.sort();
    blockers.dedup();

    Ok(WorkflowPackConnectorQualityAssessment {
        installation_id: installation.id,
        pack_id: installation.pack_id.clone(),
        version: installation.version.clone(),
        status: if blockers.is_empty()
            && connector_results
                .iter()
                .any(|connector| connector.status == "degraded")
        {
            "degraded".to_string()
        } else if blockers.is_empty() {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        connector_requirement_count: manifest.connectors.len(),
        ready_connector_count,
        connector_results,
        blockers,
        checked_at,
    })
}

pub(crate) fn workflow_pack_connector_required_secret_refs(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    connector: &workflow_pack::ConnectorRef,
) -> Result<BTreeMap<String, String>, AppError> {
    let mut refs =
        workflow_pack_connector_account_secret_refs(manifest, package_dir, &connector.id)?;
    if !refs.is_empty() {
        return Ok(refs);
    }

    let Some(connector_file) = workflow_pack_connector_file(package_dir, connector)? else {
        return Ok(refs);
    };
    if let Some(required_secrets) = workflow_pack_connector_yaml_array(
        &connector_file,
        &connector.id,
        &["auth", "required_secrets"],
    )
    .or_else(|| {
        workflow_pack_connector_yaml_array(
            &connector_file,
            &connector.id,
            &["readiness_probes", "credential_probe", "required_secrets"],
        )
    }) {
        for secret in required_secrets {
            refs.insert(secret.clone(), secret);
        }
    }
    Ok(refs)
}

pub(crate) fn workflow_pack_connector_account_secret_refs(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    connector_id: &str,
) -> Result<BTreeMap<String, String>, AppError> {
    let Some(account_profile) =
        workflow_pack_profile_value(manifest, package_dir, "connector-account")?
    else {
        return Ok(BTreeMap::new());
    };
    let Some(secret_refs) = account_profile
        .get("accounts")
        .and_then(Value::as_object)
        .and_then(|accounts| accounts.get(connector_id))
        .and_then(|account| account.get("auth_binding"))
        .and_then(|auth| auth.get("secret_refs"))
        .and_then(Value::as_object)
    else {
        return Ok(BTreeMap::new());
    };
    let mut refs = BTreeMap::new();
    for (alias, reference) in secret_refs {
        if let Some(reference) = reference
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            refs.insert(alias.trim().to_string(), reference.to_string());
        }
    }
    Ok(refs)
}

pub(crate) fn workflow_pack_connector_secret_ref_statuses(
    connector_id: &str,
    required_secret_refs: &BTreeMap<String, String>,
    secret_records: &[SecretRecord],
) -> Vec<WorkflowPackConnectorSecretRefStatus> {
    required_secret_refs
        .iter()
        .map(|(alias, reference)| {
            let matched = secret_records.iter().find(|record| {
                record.name == *reference
                    || record.key == *reference
                    || secret_record_ref(record) == *reference
                    || format!("{}#{}", record.path, record.key) == *reference
            });
            match matched {
                Some(record) if record.status == "active" => WorkflowPackConnectorSecretRefStatus {
                    alias: alias.clone(),
                    reference: reference.clone(),
                    status: "ready".to_string(),
                    catalog_ref: Some(secret_record_ref(record)),
                    blockers: Vec::new(),
                },
                Some(record) => WorkflowPackConnectorSecretRefStatus {
                    alias: alias.clone(),
                    reference: reference.clone(),
                    status: "blocked".to_string(),
                    catalog_ref: Some(secret_record_ref(record)),
                    blockers: vec![format!(
                        "connector {} secret {} catalog record is {}",
                        connector_id, reference, record.status
                    )],
                },
                None => WorkflowPackConnectorSecretRefStatus {
                    alias: alias.clone(),
                    reference: reference.clone(),
                    status: "blocked".to_string(),
                    catalog_ref: None,
                    blockers: vec![format!(
                        "connector {} secret {} is missing from Vault catalog",
                        connector_id, reference
                    )],
                },
            }
        })
        .collect()
}

pub(crate) fn workflow_pack_connector_required_tenant_fields(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    connector: &workflow_pack::ConnectorRef,
) -> Result<BTreeSet<String>, AppError> {
    let mut fields = BTreeSet::new();
    if let Some(account_profile) =
        workflow_pack_profile_value(manifest, package_dir, "connector-account")?
        && let Some(tenant_binding) = account_profile
            .get("accounts")
            .and_then(Value::as_object)
            .and_then(|accounts| accounts.get(&connector.id))
            .and_then(|account| account.get("tenant_binding"))
            .and_then(Value::as_object)
    {
        fields.extend(tenant_binding.keys().map(ToString::to_string));
    }

    if let Some(connector_file) = workflow_pack_connector_file(package_dir, connector)?
        && let Some(required_fields) = workflow_pack_connector_yaml_array(
            &connector_file,
            &connector.id,
            &["readiness_probes", "tenant_scope_probe", "required_fields"],
        )
    {
        fields.extend(required_fields);
    }
    Ok(fields)
}

pub(crate) fn workflow_pack_connector_tenant_field_present(
    tenant_binding: Option<&WorkflowPackConnectorTenantBindingInput>,
    field: &str,
) -> bool {
    let Some(tenant_binding) = tenant_binding else {
        return false;
    };
    let value = match field {
        "tenant_id" => tenant_binding.tenant_id.as_deref(),
        "workspace_id" => tenant_binding.workspace_id.as_deref(),
        "shop_id" => tenant_binding.shop_id.as_deref(),
        "seller_nick" => tenant_binding.seller_nick.as_deref(),
        _ => None,
    };
    value.map(str::trim).is_some_and(|value| !value.is_empty())
}

pub(crate) fn workflow_pack_connector_operation_contracts(
    package_dir: &FsPath,
    connector: &workflow_pack::ConnectorRef,
) -> Result<BTreeMap<String, WorkflowPackConnectorOperationContract>, AppError> {
    let Some(connector_file) = workflow_pack_connector_file(package_dir, connector)? else {
        return Ok(BTreeMap::new());
    };
    let Some(connector_value) = workflow_pack_connector_yaml_entry(&connector_file, &connector.id)
    else {
        return Ok(BTreeMap::new());
    };
    let mut contracts = BTreeMap::new();
    for (field, operation_type) in [("read_operations", "read"), ("write_operations", "write")] {
        let Some(operations) = connector_value.get(field).and_then(Value::as_array) else {
            continue;
        };
        for operation in operations {
            let Some(operation_id) = operation
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            contracts.insert(
                operation_id.to_string(),
                WorkflowPackConnectorOperationContract {
                    api_name: operation
                        .get("api_name")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    permission: operation
                        .get("permission")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    operation_type: operation_type.to_string(),
                },
            );
        }
    }
    Ok(contracts)
}

pub(crate) fn workflow_pack_connector_lane_requirements(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    connector: &workflow_pack::ConnectorRef,
) -> Result<Vec<WorkflowPackConnectorLaneRequirement>, AppError> {
    let Some(connector_map) = workflow_pack_profile_value(manifest, package_dir, "connector-map")?
    else {
        return Ok(Vec::new());
    };
    let Some(lanes) = connector_map
        .get("workflow_lanes")
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    let operation_contracts = workflow_pack_connector_operation_contracts(package_dir, connector)?;
    let operation_ids: BTreeSet<_> = operation_contracts.keys().cloned().collect();
    Ok(lanes
        .iter()
        .map(|(lane_id, lane)| WorkflowPackConnectorLaneRequirement {
            lane_id: lane_id.clone(),
            required_operations: workflow_pack_string_set(lane.get("required_read_operations"))
                .intersection(&operation_ids)
                .cloned()
                .collect(),
            optional_operations: workflow_pack_string_set(lane.get("optional_read_operations"))
                .intersection(&operation_ids)
                .cloned()
                .collect(),
            controlled_write_operations: workflow_pack_string_set(
                lane.get("controlled_write_operations"),
            )
            .intersection(&operation_ids)
            .cloned()
            .collect(),
        })
        .collect())
}

pub(crate) fn workflow_pack_connector_operation_statuses(
    connector: &workflow_pack::ConnectorRef,
    operation_contracts: &BTreeMap<String, WorkflowPackConnectorOperationContract>,
    lane_requirements: &[WorkflowPackConnectorLaneRequirement],
    input_statuses: &[WorkflowPackConnectorOperationStatusInput],
) -> Result<Vec<WorkflowPackConnectorOperationStatus>, AppError> {
    let mut required_operations = BTreeSet::new();
    for lane in lane_requirements {
        required_operations.extend(lane.required_operations.iter().cloned());
    }
    let mut statuses = BTreeMap::new();
    for input in input_statuses {
        let operation_id = input.operation_id.trim();
        if operation_id.is_empty() {
            return Err(AppError::bad_request(format!(
                "connector {} operation_statuses must use non-empty operation_id",
                connector.id
            )));
        }
        let status = input.status.trim();
        if !matches!(status, "ready" | "degraded" | "blocked") {
            return Err(AppError::bad_request(format!(
                "connector {} operation {} status must be ready, degraded, or blocked",
                connector.id, operation_id
            )));
        }
        let contract = operation_contracts.get(operation_id);
        let mut blockers = Vec::new();
        if contract.is_none() {
            blockers.push(format!(
                "connector {} operation {} is not declared in connector contract",
                connector.id, operation_id
            ));
        }
        if required_operations.contains(operation_id) && status != "ready" {
            blockers.push(format!(
                "connector {} required read operation {} is {}",
                connector.id, operation_id, status
            ));
        }
        statuses.insert(
            operation_id.to_string(),
            WorkflowPackConnectorOperationStatus {
                operation_id: operation_id.to_string(),
                api_name: input
                    .api_name
                    .clone()
                    .or_else(|| contract.and_then(|contract| contract.api_name.clone())),
                permission: input
                    .permission
                    .clone()
                    .or_else(|| contract.and_then(|contract| contract.permission.clone())),
                operation_type: contract.map(|contract| contract.operation_type.clone()),
                status: status.to_string(),
                last_probe_at: input.last_probe_at,
                sample_count: input.sample_count,
                error_class: input.error_class.clone(),
                evidence_refs: input.evidence_refs.clone(),
                blockers,
            },
        );
    }
    for operation_id in required_operations {
        if !statuses.contains_key(&operation_id) {
            let contract = operation_contracts.get(&operation_id);
            statuses.insert(
                operation_id.clone(),
                WorkflowPackConnectorOperationStatus {
                    operation_id: operation_id.clone(),
                    api_name: contract.and_then(|contract| contract.api_name.clone()),
                    permission: contract.and_then(|contract| contract.permission.clone()),
                    operation_type: contract.map(|contract| contract.operation_type.clone()),
                    status: "blocked".to_string(),
                    last_probe_at: None,
                    sample_count: None,
                    error_class: Some("missing_probe".to_string()),
                    evidence_refs: Vec::new(),
                    blockers: vec![format!(
                        "connector {} required read operation {} is missing readiness evidence",
                        connector.id, operation_id
                    )],
                },
            );
        }
    }
    Ok(statuses.into_values().collect())
}

pub(crate) fn workflow_pack_connector_lane_impacts(
    lane_requirements: &[WorkflowPackConnectorLaneRequirement],
    operation_statuses: &[WorkflowPackConnectorOperationStatus],
) -> BTreeMap<String, WorkflowPackConnectorLaneImpact> {
    let status_by_operation: BTreeMap<_, _> = operation_statuses
        .iter()
        .map(|operation| (operation.operation_id.as_str(), operation.status.as_str()))
        .collect();
    lane_requirements
        .iter()
        .map(|lane| {
            let blocked_required: Vec<_> = lane
                .required_operations
                .iter()
                .filter(|operation_id| {
                    status_by_operation.get(operation_id.as_str()) != Some(&"ready")
                })
                .cloned()
                .collect();
            let degraded_optional: Vec<_> = lane
                .optional_operations
                .iter()
                .chain(lane.controlled_write_operations.iter())
                .filter(|operation_id| {
                    matches!(
                        status_by_operation.get(operation_id.as_str()),
                        Some(&"degraded" | &"blocked")
                    )
                })
                .cloned()
                .collect();
            let (status, degraded_reason) = if !blocked_required.is_empty() {
                (
                    "blocked".to_string(),
                    format!(
                        "missing required operations: {}",
                        blocked_required.join(", ")
                    ),
                )
            } else if !degraded_optional.is_empty() {
                (
                    "degraded".to_string(),
                    format!(
                        "optional or controlled operations degraded: {}",
                        degraded_optional.join(", ")
                    ),
                )
            } else {
                ("ready".to_string(), String::new())
            };
            (
                lane.lane_id.clone(),
                WorkflowPackConnectorLaneImpact {
                    status: status.clone(),
                    enabled_workflows: if status == "blocked" {
                        Vec::new()
                    } else {
                        vec![lane.lane_id.clone()]
                    },
                    blocked_workflows: if status == "blocked" {
                        vec![lane.lane_id.clone()]
                    } else {
                        Vec::new()
                    },
                    degraded_reason,
                },
            )
        })
        .collect()
}

pub(crate) fn workflow_pack_profile_value(
    manifest: &workflow_pack::WorkflowPackManifest,
    package_dir: &FsPath,
    profile_id: &str,
) -> Result<Option<Value>, AppError> {
    let Some(profile) = manifest
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
    else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(package_dir.join(&profile.path)).with_context(|| {
        format!(
            "failed to read workflow pack profile {} at {}",
            profile.id, profile.path
        )
    })?;
    serde_yaml::from_str::<Value>(&content)
        .map(Some)
        .map_err(|error| AppError::bad_request(error.to_string()))
}

pub(crate) fn workflow_pack_connector_file(
    package_dir: &FsPath,
    connector: &workflow_pack::ConnectorRef,
) -> Result<Option<Value>, AppError> {
    let path = package_dir.join(&connector.path);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read workflow pack connector {} at {}",
            connector.id, connector.path
        )
    })?;
    serde_yaml::from_str::<Value>(&content)
        .map(Some)
        .map_err(|error| AppError::bad_request(error.to_string()))
}

pub(crate) fn workflow_pack_connector_yaml_entry<'a>(
    connector_file: &'a Value,
    connector_id: &str,
) -> Option<&'a Value> {
    connector_file
        .get("connectors")
        .and_then(Value::as_array)
        .and_then(|connectors| {
            connectors.iter().find(|candidate| {
                candidate
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == connector_id)
            })
        })
}

pub(crate) fn workflow_pack_connector_yaml_array(
    connector_file: &Value,
    connector_id: &str,
    path: &[&str],
) -> Option<Vec<String>> {
    let mut cursor = workflow_pack_connector_yaml_entry(connector_file, connector_id)?;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    Some(workflow_pack_string_vec(Some(cursor)))
}

pub(crate) fn workflow_pack_string_set(value: Option<&Value>) -> BTreeSet<String> {
    workflow_pack_string_vec(value).into_iter().collect()
}

pub(crate) fn workflow_pack_string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn json_field_present(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(map)) => !map.is_empty(),
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

pub(crate) fn resolve_workflow_pack_manifest_path(input: &str) -> Result<PathBuf, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request("manifest_path is required"));
    }
    let path = PathBuf::from(trimmed);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AppError::bad_request(
            "manifest_path must not contain parent directory components",
        ));
    }
    let path = if path.is_file() {
        path
    } else if path.is_relative() {
        project_file_path(trimmed).unwrap_or(path)
    } else {
        path
    };
    if !path.is_file() {
        return Err(AppError::bad_request(format!(
            "workflow pack manifest {} does not exist",
            path.display()
        )));
    }
    // Verify the resolved path is inside the project tree to prevent path traversal
    // via symlinks or absolute paths pointing outside the repo.
    let canonical = std::fs::canonicalize(&path).map_err(|_| {
        AppError::bad_request(format!(
            "workflow pack manifest {} cannot be resolved",
            path.display()
        ))
    })?;
    let project_root =
        std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    if !canonical.starts_with(&project_root) {
        return Err(AppError::bad_request(
            "manifest_path must point to a file within the project directory",
        ));
    }
    Ok(canonical)
}

pub(crate) fn workflow_pack_kind_label(kind: &workflow_pack::PackKind) -> &'static str {
    match kind {
        workflow_pack::PackKind::WorkflowPack => "WorkflowPack",
        workflow_pack::PackKind::DomainPack => "DomainPack",
    }
}

pub(crate) fn workflow_pack_connector_kind_label(
    kind: &workflow_pack::ConnectorKind,
) -> &'static str {
    match kind {
        workflow_pack::ConnectorKind::Mcp => "mcp",
        workflow_pack::ConnectorKind::Native => "native",
    }
}

pub(crate) fn workflow_pack_manifest_summary(
    manifest: &workflow_pack::WorkflowPackManifest,
    report: &workflow_pack::WorkflowPackValidationReport,
) -> Value {
    json!({
        "validated_file_count": report.validated_file_count,
        "agent_count": report.agent_count,
        "connector_count": report.connector_count,
        "required_eval_gate_count": report.required_eval_gate_count,
        "semantic_scopes": manifest.semantic_scopes,
        "capabilities": manifest.capabilities,
        "profiles": manifest.profiles.iter().map(|profile| json!({
            "id": profile.id,
            "required": profile.required,
        })).collect::<Vec<_>>(),
        "workflows": manifest.workflows.iter().map(|workflow| json!({
            "id": workflow.id,
            "entry_agent": workflow.entry_agent,
        })).collect::<Vec<_>>(),
        "agents": manifest.agents.iter().map(|agent| json!({
            "id": agent.id,
            "role": agent.role,
            "external_write_count": agent.tool_scope.external_write.len(),
            "handoff_count": agent.handoffs.len(),
            "approval_handoff_count": agent.handoffs.iter().filter(|handoff| handoff.approval_required).count(),
        })).collect::<Vec<_>>(),
        "connectors": manifest.connectors.iter().map(|connector| json!({
            "id": connector.id,
            "kind": workflow_pack_connector_kind_label(&connector.kind),
            "required_permission_count": connector.required_permissions.len(),
            "writes": {
                "enabled": connector.writes.enabled,
                "approval_required": connector.writes.approval_required,
            },
            "provenance_required": connector.provenance.required,
            "data_quality": connector.data_quality.as_ref().map(|quality| json!({
                "min_sample_count": quality.min_sample_count,
                "max_age_hours": quality.max_age_hours,
                "citation_required": quality.citation_required,
                "required_metadata_field_count": quality.required_metadata_fields.len(),
                "required_content_field_count": quality.required_content_fields.len(),
            })),
        })).collect::<Vec<_>>(),
        "actions": manifest.actions.iter().map(|action| json!({
            "id": action.id,
            "required": action.required,
        })).collect::<Vec<_>>(),
        "policies": manifest.policies.iter().map(|policy| json!({
            "id": policy.id,
            "required": policy.required,
        })).collect::<Vec<_>>(),
        "evals": manifest.evals.iter().map(|eval| json!({
            "id": eval.id,
            "required": eval.gate.required,
            "min_score": eval.gate.min_score,
        })).collect::<Vec<_>>(),
        "release_gates": manifest.release_gates.iter().map(|gate| json!({
            "id": gate.id,
            "gate_type": gate.gate_type,
            "required": gate.required,
        })).collect::<Vec<_>>(),
        "onboarding": manifest.onboarding.as_ref().map(|onboarding| json!({
            "workflow": onboarding.workflow,
            "required_profile_count": onboarding.required_profiles.len(),
            "profile_schema_count": onboarding.profile_schemas.len(),
            "eval": onboarding.eval,
        })),
    })
}

pub(crate) async fn record_workflow_pack_installation_audit(
    state: &AppState,
    installation: &WorkflowPackInstallation,
    action: &str,
    details: Value,
) -> Result<AuditLog, AppError> {
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            action,
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "pack_id": installation.pack_id,
                "kind": installation.kind,
                "version": installation.version,
                "status": installation.status,
                "eval_gate_status": installation.eval_gate_status,
                "release_gate_status": installation.release_gate_status,
                "details": details,
            }),
        ))
        .await
}

pub(crate) async fn record_workflow_pack_profile_asset_bootstrap_audit(
    state: &AppState,
    installation: &WorkflowPackInstallation,
    profile_assets: &[WorkflowPackProfileAsset],
) -> Result<(), AppError> {
    if profile_assets.is_empty() {
        return Ok(());
    }
    state
        .append_audit_log(new_audit_log(
            None,
            "user",
            None,
            "workflow_pack.onboarding_defaults_bootstrapped",
            "workflow_pack_installation",
            Some(installation.id),
            json!({
                "pack_id": installation.pack_id,
                "version": installation.version,
                "profile_count": profile_assets.len(),
                "profile_ids": profile_assets
                    .iter()
                    .map(|profile| profile.profile_id.clone())
                    .collect::<Vec<_>>(),
                "versions": profile_assets
                    .iter()
                    .map(|profile| json!({
                        "profile_id": profile.profile_id,
                        "version": profile.version,
                    }))
                    .collect::<Vec<_>>(),
            }),
        ))
        .await?;
    Ok(())
}
