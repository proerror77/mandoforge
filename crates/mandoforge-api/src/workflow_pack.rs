use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

const SUPPORTED_SCHEMA_VERSION: &str = "workflowpack.mandoforge.dev/v1";

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowPackManifest {
    pub schema_version: String,
    pub kind: PackKind,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub semantic_scopes: BTreeMap<String, String>,
    #[serde(default)]
    pub extends: Vec<PackExtensionRef>,
    #[serde(default)]
    pub profiles: Vec<PackFileRef>,
    #[serde(default)]
    pub skills: Vec<PackFileRef>,
    #[serde(default)]
    pub workflows: Vec<WorkflowRef>,
    #[serde(default)]
    pub agents: Vec<AgentRef>,
    #[serde(default)]
    pub connectors: Vec<ConnectorRef>,
    #[serde(default)]
    pub actions: Vec<PackFileRef>,
    #[serde(default)]
    pub schemas: Vec<PackFileRef>,
    #[serde(default)]
    pub policies: Vec<PackFileRef>,
    #[serde(default)]
    pub evals: Vec<EvalRef>,
    #[serde(default)]
    pub release_gates: Vec<ReleaseGateRef>,
    #[serde(default)]
    pub onboarding: Option<OnboardingContract>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PackKind {
    WorkflowPack,
    DomainPack,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackFileRef {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackExtensionRef {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub semantic_scopes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowRef {
    pub id: String,
    pub path: String,
    pub entry_agent: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentRef {
    pub id: String,
    pub path: String,
    pub role: AgentRole,
    #[serde(default)]
    pub tool_scope: ToolScope,
    #[serde(default)]
    pub handoffs: Vec<HandoffRule>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Reader,
    Analyzer,
    Writer,
    Manager,
    Orchestrator,
    Executor,
    Monitor,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ToolScope {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
    #[serde(default)]
    pub external_write: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HandoffRule {
    pub target_agent: String,
    #[serde(default)]
    pub intents: Vec<String>,
    pub risk_level: RiskLevel,
    #[serde(default)]
    pub approval_required: bool,
    pub schema: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectorRef {
    pub id: String,
    pub kind: ConnectorKind,
    pub path: String,
    #[serde(default)]
    pub required_permissions: Vec<String>,
    #[serde(default)]
    pub writes: ConnectorWrites,
    pub provenance: ConnectorProvenance,
    pub tenant_scope: TenantScope,
    pub prompt_injection_boundary: PromptInjectionBoundary,
    #[serde(default)]
    pub data_quality: Option<ConnectorDataQualityContract>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    Mcp,
    Native,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ConnectorWrites {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub approval_required: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectorProvenance {
    pub required: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TenantScope {
    pub tenant_id: String,
    pub workspace_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PromptInjectionBoundary {
    pub treat_results_as_data: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectorDataQualityContract {
    pub min_sample_count: usize,
    pub max_age_hours: i64,
    #[serde(default)]
    pub citation_required: bool,
    #[serde(default)]
    pub required_metadata_fields: Vec<String>,
    #[serde(default)]
    pub required_content_fields: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalRef {
    pub id: String,
    pub path: String,
    pub gate: EvalGate,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalGate {
    #[serde(default)]
    pub required: bool,
    pub min_score: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReleaseGateRef {
    pub id: String,
    pub gate_type: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OnboardingContract {
    pub workflow: String,
    #[serde(default)]
    pub required_profiles: Vec<String>,
    #[serde(default)]
    pub profile_schemas: Vec<PackFileRef>,
    pub eval: String,
}

#[derive(Debug, Deserialize)]
struct AgentFileContract {
    id: String,
    role: AgentRole,
    instructions: String,
}

#[derive(Debug, Deserialize)]
struct ToolScopePolicy {
    roles: BTreeMap<String, ToolScope>,
}

#[derive(Debug, Deserialize)]
struct WorkflowFileContract {
    id: String,
    entry_agent: String,
    #[serde(default)]
    semantic_scopes: BTreeMap<String, String>,
    #[serde(default)]
    observability: Option<WorkflowObservabilityContract>,
    #[serde(default)]
    steps: Vec<WorkflowStepContract>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepContract {
    agent: String,
    #[serde(default)]
    handoff_intent: Option<String>,
    #[serde(default)]
    required_profiles: Vec<String>,
    #[serde(default)]
    required_schemas: Vec<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    output_schema: Option<String>,
    #[serde(default)]
    observability: Option<WorkflowStepObservabilityContract>,
}

#[derive(Debug, Deserialize)]
struct WorkflowObservabilityContract {
    #[serde(default)]
    expected_events: Vec<String>,
    #[serde(default)]
    required_evidence: Vec<String>,
    #[serde(default)]
    budget: Option<WorkflowBudgetContract>,
    #[serde(default)]
    failure_policy: Option<WorkflowFailurePolicyContract>,
}

#[derive(Debug, Deserialize)]
struct WorkflowBudgetContract {
    #[serde(default)]
    max_turns: Option<i64>,
    #[serde(default)]
    max_tool_calls: Option<i64>,
    #[serde(default)]
    max_runtime_seconds: Option<i64>,
    #[serde(default)]
    max_cost_usd_micros: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct WorkflowFailurePolicyContract {
    #[serde(default)]
    status_values: Vec<String>,
    #[serde(default)]
    report_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepObservabilityContract {
    step_key: String,
    #[serde(default)]
    expected_events: Vec<String>,
    #[serde(default)]
    required_evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ActionTypeContract {
    id: String,
    object_type: String,
    connector_id: String,
    operation_id: String,
    side_effect_class: String,
    #[serde(default)]
    parameters: Vec<ActionParameterContract>,
    validations: ActionValidationContract,
    effects: ActionEffectsContract,
    approval: ActionApprovalContract,
}

#[derive(Debug, Deserialize)]
struct ActionParameterContract {
    name: String,
    #[serde(default)]
    required: bool,
}

#[derive(Debug, Deserialize)]
struct ActionValidationContract {
    #[serde(default)]
    rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ActionEffectsContract {
    native_connector_call: String,
    audit_event: String,
}

#[derive(Debug, Deserialize)]
struct ActionApprovalContract {
    approval_required: bool,
    approval_commit_token_required: bool,
    payload_digest_required: bool,
}

#[derive(Debug, Deserialize)]
struct ConnectorFileContract {
    #[serde(default)]
    connectors: Vec<ConnectorFileEntry>,
}

#[derive(Debug, Deserialize)]
struct ConnectorFileEntry {
    id: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    auth: Option<ConnectorAuthContract>,
    #[serde(default)]
    read_operations: Vec<ConnectorOperationContract>,
    #[serde(default)]
    write_operations: Vec<ConnectorOperationContract>,
    #[serde(default)]
    readiness_probes: serde_yaml::Value,
    #[serde(default)]
    approval_commit_binding: Option<ConnectorApprovalCommitBindingContract>,
    #[serde(default)]
    prompt_injection_boundary: Option<ConnectorPromptBoundaryContract>,
    #[serde(default)]
    adapter_contract: Option<ConnectorAdapterContract>,
}

#[derive(Debug, Deserialize)]
struct ConnectorAuthContract {
    #[serde(default)]
    required_secrets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectorOperationContract {
    id: String,
    #[serde(default)]
    api_name: Option<String>,
    permission: String,
    #[serde(default)]
    object_types: Vec<String>,
    #[serde(default)]
    approval_required: bool,
    request_contract: ConnectorRequestContract,
    response_contract: ConnectorResponseContract,
}

#[derive(Debug, Deserialize)]
struct ConnectorRequestContract {
    #[serde(default)]
    required_fields: Vec<String>,
    #[serde(default)]
    forbidden_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectorResponseContract {
    #[serde(default)]
    required_fields: Vec<String>,
    #[serde(default)]
    evidence_id_field: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectorApprovalCommitBindingContract {
    #[serde(default)]
    required_for_write_operations: bool,
    #[serde(default)]
    bind_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectorPromptBoundaryContract {
    #[serde(default)]
    treat_results_as_data: bool,
}

#[derive(Debug, Deserialize)]
struct ConnectorAdapterContract {
    adapter_id: String,
    runtime: String,
    implementation: String,
    live_execution: String,
    #[serde(default)]
    dry_run_supported: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorkflowPackValidationReport {
    pub pack_id: String,
    pub schema_version: String,
    pub validated_file_count: usize,
    pub agent_count: usize,
    pub connector_count: usize,
    pub required_eval_gate_count: usize,
}

impl WorkflowPackManifest {
    pub fn from_yaml_str(input: &str) -> Result<Self> {
        serde_yaml::from_str(input).map_err(Into::into)
    }

    pub fn validate_package_dir(&self, package_dir: &Path) -> Result<WorkflowPackValidationReport> {
        self.validate_metadata()?;

        let mut ids_by_section: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
        let mut file_count = 0usize;
        self.validate_extensions(package_dir)?;

        for item in &self.profiles {
            validate_ref("profiles", item, package_dir, &mut ids_by_section)?;
            file_count += 1;
        }
        for item in &self.skills {
            validate_ref("skills", item, package_dir, &mut ids_by_section)?;
            file_count += 1;
        }
        for item in &self.schemas {
            validate_ref("schemas", item, package_dir, &mut ids_by_section)?;
            file_count += 1;
        }
        for item in &self.policies {
            validate_ref("policies", item, package_dir, &mut ids_by_section)?;
            file_count += 1;
        }
        for action in &self.actions {
            validate_ref("actions", action, package_dir, &mut ids_by_section)?;
            self.validate_action_type_file(action, package_dir)?;
            file_count += 1;
        }
        let tool_scope_policy = self.load_tool_scope_policy(package_dir)?;

        let agent_ids =
            self.validate_agents(package_dir, &tool_scope_policy, &mut ids_by_section)?;
        file_count += self.agents.len();

        for workflow in &self.workflows {
            validate_id("workflows", &workflow.id)?;
            insert_unique_id(&mut ids_by_section, "workflows", &workflow.id)?;
            validate_relative_existing_path(package_dir, &workflow.path)?;
            if !agent_ids.contains(workflow.entry_agent.as_str()) {
                bail!(
                    "workflow {} references missing entry_agent {}",
                    workflow.id,
                    workflow.entry_agent
                );
            }
            self.validate_workflow_file_contract(
                workflow,
                package_dir,
                &agent_ids,
                &ids_by_section,
            )?;
            file_count += 1;
        }

        for connector in &self.connectors {
            self.validate_connector(connector, package_dir, &mut ids_by_section)?;
            if matches!(&connector.kind, ConnectorKind::Native) {
                self.validate_connector_file_contract(connector, package_dir)?;
            }
            file_count += 1;
        }

        let required_eval_gate_count = self.validate_evals(package_dir, &mut ids_by_section)?;
        file_count += self.evals.len();

        file_count += self.validate_onboarding(package_dir, &mut ids_by_section)?;

        if self.workflows.is_empty() {
            bail!("manifest must declare at least one workflow");
        }
        if self.profiles.is_empty() {
            bail!("manifest must declare at least one profile");
        }
        if self.skills.is_empty() {
            bail!("manifest must declare at least one skill");
        }
        if self.agents.is_empty() {
            bail!("manifest must declare at least one agent");
        }
        if self.connectors.is_empty() {
            bail!("manifest must declare at least one connector boundary");
        }
        if self.schemas.is_empty() {
            bail!("manifest must declare at least one schema");
        }
        if self.policies.is_empty() {
            bail!("manifest must declare at least one policy");
        }
        if required_eval_gate_count == 0 {
            bail!("manifest must declare at least one required eval gate");
        }
        if !self.release_gates.iter().any(|gate| gate.required) {
            bail!("manifest must declare at least one required release gate");
        }

        for gate in &self.release_gates {
            validate_id("release_gates", &gate.id)?;
            insert_unique_id(&mut ids_by_section, "release_gates", &gate.id)?;
            if gate.gate_type.trim().is_empty() {
                bail!("release gate {} must declare gate_type", gate.id);
            }
        }

        Ok(WorkflowPackValidationReport {
            pack_id: self.id.clone(),
            schema_version: self.schema_version.clone(),
            validated_file_count: file_count,
            agent_count: self.agents.len(),
            connector_count: self.connectors.len(),
            required_eval_gate_count,
        })
    }

    fn validate_metadata(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            bail!(
                "unsupported workflow pack schema_version {}; expected {}",
                self.schema_version,
                SUPPORTED_SCHEMA_VERSION
            );
        }
        validate_id("manifest", &self.id)?;
        validate_semver_like(&self.version)?;
        if self.name.trim().is_empty() {
            bail!("manifest name is required");
        }
        if self.description.trim().is_empty() {
            bail!("manifest description is required");
        }
        if self.capabilities.is_empty() {
            bail!("manifest must declare at least one capability");
        }
        for capability in &self.capabilities {
            validate_id("capabilities", capability)?;
        }
        if self.kind == PackKind::DomainPack {
            validate_domain_semantic_scopes("manifest semantic_scopes", &self.semantic_scopes)?;
        }
        Ok(())
    }

    fn validate_extensions(&self, package_dir: &Path) -> Result<()> {
        for extension in &self.extends {
            validate_id("extends", &extension.id)?;
            if let Some(version) = extension.version.as_deref() {
                validate_semver_like(version)?;
            }
            if !extension.semantic_scopes.is_empty() {
                validate_domain_semantic_scopes(
                    &format!("extends {} semantic_scopes", extension.id),
                    &extension.semantic_scopes,
                )?;
            }
            if extension.required {
                let Some(packs_root) = package_dir.parent() else {
                    bail!("extends {} cannot resolve packs root", extension.id);
                };
                let extended_manifest = packs_root.join(&extension.id).join("package.yaml");
                if !extended_manifest.is_file() {
                    bail!(
                        "extends {} requires sibling pack manifest {}",
                        extension.id,
                        extended_manifest.display()
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_agents(
        &self,
        package_dir: &Path,
        tool_scope_policy: &ToolScopePolicy,
        ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<BTreeSet<String>> {
        let mut agent_ids = BTreeSet::new();
        let mut declared_roles = BTreeSet::new();
        for agent in &self.agents {
            validate_id("agents", &agent.id)?;
            insert_unique_id(ids_by_section, "agents", &agent.id)?;
            validate_relative_existing_path(package_dir, &agent.path)?;
            self.validate_agent_file_contract(agent, package_dir)?;
            self.validate_agent_tool_scope(agent, tool_scope_policy)?;
            agent_ids.insert(agent.id.clone());
            declared_roles.insert(agent.role);
        }
        for role in [AgentRole::Reader, AgentRole::Analyzer, AgentRole::Writer] {
            if !declared_roles.contains(&role) {
                bail!(
                    "workflow pack must declare a {} agent for untrusted input workflows",
                    role.as_slug()
                );
            }
        }

        for agent in &self.agents {
            for handoff in &agent.handoffs {
                if !agent_ids.contains(&handoff.target_agent) {
                    bail!(
                        "agent {} handoff references missing target_agent {}",
                        agent.id,
                        handoff.target_agent
                    );
                }
                if handoff.intents.is_empty() {
                    bail!(
                        "agent {} handoff to {} must declare intents",
                        agent.id,
                        handoff.target_agent
                    );
                }
                for intent in &handoff.intents {
                    validate_id("handoff intents", intent)?;
                }
                if handoff.risk_level == RiskLevel::High && !handoff.approval_required {
                    bail!(
                        "agent {} high-risk handoff to {} must require approval",
                        agent.id,
                        handoff.target_agent
                    );
                }
                validate_relative_existing_path(package_dir, &handoff.schema)?;
            }
        }

        Ok(agent_ids)
    }

    fn validate_workflow_file_contract(
        &self,
        workflow: &WorkflowRef,
        package_dir: &Path,
        agent_ids: &BTreeSet<String>,
        ids_by_section: &BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<()> {
        let input = fs::read_to_string(package_dir.join(&workflow.path))?;
        let contract: WorkflowFileContract = serde_yaml::from_str(&input)?;
        if contract.id != workflow.id {
            bail!(
                "workflow file {} id {} must match manifest workflow {}",
                workflow.path,
                contract.id,
                workflow.id
            );
        }
        if contract.entry_agent != workflow.entry_agent {
            bail!(
                "workflow file {} entry_agent {} must match manifest entry_agent {}",
                workflow.path,
                contract.entry_agent,
                workflow.entry_agent
            );
        }
        if self.kind == PackKind::DomainPack {
            validate_domain_semantic_scopes(
                &format!("workflow {} semantic_scopes", workflow.id),
                &contract.semantic_scopes,
            )?;
            self.validate_domain_workflow_observability(workflow.id.as_str(), &contract)?;
        }
        if contract.steps.is_empty() {
            bail!("workflow {} must declare steps", workflow.id);
        }

        let mut previous_agent: Option<&str> = None;
        let mut step_keys = BTreeSet::new();
        for (index, step) in contract.steps.iter().enumerate() {
            if !agent_ids.contains(&step.agent) {
                bail!(
                    "workflow {} step {} references missing agent {}",
                    workflow.id,
                    index + 1,
                    step.agent
                );
            }
            if index == 0 && step.agent != workflow.entry_agent {
                bail!(
                    "workflow {} first step agent {} must match entry_agent {}",
                    workflow.id,
                    step.agent,
                    workflow.entry_agent
                );
            }
            for profile_id in &step.required_profiles {
                validate_id("workflow required profile", profile_id)?;
                if !contains_id(ids_by_section, "profiles", profile_id) {
                    bail!(
                        "workflow {} step {} required_profile {} must reference a declared profile",
                        workflow.id,
                        index + 1,
                        profile_id
                    );
                }
            }
            for schema_id in step
                .required_schemas
                .iter()
                .chain(step.output_schema.iter())
            {
                validate_id("workflow schema", schema_id)?;
                if !contains_id(ids_by_section, "schemas", schema_id) {
                    bail!(
                        "workflow {} step {} schema {} must reference a declared schema",
                        workflow.id,
                        index + 1,
                        schema_id
                    );
                }
            }
            for skill_id in &step.skills {
                validate_id("workflow skill", skill_id)?;
                if !contains_id(ids_by_section, "skills", skill_id) {
                    bail!(
                        "workflow {} step {} skill {} must reference a declared skill",
                        workflow.id,
                        index + 1,
                        skill_id
                    );
                }
            }
            if let Some(intent) = step.handoff_intent.as_deref() {
                let Some(source_agent) = previous_agent else {
                    bail!(
                        "workflow {} first step cannot declare handoff_intent {}",
                        workflow.id,
                        intent
                    );
                };
                validate_id("workflow handoff_intent", intent)?;
                if !self.agent_handoff_allows_intent(source_agent, &step.agent, intent) {
                    bail!(
                        "workflow {} handoff {} -> {} must declare intent {} in manifest",
                        workflow.id,
                        source_agent,
                        step.agent,
                        intent
                    );
                }
            }
            if self.kind == PackKind::DomainPack {
                self.validate_domain_workflow_step_observability(
                    workflow.id.as_str(),
                    index + 1,
                    step,
                    &mut step_keys,
                )?;
            }
            previous_agent = Some(&step.agent);
        }
        Ok(())
    }

    fn validate_domain_workflow_observability(
        &self,
        workflow_id: &str,
        contract: &WorkflowFileContract,
    ) -> Result<()> {
        let observability = contract.observability.as_ref().ok_or_else(|| {
            anyhow::anyhow!("domain workflow {} must declare observability", workflow_id)
        })?;
        validate_non_empty_string_list(
            &format!("workflow {} observability.expected_events", workflow_id),
            &observability.expected_events,
        )?;
        validate_non_empty_string_list(
            &format!("workflow {} observability.required_evidence", workflow_id),
            &observability.required_evidence,
        )?;
        let budget = observability.budget.as_ref().ok_or_else(|| {
            anyhow::anyhow!("workflow {} observability must declare budget", workflow_id)
        })?;
        validate_positive_optional_budget(workflow_id, "max_turns", budget.max_turns)?;
        validate_positive_optional_budget(workflow_id, "max_tool_calls", budget.max_tool_calls)?;
        validate_positive_optional_budget(
            workflow_id,
            "max_runtime_seconds",
            budget.max_runtime_seconds,
        )?;
        validate_positive_optional_budget(
            workflow_id,
            "max_cost_usd_micros",
            budget.max_cost_usd_micros,
        )?;
        if budget.max_turns.is_none()
            && budget.max_tool_calls.is_none()
            && budget.max_runtime_seconds.is_none()
            && budget.max_cost_usd_micros.is_none()
        {
            bail!(
                "workflow {} observability budget must declare at least one limit",
                workflow_id
            );
        }
        let failure_policy = observability.failure_policy.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "workflow {} observability must declare failure_policy",
                workflow_id
            )
        })?;
        validate_non_empty_string_list(
            &format!(
                "workflow {} observability.failure_policy.status_values",
                workflow_id
            ),
            &failure_policy.status_values,
        )?;
        validate_non_empty_string_list(
            &format!(
                "workflow {} observability.failure_policy.report_fields",
                workflow_id
            ),
            &failure_policy.report_fields,
        )?;
        Ok(())
    }

    fn validate_domain_workflow_step_observability(
        &self,
        workflow_id: &str,
        step_index: usize,
        step: &WorkflowStepContract,
        step_keys: &mut BTreeSet<String>,
    ) -> Result<()> {
        let observability = step.observability.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "workflow {} step {} must declare observability",
                workflow_id,
                step_index
            )
        })?;
        validate_id(
            "workflow step observability step_key",
            &observability.step_key,
        )?;
        if !step_keys.insert(observability.step_key.clone()) {
            bail!(
                "workflow {} step observability step_key {} must be unique",
                workflow_id,
                observability.step_key
            );
        }
        validate_non_empty_string_list(
            &format!(
                "workflow {} step {} observability.expected_events",
                workflow_id, step_index
            ),
            &observability.expected_events,
        )?;
        validate_non_empty_string_list(
            &format!(
                "workflow {} step {} observability.required_evidence",
                workflow_id, step_index
            ),
            &observability.required_evidence,
        )?;
        Ok(())
    }

    fn agent_handoff_allows_intent(
        &self,
        source_agent: &str,
        target_agent: &str,
        intent: &str,
    ) -> bool {
        self.agents
            .iter()
            .find(|agent| agent.id == source_agent)
            .is_some_and(|agent| {
                agent.handoffs.iter().any(|handoff| {
                    handoff.target_agent == target_agent
                        && handoff.intents.iter().any(|declared| declared == intent)
                })
            })
    }

    fn load_tool_scope_policy(&self, package_dir: &Path) -> Result<ToolScopePolicy> {
        let policy = self
            .policies
            .iter()
            .find(|policy| policy.id == "tool-scope")
            .ok_or_else(|| anyhow::anyhow!("manifest must declare tool-scope policy"))?;
        validate_relative_existing_path(package_dir, &policy.path)?;
        let input = fs::read_to_string(package_dir.join(&policy.path))?;
        let policy: ToolScopePolicy = serde_yaml::from_str(&input)?;
        if policy.roles.is_empty() {
            bail!("tool-scope policy must declare roles");
        }
        Ok(policy)
    }

    fn validate_agent_file_contract(&self, agent: &AgentRef, package_dir: &Path) -> Result<()> {
        let input = fs::read_to_string(package_dir.join(&agent.path))?;
        let contract: AgentFileContract = serde_yaml::from_str(&input)?;
        if contract.id != agent.id {
            bail!(
                "agent file {} id {} must match manifest agent {}",
                agent.path,
                contract.id,
                agent.id
            );
        }
        if contract.role != agent.role {
            bail!(
                "agent file {} role {} must match manifest role {}",
                agent.path,
                contract.role.as_slug(),
                agent.role.as_slug()
            );
        }
        if contract.instructions.trim().is_empty() {
            bail!("agent file {} must declare instructions", agent.path);
        }
        Ok(())
    }

    fn validate_agent_tool_scope(
        &self,
        agent: &AgentRef,
        tool_scope_policy: &ToolScopePolicy,
    ) -> Result<()> {
        let role_key = agent.role.as_slug();
        let policy_scope = tool_scope_policy
            .roles
            .get(role_key)
            .ok_or_else(|| anyhow::anyhow!("tool-scope policy must declare {} role", role_key))?;
        if policy_scope != &agent.tool_scope {
            bail!(
                "agent {} tool_scope must match tool-scope policy role {}",
                agent.id,
                role_key
            );
        }
        enforce_worker_role_tool_scope(agent)
    }

    fn validate_connector(
        &self,
        connector: &ConnectorRef,
        package_dir: &Path,
        ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<()> {
        validate_id("connectors", &connector.id)?;
        insert_unique_id(ids_by_section, "connectors", &connector.id)?;
        validate_relative_existing_path(package_dir, &connector.path)?;
        if connector.required_permissions.is_empty() {
            bail!(
                "connector {} must declare required_permissions",
                connector.id
            );
        }
        if connector.writes.enabled && !connector.writes.approval_required {
            bail!("connector {} writes must be approval-gated", connector.id);
        }
        if !connector.provenance.required {
            bail!("connector {} must require provenance", connector.id);
        }
        if connector.tenant_scope.tenant_id != "required" {
            bail!("connector {} must require tenant_id scope", connector.id);
        }
        if connector.tenant_scope.workspace_id != "required" {
            bail!("connector {} must require workspace_id scope", connector.id);
        }
        if !connector.prompt_injection_boundary.treat_results_as_data {
            bail!(
                "connector {} must treat connector results as data, not instructions",
                connector.id
            );
        }
        if let Some(contract) = connector.data_quality.as_ref() {
            if contract.min_sample_count == 0 {
                bail!(
                    "connector {} data_quality min_sample_count must be at least 1",
                    connector.id
                );
            }
            if contract.max_age_hours <= 0 {
                bail!(
                    "connector {} data_quality max_age_hours must be greater than 0",
                    connector.id
                );
            }
            if contract.required_metadata_fields.is_empty() {
                bail!(
                    "connector {} data_quality must declare required_metadata_fields",
                    connector.id
                );
            }
            if contract.required_content_fields.is_empty() {
                bail!(
                    "connector {} data_quality must declare required_content_fields",
                    connector.id
                );
            }
            for field in contract
                .required_metadata_fields
                .iter()
                .chain(contract.required_content_fields.iter())
            {
                if field.trim().is_empty() {
                    bail!(
                        "connector {} data_quality field names must be non-empty",
                        connector.id
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_connector_file_contract(
        &self,
        connector: &ConnectorRef,
        package_dir: &Path,
    ) -> Result<()> {
        let input = fs::read_to_string(package_dir.join(&connector.path))?;
        let contract: ConnectorFileContract = serde_yaml::from_str(&input)?;
        let connector_file = contract
            .connectors
            .iter()
            .find(|entry| entry.id == connector.id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "connector file {} must declare connector {}",
                    connector.path,
                    connector.id
                )
            })?;
        if connector_file.provider.trim().is_empty() {
            bail!("connector {} must declare provider", connector.id);
        }
        let adapter = connector_file.adapter_contract.as_ref().ok_or_else(|| {
            anyhow::anyhow!("connector {} must declare adapter_contract", connector.id)
        })?;
        validate_id("connector adapter_id", &adapter.adapter_id)?;
        if adapter.runtime != "native.connector.call" {
            bail!(
                "connector {} adapter_contract.runtime must be native.connector.call",
                connector.id
            );
        }
        if adapter.implementation.trim().is_empty() {
            bail!(
                "connector {} adapter_contract must declare implementation",
                connector.id
            );
        }
        if !matches!(
            adapter.live_execution.as_str(),
            "disabled_until_credentials_verified" | "approval_commit_only"
        ) {
            bail!(
                "connector {} adapter_contract.live_execution must be disabled_until_credentials_verified or approval_commit_only",
                connector.id
            );
        }
        if !adapter.dry_run_supported {
            bail!(
                "connector {} adapter_contract must support dry_run",
                connector.id
            );
        }
        let auth = connector_file
            .auth
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("connector {} must declare auth", connector.id))?;
        validate_non_empty_string_list(
            &format!("connector {} auth.required_secrets", connector.id),
            &auth.required_secrets,
        )?;
        if connector_file.read_operations.is_empty() {
            bail!("connector {} must declare read_operations", connector.id);
        }
        let required_permissions = connector
            .required_permissions
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut operation_ids = BTreeSet::new();
        for operation in &connector_file.read_operations {
            self.validate_connector_operation(
                connector,
                operation,
                "read_operations",
                &required_permissions,
                &mut operation_ids,
                true,
            )?;
            if operation
                .response_contract
                .evidence_id_field
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                bail!(
                    "connector {} read operation {} must declare response_contract.evidence_id_field",
                    connector.id,
                    operation.id
                );
            }
        }
        for operation in &connector_file.write_operations {
            self.validate_connector_operation(
                connector,
                operation,
                "write_operations",
                &required_permissions,
                &mut operation_ids,
                false,
            )?;
            if !operation.approval_required {
                bail!(
                    "connector {} write operation {} must require approval",
                    connector.id,
                    operation.id
                );
            }
            if operation.request_contract.forbidden_fields.is_empty() {
                bail!(
                    "connector {} write operation {} must declare request_contract.forbidden_fields",
                    connector.id,
                    operation.id
                );
            }
        }
        self.validate_connector_readiness_probes(connector, &connector_file.readiness_probes)?;
        let prompt_boundary = connector_file
            .prompt_injection_boundary
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "connector {} file must declare prompt_injection_boundary",
                    connector.id
                )
            })?;
        if !prompt_boundary.treat_results_as_data {
            bail!(
                "connector {} file must treat connector results as data",
                connector.id
            );
        }
        if connector.writes.enabled {
            if connector_file.write_operations.is_empty() {
                bail!(
                    "connector {} writes enabled but file has no write_operations",
                    connector.id
                );
            }
            let approval = connector_file
                .approval_commit_binding
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "connector {} writes require approval_commit_binding",
                        connector.id
                    )
                })?;
            if !approval.required_for_write_operations {
                bail!(
                    "connector {} approval_commit_binding must be required for write operations",
                    connector.id
                );
            }
            for required_field in [
                "tenant_id",
                "workspace_id",
                "connector_id",
                "operation_id",
                "object_id",
                "payload_digest",
                "approval_commit_token",
            ] {
                if !approval
                    .bind_fields
                    .iter()
                    .any(|field| field == required_field)
                {
                    bail!(
                        "connector {} approval_commit_binding missing {}",
                        connector.id,
                        required_field
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_connector_operation(
        &self,
        connector: &ConnectorRef,
        operation: &ConnectorOperationContract,
        section: &str,
        required_permissions: &BTreeSet<&str>,
        operation_ids: &mut BTreeSet<String>,
        require_object_types: bool,
    ) -> Result<()> {
        validate_id(
            &format!("connector {} operation", connector.id),
            &operation.id,
        )?;
        if !operation_ids.insert(operation.id.clone()) {
            bail!(
                "connector {} operation id {} must be unique",
                connector.id,
                operation.id
            );
        }
        if operation
            .api_name
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            bail!(
                "connector {} {} operation {} must declare api_name",
                connector.id,
                section,
                operation.id
            );
        }
        if !required_permissions.contains(operation.permission.as_str()) {
            bail!(
                "connector {} operation {} permission {} must be declared in manifest required_permissions",
                connector.id,
                operation.id,
                operation.permission
            );
        }
        if require_object_types {
            validate_non_empty_string_list(
                &format!(
                    "connector {} operation {} object_types",
                    connector.id, operation.id
                ),
                &operation.object_types,
            )?;
        }
        validate_non_empty_string_list(
            &format!(
                "connector {} operation {} request_contract.required_fields",
                connector.id, operation.id
            ),
            &operation.request_contract.required_fields,
        )?;
        validate_non_empty_string_list(
            &format!(
                "connector {} operation {} response_contract.required_fields",
                connector.id, operation.id
            ),
            &operation.response_contract.required_fields,
        )?;
        Ok(())
    }

    fn validate_connector_readiness_probes(
        &self,
        connector: &ConnectorRef,
        readiness_probes: &serde_yaml::Value,
    ) -> Result<()> {
        let object = readiness_probes.as_mapping().ok_or_else(|| {
            anyhow::anyhow!("connector {} must declare readiness_probes", connector.id)
        })?;
        for key in ["credential_probe", "tenant_scope_probe", "permission_probe"] {
            if !object.contains_key(serde_yaml::Value::String(key.to_string())) {
                bail!(
                    "connector {} readiness_probes missing {}",
                    connector.id,
                    key
                );
            }
        }
        Ok(())
    }

    fn validate_action_type_file(&self, action: &PackFileRef, package_dir: &Path) -> Result<()> {
        let input = fs::read_to_string(package_dir.join(&action.path))?;
        let contract: ActionTypeContract = serde_yaml::from_str(&input)?;
        if contract.id != action.id {
            bail!(
                "action file {} id {} must match manifest action {}",
                action.path,
                contract.id,
                action.id
            );
        }
        validate_id("actions", &contract.id)?;
        if contract.object_type.trim().is_empty() {
            bail!("action {} must declare object_type", contract.id);
        }
        validate_id("action connector_id", &contract.connector_id)?;
        validate_id("action operation_id", &contract.operation_id)?;
        if !self
            .connectors
            .iter()
            .any(|connector| connector.id == contract.connector_id)
        {
            bail!(
                "action {} references missing connector {}",
                contract.id,
                contract.connector_id
            );
        }
        if contract.side_effect_class.trim().is_empty() {
            bail!("action {} must declare side_effect_class", contract.id);
        }
        if contract.parameters.is_empty() {
            bail!("action {} must declare parameters", contract.id);
        }
        if !contract
            .parameters
            .iter()
            .any(|parameter| parameter.required)
        {
            bail!(
                "action {} must declare at least one required parameter",
                contract.id
            );
        }
        for parameter in &contract.parameters {
            if parameter.name.trim().is_empty() {
                bail!("action {} parameter name must not be empty", contract.id);
            }
        }
        validate_non_empty_string_list(
            &format!("action {} validations.rules", contract.id),
            &contract.validations.rules,
        )?;
        if contract.effects.native_connector_call != "native.connector.call" {
            bail!(
                "action {} effects.native_connector_call must be native.connector.call",
                contract.id
            );
        }
        if contract.effects.audit_event.trim().is_empty() {
            bail!("action {} effects.audit_event is required", contract.id);
        }
        if !contract.approval.approval_required {
            bail!("action {} must require approval", contract.id);
        }
        if !contract.approval.approval_commit_token_required {
            bail!("action {} must require approval_commit_token", contract.id);
        }
        if !contract.approval.payload_digest_required {
            bail!("action {} must require payload_digest", contract.id);
        }
        Ok(())
    }

    fn validate_evals(
        &self,
        package_dir: &Path,
        ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<usize> {
        let mut required_count = 0usize;
        for eval in &self.evals {
            validate_id("evals", &eval.id)?;
            insert_unique_id(ids_by_section, "evals", &eval.id)?;
            validate_relative_existing_path(package_dir, &eval.path)?;
            if !(0.0..=1.0).contains(&eval.gate.min_score) {
                bail!("eval {} gate min_score must be between 0 and 1", eval.id);
            }
            if eval.gate.required {
                required_count += 1;
            }
        }
        Ok(required_count)
    }

    fn validate_onboarding(
        &self,
        package_dir: &Path,
        ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<usize> {
        let onboarding = self
            .onboarding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("manifest must declare onboarding contract"))?;
        validate_id("onboarding workflow", &onboarding.workflow)?;
        if !contains_id(ids_by_section, "workflows", &onboarding.workflow) {
            bail!(
                "onboarding workflow {} must reference a declared workflow",
                onboarding.workflow
            );
        }
        validate_id("onboarding eval", &onboarding.eval)?;
        if !contains_id(ids_by_section, "evals", &onboarding.eval) {
            bail!(
                "onboarding eval {} must reference a declared eval",
                onboarding.eval
            );
        }
        if onboarding.required_profiles.is_empty() {
            bail!("onboarding contract must declare required_profiles");
        }
        for profile_id in &onboarding.required_profiles {
            validate_id("onboarding required profile", profile_id)?;
            if !contains_id(ids_by_section, "profiles", profile_id) {
                bail!(
                    "onboarding required profile {} must reference a declared profile",
                    profile_id
                );
            }
        }
        if onboarding.profile_schemas.is_empty() {
            bail!("onboarding contract must declare profile_schemas");
        }
        for schema in &onboarding.profile_schemas {
            validate_ref(
                "onboarding_profile_schemas",
                schema,
                package_dir,
                ids_by_section,
            )?;
        }
        Ok(onboarding.profile_schemas.len())
    }
}

impl AgentRole {
    fn as_slug(&self) -> &'static str {
        match self {
            AgentRole::Reader => "reader",
            AgentRole::Analyzer => "analyzer",
            AgentRole::Writer => "writer",
            AgentRole::Manager => "manager",
            AgentRole::Orchestrator => "orchestrator",
            AgentRole::Executor => "executor",
            AgentRole::Monitor => "monitor",
        }
    }
}

fn enforce_worker_role_tool_scope(agent: &AgentRef) -> Result<()> {
    match agent.role {
        AgentRole::Reader => {
            ensure_scope_subset(
                &agent.id,
                "reader read",
                &agent.tool_scope.read,
                &["connector.read", "file.read"],
            )?;
            if !agent.tool_scope.write.is_empty() || !agent.tool_scope.external_write.is_empty() {
                bail!("reader agent {} cannot declare write tool scopes", agent.id);
            }
        }
        AgentRole::Analyzer => {
            ensure_scope_subset(
                &agent.id,
                "analyzer read",
                &agent.tool_scope.read,
                &["profile.read", "schema.read"],
            )?;
            ensure_scope_contains(
                &agent.id,
                "analyzer read",
                &agent.tool_scope.read,
                "profile.read",
            )?;
            ensure_scope_contains(
                &agent.id,
                "analyzer read",
                &agent.tool_scope.read,
                "schema.read",
            )?;
            if !agent.tool_scope.write.is_empty() || !agent.tool_scope.external_write.is_empty() {
                bail!(
                    "analyzer agent {} cannot declare write tool scopes",
                    agent.id
                );
            }
        }
        AgentRole::Writer => {
            ensure_scope_subset(
                &agent.id,
                "writer read",
                &agent.tool_scope.read,
                &["profile.read"],
            )?;
            ensure_scope_subset(
                &agent.id,
                "writer write",
                &agent.tool_scope.write,
                &["artifact.write"],
            )?;
            if !agent
                .tool_scope
                .write
                .iter()
                .any(|scope| scope == "artifact.write")
            {
                bail!(
                    "writer agent {} must be limited to artifact.write",
                    agent.id
                );
            }
            if !agent.tool_scope.external_write.is_empty() {
                bail!(
                    "writer agent {} cannot declare external_write tool scopes",
                    agent.id
                );
            }
        }
        _ => {}
    }
    Ok(())
}

fn ensure_scope_subset(
    agent_id: &str,
    scope_name: &str,
    scopes: &[String],
    allowed: &[&str],
) -> Result<()> {
    for scope in scopes {
        if !allowed.iter().any(|allowed_scope| scope == allowed_scope) {
            bail!(
                "agent {} {} scope {} is outside allowed set {:?}",
                agent_id,
                scope_name,
                scope,
                allowed
            );
        }
    }
    Ok(())
}

fn ensure_scope_contains(
    agent_id: &str,
    scope_name: &str,
    scopes: &[String],
    required: &str,
) -> Result<()> {
    if scopes.iter().any(|scope| scope == required) {
        Ok(())
    } else {
        bail!(
            "agent {} {} scope must include {}",
            agent_id,
            scope_name,
            required
        )
    }
}

fn validate_ref(
    section: &'static str,
    item: &PackFileRef,
    package_dir: &Path,
    ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
) -> Result<()> {
    validate_id(section, &item.id)?;
    insert_unique_id(ids_by_section, section, &item.id)?;
    validate_relative_existing_path(package_dir, &item.path)
}

fn insert_unique_id<'a>(
    ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    section: &'static str,
    id: &str,
) -> Result<()> {
    let ids = ids_by_section.entry(section).or_default();
    if !ids.insert(id.to_string()) {
        bail!("duplicate {} id {}", section, id);
    }
    Ok(())
}

fn contains_id(
    ids_by_section: &BTreeMap<&'static str, BTreeSet<String>>,
    section: &'static str,
    id: &str,
) -> bool {
    ids_by_section
        .get(section)
        .is_some_and(|ids| ids.contains(id))
}

fn validate_domain_semantic_scopes(
    label: &str,
    semantic_scopes: &BTreeMap<String, String>,
) -> Result<()> {
    for required_key in ["domain_scope", "workflow_scope", "share_policy"] {
        let value = semantic_scopes
            .get(required_key)
            .map(|value| value.trim())
            .unwrap_or_default();
        if value.is_empty() {
            bail!("{label} must declare non-empty {required_key}");
        }
    }
    Ok(())
}

fn validate_non_empty_string_list(label: &str, values: &[String]) -> Result<()> {
    if values.is_empty() {
        bail!("{label} must not be empty");
    }
    for value in values {
        if value.trim().is_empty() {
            bail!("{label} must not contain empty values");
        }
    }
    Ok(())
}

fn validate_positive_optional_budget(
    workflow_id: &str,
    field_name: &str,
    value: Option<i64>,
) -> Result<()> {
    if value.is_some_and(|value| value <= 0) {
        bail!(
            "workflow {} observability budget {} must be greater than 0",
            workflow_id,
            field_name
        );
    }
    Ok(())
}

fn validate_id(section: &str, id: &str) -> Result<()> {
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
        && id
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && id
            .chars()
            .last()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if !valid {
        bail!("{} id {} must be lowercase slug text", section, id);
    }
    Ok(())
}

fn validate_semver_like(version: &str) -> Result<()> {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        bail!("manifest version {} must use MAJOR.MINOR.PATCH", version);
    }
    Ok(())
}

fn validate_relative_existing_path(package_dir: &Path, relative_path: &str) -> Result<()> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        bail!("pack path {} must be relative", relative_path);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "pack path {} must not escape the package directory",
            relative_path
        );
    }
    let full_path = package_dir.join(path);
    if !full_path.is_file() {
        bail!("pack path {} does not exist", relative_path);
    }
    Ok(())
}

pub fn validate_workflow_pack_manifest_path(
    manifest_path: &Path,
) -> Result<WorkflowPackValidationReport> {
    let input = fs::read_to_string(manifest_path)?;
    let manifest = WorkflowPackManifest::from_yaml_str(&input)?;
    let package_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    manifest.validate_package_dir(package_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_manifest_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("packs/ai-governance/package.yaml")
    }

    fn ecommerce_tmall_manifest_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("packs/ecommerce-tmall/package.yaml")
    }

    fn ecommerce_manifest_path(pack_id: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("packs")
            .join(pack_id)
            .join("package.yaml")
    }

    fn legal_manifest_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("packs/legal/package.yaml")
    }

    #[test]
    fn validates_ai_governance_workflow_pack_fixture() {
        let report = validate_workflow_pack_manifest_path(&fixture_manifest_path())
            .expect("AI Governance workflow pack fixture should validate");

        assert_eq!(report.pack_id, "ai-governance");
        assert_eq!(report.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(report.agent_count, 3);
        assert_eq!(report.connector_count, 1);
        assert_eq!(report.required_eval_gate_count, 2);
        assert!(report.validated_file_count >= 10);
    }

    #[test]
    fn validates_ecommerce_tmall_domain_pack_fixture() {
        let report = validate_workflow_pack_manifest_path(&ecommerce_tmall_manifest_path())
            .expect("Tmall ecommerce domain pack fixture should validate");

        assert_eq!(report.pack_id, "ecommerce-tmall");
        assert_eq!(report.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(report.agent_count, 5);
        assert_eq!(report.connector_count, 1);
        assert_eq!(report.required_eval_gate_count, 2);
        assert!(report.validated_file_count >= 30);
    }

    #[test]
    fn validates_ecommerce_expansion_domain_pack_fixtures() {
        for pack_id in [
            "ecommerce-core",
            "ecommerce-taobao",
            "ecommerce-xiaohongshu",
            "ecommerce-xianyu",
            "ecommerce-tiktok-shop",
            "ecommerce-amazon",
        ] {
            let report = validate_workflow_pack_manifest_path(&ecommerce_manifest_path(pack_id))
                .unwrap_or_else(|error| panic!("{pack_id} should validate: {error}"));

            assert_eq!(report.pack_id, pack_id);
            assert_eq!(report.schema_version, SUPPORTED_SCHEMA_VERSION);
            assert_eq!(report.agent_count, 3);
            assert_eq!(report.connector_count, 1);
            assert_eq!(report.required_eval_gate_count, 1);
            assert!(report.validated_file_count >= 20);
        }
    }

    #[test]
    fn validates_legal_domain_pack_fixture() {
        let report = validate_workflow_pack_manifest_path(&legal_manifest_path())
            .expect("Legal domain pack fixture should validate");

        assert_eq!(report.pack_id, "legal");
        assert_eq!(report.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(report.agent_count, 3);
        assert_eq!(report.connector_count, 1);
        assert_eq!(report.required_eval_gate_count, 2);
        assert!(report.validated_file_count >= 30);
    }

    #[test]
    fn rejects_domain_workflow_without_observability_contract() {
        let manifest = WorkflowPackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_string(),
            kind: PackKind::DomainPack,
            id: "domain-pack".to_string(),
            name: "Domain Pack".to_string(),
            version: "0.1.0".to_string(),
            description: "domain pack".to_string(),
            capabilities: vec!["review".to_string()],
            semantic_scopes: BTreeMap::new(),
            extends: vec![],
            profiles: vec![],
            skills: vec![],
            workflows: vec![],
            agents: vec![],
            connectors: vec![],
            actions: vec![],
            schemas: vec![],
            policies: vec![],
            evals: vec![],
            release_gates: vec![],
            onboarding: None,
        };
        let workflow = WorkflowFileContract {
            id: "review".to_string(),
            entry_agent: "reader".to_string(),
            semantic_scopes: BTreeMap::new(),
            observability: None,
            steps: vec![],
        };

        let error = manifest
            .validate_domain_workflow_observability("review", &workflow)
            .expect_err("domain workflow without observability must fail");

        assert!(
            error
                .to_string()
                .contains("domain workflow review must declare observability")
        );
    }

    #[test]
    fn rejects_domain_workflow_step_without_observability_contract() {
        let manifest = WorkflowPackManifest {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_string(),
            kind: PackKind::DomainPack,
            id: "domain-pack".to_string(),
            name: "Domain Pack".to_string(),
            version: "0.1.0".to_string(),
            description: "domain pack".to_string(),
            capabilities: vec!["review".to_string()],
            semantic_scopes: BTreeMap::new(),
            extends: vec![],
            profiles: vec![],
            skills: vec![],
            workflows: vec![],
            agents: vec![],
            connectors: vec![],
            actions: vec![],
            schemas: vec![],
            policies: vec![],
            evals: vec![],
            release_gates: vec![],
            onboarding: None,
        };
        let step = WorkflowStepContract {
            agent: "reader".to_string(),
            handoff_intent: None,
            required_profiles: vec![],
            required_schemas: vec![],
            skills: vec![],
            output_schema: None,
            observability: None,
        };
        let mut step_keys = BTreeSet::new();

        let error = manifest
            .validate_domain_workflow_step_observability("review", 1, &step, &mut step_keys)
            .expect_err("domain workflow step without observability must fail");

        assert!(
            error
                .to_string()
                .contains("workflow review step 1 must declare observability")
        );
    }

    #[test]
    fn rejects_high_risk_handoff_without_approval() {
        let input = r#"
schema_version: workflowpack.mandoforge.dev/v1
kind: WorkflowPack
id: bad-pack
name: Bad Pack
version: 0.1.0
description: invalid high-risk handoff
capabilities:
  - ai-governance
agents:
  - id: reader
    path: agents/reader.agent.yaml
    role: reader
    tool_scope:
      read: [connector.read, file.read]
    handoffs:
      - target_agent: writer
        intents: [publish]
        risk_level: high
        approval_required: false
        schema: schemas/handoff.schema.json
  - id: analyzer
    path: agents/analyzer.agent.yaml
    role: analyzer
    tool_scope:
      read: [profile.read, schema.read]
  - id: writer
    path: agents/writer.agent.yaml
    role: writer
    tool_scope:
      read: [profile.read]
      write: [artifact.write]
policies:
  - id: tool-scope
    path: policies/tool_scope.yaml
workflows:
  - id: triage
    path: workflows/triage.workflow.yaml
    entry_agent: reader
connectors:
  - id: docs
    kind: mcp
    path: connectors/mcp.yaml
    required_permissions: [document.read]
    writes:
      enabled: false
    provenance:
      required: true
    tenant_scope:
      tenant_id: required
      workspace_id: required
    prompt_injection_boundary:
      treat_results_as_data: true
evals:
  - id: regression
    path: evals/golden_cases.jsonl
    gate:
      required: true
      min_score: 1.0
release_gates:
  - id: eval-gate
    gate_type: eval
    required: true
"#;
        let manifest = WorkflowPackManifest::from_yaml_str(input).expect("manifest parses");
        let error = manifest
            .validate_package_dir(&fixture_manifest_path().parent().unwrap().to_path_buf())
            .expect_err("high-risk handoff without approval must fail");

        assert!(error.to_string().contains("must require approval"));
    }

    #[test]
    fn rejects_tool_scope_policy_mismatch_for_worker_roles() {
        let input = std::fs::read_to_string(fixture_manifest_path()).expect("fixture manifest");
        let input = input.replace(
            "write:\n        - artifact.write",
            "write:\n        - artifact.write\n        - external.message",
        );
        let manifest = WorkflowPackManifest::from_yaml_str(&input).expect("manifest parses");
        let error = manifest
            .validate_package_dir(&fixture_manifest_path().parent().unwrap().to_path_buf())
            .expect_err("manifest scope drift from policy must fail");

        assert!(
            error
                .to_string()
                .contains("tool_scope must match tool-scope policy role writer")
        );
    }

    #[test]
    fn rejects_unsafe_worker_role_tool_scopes() {
        let reader = AgentRef {
            id: "reader".to_string(),
            path: "agents/reader.agent.yaml".to_string(),
            role: AgentRole::Reader,
            tool_scope: ToolScope {
                write: vec!["artifact.write".to_string()],
                ..ToolScope::default()
            },
            handoffs: vec![],
        };
        let reader_error =
            enforce_worker_role_tool_scope(&reader).expect_err("reader writes must fail");
        assert!(
            reader_error
                .to_string()
                .contains("reader agent reader cannot declare write tool scopes")
        );

        let analyzer = AgentRef {
            id: "analyzer".to_string(),
            path: "agents/analyzer.agent.yaml".to_string(),
            role: AgentRole::Analyzer,
            tool_scope: ToolScope {
                read: vec!["connector.read".to_string()],
                ..ToolScope::default()
            },
            handoffs: vec![],
        };
        let analyzer_error =
            enforce_worker_role_tool_scope(&analyzer).expect_err("analyzer raw reads must fail");
        assert!(analyzer_error.to_string().contains("outside allowed set"));

        let writer = AgentRef {
            id: "writer".to_string(),
            path: "agents/writer.agent.yaml".to_string(),
            role: AgentRole::Writer,
            tool_scope: ToolScope {
                read: vec!["profile.read".to_string()],
                write: vec!["artifact.write".to_string()],
                external_write: vec!["email.send".to_string()],
            },
            handoffs: vec![],
        };
        let writer_error =
            enforce_worker_role_tool_scope(&writer).expect_err("writer external writes must fail");
        assert!(
            writer_error
                .to_string()
                .contains("writer agent writer cannot declare external_write tool scopes")
        );
    }

    #[test]
    fn rejects_connector_policy_boundary_drift() {
        let input = std::fs::read_to_string(fixture_manifest_path()).expect("fixture manifest");
        let input = input.replace(
            "treat_results_as_data: true",
            "treat_results_as_data: false",
        );
        let manifest = WorkflowPackManifest::from_yaml_str(&input).expect("manifest parses");
        let error = manifest
            .validate_package_dir(&fixture_manifest_path().parent().unwrap().to_path_buf())
            .expect_err("connector prompt-injection boundary drift must fail");

        assert!(
            error
                .to_string()
                .contains("must treat connector results as data")
        );
    }

    #[test]
    fn rejects_invalid_connector_data_quality_contract() {
        let input = std::fs::read_to_string(fixture_manifest_path()).expect("fixture manifest");
        let mut manifest = WorkflowPackManifest::from_yaml_str(&input).expect("manifest parses");
        manifest.connectors[0]
            .data_quality
            .as_mut()
            .expect("fixture declares connector data quality")
            .required_content_fields
            .clear();
        let error = manifest
            .validate_package_dir(&fixture_manifest_path().parent().unwrap().to_path_buf())
            .expect_err("connector data_quality contract should fail");

        assert!(
            error
                .to_string()
                .contains("data_quality must declare required_content_fields")
        );
    }
}
