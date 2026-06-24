use crate::api::{OntologyOnboardingRun, OntologyRelease};
use crate::components::KeyMetrics;
use crate::views::semantic_i18n::{SemanticLang, localized_status};
use crate::{label_or, short_id, status_tone};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct OntologyReleaseControlProps {
    pub(crate) lang: SemanticLang,
    pub(crate) run: Option<OntologyOnboardingRun>,
    pub(crate) releases: Vec<OntologyRelease>,
    pub(crate) readiness: Value,
    pub(crate) release_version: String,
    pub(crate) on_release_version: Callback<InputEvent>,
    pub(crate) on_create_candidate: Callback<MouseEvent>,
    pub(crate) on_gate: Callback<String>,
    pub(crate) on_promote: Callback<String>,
    pub(crate) on_rollback: Callback<String>,
    pub(crate) on_archive: Callback<String>,
}

#[component]
pub(crate) fn OntologyReleaseControl(props: &OntologyReleaseControlProps) -> Html {
    let active_release = props
        .releases
        .iter()
        .rev()
        .find(|release| ontology_release_current_status(&release.status))
        .cloned();
    let latest_release = props.releases.iter().rev().next().cloned();
    let candidate_count = props
        .releases
        .iter()
        .filter(|release| matches!(release.status.as_str(), "candidate" | "failed_gate"))
        .count();
    let can_create_candidate = props
        .run
        .as_ref()
        .map(|run| run.materialized_count > 0)
        .unwrap_or(false);
    let readiness_rows = release_readiness_rows(props.lang, &props.readiness);
    let release_rows = props
        .releases
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>();

    html! {
        <section class="ontology-release-control" aria-label={props.lang.text("Ontology release control", "本体发布控制")}>
            <header class="semantic-section-head">
                <div>
                    <span>{ props.lang.text("Release control", "发布控制") }</span>
                    <h2>{ props.lang.text("Promote reviewed ontology into runtime", "把已审核本体发布到运行时") }</h2>
                </div>
                <small>{ props.lang.text(
                    "Runtime context pins only the active domain release; rollback restores the previous active release without deleting semantics.",
                    "运行时上下文只绑定当前 active 的领域版本；回滚会恢复上一 active 版本，不删除语义对象。"
                ) }</small>
            </header>
            <div class="ontology-release-grid">
                <section class="ontology-release-current">
                    <div class="ontology-release-current-head">
                        <span>{ props.lang.text("Active release", "当前 active 版本") }</span>
                        <strong>{ active_release.as_ref().map(|release| release.version.clone()).unwrap_or_else(|| props.lang.text("No active release", "暂无 active 版本").to_string()) }</strong>
                    </div>
                    <KeyMetrics values={vec![
                        (props.lang.text("Domain", "领域").to_string(), active_release.as_ref().map(|release| label_or(&release.domain_scope, "none").to_string()).unwrap_or_else(|| "none".to_string())),
                        (props.lang.text("Class", "等级").to_string(), active_release.as_ref().map(|release| localized_status(props.lang, label_or(&release.release_class, "repo_controlled"))).unwrap_or_else(|| "none".to_string())),
                        (props.lang.text("Candidates", "候选").to_string(), candidate_count.to_string()),
                        (props.lang.text("Latest", "最新").to_string(), latest_release.as_ref().map(|release| localized_status(props.lang, label_or(&release.status, "none"))).unwrap_or_else(|| "none".to_string())),
                    ]} />
                    <div class="ontology-release-create">
                        <input
                            id="ontology-release-version"
                            name="ontology-release-version"
                            value={props.release_version.clone()}
                            placeholder={props.lang.text("Optional version, for example commerce-v1", "可选版本号，例如 commerce-v1")}
                            oninput={props.on_release_version.clone()}
                        />
                        <button onclick={props.on_create_candidate.clone()} disabled={!can_create_candidate}>
                            { props.lang.text("Create candidate", "创建候选版本") }
                        </button>
                    </div>
                    <small class="ontology-release-hint">{ if can_create_candidate {
                        props.lang.text("Uses materialized reviewed proposals from the current run.", "使用当前运行中已发布的审核提案。")
                    } else {
                        props.lang.text("Materialize at least one approved proposal before creating a release candidate.", "先发布至少一个已批准提案，再创建候选版本。")
                    } }</small>
                </section>
                <section class="ontology-release-readiness">
                    <span>{ props.lang.text("Release-backed readiness", "发布证据就绪项") }</span>
                    <div class="ontology-release-readiness-list">
                        { for readiness_rows.into_iter().map(|(id, status, evidence)| html! {
                            <div class="ontology-release-readiness-row" key={id.clone()}>
                                <strong>{ id }</strong>
                                <span>{ localized_status(props.lang, &status) }</span>
                                <small>{ localized_status(props.lang, &evidence) }</small>
                            </div>
                        }) }
                    </div>
                </section>
            </div>
            <div class="ontology-release-list">
                { if release_rows.is_empty() {
                    html! {
                        <div class="ontology-release-empty">
                            <strong>{ props.lang.text("No release records yet.", "还没有发布记录。") }</strong>
                            <small>{ props.lang.text("Create a candidate after materialization to close the production loop.", "materialize 后创建候选版本，才能闭合生产发布链路。") }</small>
                        </div>
                    }
                } else {
                    html! { { for release_rows.into_iter().map(|release| {
                        let release_key = release.id.clone();
                        html! {
                            <OntologyReleaseRow
                                key={release_key}
                                lang={props.lang}
                                release={release}
                                on_gate={props.on_gate.clone()}
                                on_promote={props.on_promote.clone()}
                                on_rollback={props.on_rollback.clone()}
                                on_archive={props.on_archive.clone()}
                            />
                        }
                    }) } }
                } }
            </div>
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyReleaseRowProps {
    lang: SemanticLang,
    release: OntologyRelease,
    on_gate: Callback<String>,
    on_promote: Callback<String>,
    on_rollback: Callback<String>,
    on_archive: Callback<String>,
}

#[component]
fn OntologyReleaseRow(props: &OntologyReleaseRowProps) -> Html {
    let gate_status = props
        .release
        .gate_result
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("not_checked")
        .to_string();
    let can_gate = matches!(props.release.status.as_str(), "candidate" | "failed_gate");
    let can_promote = props.release.status == "candidate" && gate_status == "passed";
    let current_release = ontology_release_current_status(&props.release.status);
    let can_rollback = current_release && props.release.rollback_target_release_id.is_some();
    let can_archive = !current_release && props.release.status != "archived";
    let gate = {
        let id = props.release.id.clone();
        let on_gate = props.on_gate.clone();
        Callback::from(move |_| on_gate.emit(id.clone()))
    };
    let promote = {
        let id = props.release.id.clone();
        let on_promote = props.on_promote.clone();
        Callback::from(move |_| on_promote.emit(id.clone()))
    };
    let rollback = {
        let id = props.release.id.clone();
        let on_rollback = props.on_rollback.clone();
        Callback::from(move |_| on_rollback.emit(id.clone()))
    };
    let archive = {
        let id = props.release.id.clone();
        let on_archive = props.on_archive.clone();
        Callback::from(move |_| on_archive.emit(id.clone()))
    };

    html! {
        <article class="ontology-release-row">
            <div class="ontology-release-row-main">
                <span class={classes!("ontology-release-status", status_tone(&props.release.status))}>
                    { localized_status(props.lang, label_or(&props.release.status, "unknown")) }
                </span>
                <strong>{ label_or(&props.release.version, "unversioned") }</strong>
                <small>{ format!(
                    "{} · {} · {}",
                    label_or(&props.release.domain_scope, "domain"),
                    localized_status(props.lang, label_or(&props.release.release_class, "repo_controlled")),
                    short_id(&props.release.id)
                ) }</small>
            </div>
            <div class="ontology-release-row-metrics">
                <span><strong>{ props.release.object_count }</strong><small>{ props.lang.text("objects", "对象") }</small></span>
                <span><strong>{ props.release.relation_count }</strong><small>{ props.lang.text("relations", "关系") }</small></span>
                <span><strong>{ props.release.action_count }</strong><small>{ props.lang.text("actions", "动作") }</small></span>
                <span><strong>{ localized_status(props.lang, &gate_status) }</strong><small>{ props.lang.text("gate", "闸门") }</small></span>
            </div>
            <div class="ontology-release-row-actions">
                <button class="secondary" onclick={gate} disabled={!can_gate}>{ props.lang.text("Gate", "闸门") }</button>
                <button onclick={promote} disabled={!can_promote}>{ props.lang.text("Promote", "发布") }</button>
                <button class="secondary" onclick={rollback} disabled={!can_rollback}>{ props.lang.text("Rollback", "回滚") }</button>
                <button class="secondary" onclick={archive} disabled={!can_archive}>{ props.lang.text("Archive", "归档") }</button>
            </div>
        </article>
    }
}

fn ontology_release_current_status(status: &str) -> bool {
    matches!(status, "active" | "active_trigger_failed")
}

fn release_readiness_rows(lang: SemanticLang, readiness: &Value) -> Vec<(String, String, String)> {
    let labels = [
        (
            "domain-ontology-lifecycle",
            lang.text("Lifecycle", "版本生命周期"),
        ),
        (
            "approved-release-materialization",
            lang.text("Materialization", "审核发布"),
        ),
        ("migration-policy", lang.text("Migration", "迁移策略")),
        (
            "conflict-trust-runtime-gates",
            lang.text("Runtime gates", "运行时闸门"),
        ),
    ];
    labels
        .into_iter()
        .map(|(id, label)| {
            let check = readiness
                .get("checks")
                .and_then(|value| value.as_array())
                .and_then(|checks| {
                    checks
                        .iter()
                        .find(|check| check.get("id").and_then(|value| value.as_str()) == Some(id))
                });
            let status = check
                .and_then(|check| check.get("status"))
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let evidence = check
                .and_then(|check| check.get("current_evidence_class"))
                .and_then(|value| value.as_str())
                .unwrap_or("none")
                .to_string();
            (label.to_string(), status, evidence)
        })
        .collect()
}
