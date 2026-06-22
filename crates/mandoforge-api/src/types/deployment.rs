use serde::{Deserialize, Serialize};

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
