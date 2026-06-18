use crate::api::{EnterpriseProductReadiness, ObservabilitySummary, Stage2Readiness};
use crate::components::{
    EnterpriseReadinessPanel, FlowMeter, JsonPreview, KeyMetrics, Panel, Rows, VersionBlock,
};
use crate::state::{ConsoleData, UiLang};
use crate::{
    compact_json, gauge_style, json_object_count, label_or, readiness_from_status, status_tone,
};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct DeployProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
    pub(crate) on_verify: Callback<MouseEvent>,
}

#[component]
pub(crate) fn DeployView(props: &DeployProps) -> Html {
    html! {
        <div class="page-stack">
            <section class="page-purpose">
                <p class="eyebrow">{ props.lang.text("System Ops / 系统运维", "系统运维 / System Ops") }</p>
                <h2>{ props.lang.text("Prove the platform can run safely; do not configure customer business workflows here.", "这里检查平台自身是否能上线，不承载客户业务流程配置。") }</h2>
                <p>{ props.lang.text(
                    "System Ops gathers deployment version, desktop and connector security, audit evidence, usage, finance, and enterprise readiness. Business actions are defined by Ontology and Capabilities, then executed by Managed Agents.",
                    "System Ops 汇总部署版本、桌面端/连接器安全、审计证据、使用量、成本和企业级 readiness。业务动作仍由 Ontology 与 Capabilities 定义，再由 Managed Agents 执行。"
                ) }</p>
            </section>
            <div class="page-grid">
                <Panel title={props.lang.text("Latest Deployment", "最新部署")}>
                    <VersionBlock version={props.data.deployment_version.data.clone()} />
                    <button onclick={props.on_verify.clone()}>{ props.lang.text("Verify deployed version", "验证部署版本") }</button>
                </Panel>
                <Panel title={props.lang.text("Stage 2 Readiness", "Stage 2 就绪状态")}>
                    <ReadinessRadar readiness={props.data.stage2_readiness.data.clone()} observability={props.data.observability.data.clone()} lang={props.lang} />
                </Panel>
                <Panel title={props.lang.text("Enterprise Product Readiness", "企业产品就绪状态")}>
                    <EnterpriseReadinessPanel readiness={props.data.enterprise_product_readiness.data.clone()} />
                </Panel>
                <Panel title={props.lang.text("Customer-grade Evidence Closure", "客户级证据闭环")}>
                    <KeyMetrics values={vec![
                        (props.lang.text("Required evidence", "所需证据").to_string(), label_or(&props.data.enterprise_product_readiness.data.required_evidence_class, "customer_grade").to_string()),
                        (props.lang.text("Completion blocked", "完成被阻塞").to_string(), props.data.enterprise_product_readiness.data.completion_blocked.to_string()),
                        (props.lang.text("Blocked lanes", "阻塞面").to_string(), props.data.enterprise_product_readiness.data.blocked_lane_count.to_string()),
                        (props.lang.text("Ready lanes", "就绪面").to_string(), props.data.enterprise_product_readiness.data.ready_lane_count.to_string()),
                    ]} />
                    <Rows empty={props.lang.text("No customer-grade evidence requirements.", "没有客户级证据要求。")} rows={customer_grade_evidence_rows(
                        props.lang,
                        &props.data.enterprise_product_readiness.data,
                        &props.data.native_connector_production_readiness.data,
                        &props.data.remote_computer_production_path.data,
                        &props.data.usage.data,
                    )} />
                </Panel>
                <Panel title={props.lang.text("Live Connector Production", "真实连接器生产状态")}>
                    <JsonPreview value={props.data.native_connector_production_readiness.data.clone()} />
                </Panel>
                <Panel title={props.lang.text("Remote Computer Path", "远程电脑路径")}>
                    <JsonPreview value={props.data.remote_computer_production_path.data.clone()} />
                </Panel>
                <Panel title={props.lang.text("Usage and Finance", "用量与成本")}>
                    <JsonPreview value={props.data.usage.data.clone()} />
                </Panel>
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct ReadinessRadarProps {
    readiness: Stage2Readiness,
    observability: ObservabilitySummary,
    lang: UiLang,
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
                <small>{ props.lang.text("score", "分数") }</small>
            </div>
            <div class="radar-bars">
                <FlowMeter label={props.lang.text("Categories", "类别")} value={category_count} max={category_count.max(1)} tone="info" />
                <FlowMeter label={props.lang.text("Signals", "信号")} value={signal_count} max={signal_count.max(1)} tone="good" />
                <FlowMeter label={props.lang.text("Observability", "可观测性")} value={usize::from(status_tone(&props.observability.status) == "good")} max={1} tone={status_tone(&props.observability.status)} />
            </div>
        </div>
    }
}

fn customer_grade_evidence_rows(
    lang: UiLang,
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
            zh_title: "真实平台凭证",
            keywords: &["credential", "oauth", "secret", "sandbox", "live"],
            fallback: "Need real live/sandbox credential evidence, expiry posture, and secret safety.",
            zh_fallback: "需要真实 live/sandbox 凭证证据、过期姿态和密钥安全证明。",
        },
        EvidenceRequirement {
            title: "Token refresh and rotation",
            zh_title: "Token 刷新与轮换",
            keywords: &["token refresh", "refresh", "rotation", "expiry", "lwa token"],
            fallback: "Need provider-specific token refresh, rotation, and expiry failure evidence.",
            zh_fallback: "需要供应商特定的 token 刷新、轮换和过期失败证据。",
        },
        EvidenceRequirement {
            title: "Reconciliation and idempotency",
            zh_title: "对账与幂等",
            keywords: &["reconciliation", "idempotency", "idempotent", "duplicate"],
            fallback: "Need replay-safe write evidence, reconciliation reports, and duplicate protection.",
            zh_fallback: "需要可重放安全写入证据、对账报告和重复保护。",
        },
        EvidenceRequirement {
            title: "Webhook or polling delivery",
            zh_title: "Webhook 或轮询交付",
            keywords: &["webhook", "polling", "delivery", "retry"],
            fallback: "Need delivery, retry, backoff, and dead-letter evidence for live platform callbacks.",
            zh_fallback: "需要真实平台回调的交付、重试、退避和死信证据。",
        },
        EvidenceRequirement {
            title: "Compensation policy",
            zh_title: "补偿策略",
            keywords: &["compensation", "rollback", "repair", "remediation"],
            fallback: "Need compensation/repair policy for partial external writes and failed workflows.",
            zh_fallback: "需要部分外部写入和失败流程的补偿/修复策略。",
        },
        EvidenceRequirement {
            title: "Archived deployment evidence",
            zh_title: "已归档部署证据",
            keywords: &["archive", "archived", "deployment evidence", "evidence archive"],
            fallback: "Need immutable customer-grade deployment archive with version, target, logs, and owner.",
            zh_fallback: "需要包含版本、目标、日志和 owner 的不可变客户级部署归档。",
        },
    ]
    .into_iter()
    .map(|requirement| requirement.to_row(lang, &evidence_text))
    .collect()
}

struct EvidenceRequirement {
    title: &'static str,
    zh_title: &'static str,
    keywords: &'static [&'static str],
    fallback: &'static str,
    zh_fallback: &'static str,
}

impl EvidenceRequirement {
    fn to_row(&self, lang: UiLang, evidence_text: &str) -> (String, String, String) {
        let matched = self
            .keywords
            .iter()
            .find(|keyword| evidence_text.contains(**keyword));
        match matched {
            Some(keyword) => (
                "pilot_ready".to_string(),
                lang.text(self.title, self.zh_title).to_string(),
                if lang == UiLang::En {
                    format!("Mentioned in current evidence (`{keyword}`), but not proven customer-grade ready.")
                } else {
                    format!("当前证据提到 `{keyword}`，但还没有证明达到客户级就绪。")
                },
            ),
            None => (
                "blocked".to_string(),
                lang.text(self.title, self.zh_title).to_string(),
                lang.text(self.fallback, self.zh_fallback).to_string(),
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
