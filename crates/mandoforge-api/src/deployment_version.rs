use crate::DeploymentVersion;

pub(crate) fn deployment_expected_value_matches(
    expected: Option<&str>,
    running: Option<&str>,
) -> bool {
    expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none_or(|expected| running == Some(expected))
}

pub(crate) fn deployment_version_from_env() -> DeploymentVersion {
    deployment_version_from_lookup(|key| std::env::var(key).ok())
}

pub(crate) fn deployment_version_from_lookup<F>(lookup: F) -> DeploymentVersion
where
    F: Fn(&str) -> Option<String>,
{
    let image_tag = trimmed_lookup(&lookup, "MANDOFORGE_IMAGE_TAG");
    let git_sha = trimmed_lookup(&lookup, "MANDOFORGE_GIT_SHA")
        .or_else(|| trimmed_lookup(&lookup, "GITHUB_SHA"));
    let build_time = trimmed_lookup(&lookup, "MANDOFORGE_BUILD_TIME");
    let source = if image_tag.is_some() || git_sha.is_some() || build_time.is_some() {
        "runtime_env".to_string()
    } else {
        "local_cargo_run".to_string()
    };
    DeploymentVersion {
        service: "mandoforge-api".to_string(),
        cargo_package_version: env!("CARGO_PKG_VERSION").to_string(),
        image_tag,
        git_sha,
        build_time,
        source,
    }
}

fn trimmed_lookup<F>(lookup: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
