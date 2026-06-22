use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DeploymentVersion {
    pub(crate) service: String,
    pub(crate) cargo_package_version: String,
    pub(crate) image_tag: Option<String>,
    pub(crate) git_sha: Option<String>,
    pub(crate) build_time: Option<String>,
    pub(crate) source: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProductionDeploymentVerifyRequest {
    #[serde(default)]
    pub(crate) expected_git_sha: Option<String>,
    #[serde(default)]
    pub(crate) expected_image_tag: Option<String>,
    #[serde(default)]
    pub(crate) target: Option<String>,
    #[serde(default)]
    pub(crate) require_match: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProductionAutoDeployRequest {
    pub(crate) target: String,
    #[serde(default)]
    pub(crate) git_sha: Option<String>,
    #[serde(default)]
    pub(crate) image_tag: Option<String>,
    #[serde(default)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Stage2CompletionReadiness {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) objective: String,
    pub(crate) audit_path: String,
    pub(crate) audit_present: bool,
    pub(crate) open_gap_count: usize,
    pub(crate) open_gaps: Vec<String>,
    pub(crate) evidence_requirements: Vec<Stage2EvidenceRequirement>,
    pub(crate) completion_blocked: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Stage2EvidenceRequirement {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) category: String,
    pub(crate) required_for_core: bool,
    pub(crate) required_for_stage2_production: bool,
    pub(crate) enterprise_optional: bool,
    pub(crate) gap: String,
    pub(crate) production_target: String,
    pub(crate) evidence_scripts: Vec<String>,
    pub(crate) evidence_job_manifests: Vec<String>,
    pub(crate) readiness_endpoints: Vec<String>,
    pub(crate) validation_endpoints: Vec<String>,
    pub(crate) required_flags: Vec<String>,
    pub(crate) required_artifacts: Vec<String>,
    pub(crate) required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EnterpriseProductCompletionReadiness {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) objective: String,
    pub(crate) contract_path: String,
    pub(crate) contract_present: bool,
    pub(crate) required_evidence_class: String,
    pub(crate) lane_count: usize,
    pub(crate) ready_lane_count: usize,
    pub(crate) pilot_ready_lane_count: usize,
    pub(crate) blocked_lane_count: usize,
    pub(crate) completion_blocked: bool,
    pub(crate) lanes: Vec<EnterpriseProductCompletionLane>,
    pub(crate) next_actions: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EnterpriseProductCompletionLane {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) current_evidence_class: String,
    pub(crate) required_evidence_class: String,
    pub(crate) current_boundary: String,
    pub(crate) production_target: String,
    pub(crate) readiness_endpoints: Vec<String>,
    pub(crate) evidence_scripts: Vec<String>,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) blockers: Vec<String>,
    pub(crate) next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EnterpriseSecurityAdminReadiness {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) status: String,
    pub(crate) required_evidence_class: String,
    pub(crate) check_count: usize,
    pub(crate) ready_check_count: usize,
    pub(crate) blocked_check_count: usize,
    pub(crate) completion_blocked: bool,
    pub(crate) checks: Vec<EnterpriseSecurityAdminCheck>,
    pub(crate) next_actions: Vec<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EnterpriseSecurityAdminCheck {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) current_evidence_class: String,
    pub(crate) required_evidence_class: String,
    pub(crate) evidence: Value,
    pub(crate) blockers: Vec<String>,
    pub(crate) next_actions: Vec<String>,
}
