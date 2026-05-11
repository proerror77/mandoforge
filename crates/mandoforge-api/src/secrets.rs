use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct SecretProviderConfig {
    pub(crate) vault_addr: String,
    pub(crate) namespace: Option<String>,
    pub(crate) mount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SecretRef {
    pub(crate) path: String,
    pub(crate) key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct SecretValue {
    value: String,
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait SecretProvider: Send + Sync {
    async fn health_check(&self, config: &SecretProviderConfig) -> Result<(), AppError>;

    async fn read_secret(
        &self,
        config: &SecretProviderConfig,
        secret_ref: &SecretRef,
    ) -> Result<SecretValue, AppError>;
}

#[allow(dead_code)]
impl SecretProviderConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub(crate) fn from_lookup<F>(lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let vault_addr = lookup("MANDOFORGE_VAULT_ADDR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::bad_request("MANDOFORGE_VAULT_ADDR is required"))?;
        let namespace = lookup("MANDOFORGE_VAULT_NAMESPACE")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mount = lookup("MANDOFORGE_VAULT_MOUNT")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "secret".to_string());

        Ok(Self {
            vault_addr,
            namespace,
            mount,
        })
    }
}

#[allow(dead_code)]
impl SecretRef {
    pub(crate) fn new(path: impl Into<String>, key: impl Into<String>) -> Result<Self, AppError> {
        let path = path.into();
        let key = key.into();
        validate_secret_component("path", &path)?;
        validate_secret_component("key", &key)?;
        Ok(Self { path, key })
    }
}

#[allow(dead_code)]
impl SecretValue {
    pub(crate) fn expose_for_provider_use(&self) -> &str {
        &self.value
    }
}

#[allow(dead_code)]
pub(crate) struct ReservedSecretProvider;

#[async_trait]
impl SecretProvider for ReservedSecretProvider {
    async fn health_check(&self, _config: &SecretProviderConfig) -> Result<(), AppError> {
        Err(AppError::bad_request(
            "secret provider health check is reserved but not implemented",
        ))
    }

    async fn read_secret(
        &self,
        _config: &SecretProviderConfig,
        _secret_ref: &SecretRef,
    ) -> Result<SecretValue, AppError> {
        Err(AppError::forbidden(
            "secret reads are disabled until a production secret provider is implemented",
        ))
    }
}

fn validate_secret_component(label: &str, value: &str) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.starts_with('/') {
        return Err(AppError::bad_request(format!("invalid secret {label}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ReservedSecretProvider, SecretProvider, SecretProviderConfig, SecretRef,
        validate_secret_component,
    };

    #[test]
    fn secret_provider_config_requires_vault_addr() {
        assert!(SecretProviderConfig::from_lookup(|_| None).is_err());
    }

    #[test]
    fn secret_provider_config_defaults_mount() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            _ => None,
        })
        .expect("vault config");

        assert_eq!(config.vault_addr, "http://vault:8200");
        assert_eq!(config.mount, "secret");
        assert_eq!(config.namespace, None);
    }

    #[test]
    fn secret_provider_config_allows_namespace_and_mount() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            "MANDOFORGE_VAULT_NAMESPACE" => Some("agent-os".to_string()),
            "MANDOFORGE_VAULT_MOUNT" => Some("kv".to_string()),
            _ => None,
        })
        .expect("vault config");

        assert_eq!(config.namespace.as_deref(), Some("agent-os"));
        assert_eq!(config.mount, "kv");
    }

    #[test]
    fn secret_refs_reject_absolute_or_parent_paths() {
        assert!(SecretRef::new("providers/openai", "api_key").is_ok());
        assert!(SecretRef::new("/providers/openai", "api_key").is_err());
        assert!(SecretRef::new("../providers/openai", "api_key").is_err());
        assert!(validate_secret_component("key", "").is_err());
    }

    #[tokio::test]
    async fn reserved_secret_provider_fails_closed() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            _ => None,
        })
        .expect("vault config");
        let secret_ref = SecretRef::new("providers/openai", "api_key").expect("secret ref");
        let provider = ReservedSecretProvider;

        assert!(provider.health_check(&config).await.is_err());
        assert!(provider.read_secret(&config, &secret_ref).await.is_err());
    }
}
