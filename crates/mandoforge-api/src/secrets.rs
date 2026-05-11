use std::fmt;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;

#[derive(Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct SecretProviderConfig {
    pub(crate) vault_addr: String,
    pub(crate) namespace: Option<String>,
    pub(crate) mount: String,
    pub(crate) token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum SecretProviderKind {
    Reserved,
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SecretRef {
    pub(crate) path: String,
    pub(crate) key: String,
}

#[derive(Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct SecretValue {
    value: String,
}

impl fmt::Debug for SecretProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretProviderConfig")
            .field("vault_addr", &self.vault_addr)
            .field("namespace", &self.namespace)
            .field("mount", &self.mount)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretValue")
            .field("value", &"<redacted>")
            .finish()
    }
}

#[allow(dead_code)]
pub(crate) fn secret_provider_from_env() -> Result<Box<dyn SecretProvider>, AppError> {
    secret_provider_from_lookup(&|key| std::env::var(key).ok())
}

#[allow(dead_code)]
pub(crate) fn secret_provider_from_lookup<F>(
    lookup: &F,
) -> Result<Box<dyn SecretProvider>, AppError>
where
    F: Fn(&str) -> Option<String>,
{
    match SecretProviderKind::from_lookup(lookup)? {
        SecretProviderKind::Reserved => Ok(Box::new(ReservedSecretProvider)),
        SecretProviderKind::Vault => Ok(Box::new(VaultSecretProvider::new()?)),
    }
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
        validate_secret_component("mount", mount.trim_matches('/'))?;
        let token = lookup("MANDOFORGE_VAULT_TOKEN")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Self {
            vault_addr,
            namespace,
            mount,
            token,
        })
    }

    fn normalized_vault_addr(&self) -> String {
        self.vault_addr.trim_end_matches('/').to_string()
    }

    fn normalized_mount(&self) -> String {
        self.mount.trim_matches('/').to_string()
    }
}

#[allow(dead_code)]
impl SecretProviderKind {
    pub(crate) fn from_lookup<F>(lookup: &F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let kind = lookup("MANDOFORGE_SECRET_PROVIDER")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "reserved".to_string());
        match kind.as_str() {
            "reserved" => Ok(Self::Reserved),
            "vault" => Ok(Self::Vault),
            _ => Err(AppError::bad_request(format!(
                "unsupported MANDOFORGE_SECRET_PROVIDER '{kind}'"
            ))),
        }
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

#[allow(dead_code)]
pub(crate) struct VaultSecretProvider {
    client: reqwest::Client,
}

#[allow(dead_code)]
impl VaultSecretProvider {
    pub(crate) fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
        })
    }

    fn health_url(config: &SecretProviderConfig) -> String {
        format!("{}/v1/sys/health", config.normalized_vault_addr())
    }

    fn kv_read_url(config: &SecretProviderConfig, secret_ref: &SecretRef) -> String {
        format!(
            "{}/v1/{}/data/{}",
            config.normalized_vault_addr(),
            config.normalized_mount(),
            secret_ref.path.trim_matches('/')
        )
    }

    fn headers(config: &SecretProviderConfig) -> Result<HeaderMap, AppError> {
        let token = config
            .token
            .as_deref()
            .ok_or_else(|| AppError::forbidden("MANDOFORGE_VAULT_TOKEN is required"))?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Vault-Token",
            HeaderValue::from_str(token)
                .map_err(|_| AppError::bad_request("invalid MANDOFORGE_VAULT_TOKEN"))?,
        );
        if let Some(namespace) = config.namespace.as_deref() {
            headers.insert(
                "X-Vault-Namespace",
                HeaderValue::from_str(namespace)
                    .map_err(|_| AppError::bad_request("invalid MANDOFORGE_VAULT_NAMESPACE"))?,
            );
        }
        Ok(headers)
    }

    fn parse_kv_v2_secret(value: Value, secret_ref: &SecretRef) -> Result<SecretValue, AppError> {
        let secret_value = value
            .get("data")
            .and_then(|data| data.get("data"))
            .and_then(|data| data.get(&secret_ref.key))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::bad_request(format!("vault secret key '{}' is missing", secret_ref.key))
            })?;
        Ok(SecretValue {
            value: secret_value.to_string(),
        })
    }
}

#[async_trait]
impl SecretProvider for VaultSecretProvider {
    async fn health_check(&self, config: &SecretProviderConfig) -> Result<(), AppError> {
        let response = self
            .client
            .get(Self::health_url(config))
            .headers(Self::headers(config)?)
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(AppError::bad_request(format!(
            "vault health check failed with status {}",
            response.status()
        )))
    }

    async fn read_secret(
        &self,
        config: &SecretProviderConfig,
        secret_ref: &SecretRef,
    ) -> Result<SecretValue, AppError> {
        let response = self
            .client
            .get(Self::kv_read_url(config, secret_ref))
            .headers(Self::headers(config)?)
            .send()
            .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!(
                "vault secret read failed with status {status}"
            )));
        }
        Self::parse_kv_v2_secret(value, secret_ref)
    }
}

fn validate_secret_component(label: &str, value: &str) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.starts_with('/')
        || trimmed.contains('#')
        || trimmed.contains('?')
        || trimmed.chars().any(char::is_whitespace)
    {
        return Err(AppError::bad_request(format!("invalid secret {label}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ReservedSecretProvider, SecretProvider, SecretProviderConfig, SecretProviderKind,
        SecretRef, VaultSecretProvider, secret_provider_from_lookup, validate_secret_component,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn secret_provider_config_requires_vault_addr() {
        assert!(SecretProviderConfig::from_lookup(|_| None).is_err());
    }

    #[test]
    fn secret_provider_kind_defaults_to_reserved() {
        let kind = SecretProviderKind::from_lookup(&|_| None).expect("kind");

        assert_eq!(kind, SecretProviderKind::Reserved);
    }

    #[test]
    fn secret_provider_kind_allows_explicit_vault() {
        let kind = SecretProviderKind::from_lookup(&|key| match key {
            "MANDOFORGE_SECRET_PROVIDER" => Some(" vault ".to_string()),
            _ => None,
        })
        .expect("kind");

        assert_eq!(kind, SecretProviderKind::Vault);
    }

    #[test]
    fn secret_provider_kind_rejects_unknown_values() {
        assert!(
            SecretProviderKind::from_lookup(&|key| match key {
                "MANDOFORGE_SECRET_PROVIDER" => Some("file".to_string()),
                _ => None,
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn secret_provider_selector_keeps_reserved_as_default() {
        let provider = secret_provider_from_lookup(&|_| None).expect("provider");
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            _ => None,
        })
        .expect("vault config");
        let secret_ref = SecretRef::new("providers/openai", "api_key").expect("secret ref");

        assert!(provider.read_secret(&config, &secret_ref).await.is_err());
    }

    #[tokio::test]
    async fn secret_provider_selector_allows_explicit_vault() {
        let provider = secret_provider_from_lookup(&|key| match key {
            "MANDOFORGE_SECRET_PROVIDER" => Some("vault".to_string()),
            _ => None,
        })
        .expect("provider");
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            _ => None,
        })
        .expect("vault config");

        let error = provider
            .health_check(&config)
            .await
            .expect_err("vault token");

        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.message, "MANDOFORGE_VAULT_TOKEN is required");
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
        assert_eq!(config.token, None);
    }

    #[test]
    fn secret_provider_config_allows_namespace_mount_and_token() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            "MANDOFORGE_VAULT_NAMESPACE" => Some("agent-os".to_string()),
            "MANDOFORGE_VAULT_MOUNT" => Some("kv".to_string()),
            "MANDOFORGE_VAULT_TOKEN" => Some("dev-token".to_string()),
            _ => None,
        })
        .expect("vault config");

        assert_eq!(config.namespace.as_deref(), Some("agent-os"));
        assert_eq!(config.mount, "kv");
        assert_eq!(config.token.as_deref(), Some("dev-token"));
    }

    #[test]
    fn secret_debug_output_redacts_sensitive_values() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            "MANDOFORGE_VAULT_TOKEN" => Some("dev-token".to_string()),
            _ => None,
        })
        .expect("vault config");
        let secret = super::SecretValue {
            value: "resolved-key".to_string(),
        };

        assert!(!format!("{config:?}").contains("dev-token"));
        assert!(!format!("{secret:?}").contains("resolved-key"));
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

    #[test]
    fn vault_secret_provider_builds_kv_v2_urls_with_trimmed_slashes() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200/".to_string()),
            "MANDOFORGE_VAULT_MOUNT" => Some("/kv/".to_string()),
            "MANDOFORGE_VAULT_TOKEN" => Some("dev-token".to_string()),
            _ => None,
        })
        .expect("vault config");
        let secret_ref = SecretRef::new("providers/openai", "api_key").expect("secret ref");

        assert_eq!(
            VaultSecretProvider::health_url(&config),
            "http://vault:8200/v1/sys/health"
        );
        assert_eq!(
            VaultSecretProvider::kv_read_url(&config, &secret_ref),
            "http://vault:8200/v1/kv/data/providers/openai"
        );
    }

    #[test]
    fn vault_secret_provider_sets_optional_namespace_header() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            "MANDOFORGE_VAULT_NAMESPACE" => Some("agent-os".to_string()),
            "MANDOFORGE_VAULT_TOKEN" => Some("dev-token".to_string()),
            _ => None,
        })
        .expect("vault config");

        let headers = VaultSecretProvider::headers(&config).expect("headers");

        assert_eq!(headers["X-Vault-Token"], "dev-token");
        assert_eq!(headers["X-Vault-Namespace"], "agent-os");
    }

    #[test]
    fn vault_secret_provider_requires_token_before_network_io() {
        let config = SecretProviderConfig::from_lookup(|key| match key {
            "MANDOFORGE_VAULT_ADDR" => Some("http://vault:8200".to_string()),
            _ => None,
        })
        .expect("vault config");

        assert!(VaultSecretProvider::headers(&config).is_err());
    }

    #[test]
    fn vault_secret_provider_parses_kv_v2_secret_values() {
        let secret_ref = SecretRef::new("providers/openai", "api_key").expect("secret ref");
        let secret = VaultSecretProvider::parse_kv_v2_secret(
            json!({"data": {"data": {"api_key": "resolved-key"}}}),
            &secret_ref,
        )
        .expect("secret");

        assert_eq!(secret.expose_for_provider_use(), "resolved-key");
        assert!(
            VaultSecretProvider::parse_kv_v2_secret(json!({"data": {"data": {}}}), &secret_ref)
                .is_err()
        );
    }
}
