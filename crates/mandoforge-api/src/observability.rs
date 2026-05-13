use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppError;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct ObservabilityConfig {
    pub(crate) service_name: String,
    pub(crate) otlp_endpoint: Option<String>,
    pub(crate) collector_health_endpoint: Option<String>,
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
        let collector_health_endpoint = lookup("MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT")
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
            collector_health_endpoint,
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

    fn normalized_collector_health_endpoint(&self) -> Option<String> {
        self.collector_health_endpoint
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
        config.normalized_collector_health_endpoint().or_else(|| {
            config
                .normalized_otlp_endpoint()
                .map(|endpoint| format!("{endpoint}/healthz"))
        })
    }

    fn logs_url(config: &ObservabilityConfig) -> Option<String> {
        config
            .normalized_otlp_endpoint()
            .map(|endpoint| format!("{endpoint}/v1/logs"))
    }

    fn traces_url(config: &ObservabilityConfig) -> Option<String> {
        config
            .normalized_otlp_endpoint()
            .map(|endpoint| format!("{endpoint}/v1/traces"))
    }

    fn metrics_url(config: &ObservabilityConfig) -> Option<String> {
        config
            .normalized_otlp_endpoint()
            .map(|endpoint| format!("{endpoint}/v1/metrics"))
    }

    fn log_payload(config: &ObservabilityConfig, event: &TelemetryEvent) -> Value {
        let time_unix_nano = telemetry_time_unix_nano();
        json!({
            "resourceLogs": [{
                "resource": {
                    "attributes": [otlp_string_attr("service.name", &config.service_name)]
                },
                "scopeLogs": [{
                    "scope": {"name": "mandoforge-runtime"},
                    "logRecords": [{
                        "timeUnixNano": time_unix_nano,
                        "severityText": telemetry_severity(event),
                        "body": {"stringValue": event.name},
                        "attributes": otlp_attributes(&event.attributes)
                    }]
                }]
            }]
        })
    }

    fn trace_payload(config: &ObservabilityConfig, event: &TelemetryEvent) -> Value {
        let time_unix_nano = telemetry_time_unix_nano();
        json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": [otlp_string_attr("service.name", &config.service_name)]
                },
                "scopeSpans": [{
                    "scope": {"name": "mandoforge-runtime"},
                    "spans": [{
                        "traceId": telemetry_trace_id(event),
                        "spanId": telemetry_span_id(event),
                        "name": event.name,
                        "kind": 1,
                        "startTimeUnixNano": time_unix_nano,
                        "endTimeUnixNano": time_unix_nano,
                        "attributes": otlp_attributes(&event.attributes),
                        "status": {"code": telemetry_status_code(event)}
                    }]
                }]
            }]
        })
    }

    fn metric_payload(config: &ObservabilityConfig, event: &TelemetryEvent) -> Value {
        let time_unix_nano = telemetry_time_unix_nano();
        let metric_name = event
            .attributes
            .get("metrics")
            .and_then(|metrics| metrics.get("metric_name"))
            .and_then(Value::as_str)
            .unwrap_or("mandoforge.events");
        json!({
            "resourceMetrics": [{
                "resource": {
                    "attributes": [otlp_string_attr("service.name", &config.service_name)]
                },
                "scopeMetrics": [{
                    "scope": {"name": "mandoforge-runtime"},
                    "metrics": [{
                        "name": metric_name,
                        "description": "MandoForge runtime event count",
                        "unit": "1",
                        "sum": {
                            "aggregationTemporality": 2,
                            "isMonotonic": true,
                            "dataPoints": [{
                                "timeUnixNano": time_unix_nano,
                                "asInt": "1",
                                "attributes": otlp_attributes(&event.attributes)
                            }]
                        }
                    }]
                }]
            }]
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
        let Some(logs_url) = Self::logs_url(config) else {
            return Ok(());
        };
        self.post_otlp(logs_url, Self::log_payload(config, &event), "logs")
            .await?;
        if let Some(traces_url) = Self::traces_url(config) {
            self.post_otlp(traces_url, Self::trace_payload(config, &event), "traces")
                .await?;
        }
        if let Some(metrics_url) = Self::metrics_url(config) {
            self.post_otlp(metrics_url, Self::metric_payload(config, &event), "metrics")
                .await?;
        }
        Ok(())
    }
}

impl HttpTelemetryExporter {
    async fn post_otlp(&self, url: String, payload: Value, signal: &str) -> Result<(), AppError> {
        let response = self.client.post(url).json(&payload).send().await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(AppError::bad_request(format!(
            "OTel {signal} export failed with status {}",
            response.status()
        )))
    }
}

fn telemetry_time_unix_nano() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn telemetry_severity(event: &TelemetryEvent) -> &'static str {
    match event
        .attributes
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("ok")
    {
        "error" => "ERROR",
        "warning" => "WARN",
        _ => "INFO",
    }
}

fn telemetry_status_code(event: &TelemetryEvent) -> i32 {
    if event
        .attributes
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "error")
    {
        2
    } else {
        1
    }
}

fn telemetry_trace_id(event: &TelemetryEvent) -> String {
    event
        .attributes
        .get("session_id")
        .and_then(Value::as_str)
        .map(hex_32_from_text)
        .unwrap_or_else(|| "00000000000000000000000000000001".to_string())
}

fn telemetry_span_id(event: &TelemetryEvent) -> String {
    event
        .attributes
        .get("event_id")
        .and_then(Value::as_str)
        .map(hex_16_from_text)
        .unwrap_or_else(|| hex_16_from_text(&event.name))
}

fn hex_32_from_text(value: &str) -> String {
    let compact: String = value
        .chars()
        .filter(|character| *character != '-')
        .collect();
    let mut output: String = compact
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .take(32)
        .collect();
    while output.len() < 32 {
        output.push('0');
    }
    output
}

fn hex_16_from_text(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn otlp_string_attr(key: &str, value: &str) -> Value {
    json!({"key": key, "value": {"stringValue": value}})
}

fn otlp_attributes(attributes: &Value) -> Vec<Value> {
    let Some(object) = attributes.as_object() else {
        return vec![];
    };
    object
        .iter()
        .map(|(key, value)| json!({"key": key, "value": otlp_any_value(value)}))
        .collect()
}

fn otlp_any_value(value: &Value) -> Value {
    match value {
        Value::Bool(value) => json!({"boolValue": value}),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                json!({"intValue": value.to_string()})
            } else if let Some(value) = value.as_u64() {
                json!({"intValue": value.to_string()})
            } else {
                json!({"doubleValue": value.as_f64().unwrap_or_default()})
            }
        }
        Value::String(value) => json!({"stringValue": value}),
        Value::Array(values) => json!({
            "arrayValue": {
                "values": values.iter().map(otlp_any_value).collect::<Vec<_>>()
            }
        }),
        Value::Object(_) => json!({"stringValue": value.to_string()}),
        Value::Null => json!({"stringValue": ""}),
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
        assert_eq!(
            payload["resourceLogs"][0]["resource"]["attributes"][0]["value"]["stringValue"],
            "agent-os-api"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]["stringValue"],
            "session.started"
        );
        Json(json!({"accepted": true}))
    }

    async fn mock_otel_traces(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
        assert_eq!(
            payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"],
            "session.started"
        );
        assert_eq!(
            payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"],
            "00000000000000000000000000000001"
        );
        Json(json!({"accepted": true}))
    }

    async fn mock_otel_metrics(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
        assert_eq!(
            payload["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0]["name"],
            "mandoforge.session.events"
        );
        assert_eq!(
            payload["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0]["sum"]["dataPoints"][0]
                ["asInt"],
            "1"
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
            "MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT" => {
                Some("http://otel:13133/healthz".to_string())
            }
            "MANDOFORGE_OTEL_SAMPLE_RATIO" => Some("0.25".to_string()),
            _ => None,
        })
        .expect("otel config");

        assert_eq!(config.service_name, "agent-os-api");
        assert_eq!(config.otlp_endpoint.as_deref(), Some("http://otel:4318"));
        assert_eq!(
            config.collector_health_endpoint.as_deref(),
            Some("http://otel:13133/healthz")
        );
        assert_eq!(
            HttpTelemetryExporter::health_url(&config).as_deref(),
            Some("http://otel:13133/healthz")
        );
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
    async fn http_telemetry_exporter_posts_logs_traces_and_metrics_to_otlp_boundary() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("local addr");
        let app = Router::new()
            .route("/healthz", get(mock_otel_health))
            .route("/v1/logs", post(mock_otel_logs))
            .route("/v1/traces", post(mock_otel_traces))
            .route("/v1/metrics", post(mock_otel_metrics));
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
                    attributes: json!({
                        "session_id": "00000000-0000-0000-0000-000000000001",
                        "event_id": "event-1",
                        "status": "ok",
                        "metrics": {"metric_name": "mandoforge.session.events"}
                    }),
                },
            )
            .await
            .expect("export");

        server.abort();
    }
}
