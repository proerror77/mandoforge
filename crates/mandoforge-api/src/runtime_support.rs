use std::path::PathBuf;

use serde_json::Value;

pub(crate) fn env_bool(name: &str) -> bool {
    env_bool_lookup(name, &|key| std::env::var(key).ok())
}

pub(crate) fn env_bool_lookup<F>(name: &str, lookup: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    lookup(name)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

pub(crate) fn read_yaml_manifest_value(relative_path: &str) -> Option<Value> {
    let resolved_path = project_file_path(relative_path)?;
    let content = std::fs::read_to_string(resolved_path).ok()?;
    serde_yaml::from_str::<Value>(&content).ok()
}

pub(crate) fn manifest_has_kind_name(relative_path: &str, kind: &str, name: &str) -> bool {
    read_yaml_manifest_value(relative_path).is_some_and(|manifest| {
        manifest.get("kind").and_then(Value::as_str) == Some(kind)
            && manifest.pointer("/metadata/name").and_then(Value::as_str) == Some(name)
    })
}

pub(crate) fn network_policy_targets_app(relative_path: &str, app: &str) -> bool {
    read_yaml_manifest_value(relative_path).is_some_and(|manifest| {
        manifest.get("kind").and_then(Value::as_str) == Some("NetworkPolicy")
            && manifest
                .pointer("/spec/podSelector/matchLabels/app")
                .and_then(Value::as_str)
                == Some(app)
    })
}

pub(crate) fn project_file_path(relative_path: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(relative_path);
    if direct.exists() {
        return Some(direct);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        let candidate = ancestor.join(relative_path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

pub(crate) fn project_file_contains(relative_path: &str, needle: &str) -> bool {
    project_file_path(relative_path)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|content| content.contains(needle))
}

pub(crate) fn env_i64(key: &str) -> Option<i64> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
}
