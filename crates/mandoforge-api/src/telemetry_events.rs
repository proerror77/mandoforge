use serde_json::{Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::{AppState, SessionEvent, TelemetryEvent};

impl AppState {
    pub(crate) async fn emit_telemetry_event(&self, event: &SessionEvent) {
        if !self.observability_config.is_enabled() || self.observability_config.sample_ratio <= 0.0
        {
            return;
        }
        let telemetry_event = TelemetryEvent {
            name: event.event_type.clone(),
            attributes: telemetry_attributes_for_event(event, self.current_tenant_id()),
        };
        if let Err(error) = self
            .telemetry_exporter
            .export_event(&self.observability_config, telemetry_event)
            .await
        {
            warn!(%error.message, "telemetry export failed");
        }
    }
}

pub(crate) fn telemetry_attributes_for_event(event: &SessionEvent, tenant_id: Uuid) -> Value {
    let category = event.event_type.split('.').next().unwrap_or("event");
    let status = telemetry_status_for_event(event);
    let duration_ms = event
        .payload
        .get("duration_ms")
        .or_else(|| event.payload.get("latency_ms"))
        .and_then(Value::as_i64);
    let mut attributes = json!({
        "tenant_id": tenant_id,
        "session_id": event.session_id,
        "event_id": event.id,
        "seq": event.seq,
        "actor_type": event.actor_type,
        "actor_id": event.actor_id,
        "signal": {
            "type": telemetry_signal_type(category),
            "category": category,
            "span_name": format!("mandoforge.{}", event.event_type),
            "status": status,
        },
        "metrics": {
            "event_count": 1,
            "metric_name": format!("mandoforge.{}.events", category),
        }
    });
    if let Some(duration_ms) = duration_ms {
        attributes["metrics"]["duration_ms"] = json!(duration_ms);
    }
    copy_payload_key(&mut attributes, &event.payload, "provider");
    copy_payload_key(&mut attributes, &event.payload, "client");
    copy_payload_key(&mut attributes, &event.payload, "tool");
    copy_payload_key(&mut attributes, &event.payload, "tool_call_id");
    copy_payload_key(&mut attributes, &event.payload, "approval_id");
    copy_payload_key(&mut attributes, &event.payload, "worker_id");
    if let Some(tool_calls) = event.payload.get("tool_calls").and_then(Value::as_array) {
        attributes["metrics"]["tool_call_count"] = json!(tool_calls.len());
    }
    attributes
}

fn telemetry_signal_type(category: &str) -> &'static str {
    match category {
        "llm" | "tool" | "approval" | "session" | "worker" | "sandbox" | "codex" => "span",
        _ => "log",
    }
}

pub(crate) fn telemetry_status_for_event(event: &SessionEvent) -> &'static str {
    if event.event_type.ends_with(".failed")
        || event.event_type.ends_with(".error")
        || event.event_type.ends_with(".denied")
        || event.payload.get("status").and_then(Value::as_str) == Some("failed")
    {
        "error"
    } else if event.event_type.ends_with(".requested")
        || event.event_type.ends_with(".started")
        || event.event_type.ends_with(".call")
        || event.event_type.ends_with(".request")
    {
        "started"
    } else {
        "ok"
    }
}

fn copy_payload_key(attributes: &mut Value, payload: &Value, key: &str) {
    if let Some(value) = payload.get(key) {
        attributes[key] = value.clone();
    }
}
