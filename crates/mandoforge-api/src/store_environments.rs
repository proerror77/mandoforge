use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use crate::store_backend::StoreBackend;
use crate::store_rows::environment_from_row;
use crate::{AppError, AppState, CreateEnvironment, Environment, UpdateEnvironment};

const ENVIRONMENT_TYPES: &[&str] = &[
    "local",
    "cloud",
    "self_hosted",
    "remote_computer",
    "codex_app_server",
];

impl AppState {
    pub(crate) async fn list_environments(&self) -> Result<Vec<Environment>, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut environments: Vec<_> = inner
                    .read()
                    .await
                    .environments
                    .values()
                    .filter(|environment| environment.archived_at.is_none())
                    .cloned()
                    .collect();
                environments.sort_by_key(|environment| environment.created_at);
                Ok(environments)
            }
            StoreBackend::Postgres(pool) => {
                let sql = environment_select_sql(
                    "WHERE tenant_id = $1 AND archived_at IS NULL ORDER BY created_at ASC",
                );
                let rows = sqlx::query(&sql)
                    .bind(self.current_tenant_id())
                    .fetch_all(pool)
                    .await?;
                rows.into_iter().map(environment_from_row).collect()
            }
        }
    }

    pub(crate) async fn get_environment(&self, id: Uuid) -> Result<Environment, AppError> {
        match &self.store {
            StoreBackend::Memory(inner) => inner
                .read()
                .await
                .environments
                .get(&id)
                .filter(|environment| environment.archived_at.is_none())
                .cloned()
                .ok_or_else(|| AppError::not_found("environment not found")),
            StoreBackend::Postgres(pool) => {
                let sql = environment_select_sql(
                    "WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
                );
                let row = sqlx::query(&sql)
                    .bind(self.current_tenant_id())
                    .bind(id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("environment not found"))?;
                environment_from_row(row)
            }
        }
    }

    pub(crate) async fn create_environment(
        &self,
        input: CreateEnvironment,
    ) -> Result<Environment, AppError> {
        let name = normalize_environment_name(&input.name)?;
        let environment_type = normalize_environment_type(&input.environment_type)?;
        validate_environment_status(&input.status)?;
        validate_environment_release_state(&input.release_state)?;
        validate_environment_json_fields(
            &environment_type,
            &input.remote_computer_profile,
            &input,
        )?;
        if let Some(runtime_profile_id) = input.runtime_profile_id {
            self.get_agent_runtime_profile(runtime_profile_id).await?;
        }

        let now = Utc::now();
        let environment = Environment {
            id: Uuid::new_v4(),
            name,
            environment_type,
            runtime_profile_id: input.runtime_profile_id,
            remote_computer_profile: input.remote_computer_profile,
            codex_app_server_profile: input.codex_app_server_profile,
            worker_queue_binding: input.worker_queue_binding,
            state_mounts: input.state_mounts,
            network_policy: input.network_policy,
            vault_requirements: input.vault_requirements,
            mcp_requirements: input.mcp_requirements,
            release_state: input.release_state,
            status: input.status,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };

        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                if store.environments.values().any(|existing| {
                    existing.archived_at.is_none()
                        && existing.name.eq_ignore_ascii_case(&environment.name)
                }) {
                    return Err(AppError::bad_request(format!(
                        "environment already exists: {}",
                        environment.name
                    )));
                }
                store
                    .environments
                    .insert(environment.id, environment.clone());
                Ok(environment)
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "INSERT INTO environments
                        (id, tenant_id, name, environment_type, runtime_profile_id,
                         remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                         state_mounts, network_policy, vault_requirements, mcp_requirements,
                         release_state, status, created_at, updated_at, archived_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NULL)
                     RETURNING id, name, environment_type, runtime_profile_id,
                               remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                               state_mounts, network_policy, vault_requirements, mcp_requirements,
                               release_state, status, created_at, updated_at, archived_at",
                )
                .bind(environment.id)
                .bind(self.current_tenant_id())
                .bind(&environment.name)
                .bind(&environment.environment_type)
                .bind(environment.runtime_profile_id)
                .bind(&environment.remote_computer_profile)
                .bind(&environment.codex_app_server_profile)
                .bind(&environment.worker_queue_binding)
                .bind(&environment.state_mounts)
                .bind(&environment.network_policy)
                .bind(&environment.vault_requirements)
                .bind(&environment.mcp_requirements)
                .bind(&environment.release_state)
                .bind(&environment.status)
                .bind(environment.created_at)
                .bind(environment.updated_at)
                .fetch_one(pool)
                .await?;
                environment_from_row(row)
            }
        }
    }

    pub(crate) async fn update_environment(
        &self,
        id: Uuid,
        input: UpdateEnvironment,
    ) -> Result<Environment, AppError> {
        let existing = self.get_environment(id).await?;
        validate_environment_update(&existing, &input)?;
        if let Some(Some(runtime_profile_id)) = input.runtime_profile_id {
            self.get_agent_runtime_profile(runtime_profile_id).await?;
        }
        let updated_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let existing_names: Vec<String> = store
                    .environments
                    .values()
                    .filter(|environment| environment.id != id && environment.archived_at.is_none())
                    .map(|environment| environment.name.clone())
                    .collect();
                let environment = store
                    .environments
                    .get_mut(&id)
                    .filter(|environment| environment.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("environment not found"))?;
                apply_environment_update(environment, input, updated_at)?;
                if existing_names
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&environment.name))
                {
                    return Err(AppError::bad_request(format!(
                        "environment already exists: {}",
                        environment.name
                    )));
                }
                Ok(environment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let mut environment = existing;
                apply_environment_update(&mut environment, input, updated_at)?;
                let row = sqlx::query(
                    "UPDATE environments
                     SET name = $3,
                         environment_type = $4,
                         runtime_profile_id = $5,
                         remote_computer_profile = $6,
                         codex_app_server_profile = $7,
                         worker_queue_binding = $8,
                         state_mounts = $9,
                         network_policy = $10,
                         vault_requirements = $11,
                         mcp_requirements = $12,
                         release_state = $13,
                         status = $14,
                         updated_at = $15
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, name, environment_type, runtime_profile_id,
                               remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                               state_mounts, network_policy, vault_requirements, mcp_requirements,
                               release_state, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(&environment.name)
                .bind(&environment.environment_type)
                .bind(environment.runtime_profile_id)
                .bind(&environment.remote_computer_profile)
                .bind(&environment.codex_app_server_profile)
                .bind(&environment.worker_queue_binding)
                .bind(&environment.state_mounts)
                .bind(&environment.network_policy)
                .bind(&environment.vault_requirements)
                .bind(&environment.mcp_requirements)
                .bind(&environment.release_state)
                .bind(&environment.status)
                .bind(environment.updated_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("environment not found"))?;
                environment_from_row(row)
            }
        }
    }

    pub(crate) async fn archive_environment(&self, id: Uuid) -> Result<Environment, AppError> {
        let archived_at = Utc::now();
        match &self.store {
            StoreBackend::Memory(inner) => {
                let mut store = inner.write().await;
                let environment = store
                    .environments
                    .get_mut(&id)
                    .filter(|environment| environment.archived_at.is_none())
                    .ok_or_else(|| AppError::not_found("environment not found"))?;
                environment.status = "disabled".to_string();
                environment.release_state = "disabled".to_string();
                environment.archived_at = Some(archived_at);
                environment.updated_at = archived_at;
                Ok(environment.clone())
            }
            StoreBackend::Postgres(pool) => {
                let row = sqlx::query(
                    "UPDATE environments
                     SET status = 'disabled',
                         release_state = 'disabled',
                         archived_at = $3,
                         updated_at = $3
                     WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
                     RETURNING id, name, environment_type, runtime_profile_id,
                               remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                               state_mounts, network_policy, vault_requirements, mcp_requirements,
                               release_state, status, created_at, updated_at, archived_at",
                )
                .bind(self.current_tenant_id())
                .bind(id)
                .bind(archived_at)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("environment not found"))?;
                environment_from_row(row)
            }
        }
    }
}

fn environment_select_sql(where_clause: &str) -> String {
    format!(
        "SELECT id, name, environment_type, runtime_profile_id,
                remote_computer_profile, codex_app_server_profile, worker_queue_binding,
                state_mounts, network_policy, vault_requirements, mcp_requirements,
                release_state, status, created_at, updated_at, archived_at
         FROM environments {where_clause}"
    )
}

fn validate_environment_update(
    existing: &Environment,
    input: &UpdateEnvironment,
) -> Result<(), AppError> {
    if let Some(name) = input.name.as_ref() {
        normalize_environment_name(name)?;
    }
    let environment_type = match input.environment_type.as_ref() {
        Some(environment_type) => normalize_environment_type(environment_type)?,
        None => existing.environment_type.clone(),
    };
    if let Some(status) = input.status.as_ref() {
        validate_environment_status(status)?;
    }
    if let Some(release_state) = input.release_state.as_ref() {
        validate_environment_release_state(release_state)?;
    }
    let remote_computer_profile = input
        .remote_computer_profile
        .as_ref()
        .unwrap_or(&existing.remote_computer_profile);
    validate_optional_json_object(&input.remote_computer_profile, "remote_computer_profile")?;
    validate_optional_json_object(&input.codex_app_server_profile, "codex_app_server_profile")?;
    validate_optional_json_object(&input.worker_queue_binding, "worker_queue_binding")?;
    validate_optional_json_object(&input.state_mounts, "state_mounts")?;
    validate_optional_json_object(&input.network_policy, "network_policy")?;
    validate_optional_json_object(&input.vault_requirements, "vault_requirements")?;
    validate_optional_json_object(&input.mcp_requirements, "mcp_requirements")?;
    validate_remote_computer_profile_schema(&environment_type, remote_computer_profile)?;
    Ok(())
}

fn validate_environment_json_fields(
    environment_type: &str,
    remote_computer_profile: &Value,
    input: &CreateEnvironment,
) -> Result<(), AppError> {
    validate_json_object(&input.remote_computer_profile, "remote_computer_profile")?;
    validate_json_object(&input.codex_app_server_profile, "codex_app_server_profile")?;
    validate_json_object(&input.worker_queue_binding, "worker_queue_binding")?;
    validate_json_object(&input.state_mounts, "state_mounts")?;
    validate_json_object(&input.network_policy, "network_policy")?;
    validate_json_object(&input.vault_requirements, "vault_requirements")?;
    validate_json_object(&input.mcp_requirements, "mcp_requirements")?;
    validate_remote_computer_profile_schema(environment_type, remote_computer_profile)?;
    Ok(())
}

fn apply_environment_update(
    environment: &mut Environment,
    input: UpdateEnvironment,
    updated_at: chrono::DateTime<Utc>,
) -> Result<(), AppError> {
    if let Some(name) = input.name {
        environment.name = normalize_environment_name(&name)?;
    }
    if let Some(environment_type) = input.environment_type {
        environment.environment_type = normalize_environment_type(&environment_type)?;
    }
    if let Some(runtime_profile_id) = input.runtime_profile_id {
        environment.runtime_profile_id = runtime_profile_id;
    }
    if let Some(remote_computer_profile) = input.remote_computer_profile {
        environment.remote_computer_profile = remote_computer_profile;
    }
    if let Some(codex_app_server_profile) = input.codex_app_server_profile {
        environment.codex_app_server_profile = codex_app_server_profile;
    }
    if let Some(worker_queue_binding) = input.worker_queue_binding {
        environment.worker_queue_binding = worker_queue_binding;
    }
    if let Some(state_mounts) = input.state_mounts {
        environment.state_mounts = state_mounts;
    }
    if let Some(network_policy) = input.network_policy {
        environment.network_policy = network_policy;
    }
    if let Some(vault_requirements) = input.vault_requirements {
        environment.vault_requirements = vault_requirements;
    }
    if let Some(mcp_requirements) = input.mcp_requirements {
        environment.mcp_requirements = mcp_requirements;
    }
    if let Some(release_state) = input.release_state {
        environment.release_state = release_state;
    }
    if let Some(status) = input.status {
        environment.status = status;
    }
    environment.updated_at = updated_at;
    Ok(())
}

fn normalize_environment_name(name: &str) -> Result<String, AppError> {
    let normalized = name.trim().to_string();
    if normalized.is_empty() || normalized.len() > 120 {
        return Err(AppError::bad_request(
            "environment name must be non-empty and at most 120 characters",
        ));
    }
    Ok(normalized)
}

fn normalize_environment_type(environment_type: &str) -> Result<String, AppError> {
    let normalized = environment_type.trim().to_ascii_lowercase();
    if ENVIRONMENT_TYPES
        .iter()
        .any(|allowed| allowed == &normalized.as_str())
    {
        Ok(normalized)
    } else {
        Err(AppError::bad_request(format!(
            "environment_type must be one of {}",
            ENVIRONMENT_TYPES.join(", ")
        )))
    }
}

fn validate_environment_status(status: &str) -> Result<(), AppError> {
    match status {
        "enabled" | "disabled" => Ok(()),
        _ => Err(AppError::bad_request(
            "environment status must be enabled or disabled",
        )),
    }
}

fn validate_environment_release_state(release_state: &str) -> Result<(), AppError> {
    match release_state {
        "draft" | "staged" | "active" | "disabled" => Ok(()),
        _ => Err(AppError::bad_request(
            "environment release_state must be draft, staged, active, or disabled",
        )),
    }
}

fn validate_optional_json_object(
    value: &Option<serde_json::Value>,
    field: &str,
) -> Result<(), AppError> {
    if let Some(value) = value {
        validate_json_object(value, field)?;
    }
    Ok(())
}

fn validate_json_object(value: &serde_json::Value, field: &str) -> Result<(), AppError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(AppError::bad_request(format!(
            "environment {field} must be a JSON object"
        )))
    }
}

fn validate_remote_computer_profile_schema(
    environment_type: &str,
    profile: &Value,
) -> Result<(), AppError> {
    if environment_type != "remote_computer" {
        return Ok(());
    }
    for key in ["pool", "profile", "namespace", "remote_computer_id"] {
        if let Some(value) = profile.get(key) {
            if !value.is_string() {
                return Err(AppError::bad_request(format!(
                    "remote_computer_profile.{key} must be a string"
                )));
            }
        }
    }
    if let Some(selector) = profile.get("metadata_selector") {
        let Some(selector) = selector.as_object() else {
            return Err(AppError::bad_request(
                "remote_computer_profile.metadata_selector must be an object",
            ));
        };
        for (key, value) in selector {
            if key.trim().is_empty() {
                return Err(AppError::bad_request(
                    "remote_computer_profile.metadata_selector keys must be non-empty",
                ));
            }
            if !value.is_string() {
                return Err(AppError::bad_request(format!(
                    "remote_computer_profile.metadata_selector.{key} must be a string"
                )));
            }
        }
    }
    Ok(())
}
