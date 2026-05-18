use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::agent_runtime_profile_from_row;
use crate::{
    AgentRuntimeProfile, AppError, AppState, CreateAgentRuntimeProfile, UpdateAgentRuntimeProfile,
    evaluate_agent_runtime_profile_release_gate,
};

impl AppState {
    pub(crate) async fn list_agent_runtime_profiles(
        &self,
    ) -> Result<Vec<AgentRuntimeProfile>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut profiles: Vec<_> = inner
                    .read()
                    .await
                    .agent_runtime_profiles
                    .values()
                    .filter(|profile| profile.archived_at.is_none())
                    .cloned()
                    .collect();
                profiles.sort_by_key(|profile| profile.created_at);
                Ok(profiles)
            }
            StoreBackend::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at
                     FROM agent_runtime_profiles
                     WHERE tenant_id = $1 AND archived_at IS NULL
                     ORDER BY created_at ASC",
                )
                .bind(self.current_tenant_id())
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(agent_runtime_profile_from_row)
                    .collect()
            }
        }
    }

    pub(crate) async fn get_agent_runtime_profile(
        &self,
        id: Uuid,
    ) -> Result<AgentRuntimeProfile, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .agent_runtime_profiles
                .get(&id)
                .filter(|profile| profile.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("agent runtime profile not found")),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at
                     FROM agent_runtime_profiles
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent runtime profile not found"))?;
                agent_runtime_profile_from_row(row)
            }
        }
    }

    pub(crate) async fn get_agent_runtime_profile_by_name(
        &self,
        name: &str,
    ) -> Result<Option<AgentRuntimeProfile>, AppError> {
        let name = normalize_runtime_profile_name(name)?;
        match &self.store {
            StoreBackend::Memory(inner) => Ok(inner
                .read()
                .await
                .agent_runtime_profiles
                .values()
                .find(|profile| {
                    profile.archived_at.is_none() && profile.name.eq_ignore_ascii_case(&name)
                })
                .cloned()),
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at
                     FROM agent_runtime_profiles
                     WHERE tenant_id = $1 AND lower(name) = lower($2) AND archived_at IS NULL",
                )
                .bind(self.current_tenant_id())
                .bind(&name)
                .fetch_optional(pool)
                .await?;
                row.map(agent_runtime_profile_from_row).transpose()
            }
        }
    }

    pub(crate) async fn create_agent_runtime_profile(
        &self,
        input: CreateAgentRuntimeProfile,
    ) -> Result<AgentRuntimeProfile, AppError> {
        let name = normalize_runtime_profile_name(&input.name)?;
        let runtime_type = normalize_runtime_type(&input.runtime_type)?;
        validate_runtime_profile_status(&input.status)?;
        validate_runtime_profile_env(&input.env)?;
        let command = input.command.trim().to_string();
        if command.is_empty() {
            return Err(AppError::bad_request(
                "agent runtime profile command cannot be empty",
            ));
        }
        if let Some(timeout_seconds) = input.timeout_seconds {
            if !(1..=3600).contains(&timeout_seconds) {
                return Err(AppError::bad_request(
                    "agent runtime profile timeout_seconds must be between 1 and 3600",
                ));
            }
        }

        let now = Utc::now();
        let profile = AgentRuntimeProfile {
            id: Uuid::new_v4(),
            name,
            runtime_type,
            command,
            default_args: input.default_args,
            env: input.env,
            timeout_seconds: input.timeout_seconds,
            remote_computer_required: input.remote_computer_required,
            status: input.status,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        ensure_enabled_runtime_profile_release_gate(&profile)?;

        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.agent_runtime_profiles.values().any(|existing| {
                    existing.archived_at.is_none()
                        && existing.name.eq_ignore_ascii_case(&profile.name)
                }) {
                    return Err(AppError::bad_request(format!(
                        "agent runtime profile already exists: {}",
                        profile.name
                    )));
                }
                store
                    .agent_runtime_profiles
                    .insert(profile.id, profile.clone());
                Ok(profile)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO agent_runtime_profiles
                        (id, tenant_id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL)
                     RETURNING id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at",
                )
                .bind(profile.id)
                .bind(self.current_tenant_id())
                .bind(&profile.name)
                .bind(&profile.runtime_type)
                .bind(&profile.command)
                .bind(json!(profile.default_args))
                .bind(&profile.env)
                .bind(profile.timeout_seconds)
                .bind(profile.remote_computer_required)
                .bind(&profile.status)
                .bind(profile.created_at)
                .bind(profile.updated_at)
                .fetch_one(pool)
                .await?;
                agent_runtime_profile_from_row(row)
            }
        }
    }

    pub(crate) async fn update_agent_runtime_profile(
        &self,
        id: Uuid,
        input: UpdateAgentRuntimeProfile,
    ) -> Result<AgentRuntimeProfile, AppError> {
        validate_runtime_profile_update(&input)?;
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let profile = store
                    .agent_runtime_profiles
                    .get_mut(&id)
                    .filter(|profile| profile.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("agent runtime profile not found"))?;
                apply_runtime_profile_update(profile, input, updated_at);
                ensure_enabled_runtime_profile_release_gate(profile)?;
                Ok(profile.clone())
            }
            StoreBackend::Postgres(pool) => {
                let existing = self.get_agent_runtime_profile(id).await?;
                let mut profile = existing;
                apply_runtime_profile_update(&mut profile, input, updated_at);
                ensure_enabled_runtime_profile_release_gate(&profile)?;
                let row = sqlx::query(
                    "UPDATE agent_runtime_profiles
                     SET command = $3,
                         default_args = $4,
                         env = $5,
                         timeout_seconds = $6,
                         remote_computer_required = $7,
                         status = $8,
                         updated_at = $9
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(&profile.command)
                .bind(json!(profile.default_args))
                .bind(&profile.env)
                .bind(profile.timeout_seconds)
                .bind(profile.remote_computer_required)
                .bind(&profile.status)
                .bind(profile.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent runtime profile not found"))?;
                agent_runtime_profile_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_agent_runtime_profile(
        &self,
        id: Uuid,
    ) -> Result<AgentRuntimeProfile, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let profile = store
                    .agent_runtime_profiles
                    .get_mut(&id)
                    .filter(|profile| profile.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("agent runtime profile not found"))?;
                profile.status = "disabled".to_string();
                profile.archived_at = Some(archived_at);
                profile.updated_at = archived_at;
                Ok(profile.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE agent_runtime_profiles
                     SET status = 'disabled', archived_at = $3, updated_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, name, runtime_type, command, default_args, env, timeout_seconds, remote_computer_required, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("agent runtime profile not found"))?;
                agent_runtime_profile_from_row(row)
            }
        }
    }
}

fn validate_runtime_profile_update(input: &UpdateAgentRuntimeProfile) -> Result<(), AppError> {
    if let Some(command) = input.command.as_ref() {
        if command.trim().is_empty() {
            return Err(AppError::bad_request(
                "agent runtime profile command cannot be empty",
            ));
        }
    }
    if let Some(env) = input.env.as_ref() {
        validate_runtime_profile_env(env)?;
    }
    if let Some(Some(timeout_seconds)) = input.timeout_seconds {
        if !(1..=3600).contains(&timeout_seconds) {
            return Err(AppError::bad_request(
                "agent runtime profile timeout_seconds must be between 1 and 3600",
            ));
        }
    }
    if let Some(status) = input.status.as_ref() {
        validate_runtime_profile_status(status)?;
    }
    Ok(())
}

fn apply_runtime_profile_update(
    profile: &mut AgentRuntimeProfile,
    input: UpdateAgentRuntimeProfile,
    updated_at: chrono::DateTime<Utc>,
) {
    if let Some(command) = input.command {
        profile.command = command.trim().to_string();
    }
    if let Some(default_args) = input.default_args {
        profile.default_args = default_args;
    }
    if let Some(env) = input.env {
        profile.env = env;
    }
    if let Some(timeout_seconds) = input.timeout_seconds {
        profile.timeout_seconds = timeout_seconds;
    }
    if let Some(remote_computer_required) = input.remote_computer_required {
        profile.remote_computer_required = remote_computer_required;
    }
    if let Some(status) = input.status {
        profile.status = status;
    }
    profile.updated_at = updated_at;
}

fn ensure_enabled_runtime_profile_release_gate(
    profile: &AgentRuntimeProfile,
) -> Result<(), AppError> {
    if profile.status != "enabled" {
        return Ok(());
    }
    let gate = evaluate_agent_runtime_profile_release_gate(profile);
    if gate.fail_closed {
        return Err(AppError::bad_request(format!(
            "agent runtime profile cannot be enabled until its release gate passes: {}",
            gate.blocking_reasons.join("; ")
        )));
    }
    Ok(())
}

fn normalize_runtime_profile_name(name: &str) -> Result<String, AppError> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(AppError::bad_request(
            "agent runtime profile name must be an allowlist-safe name",
        ));
    }
    Ok(normalized)
}

fn normalize_runtime_type(runtime_type: &str) -> Result<String, AppError> {
    let normalized = runtime_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "agent_cli" | "codex_app_server" | "claude_code" | "gemini" | "opencode" | "aider"
        | "hosted" => Ok(normalized),
        _ => Err(AppError::bad_request(format!(
            "unsupported agent runtime profile type: {normalized}"
        ))),
    }
}

fn validate_runtime_profile_status(status: &str) -> Result<(), AppError> {
    match status {
        "enabled" | "disabled" => Ok(()),
        _ => Err(AppError::bad_request(
            "agent runtime profile status must be enabled or disabled",
        )),
    }
}

fn validate_runtime_profile_env(env: &serde_json::Value) -> Result<(), AppError> {
    let Some(object) = env.as_object() else {
        return Err(AppError::bad_request(
            "agent runtime profile env must be a JSON object",
        ));
    };
    for (key, value) in object {
        if key.trim().is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return Err(AppError::bad_request(
                "agent runtime profile env keys must be shell-safe names",
            ));
        }
        if !value.is_string() {
            return Err(AppError::bad_request(
                "agent runtime profile env values must be strings",
            ));
        }
    }
    Ok(())
}
