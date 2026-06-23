use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use bytes::Bytes;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::{AppError, AppState, ProjectGitHubBinding, UpsertProjectGitHubBinding};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/webhooks/github", post(github_webhook))
        .route(
            "/api/github/project-bindings",
            get(list_bindings).post(upsert_binding),
        )
        .route("/api/github/project-bindings/{id}", get(get_binding))
}

// ── Webhook ───────────────────────────────────────────────────────────────────

async fn github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();

    // Only handle issues and pull_request events.
    if event_type != "issues" && event_type != "pull_request" {
        return Ok(StatusCode::NO_CONTENT);
    }

    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| AppError::bad_request("invalid JSON body"))?;

    let repo_full_name = payload
        .pointer("/repository/full_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("missing repository.full_name"))?;

    let binding = state
        .get_project_github_binding_by_repo(repo_full_name)
        .await?;

    // Verify HMAC-SHA256 signature when a secret env var is configured.
    if !binding.webhook_secret_ref.is_empty() {
        let secret = std::env::var(&binding.webhook_secret_ref)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::unauthorized("webhook secret not configured"))?;
        verify_github_signature(&headers, &body, &secret)?;
    }

    let workflow_name = match event_type.as_str() {
        "issues" => "swe_issue_triage",
        "pull_request" => "swe_pr_review",
        _ => return Ok(StatusCode::NO_CONTENT),
    };

    // Find the workflow definition by pack installation + name, then trigger a run.
    let definitions = state.list_workflow_definitions().await?;
    let definition = definitions
        .into_iter()
        .find(|d| {
            d.pack_installation_id == Some(binding.pack_installation_id) && d.name == workflow_name
        })
        .ok_or_else(|| {
            AppError::not_found(format!(
                "workflow definition '{workflow_name}' not found in pack installation"
            ))
        })?;

    crate::trigger_workflow_run_from_webhook(&state, definition.id, payload).await?;

    Ok(StatusCode::ACCEPTED)
}

fn verify_github_signature(headers: &HeaderMap, body: &[u8], secret: &str) -> Result<(), AppError> {
    let sig_header = headers
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("missing X-Hub-Signature-256"))?;

    let sig_hex = sig_header
        .strip_prefix("sha256=")
        .ok_or_else(|| AppError::unauthorized("malformed signature header"))?;

    let sig_bytes =
        hex::decode(sig_hex).map_err(|_| AppError::unauthorized("non-hex signature"))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::internal("hmac key error"))?;
    mac.update(body);

    mac.verify_slice(&sig_bytes)
        .map_err(|_| AppError::unauthorized("signature mismatch"))
}

// ── Binding CRUD ──────────────────────────────────────────────────────────────

async fn list_bindings(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectGitHubBinding>>, AppError> {
    let bindings = state.list_project_github_bindings().await?;
    Ok(Json(bindings))
}

async fn get_binding(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectGitHubBinding>, AppError> {
    let bindings = state.list_project_github_bindings().await?;
    bindings
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::not_found("binding not found"))
        .map(Json)
}

async fn upsert_binding(
    State(state): State<AppState>,
    Json(req): Json<UpsertProjectGitHubBinding>,
) -> Result<(StatusCode, Json<ProjectGitHubBinding>), AppError> {
    validate_repo_full_name(&req.repo_full_name)?;
    let now = Utc::now();
    let binding = ProjectGitHubBinding {
        id: Uuid::new_v4(),
        repo_full_name: req.repo_full_name,
        pack_installation_id: req.pack_installation_id,
        webhook_secret_ref: req.webhook_secret_ref.unwrap_or_default(),
        active: req.active.unwrap_or(true),
        created_at: now,
        updated_at: now,
    };
    let saved = state.upsert_project_github_binding(binding).await?;
    Ok((StatusCode::CREATED, Json(saved)))
}

fn validate_repo_full_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || !name.contains('/') || name.contains("..") {
        return Err(AppError::bad_request(
            "repo_full_name must be in 'owner/repo' format",
        ));
    }
    Ok(())
}
