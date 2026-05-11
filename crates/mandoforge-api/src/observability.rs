use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

    fn normalized_otlp_endpoint(&self) -> Option<String> {
        self.otlp_endpoint
            .as_deref()
            .map(|endpoint| endpoint.trim_end_matches('/').to_string())
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

#[allow(dead_code)]
pub(crate) struct HttpTelemetryExporter {
    client: reqwest::Client,
}

#[allow(dead_code)]
impl HttpTelemetryExporter {
    pub(crate) fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
        })
    }

    fn health_url(config: &ObservabilityConfig) -> Option<String> {
        config
            .normalized_otlp_endpoint()
            .map(|endpoint| format!("{endpoint}/healthz"))
    }

    fn logs_url(config: &ObservabilityConfig) -> Option<String> {
        config
            .normalized_otlp_endpoint()
            .map(|endpoint| format!("{endpoint}/v1/logs"))
    }

    fn event_payload(config: &ObservabilityConfig, event: TelemetryEvent) -> Value {
        json!({
            "resource": {
                "service.name": config.service_name,
            },
            "events": [event],
        })
    }
}

#[async_trait]
impl TelemetryExporter for HttpTelemetryExporter {
    async fn health_check(&self, config: &ObservabilityConfig) -> Result<(), AppError> {
        let Some(url) = Self::health_url(config) else {
            return Ok(());
        };
        let response = self.client.get(url).send().await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(AppError::bad_request(format!(
            "OTel exporter health check failed with status {}",
            response.status()
        )))
    }

    async fn export_event(
        &self,
        config: &ObservabilityConfig,
        event: TelemetryEvent,
    ) -> Result<(), AppError> {
        let Some(url) = Self::logs_url(config) else {
            return Ok(());
        };
        let response = self
            .client
            .post(url)
            .json(&Self::event_payload(config, event))
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(AppError::bad_request(format!(
            "OTel event export failed with status {}",
            response.status()
        )))
    }
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, routing::get, routing::post};
    use serde_json::json;

    use super::{
        HttpTelemetryExporter, ObservabilityConfig, ReservedTelemetryExporter, TelemetryEvent,
        TelemetryExporter,
    };

    async fn mock_otel_health() -> Json<serde_json::Value> {
        Json(json!({"status": "ok"}))
    }

    async fn mock_otel_logs(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
        assert_eq!(payload["resource"]["service.name"], "agent-os-api");
        assert_eq!(payload["events"][0]["name"], "session.started");
        assert_eq!(
            payload["events"][0]["attributes"]["session_id"],
            "session-1"
        );
        Json(json!({"accepted": true}))
    }

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

    #[tokio::test]
    async fn http_telemetry_exporter_noops_until_enabled() {
        let config = ObservabilityConfig::from_lookup(|_| None).expect("default config");
        let exporter = HttpTelemetryExporter::new().expect("exporter");

        exporter.health_check(&config).await.expect("noop health");
        exporter
            .export_event(
                &config,
                TelemetryEvent {
                    name: "session.started".to_string(),
                    attributes: json!({"session_id": "session-1"}),
                },
            )
            .await
            .expect("noop export");
    }

    #[tokio::test]
    async fn http_telemetry_exporter_posts_logs_to_otlp_boundary() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let app = Router::new()
            .route("/healthz", get(mock_otel_health))
            .route("/v1/logs", post(mock_otel_logs));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("mock otel");
        });
        let config = ObservabilityConfig::from_lookup(|key| match key {
            "MANDOFORGE_SERVICE_NAME" => Some("agent-os-api".to_string()),
            "MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT" => Some(format!("http://{addr}/")),
            _ => None,
        })
        .expect("otel config");
        let exporter = HttpTelemetryExporter::new().expect("exporter");

        exporter.health_check(&config).await.expect("health");
        exporter
            .export_event(
                &config,
                TelemetryEvent {
                    name: "session.started".to_string(),
                    attributes: json!({"session_id": "session-1"}),
                },
            )
            .await
            .expect("export");

        server.abort();
    }
}
