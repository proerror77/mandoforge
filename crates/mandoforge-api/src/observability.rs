use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct ObservabilityConfig {
    pub(crate) service_name: String,
    pub(crate) otlp_endpoint: Option<String>,
    pub(crate) sample_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct TelemetryEvent {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) attributes: Value,
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait TelemetryExporter: Send + Sync {
    async fn health_check(&self, config: &ObservabilityConfig) -> Result<(), AppError>;

    async fn export_event(
        &self,
        config: &ObservabilityConfig,
        event: TelemetryEvent,
    ) -> Result<(), AppError>;
}

#[allow(dead_code)]
impl ObservabilityConfig {
    pub(crate) fn from_env() -> Result<Self, AppError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, AppError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let service_name = lookup("MANDOFORGE_SERVICE_NAME")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "mandoforge-api".to_string());
        let otlp_endpoint = lookup("MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let sample_ratio = lookup("MANDOFORGE_OTEL_SAMPLE_RATIO")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(1.0);
        if !(0.0..=1.0).contains(&sample_ratio) {
            return Err(AppError::bad_request(
                "MANDOFORGE_OTEL_SAMPLE_RATIO must be between 0 and 1",
            ));
        }

        Ok(Self {
            service_name,
            otlp_endpoint,
            sample_ratio,
        })
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.otlp_endpoint.is_some()
    }
}

#[allow(dead_code)]
pub(crate) struct ReservedTelemetryExporter;

#[async_trait]
impl TelemetryExporter for ReservedTelemetryExporter {
    async fn health_check(&self, config: &ObservabilityConfig) -> Result<(), AppError> {
        if !config.is_enabled() {
            return Ok(());
        }

        Err(AppError::bad_request(
            "OTel exporter health check is reserved but not implemented",
        ))
    }

    async fn export_event(
        &self,
        config: &ObservabilityConfig,
        _event: TelemetryEvent,
    ) -> Result<(), AppError> {
        if !config.is_enabled() {
            return Ok(());
        }

        Err(AppError::bad_request(
            "OTel event export is reserved but not implemented",
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ObservabilityConfig, ReservedTelemetryExporter, TelemetryEvent, TelemetryExporter,
    };

    #[test]
    fn observability_config_defaults_to_disabled_exporter() {
        let config = ObservabilityConfig::from_lookup(|_| None).expect("default config");

        assert_eq!(config.service_name, "mandoforge-api");
        assert_eq!(config.sample_ratio, 1.0);
        assert!(!config.is_enabled());
    }

    #[test]
    fn observability_config_parses_endpoint_and_sampling() {
        let config = ObservabilityConfig::from_lookup(|key| match key {
            "MANDOFORGE_SERVICE_NAME" => Some("agent-os-api".to_string()),
            "MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT" => Some("http://otel:4318".to_string()),
            "MANDOFORGE_OTEL_SAMPLE_RATIO" => Some("0.25".to_string()),
            _ => None,
        })
        .expect("otel config");

        assert_eq!(config.service_name, "agent-os-api");
        assert_eq!(config.otlp_endpoint.as_deref(), Some("http://otel:4318"));
        assert_eq!(config.sample_ratio, 0.25);
        assert!(config.is_enabled());
    }

    #[test]
    fn observability_config_rejects_invalid_sampling() {
        assert!(
            ObservabilityConfig::from_lookup(|key| match key {
                "MANDOFORGE_OTEL_SAMPLE_RATIO" => Some("1.5".to_string()),
                _ => None,
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn reserved_telemetry_exporter_is_noop_until_enabled() {
        let config = ObservabilityConfig::from_lookup(|_| None).expect("default config");
        let exporter = ReservedTelemetryExporter;

        exporter.health_check(&config).await.expect("noop health");
        exporter
            .export_event(
                &config,
                TelemetryEvent {
                    name: "session.started".to_string(),
                    attributes: json!({}),
                },
            )
            .await
            .expect("noop export");
    }

    #[tokio::test]
    async fn reserved_telemetry_exporter_fails_closed_when_enabled() {
        let config = ObservabilityConfig::from_lookup(|key| match key {
            "MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT" => Some("http://otel:4318".to_string()),
            _ => None,
        })
        .expect("otel config");
        let exporter = ReservedTelemetryExporter;

        assert!(exporter.health_check(&config).await.is_err());
        assert!(
            exporter
                .export_event(
                    &config,
                    TelemetryEvent {
                        name: "session.started".to_string(),
                        attributes: json!({}),
                    },
                )
                .await
                .is_err()
        );
    }
}
