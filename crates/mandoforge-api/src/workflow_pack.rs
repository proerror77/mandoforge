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
    pub schemas: Vec<PackFileRef>,
    #[serde(default)]
    pub policies: Vec<PackFileRef>,
    #[serde(default)]
    pub evals: Vec<EvalRef>,
    #[serde(default)]
    pub release_gates: Vec<ReleaseGateRef>,
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Reader,
    Analyzer,
    Writer,
    Orchestrator,
    Executor,
    Monitor,
}

#[derive(Debug, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        serde_yml::from_str(input).map_err(Into::into)
    }

    pub fn validate_package_dir(&self, package_dir: &Path) -> Result<WorkflowPackValidationReport> {
        self.validate_metadata()?;

        let mut ids_by_section: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
        let mut file_count = 0usize;

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

        let agent_ids = self.validate_agents(package_dir, &mut ids_by_section)?;
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
            file_count += 1;
        }

        for connector in &self.connectors {
            self.validate_connector(connector, package_dir, &mut ids_by_section)?;
            file_count += 1;
        }

        let required_eval_gate_count = self.validate_evals(package_dir, &mut ids_by_section)?;
        file_count += self.evals.len();

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
        Ok(())
    }

    fn validate_agents(
        &self,
        package_dir: &Path,
        ids_by_section: &mut BTreeMap<&'static str, BTreeSet<String>>,
    ) -> Result<BTreeSet<String>> {
        let mut agent_ids = BTreeSet::new();
        for agent in &self.agents {
            validate_id("agents", &agent.id)?;
            insert_unique_id(ids_by_section, "agents", &agent.id)?;
            validate_relative_existing_path(package_dir, &agent.path)?;
            agent_ids.insert(agent.id.clone());

            match agent.role {
                AgentRole::Reader => {
                    if !agent.tool_scope.write.is_empty()
                        || !agent.tool_scope.external_write.is_empty()
                    {
                        bail!("reader agent {} cannot declare write tool scopes", agent.id);
                    }
                }
                AgentRole::Writer => {
                    if !agent.tool_scope.external_write.is_empty() {
                        bail!(
                            "writer agent {} cannot declare external_write tool scopes",
                            agent.id
                        );
                    }
                }
                _ => {}
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

    #[test]
    fn validates_ai_governance_workflow_pack_fixture() {
        let report = validate_workflow_pack_manifest_path(&fixture_manifest_path())
            .expect("AI Governance workflow pack fixture should validate");

        assert_eq!(report.pack_id, "ai-governance");
        assert_eq!(report.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(report.agent_count, 3);
        assert_eq!(report.connector_count, 1);
        assert_eq!(report.required_eval_gate_count, 1);
        assert!(report.validated_file_count >= 10);
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
    handoffs:
      - target_agent: writer
        intents: [publish]
        risk_level: high
        approval_required: false
        schema: schemas/handoff.schema.json
  - id: writer
    path: agents/writer.agent.yaml
    role: writer
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
}
