use crate::api::{EnterpriseProductReadiness, ObservabilitySummary, Stage2Readiness};
use crate::components::{
    EnterpriseReadinessPanel, FlowMeter, JsonPreview, KeyMetrics, Panel, Rows, VersionBlock,
};
use crate::state::ConsoleData;
use crate::{
    compact_json, gauge_style, json_object_count, label_or, readiness_from_status, status_tone,
};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct DeployProps {
    pub(crate) data: ConsoleData,
    pub(crate) on_verify: Callback<MouseEvent>,
}

#[component]
pub(crate) fn DeployView(props: &DeployProps) -> Html {
    html! {
        <div class="page-grid">
            <Panel title="Latest deployment">
                <VersionBlock version={props.data.deployment_version.data.clone()} />
                <button onclick={props.on_verify.clone()}>{ "Verify deployed version" }</button>
            </Panel>
            <Panel title="Stage 2 readiness">
                <ReadinessRadar readiness={props.data.stage2_readiness.data.clone()} observability={props.data.observability.data.clone()} />
            </Panel>
            <Panel title="Enterprise product readiness">
                <EnterpriseReadinessPanel readiness={props.data.enterprise_product_readiness.data.clone()} />
            </Panel>
            <Panel title="Customer-grade evidence closure">
                <KeyMetrics values={vec![
                    ("Required evidence".to_string(), label_or(&props.data.enterprise_product_readiness.data.required_evidence_class, "customer_grade").to_string()),
                    ("Completion blocked".to_string(), props.data.enterprise_product_readiness.data.completion_blocked.to_string()),
                    ("Blocked lanes".to_string(), props.data.enterprise_product_readiness.data.blocked_lane_count.to_string()),
                    ("Ready lanes".to_string(), props.data.enterprise_product_readiness.data.ready_lane_count.to_string()),
                ]} />
                <Rows empty="No customer-grade evidence requirements." rows={customer_grade_evidence_rows(
                    &props.data.enterprise_product_readiness.data,
                    &props.data.native_connector_production_readiness.data,
                    &props.data.remote_computer_production_path.data,
                    &props.data.usage.data,
                )} />
            </Panel>
            <Panel title="Live connector production">
                <JsonPreview value={props.data.native_connector_production_readiness.data.clone()} />
            </Panel>
            <Panel title="Remote computer path">
                <JsonPreview value={props.data.remote_computer_production_path.data.clone()} />
            </Panel>
            <Panel title="Usage and finance">
                <JsonPreview value={props.data.usage.data.clone()} />
            </Panel>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct ReadinessRadarProps {
    readiness: Stage2Readiness,
    observability: ObservabilitySummary,
}

#[component]
fn ReadinessRadar(props: &ReadinessRadarProps) -> Html {
    let score = props
        .readiness
        .readiness_score
        .unwrap_or_else(|| readiness_from_status(&props.readiness.status));
    let category_count = json_object_count(&props.readiness.categories);
    let signal_count = json_object_count(&props.observability.signals);
    html! {
        <div class="readiness-radar">
            <div class="radar-dial" style={gauge_style(score)}>
                <span>{ format!("{:.0}", score * 100.0) }</span>
                <small>{ "score" }</small>
            </div>
            <div class="radar-bars">
                <FlowMeter label="Categories" value={category_count} max={category_count.max(1)} tone="info" />
                <FlowMeter label="Signals" value={signal_count} max={signal_count.max(1)} tone="good" />
                <FlowMeter label="Observability" value={usize::from(status_tone(&props.observability.status) == "good")} max={1} tone={status_tone(&props.observability.status)} />
            </div>
        </div>
    }
}

fn customer_grade_evidence_rows(
    readiness: &EnterpriseProductReadiness,
    connector_readiness: &Value,
    remote_path: &Value,
    usage: &Value,
) -> Vec<(String, String, String)> {
    let evidence_text = [
        readiness_text(readiness),
        compact_json(connector_readiness),
        compact_json(remote_path),
        compact_json(usage),
    ]
    .join("\n")
    .to_ascii_lowercase();

    [
        EvidenceRequirement {
            title: "Real platform credentials",
            keywords: &["credential", "oauth", "secret", "sandbox", "live"],
            fallback: "Need real live/sandbox credential evidence, expiry posture, and secret safety.",
        },
        EvidenceRequirement {
            title: "Token refresh and rotation",
            keywords: &["token refresh", "refresh", "rotation", "expiry", "lwa token"],
            fallback: "Need provider-specific token refresh, rotation, and expiry failure evidence.",
        },
        EvidenceRequirement {
            title: "Reconciliation and idempotency",
            keywords: &["reconciliation", "idempotency", "idempotent", "duplicate"],
            fallback: "Need replay-safe write evidence, reconciliation reports, and duplicate protection.",
        },
        EvidenceRequirement {
            title: "Webhook or polling delivery",
            keywords: &["webhook", "polling", "delivery", "retry"],
            fallback: "Need delivery, retry, backoff, and dead-letter evidence for live platform callbacks.",
        },
        EvidenceRequirement {
            title: "Compensation policy",
            keywords: &["compensation", "rollback", "repair", "remediation"],
            fallback: "Need compensation/repair policy for partial external writes and failed workflows.",
        },
        EvidenceRequirement {
            title: "Archived deployment evidence",
            keywords: &["archive", "archived", "deployment evidence", "evidence archive"],
            fallback: "Need immutable customer-grade deployment archive with version, target, logs, and owner.",
        },
    ]
    .into_iter()
    .map(|requirement| requirement.to_row(&evidence_text))
    .collect()
}

struct EvidenceRequirement {
    title: &'static str,
    keywords: &'static [&'static str],
    fallback: &'static str,
}

impl EvidenceRequirement {
    fn to_row(&self, evidence_text: &str) -> (String, String, String) {
        let matched = self
            .keywords
            .iter()
            .find(|keyword| evidence_text.contains(**keyword));
        match matched {
            Some(keyword) => (
                "pilot_ready".to_string(),
                self.title.to_string(),
                format!(
                    "Mentioned in current evidence (`{keyword}`), but not proven customer-grade ready."
                ),
            ),
            None => (
                "blocked".to_string(),
                self.title.to_string(),
                self.fallback.to_string(),
            ),
        }
    }
}

fn readiness_text(readiness: &EnterpriseProductReadiness) -> String {
    let mut parts = vec![
        readiness.status.clone(),
        readiness.required_evidence_class.clone(),
        readiness.message.clone(),
    ];
    parts.extend(readiness.next_actions.clone());
    for lane in &readiness.lanes {
        parts.push(lane.id.clone());
        parts.push(lane.title.clone());
        parts.push(lane.status.clone());
        parts.push(lane.current_evidence_class.clone());
        parts.push(lane.required_evidence_class.clone());
        parts.push(lane.production_target.clone());
        parts.extend(lane.blockers.clone());
        parts.extend(lane.next_actions.clone());
    }
    parts.join("\n")
}
