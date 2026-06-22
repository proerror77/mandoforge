use std::path::PathBuf;

use crate::AppError;

pub(crate) fn normalize_codex_artifact_path(path: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return Ok(None);
    };
    if path.starts_with('/') || path.split('/').any(|segment| segment == "..") {
        return Err(AppError::bad_request(
            "Codex artifact path must be relative and stay inside the session workspace",
        ));
    }
    Ok(Some(path.to_string()))
}

pub(crate) fn normalize_remote_computer_artifact_dir(path: &str) -> Result<String, AppError> {
    let path = path.trim();
    let path = if path.is_empty() { "artifacts" } else { path };
    if path.starts_with('/') || path.split('/').any(|segment| segment == "..") {
        return Err(AppError::bad_request(
            "Remote Computer artifact discovery path must be relative and stay inside the workspace",
        ));
    }
    Ok(path.to_string())
}

pub(crate) struct DiscoveredArtifactFile {
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
    pub(crate) content: String,
}

pub(crate) async fn discover_artifact_files(
    root: &PathBuf,
    max_files: usize,
    max_file_bytes: u64,
) -> Result<Vec<DiscoveredArtifactFile>, AppError> {
    let mut files = Vec::new();
    let mut directories = vec![root.clone()];
    let mut visited_directories = 0_usize;
    while let Some(directory) = directories.pop() {
        visited_directories += 1;
        if visited_directories > 1000 {
            return Err(AppError::bad_request(
                "Remote Computer artifact discovery exceeded directory traversal limit",
            ));
        }
        let mut read_dir = tokio::fs::read_dir(&directory).await?;
        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            entries.push(entry.path());
        }
        entries.sort();
        for entry_path in entries.into_iter().rev() {
            let metadata = tokio::fs::symlink_metadata(&entry_path).await?;
            if metadata.is_dir() {
                directories.push(entry_path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if metadata.len() > max_file_bytes {
                return Err(AppError::bad_request(format!(
                    "Remote Computer artifact {} exceeds 1 MiB discovery limit",
                    entry_path.display()
                )));
            }
            let bytes = tokio::fs::read(&entry_path).await?;
            let content = match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(error) => String::from_utf8_lossy(error.as_bytes()).to_string(),
            };
            files.push(DiscoveredArtifactFile {
                path: entry_path,
                bytes: metadata.len(),
                content,
            });
            if files.len() >= max_files {
                return Ok(files);
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

pub(crate) fn artifact_type_from_path(path: &PathBuf) -> String {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "markdown" => "markdown",
        "json" => "json",
        "sql" => "sql",
        "csv" => "csv",
        "log" => "log",
        "py" | "sh" | "rs" | "ts" | "tsx" | "js" | "jsx" => "script",
        _ => "file",
    }
    .to_string()
}
