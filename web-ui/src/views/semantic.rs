use crate::api::{
    ConfidenceCalibrationResponse, OntologyOnboardingProposal, OntologyOnboardingRun,
    OntologyOnboardingToolSpec, OntologyReviewGraph, OntologyReviewGraphEdge,
    OntologyReviewGraphNode, RenderedExecutionContext, SemanticGraphSnapshot, SemanticObject,
};
use crate::components::{FlowMeter, JsonPreview, KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{
    compact_json, label_or, orbit_point, position_style, semantic_scope_summary, short_id,
    status_tone,
};
use serde_json::Value;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct SemanticProps {
    pub(crate) data: ConsoleData,
    pub(crate) source_text: String,
    pub(crate) context_packet_id: String,
    pub(crate) rendered_context: Option<RenderedExecutionContext>,
    pub(crate) onboarding_run: Option<OntologyOnboardingRun>,
    pub(crate) onboarding_tool_specs: Vec<OntologyOnboardingToolSpec>,
    pub(crate) onboarding_review_graph: Option<OntologyReviewGraph>,
    pub(crate) onboarding_calibration: Option<ConfidenceCalibrationResponse>,
    pub(crate) on_source: Callback<InputEvent>,
    pub(crate) on_build: Callback<MouseEvent>,
    pub(crate) on_context_packet_id: Callback<InputEvent>,
    pub(crate) on_render_context: Callback<MouseEvent>,
    pub(crate) on_start_onboarding: Callback<MouseEvent>,
    pub(crate) on_approve_onboarding_proposal: Callback<String>,
    pub(crate) on_reject_onboarding_proposal: Callback<String>,
    pub(crate) on_materialize_onboarding: Callback<MouseEvent>,
}

#[derive(Clone, Copy, PartialEq)]
enum SemanticLang {
    Zh,
    En,
}

impl SemanticLang {
    fn text(self, en: &'static str, zh: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::Zh => zh,
        }
    }
}

fn localized_status(lang: SemanticLang, status: &str) -> String {
    if lang == SemanticLang::En {
        return status.to_string();
    }
    match status {
        "approved" => "已批准",
        "rejected" => "已拒绝",
        "pending" => "待审核",
        "pending_review" => "待审核",
        "materialized" => "已发布",
        "ready" => "就绪",
        "approval" | "approval_required" | "write_approval_required" => "需审批",
        "needs_review" => "需复核",
        "blocked" => "已阻塞",
        "pilot_ready" => "试点就绪",
        "active" => "运行中",
        "completed" => "已完成",
        "proposal_only" => "仅提案",
        "profiled" => "已画像",
        "proposed" => "已提议",
        "compiled" => "已生成",
        "referenced" => "被引用",
        "profile_unset" => "事务未设置",
        "mode_unset" => "模式未设置",
        "read_only" => "只读",
        "" => "未设置",
        other => other,
    }
    .to_string()
}

fn localized_source_mode(lang: SemanticLang, source_mode: &str) -> String {
    if lang == SemanticLang::En {
        return label_or(source_mode, "demo").to_string();
    }
    match source_mode {
        "demo_ecommerce" | "demo" => "电商示例",
        "demo_insurance" => "保险示例",
        "" => "示例",
        other => other,
    }
    .to_string()
}

fn localized_risk(lang: SemanticLang, risk: &str) -> String {
    if lang == SemanticLang::En {
        return risk.to_string();
    }
    match risk {
        "low" => "低风险",
        "medium" => "中风险",
        "high" => "高风险",
        "needs_review" => "需复核",
        "approval_required" => "需审批",
        "merge" => "合并",
        "pii_review" => "PII 复核",
        "merge_review_required" => "合并复核",
        "possible_match" => "可能匹配",
        "blocked" => "已阻塞",
        "risk_unset" => "风险未设置",
        "" => "未设置",
        other => other,
    }
    .to_string()
}

#[component]
pub(crate) fn SemanticView(props: &SemanticProps) -> Html {
    let lang = use_state(|| SemanticLang::Zh);
    let current_lang = *lang;
    let set_zh = {
        let lang = lang.clone();
        Callback::from(move |_| lang.set(SemanticLang::Zh))
    };
    let set_en = {
        let lang = lang.clone();
        Callback::from(move |_| lang.set(SemanticLang::En))
    };
    let run = props.onboarding_run.as_ref();
    html! {
        <div class="semantic-workbench">
            <section class="semantic-hero">
                <div class="semantic-hero-copy">
                    <span class="semantic-kicker">{ current_lang.text("Ontology Builder", "本体构建器") }</span>
                    <h1>{ current_lang.text("Turn enterprise data into reviewed agent tools.", "把企业数据变成可审核、可发布的智能体工具。") }</h1>
                    <p>{ current_lang.text(
                        "The LLM loop mines schemas, profiles, samples, and lineage into ontology proposals. Humans approve the business meaning before anything is materialized.",
                        "LLM 循环挖掘 schema、画像、样本和血缘，生成本体提案；人工确认业务含义之后，才写入语义层。"
                    ) }</p>
                </div>
                <div class="semantic-hero-controls">
                    <div class="semantic-language-toggle" aria-label="Language">
                        <button class={classes!((*lang == SemanticLang::Zh).then_some("active"))} onclick={set_zh}>{ "中文" }</button>
                        <button class={classes!((*lang == SemanticLang::En).then_some("active"))} onclick={set_en}>{ "EN" }</button>
                    </div>
                    <div class="semantic-hero-actions">
                        <button onclick={props.on_start_onboarding.clone()}>{ current_lang.text("Start ecommerce sample", "开始电商示例") }</button>
                        <button
                            class="secondary"
                            onclick={props.on_materialize_onboarding.clone()}
                            disabled={run.map(|run| run.approved_count == 0).unwrap_or(true)}
                        >
                            { current_lang.text("Materialize approved", "发布已批准项") }
                        </button>
                    </div>
                    <KeyMetrics values={vec![
                        (current_lang.text("Run", "运行").to_string(), run.map(|run| short_id(&run.id)).unwrap_or_else(|| current_lang.text("Not started", "未开始").to_string())),
                        (current_lang.text("Datasets", "数据集").to_string(), run.map(|run| run.dataset_count.to_string()).unwrap_or_else(|| "0".to_string())),
                        (current_lang.text("Proposals", "提案").to_string(), run.map(|run| run.proposal_count.to_string()).unwrap_or_else(|| "0".to_string())),
                        (current_lang.text("Approved", "已批准").to_string(), run.map(|run| run.approved_count.to_string()).unwrap_or_else(|| "0".to_string())),
                    ]} />
                </div>
            </section>

            <SemanticJourney lang={current_lang} run={props.onboarding_run.clone()} tool_count={props.onboarding_tool_specs.len()} />

            <div class="semantic-main-layout">
                <section class="semantic-primary-card">
                    <header class="semantic-section-head">
                        <div>
                            <span>{ current_lang.text("Ontology map", "本体关系图") }</span>
                            <h2>{ current_lang.text("Inspect tables, objects, links, and actions", "检查资料表、业务对象、关系和动作") }</h2>
                        </div>
                        <small>{ current_lang.text("Start from the graph, then approve the exact proposals behind each edge.", "先看业务关系图，再审核每条关系背后的提案证据。") }</small>
                    </header>
                <OnboardingPanel
                    lang={current_lang}
                    run={props.onboarding_run.clone()}
                    tool_specs={props.onboarding_tool_specs.clone()}
                    review_graph={props.onboarding_review_graph.clone()}
                    calibration={props.onboarding_calibration.clone()}
                    on_start={props.on_start_onboarding.clone()}
                    on_approve={props.on_approve_onboarding_proposal.clone()}
                    on_reject={props.on_reject_onboarding_proposal.clone()}
                    on_materialize={props.on_materialize_onboarding.clone()}
                />
                </section>

                <details class="semantic-operator-tools">
                    <summary>{ current_lang.text("Advanced tools: draft, readiness, and context", "高级工具：草稿、状态和上下文") }</summary>
                    <aside class="semantic-side-rail">
                        <section class="semantic-utility-card">
                            <header class="semantic-section-head compact">
                                <div>
                                    <span>{ current_lang.text("Quick draft", "快速草稿") }</span>
                                    <h2>{ current_lang.text("Model extraction V1", "模型抽取 V1") }</h2>
                                </div>
                            </header>
                            <div class="form-stack">
                                <textarea
                                    id="ontology-builder-source"
                                    name="ontology-builder-source"
                                    aria-label={current_lang.text("Ontology builder source text", "本体构建来源文本")}
                                    value={props.source_text.clone()}
                                    oninput={props.on_source.clone()}
                                />
                                <button onclick={props.on_build.clone()}>{ current_lang.text("Preview proposal", "预览提案") }</button>
                            </div>
                        </section>

                        <section class="semantic-utility-card">
                            <header class="semantic-section-head compact">
                                <div>
                                    <span>{ current_lang.text("Readiness", "就绪状态") }</span>
                                    <h2>{ current_lang.text("Engine status", "引擎状态") }</h2>
                                </div>
                            </header>
                            <OntologyReadinessSummary
                                lang={current_lang}
                                value={props.data.ontology_engine_readiness.data.clone()}
                            />
                        </section>

                        <section class="semantic-utility-card">
                            <header class="semantic-section-head compact">
                                <div>
                                    <span>{ current_lang.text("Context", "上下文") }</span>
                                    <h2>{ current_lang.text("Prompt compiler", "提示词上下文编译") }</h2>
                                </div>
                            </header>
                            <div class="form-stack">
                                <input
                                    id="context-packet-id"
                                    name="context-packet-id"
                                    value={props.context_packet_id.clone()}
                                    placeholder={current_lang.text("Context packet ID", "上下文包 ID")}
                                    oninput={props.on_context_packet_id.clone()}
                                />
                                <button onclick={props.on_render_context.clone()}>{ current_lang.text("Render context", "渲染上下文") }</button>
                                <RenderedContextPreview lang={current_lang} rendered={props.rendered_context.clone()} />
                            </div>
                        </section>
                    </aside>
                </details>
            </div>

            <details class="semantic-advanced">
                <summary>{ current_lang.text("Advanced semantic state, graph, and governance", "高级：语义图、对象和治理状态") }</summary>
                <div class="page-grid semantic-debug-grid">
                    <Panel title="语义图快照">
                        <SemanticMap snapshot={props.data.semantic_graph.data.clone()} objects={props.data.semantic_objects.data.clone()} />
                    </Panel>
                    <Panel title="语义对象">
                        <Rows empty="还没有语义对象。" rows={props.data.semantic_objects.data.iter().take(10).map(|object| {
                            (
                                object.status.clone(),
                                label_or(&object.title, &object.object_key).to_string(),
                                format!("{} / {} / {}", object.object_type, object.trust_level, semantic_scope_summary(&object.semantic_scopes))
                            )
                        }).collect::<Vec<_>>()} />
                    </Panel>
                    <Panel title="治理状态">
                        <KeyMetrics values={vec![
                            ("写回".to_string(), compact_json(&props.data.memory_writebacks.data)),
                            ("候选写回".to_string(), compact_json(&props.data.memory_writeback_candidates.data)),
                            ("本体版本".to_string(), props.data.ontology_registry.data.version.clone()),
                            ("反思队列".to_string(), props.data.semantic_reflection_queue.data.status.clone()),
                        ]} />
                    </Panel>
                </div>
            </details>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyReadinessSummaryProps {
    lang: SemanticLang,
    value: Value,
}

#[component]
fn OntologyReadinessSummary(props: &OntologyReadinessSummaryProps) -> Html {
    let status = props
        .value
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let ready = props
        .value
        .get("ready_check_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let pilot = props
        .value
        .get("pilot_ready_check_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let blocked = props
        .value
        .get("blocked_check_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let object_types = props
        .value
        .get("object_type_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let relation_types = props
        .value
        .get("relation_type_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let registry = props
        .value
        .get("registry_version")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");

    html! {
        <div class="ontology-readiness-summary">
            <KeyMetrics values={vec![
                (props.lang.text("Status", "状态").to_string(), localized_status(props.lang, status)),
                (props.lang.text("Ready", "已就绪").to_string(), ready.to_string()),
                (props.lang.text("Pilot", "试点").to_string(), pilot.to_string()),
                (props.lang.text("Blocked", "阻塞").to_string(), blocked.to_string()),
                (props.lang.text("Objects", "对象类型").to_string(), object_types.to_string()),
                (props.lang.text("Relations", "关系类型").to_string(), relation_types.to_string()),
                (props.lang.text("Registry", "注册表").to_string(), registry.to_string()),
            ]} />
            <p>{ props.lang.text(
                "This summarizes whether ontology proposals can be reviewed and materialized safely. Raw evidence is available below for debugging.",
                "这里汇总本体提案是否能被安全审核和发布。原始证据保留在下方，供调试使用。"
            ) }</p>
            <details class="semantic-inline-details">
                <summary>{ props.lang.text("Raw readiness JSON", "原始就绪 JSON") }</summary>
                <JsonPreview value={props.value.clone()} />
            </details>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SemanticJourneyProps {
    lang: SemanticLang,
    run: Option<OntologyOnboardingRun>,
    tool_count: usize,
}

#[component]
fn SemanticJourney(props: &SemanticJourneyProps) -> Html {
    let run_started = props.run.is_some();
    let proposed = props
        .run
        .as_ref()
        .map(|run| run.proposal_count > 0)
        .unwrap_or(false);
    let reviewed = props
        .run
        .as_ref()
        .map(|run| {
            run.proposals.iter().any(|proposal| {
                proposal.review_status == "approved" || proposal.review_status == "rejected"
            })
        })
        .unwrap_or(false);
    let materialized = props
        .run
        .as_ref()
        .map(|run| run.materialized_count > 0)
        .unwrap_or(false);
    let tool_ready = props.tool_count > 0;
    let steps = vec![
        (
            "01",
            props.lang.text("Connect source", "接入数据"),
            props
                .lang
                .text("Use connector or demo bundle", "使用连接器或示例数据包"),
            true,
        ),
        (
            "02",
            props.lang.text("Profile evidence", "扫描画像"),
            props
                .lang
                .text("Keys, joins, nulls, samples", "主键、关联、空值、样本"),
            run_started,
        ),
        (
            "03",
            props.lang.text("LLM mining loop", "LLM 循环挖掘"),
            props
                .lang
                .text("Objects, links, metrics, actions", "挖掘对象、关系、指标、动作"),
            proposed,
        ),
        (
            "04",
            props.lang.text("Human review", "人工确认"),
            props
                .lang
                .text("Approve business meaning", "确认业务含义再批准"),
            reviewed,
        ),
        (
            "05",
            props.lang.text("Publish ontology", "发布本体"),
            props
                .lang
                .text("Materialize approved semantics", "写入已批准语义"),
            materialized,
        ),
        (
            "06",
            props.lang.text("Compile tools", "生成工具"),
            props
                .lang
                .text("Agent tools stay governed", "智能体工具仍受治理"),
            tool_ready,
        ),
    ];

    html! {
        <section class="semantic-journey" aria-label={props.lang.text("Ontology onboarding journey", "本体入职流程")}>
            { for steps.into_iter().map(|(number, title, detail, done)| html! {
                <article class={classes!("semantic-journey-step", done.then_some("done"))} key={number}>
                    <span>{ number }</span>
                    <strong>{ title }</strong>
                    <small>{ detail }</small>
                </article>
            }) }
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OnboardingPanelProps {
    lang: SemanticLang,
    run: Option<OntologyOnboardingRun>,
    tool_specs: Vec<OntologyOnboardingToolSpec>,
    review_graph: Option<OntologyReviewGraph>,
    calibration: Option<ConfidenceCalibrationResponse>,
    on_start: Callback<MouseEvent>,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
    on_materialize: Callback<MouseEvent>,
}

#[component]
fn OnboardingPanel(props: &OnboardingPanelProps) -> Html {
    let Some(run) = props.run.as_ref() else {
        return html! {
            <div class="ontology-onboarding empty-state">
                <div>
                    <strong>{ props.lang.text("No onboarding run yet.", "还没有本体入职运行。") }</strong>
                    <p>{ props.lang.text(
                        "Start the ecommerce sample to see how raw tables become reviewable business objects, relations, metrics, and actions.",
                        "启动电商示例，查看原始数据表如何变成可审核的业务对象、关系、指标和动作。"
                    ) }</p>
                </div>
                <button onclick={props.on_start.clone()}>{ props.lang.text("Start ecommerce sample", "开始电商示例") }</button>
            </div>
        };
    };
    html! {
        <div class="ontology-onboarding">
            <OntologyMindMapPanel
                lang={props.lang}
                graph={props.review_graph.clone()}
                on_approve={props.on_approve.clone()}
                on_reject={props.on_reject.clone()}
            />
            <OntologyRunSummary
                lang={props.lang}
                run={run.clone()}
                graph={props.review_graph.clone()}
                tool_count={props.tool_specs.len()}
                on_materialize={props.on_materialize.clone()}
            />
            <details class="ontology-advanced-review">
                <summary>{ props.lang.text("Advanced review queue and evidence", "高级审核队列与证据") }</summary>
                <div class="ontology-advanced-review-body">
                    <KeyMetrics values={vec![
                        (props.lang.text("Run", "运行").to_string(), short_id(&run.id)),
                        (props.lang.text("Status", "状态").to_string(), localized_status(props.lang, label_or(&run.status, "pending"))),
                        (props.lang.text("Source", "来源").to_string(), localized_source_mode(props.lang, &run.source_mode)),
                        (props.lang.text("Datasets", "数据集").to_string(), run.dataset_count.to_string()),
                        (props.lang.text("Profiles", "画像").to_string(), run.profile_count.to_string()),
                        (props.lang.text("Proposals", "提案").to_string(), run.proposal_count.to_string()),
                        (props.lang.text("Approved", "已批准").to_string(), run.approved_count.to_string()),
                        (props.lang.text("Materialized", "已发布").to_string(), run.materialized_count.to_string()),
                    ]} />
                    <OntologyIntelligenceReviewPanel
                        lang={props.lang}
                        graph={props.review_graph.clone()}
                        calibration={props.calibration.clone()}
                    />
                    <details class="ontology-proposal-details">
                        <summary>{ props.lang.text("Review proposal details", "审核提案明细") }</summary>
                        <div class="ontology-proposal-list">
                            { for ["object", "relation", "metric", "logic", "action"].iter().map(|proposal_type| html! {
                                <section class="ontology-proposal-group" key={proposal_type.to_string()}>
                                    <h4>{ localized_proposal_type(props.lang, proposal_type) }</h4>
                                    { for run.proposals.iter().filter(|proposal| proposal.proposal_type == *proposal_type).map(|proposal| {
                                        html! {
                                            <OnboardingProposalRow
                                                key={proposal.id.clone()}
                                                lang={props.lang}
                                                proposal={proposal.clone()}
                                                on_approve={props.on_approve.clone()}
                                                on_reject={props.on_reject.clone()}
                                            />
                                        }
                                    }) }
                                </section>
                            }) }
                        </div>
                    </details>
                    <div class="ontology-tool-specs">
                        <h4>{ props.lang.text("Compiled agent tools", "已生成智能体工具") }</h4>
                        <Rows empty={props.lang.text("No tool specs compiled.", "还没有生成工具。")} rows={props.tool_specs.iter().map(|spec| {
                            (
                                localized_status(props.lang, if spec.approval_required { "approval" } else { "ready" }),
                                spec.name.clone(),
                                format!(
                                    "{} / {} / {} / {}",
                                    spec.target_object,
                                    localized_risk(props.lang, label_or(&spec.read_write_risk, "risk_unset")),
                                    localized_status(props.lang, label_or(&spec.transaction_profile, "profile_unset")),
                                    localized_status(props.lang, label_or(&spec.execution_mode, "mode_unset"))
                                )
                            )
                        }).collect::<Vec<_>>()} />
                    </div>
                </div>
            </details>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyRunSummaryProps {
    lang: SemanticLang,
    run: OntologyOnboardingRun,
    graph: Option<OntologyReviewGraph>,
    tool_count: usize,
    on_materialize: Callback<MouseEvent>,
}

#[component]
fn OntologyRunSummary(props: &OntologyRunSummaryProps) -> Html {
    let node_count = props
        .graph
        .as_ref()
        .map(|graph| graph.nodes.len())
        .unwrap_or_default();
    let edge_count = props
        .graph
        .as_ref()
        .map(|graph| graph.edges.len())
        .unwrap_or_default();
    let pending_count = props
        .run
        .proposals
        .iter()
        .filter(|proposal| proposal.review_status != "approved" && proposal.review_status != "rejected")
        .count();
    let can_materialize = props.run.approved_count > props.run.materialized_count;
    let next_step = if props.run.proposal_count == 0 {
        props.lang.text(
            "Start from the graph. The system has not produced review proposals yet.",
            "先看图谱。系统还没有生成可审核提案。",
        )
    } else if props.run.approved_count == 0 {
        props.lang.text(
            "Select a node or edge, then approve the proposals that match the business logic.",
            "先点选节点或关系，再批准符合业务逻辑的提案。",
        )
    } else if can_materialize {
        props.lang.text(
            "Approved changes are ready to publish into the ontology.",
            "已批准的变更可以发布到本体层。",
        )
    } else {
        props.lang.text(
            "Published changes are ready for downstream agent use.",
            "已发布变更可供下游智能体使用。",
        )
    };

    html! {
        <section class="ontology-run-summary">
            <div class="ontology-run-summary-main">
                <span>{ props.lang.text("Current run", "当前运行") }</span>
                <strong>{ format!("{} · {}", short_id(&props.run.id), localized_source_mode(props.lang, &props.run.source_mode)) }</strong>
                <small>{ next_step }</small>
            </div>
            <div class="ontology-run-summary-stats" aria-label={props.lang.text("Ontology run summary", "本体运行摘要")}>
                <span>
                    <strong>{ format!("{} / {}", node_count, edge_count) }</strong>
                    <small>{ props.lang.text("nodes / links", "节点 / 关系") }</small>
                </span>
                <span>
                    <strong>{ format!("{} / {}", props.run.approved_count, props.run.proposal_count) }</strong>
                    <small>{ props.lang.text("approved / proposals", "已批准 / 提案") }</small>
                </span>
                <span>
                    <strong>{ props.tool_count }</strong>
                    <small>{ props.lang.text("agent-ready specs", "智能体规格") }</small>
                </span>
                <span>
                    <strong>{ pending_count }</strong>
                    <small>{ props.lang.text("pending review", "待审核") }</small>
                </span>
            </div>
            <div class="ontology-run-summary-action">
                <button onclick={props.on_materialize.clone()} disabled={!can_materialize}>
                    { props.lang.text("Publish approved changes", "发布已批准变更") }
                </button>
                <small>{ if can_materialize {
                    props.lang.text("Writes only reviewed ontology changes.", "只写入已审核的本体变更。")
                } else {
                    props.lang.text("Approve at least one proposal first.", "请先批准至少一个提案。")
                } }</small>
            </div>
        </section>
    }
}

fn localized_proposal_type(lang: SemanticLang, proposal_type: &str) -> String {
    if lang == SemanticLang::En {
        return proposal_type.to_ascii_uppercase();
    }
    match proposal_type {
        "object" => "业务对象",
        "relation" => "关系 Link",
        "metric" => "指标 Metric",
        "logic" | "logic_rule" => "规则 Logic",
        "action" => "动作 Action",
        other => other,
    }
    .to_string()
}

fn localized_node_type(lang: SemanticLang, node_type: &str) -> String {
    if lang == SemanticLang::En {
        return node_type.to_string();
    }
    match node_type {
        "dataset" => "资料表",
        "object" => "业务对象",
        "metric" => "指标",
        "logic" => "规则",
        "action" => "动作",
        "tool" => "工具",
        "subgraph" => "子图",
        "merge_candidate" => "合并候选",
        other => other,
    }
    .to_string()
}

fn localized_edge_type(lang: SemanticLang, edge_type: &str) -> String {
    if lang == SemanticLang::En {
        return edge_type.to_string();
    }
    match edge_type {
        "maps_to" => "映射为",
        "relates_to" => "业务关联",
        "uses_metric" => "计算指标",
        "depends_on" => "依赖",
        "validates" => "校验",
        "acts_on" => "作用于",
        "compiles_to" => "生成工具",
        "groups" => "归入子图",
        "merge_suggests" => "建议合并",
        other => other,
    }
    .to_string()
}

fn evidence_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| {
        if let Some(text) = value.as_str() {
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        } else if value.is_null() {
            None
        } else {
            Some(compact_json(value))
        }
    })
}

fn graph_node_label(graph: &OntologyReviewGraph, node_id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .map(|node| node.label.clone())
        .unwrap_or_else(|| node_id.to_string())
}

fn relation_detail(
    lang: SemanticLang,
    graph: &OntologyReviewGraph,
    edge: &OntologyReviewGraphEdge,
) -> (String, String, String) {
    let from = graph_node_label(graph, &edge.from);
    let to = graph_node_label(graph, &edge.to);
    let detail = evidence_string(&edge.evidence, "source_mapping")
        .or_else(|| evidence_string(&edge.evidence, "primary_key"))
        .or_else(|| evidence_string(&edge.evidence, "expression"))
        .unwrap_or_else(|| {
            format!(
                "{:.0}% / {}",
                edge.confidence * 100.0,
                localized_risk(lang, label_or(&edge.risk, "low"))
            )
        });
    (
        localized_edge_type(lang, &edge.edge_type),
        format!("{from} -> {to}"),
        detail,
    )
}

fn graph_nodes_of_type<'a>(
    graph: &'a OntologyReviewGraph,
    node_types: &[&str],
    limit: usize,
) -> Vec<&'a OntologyReviewGraphNode> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node_types
                .iter()
                .any(|node_type| *node_type == node.node_type)
        })
        .take(limit)
        .collect()
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyMindMapPanelProps {
    lang: SemanticLang,
    graph: Option<OntologyReviewGraph>,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
}

#[component]
fn OntologyMindMapPanel(props: &OntologyMindMapPanelProps) -> Html {
    let Some(graph) = props.graph.as_ref() else {
        return html! {
            <div class="ontology-mindmap empty-state">
                <div>
                    <strong>{ props.lang.text("No graph yet.", "还没有关系图。") }</strong>
                    <p>{ props.lang.text(
                        "Start an onboarding run to generate table mappings, object links, metrics, actions, and tool edges.",
                        "先启动入职运行，系统会生成资料表映射、对象关系、指标、动作和工具边。"
                    ) }</p>
                </div>
            </div>
        };
    };

    let datasets = graph_nodes_of_type(graph, &["dataset"], 8);
    let objects = graph_nodes_of_type(graph, &["object", "subgraph", "merge_candidate"], 10);
    let actions = graph_nodes_of_type(graph, &["metric", "logic", "action", "tool"], 10);
    let selected_node_id = use_state(|| {
        graph_focus_node(graph)
            .or_else(|| datasets.first().copied())
            .or_else(|| actions.first().copied())
            .map(|node| node.id.clone())
            .unwrap_or_default()
    });
    if !graph.nodes.iter().any(|node| node.id == *selected_node_id) {
        if let Some(node) = objects
            .first()
            .or_else(|| datasets.first())
            .or_else(|| actions.first())
        {
            selected_node_id.set(node.id.clone());
        }
    }
    let selected_node = graph
        .nodes
        .iter()
        .find(|node| node.id == *selected_node_id)
        .or_else(|| objects.first().copied())
        .or_else(|| datasets.first().copied())
        .or_else(|| actions.first().copied());
    let connected_edges = selected_node
        .map(|node| {
            graph
                .edges
                .iter()
                .filter(|edge| edge.from == node.id || edge.to == node.id)
                .take(8)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let relation_rows = graph
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.edge_type.as_str(),
                "maps_to" | "relates_to" | "acts_on" | "compiles_to" | "uses_metric" | "validates"
            )
        })
        .take(14)
        .map(|edge| relation_detail(props.lang, graph, edge))
        .collect::<Vec<_>>();

    html! {
        <section class="ontology-mindmap">
            <header class="ontology-mindmap-head">
                <div>
                    <span>{ props.lang.text("Ontology graph", "本体图谱") }</span>
                    <strong>{ props.lang.text("Business objects, links, metrics, actions, and tools", "业务对象、关系、指标、动作和工具") }</strong>
                </div>
                <small>{ format!("{} {} / {} {}", graph.nodes.len(), props.lang.text("nodes", "节点"), graph.edges.len(), props.lang.text("edges", "关系")) }</small>
            </header>
            <div class="ontology-graph-workbench">
                <OntologyNetworkGraph
                    lang={props.lang}
                    graph={graph.clone()}
                    selected_node_id={(*selected_node_id).clone()}
                    on_select={Callback::from({
                        let selected_node_id = selected_node_id.clone();
                        move |id: String| selected_node_id.set(id)
                    })}
                />
                <aside class="ontology-node-inspector">
                    <h4>{ props.lang.text("Selected node evidence", "选中节点证据") }</h4>
                    <OntologyNodeInspector
                        lang={props.lang}
                        graph={graph.clone()}
                        node={selected_node.cloned()}
                        connected_edges={connected_edges.into_iter().cloned().collect::<Vec<_>>()}
                        on_approve={props.on_approve.clone()}
                        on_reject={props.on_reject.clone()}
                    />
                </aside>
            </div>
            <details class="ontology-field-map">
                <summary>
                    <span>{ props.lang.text("Relationship evidence", "关系证据") }</span>
                    <small>{ props.lang.text("Open only when you need to inspect the exact source fields behind the graph.", "需要核对图谱背后的来源字段时再展开。") }</small>
                </summary>
                <Rows empty={props.lang.text("No relationship evidence yet.", "还没有关系证据。")} rows={relation_rows} />
            </details>
            {
                if graph.truncated {
                    html! {
                        <p class="ontology-map-note">
                            { format!("{} +{} {} / +{} {}", props.lang.text("Graph truncated:", "关系图已截断："), graph.omitted_node_count, props.lang.text("nodes", "节点"), graph.omitted_edge_count, props.lang.text("edges", "关系")) }
                        </p>
                    }
                } else {
                    html! {}
                }
            }
        </section>
    }
}

#[derive(Clone)]
struct PositionedOntologyNode {
    node: OntologyReviewGraphNode,
    x: f64,
    y: f64,
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyNetworkGraphProps {
    lang: SemanticLang,
    graph: OntologyReviewGraph,
    selected_node_id: String,
    on_select: Callback<String>,
}

#[component]
fn OntologyNetworkGraph(props: &OntologyNetworkGraphProps) -> Html {
    let nodes = positioned_ontology_nodes(&props.graph);
    let selected_node_id = props.selected_node_id.clone();
    html! {
        <section class="ontology-network-canvas" aria-label={props.lang.text("Ontology knowledge graph", "本体知识图谱")}>
            <svg class="ontology-network-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                <defs>
                    <marker id="ontology-network-arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto">
                        <path d="M0,0 L7,3.5 L0,7 Z"></path>
                    </marker>
                </defs>
                { for props.graph.edges.iter().filter_map(|edge| {
                    let from = nodes.iter().find(|node| node.node.id == edge.from)?;
                    let to = nodes.iter().find(|node| node.node.id == edge.to)?;
                    let selected = selected_node_id == edge.from || selected_node_id == edge.to;
                    Some(html! {
                        <line
                            class={classes!("ontology-network-edge", edge.edge_type.clone(), selected.then_some("selected"))}
                            x1={format!("{:.2}", from.x)}
                            y1={format!("{:.2}", from.y)}
                            x2={format!("{:.2}", to.x)}
                            y2={format!("{:.2}", to.y)}
                        />
                    })
                }) }
            </svg>
            { for nodes.iter().map(|positioned| {
                let node = positioned.node.clone();
                let selected = props.selected_node_id == node.id;
                html! {
                    <OntologyNetworkNode
                        key={node.id.clone()}
                        lang={props.lang}
                        node={node.clone()}
                        x={positioned.x}
                        y={positioned.y}
                        selected={selected}
                        on_select={props.on_select.clone()}
                    />
                }
            }) }
        </section>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyNetworkNodeProps {
    lang: SemanticLang,
    node: OntologyReviewGraphNode,
    x: f64,
    y: f64,
    selected: bool,
    on_select: Callback<String>,
}

#[component]
fn OntologyNetworkNode(props: &OntologyNetworkNodeProps) -> Html {
    let node_id = props.node.id.clone();
    let on_select = props.on_select.clone();
    html! {
        <button
            class={classes!(
                "ontology-network-node",
                props.node.node_type.clone(),
                status_tone(&props.node.status),
                props.selected.then_some("selected")
            )}
            style={format!("left: {:.2}%; top: {:.2}%;", props.x, props.y)}
            onclick={Callback::from(move |_| on_select.emit(node_id.clone()))}
        >
            <span>{ localized_node_type(props.lang, &props.node.node_type) }</span>
            <strong>{ props.node.label.clone() }</strong>
            <small>{ format!("{:.0}% / {}", props.node.confidence * 100.0, localized_risk(props.lang, label_or(&props.node.risk, "low"))) }</small>
        </button>
    }
}

fn positioned_ontology_nodes(graph: &OntologyReviewGraph) -> Vec<PositionedOntologyNode> {
    let mut positioned = Vec::new();
    let objects = graph_nodes_of_type(graph, &["object"], 8);
    let datasets = graph_nodes_of_type(graph, &["dataset"], 8);
    let metrics = graph_nodes_of_type(graph, &["metric"], 5);
    let actions = graph_nodes_of_type(graph, &["action"], 4);
    let focus_id = graph_focus_node(graph).map(|node| node.id.clone());

    if let Some(focus_node) = graph_focus_node(graph) {
        positioned.push(PositionedOntologyNode {
            node: focus_node.clone(),
            x: 50.0,
            y: 50.0,
        });
    }

    push_positioned_orbit(
        &mut positioned,
        objects
            .into_iter()
            .filter(|node| Some(&node.id) != focus_id.as_ref())
            .collect(),
        50.0,
        50.0,
        23.0,
        28.0,
        -135.0,
        190.0,
    );
    push_positioned_orbit(&mut positioned, datasets, 50.0, 50.0, 41.0, 37.0, 132.0, 228.0);
    push_positioned_orbit(&mut positioned, metrics, 50.0, 50.0, 39.0, 35.0, -48.0, 42.0);
    push_positioned_orbit(&mut positioned, actions, 50.0, 50.0, 39.0, 35.0, 52.0, 82.0);
    positioned
}

fn graph_focus_node(graph: &OntologyReviewGraph) -> Option<&OntologyReviewGraphNode> {
    graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "object")
        .max_by_key(|node| {
            let preferred = match node.label.as_str() {
                "Order" | "Customer" | "Product" => 10_000,
                _ => 0,
            };
            preferred + graph_node_degree(graph, &node.id)
        })
}

fn graph_node_degree(graph: &OntologyReviewGraph, node_id: &str) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| edge.from == node_id || edge.to == node_id)
        .count()
}

fn push_positioned_orbit(
    positioned: &mut Vec<PositionedOntologyNode>,
    nodes: Vec<&OntologyReviewGraphNode>,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    start_degrees: f64,
    end_degrees: f64,
) {
    let count = nodes.len();
    if count == 0 {
        return;
    }
    for (index, node) in nodes.into_iter().enumerate() {
        let angle = if count == 1 {
            (start_degrees + end_degrees) / 2.0
        } else {
            start_degrees + ((end_degrees - start_degrees) * index as f64 / (count - 1) as f64)
        };
        let radians = angle.to_radians();
        let x = cx + rx * radians.cos();
        let y = cy + ry * radians.sin();
        positioned.push(PositionedOntologyNode {
            node: node.clone(),
            x,
            y,
        });
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyNodeInspectorProps {
    lang: SemanticLang,
    graph: OntologyReviewGraph,
    node: Option<OntologyReviewGraphNode>,
    connected_edges: Vec<OntologyReviewGraphEdge>,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
}

#[component]
fn OntologyNodeInspector(props: &OntologyNodeInspectorProps) -> Html {
    let Some(node) = props.node.as_ref() else {
        return html! { <p class="empty">{ props.lang.text("Select a node to inspect evidence.", "选择一个节点查看证据。") }</p> };
    };
    let evidence_rows = ["source_system", "source_object", "source_mapping", "target_object", "primary_key", "expression", "executor"]
        .iter()
        .filter_map(|key| evidence_string(&node.evidence, key).map(|value| ((*key).to_string(), value)))
        .collect::<Vec<_>>();
    let source_proposal_id = node.source_proposal_id.clone();
    let approve = {
        let source_proposal_id = source_proposal_id.clone();
        let on_approve = props.on_approve.clone();
        Callback::from(move |_| {
            if let Some(id) = source_proposal_id.clone() {
                on_approve.emit(id);
            }
        })
    };
    let reject = {
        let source_proposal_id = source_proposal_id.clone();
        let on_reject = props.on_reject.clone();
        Callback::from(move |_| {
            if let Some(id) = source_proposal_id.clone() {
                on_reject.emit(id);
            }
        })
    };
    let review_done = node.status == "approved" || node.status == "rejected";
    let can_review = source_proposal_id.is_some() && !review_done;
    html! {
        <div class="ontology-node-detail">
            <div class="ontology-node-detail-head">
                <span>{ localized_node_type(props.lang, &node.node_type) }</span>
                <strong>{ node.label.clone() }</strong>
                <small>{ format!("{}% / {}", (node.confidence * 100.0).round(), localized_status(props.lang, label_or(&node.status, "pending"))) }</small>
            </div>
            <div class="ontology-node-review-actions">
                <button onclick={approve} disabled={!can_review}>
                    { props.lang.text("Approve proposal", "批准此提案") }
                </button>
                <button class="secondary" onclick={reject} disabled={!can_review}>
                    { props.lang.text("Reject", "拒绝") }
                </button>
                <small>{ if source_proposal_id.is_some() {
                    if review_done {
                        props.lang.text("This graph proposal has already been reviewed.", "这个图谱提案已经审核过。")
                    } else {
                        props.lang.text("Approve from the graph when the selected business meaning is correct.", "当选中节点的业务含义正确时，可直接在图谱上批准。")
                    }
                } else {
                    props.lang.text("This node is context only and has no direct proposal to approve.", "此节点只是上下文，没有可直接批准的提案。")
                } }</small>
            </div>
            <div class="ontology-node-evidence">
                { for evidence_rows.iter().map(|(key, value)| html! {
                    <div class="ontology-evidence-row" key={key.clone()}>
                        <span>{ key }</span>
                        <strong>{ value }</strong>
                    </div>
                }) }
                { if evidence_rows.is_empty() {
                    html! { <p class="empty">{ props.lang.text("No compact evidence fields; raw proposal evidence remains in review details.", "没有可摘要证据；原始提案证据保留在审核明细中。") }</p> }
                } else {
                    html! {}
                }}
            </div>
            <div class="ontology-connected-edges">
                <h5>{ props.lang.text("Connected links", "相连关系") }</h5>
                <Rows empty={props.lang.text("No connected links.", "没有相连关系。")} rows={props.connected_edges.iter().map(|edge| {
                    relation_detail(props.lang, &props.graph, edge)
                }).collect::<Vec<_>>()} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct OntologyIntelligenceReviewPanelProps {
    lang: SemanticLang,
    graph: Option<OntologyReviewGraph>,
    calibration: Option<ConfidenceCalibrationResponse>,
}

#[component]
fn OntologyIntelligenceReviewPanel(props: &OntologyIntelligenceReviewPanelProps) -> Html {
    let merge_rows = props
        .graph
        .as_ref()
        .map(|graph| ontology_merge_rows(props.lang, graph))
        .unwrap_or_default();
    let low_confidence_rows = props
        .graph
        .as_ref()
        .map(|graph| ontology_low_confidence_rows(props.lang, graph))
        .unwrap_or_default();
    let taxonomy_rows = props
        .graph
        .as_ref()
        .map(|graph| ontology_taxonomy_rows(props.lang, graph))
        .unwrap_or_default();
    let transaction_rows = props
        .graph
        .as_ref()
        .map(|graph| ontology_transaction_rows(props.lang, graph))
        .unwrap_or_default();
    let calibration_rows = props
        .calibration
        .as_ref()
        .map(|calibration| ontology_calibration_rows(props.lang, calibration))
        .unwrap_or_default();

    html! {
        <div class="ontology-intelligence-review">
            <div class="ontology-intelligence-review-head">
                <strong>{ props.lang.text("Evidence review", "证据审核") }</strong>
                <span>{ format!("{} {}", props.calibration.as_ref().map(|calibration| calibration.record_count).unwrap_or_default(), props.lang.text("calibration records", "条校准记录")) }</span>
                <span>{ props.calibration.as_ref().and_then(|calibration| calibration.threshold_policy.get("configuration_surface")).and_then(|surface| surface.get("scope")).and_then(|scope| scope.as_str()).unwrap_or("customer_or_domain_policy") }</span>
            </div>
            <div class="ontology-intelligence-grid">
                <section>
                    <h4>{ props.lang.text("Merge", "合并建议") }</h4>
                    <Rows empty={props.lang.text("No merge warnings.", "没有合并风险。")} rows={merge_rows} />
                </section>
                <section>
                    <h4>{ props.lang.text("Low Confidence", "低置信度") }</h4>
                    <Rows empty={props.lang.text("No low-confidence items.", "没有低置信度项目。")} rows={low_confidence_rows} />
                </section>
                <section>
                    <h4>{ props.lang.text("Taxonomy", "层级结构") }</h4>
                    <Rows empty={props.lang.text("No taxonomy warnings.", "没有层级风险。")} rows={taxonomy_rows} />
                </section>
                <section>
                    <h4>{ props.lang.text("Transactions", "业务动作") }</h4>
                    <Rows empty={props.lang.text("No transaction warnings.", "没有动作风险。")} rows={transaction_rows} />
                </section>
                <section class="ontology-intelligence-wide">
                    <h4>{ props.lang.text("Calibration", "置信度校准") }</h4>
                    <Rows empty={props.lang.text("No calibration records.", "没有校准记录。")} rows={calibration_rows} />
                </section>
            </div>
        </div>
    }
}

fn ontology_merge_rows(
    lang: SemanticLang,
    graph: &OntologyReviewGraph,
) -> Vec<(String, String, String)> {
    graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "merge_candidate")
        .take(6)
        .map(|node| {
            (
                localized_risk(lang, label_or(&node.risk, "merge")),
                node.label.clone(),
                format!(
                    "{} / {:.0}% / {}",
                    localized_status(lang, label_or(&node.status, "pending")),
                    node.confidence * 100.0,
                    node.source_proposal_id
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| lang.text("source", "来源").to_string())
                ),
            )
        })
        .collect()
}

fn ontology_low_confidence_rows(
    lang: SemanticLang,
    graph: &OntologyReviewGraph,
) -> Vec<(String, String, String)> {
    graph
        .nodes
        .iter()
        .filter(|node| node.confidence < 0.90 || node.risk == "needs_review")
        .take(6)
        .map(|node| {
            (
                localized_node_type(lang, &node.node_type),
                node.label.clone(),
                format!(
                    "{:.0}% / {}",
                    node.confidence * 100.0,
                    localized_risk(lang, label_or(&node.risk, "low"))
                ),
            )
        })
        .collect()
}

fn ontology_taxonomy_rows(
    lang: SemanticLang,
    graph: &OntologyReviewGraph,
) -> Vec<(String, String, String)> {
    let object_count = graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "object")
        .count();
    let logic_count = graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "logic")
        .count();
    let mut rows = Vec::new();
    if object_count >= 8 {
        rows.push((
            lang.text("object_scope", "对象范围").to_string(),
            format!(
                "{object_count} {}",
                lang.text("object candidates", "个对象候选")
            ),
            lang.text(
                "review granularity before publishing",
                "发布前确认颗粒度是否过细",
            )
            .to_string(),
        ));
    }
    if logic_count > 0 {
        rows.push((
            lang.text("identity_rules", "识别规则").to_string(),
            format!(
                "{logic_count} {}",
                lang.text("disabled logic rules", "条待启用规则")
            ),
            lang.text("publish only after owner review", "业务负责人确认后才发布")
                .to_string(),
        ));
    }
    rows
}

fn ontology_transaction_rows(
    lang: SemanticLang,
    graph: &OntologyReviewGraph,
) -> Vec<(String, String, String)> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            (node.node_type == "action" || node.node_type == "tool")
                && (node.risk == "approval_required"
                    || node
                        .evidence
                        .get("transaction_profile")
                        .and_then(|value| value.as_str())
                        == Some("proposal_only"))
        })
        .take(6)
        .map(|node| {
            let transaction_profile = node
                .evidence
                .get("transaction_profile")
                .and_then(|value| value.as_str())
                .unwrap_or("profile_unset");
            let execution_mode = node
                .evidence
                .get("execution_mode")
                .and_then(|value| value.as_str())
                .unwrap_or(if transaction_profile == "proposal_only" {
                    "proposal_only"
                } else {
                    "mode_unset"
                });
            (
                localized_risk(lang, label_or(&node.risk, "approval")),
                node.label.clone(),
                format!(
                    "{} / {}",
                    localized_status(lang, transaction_profile),
                    localized_status(lang, execution_mode)
                ),
            )
        })
        .collect()
}

fn ontology_calibration_rows(
    lang: SemanticLang,
    calibration: &ConfidenceCalibrationResponse,
) -> Vec<(String, String, String)> {
    calibration
        .buckets
        .iter()
        .take(6)
        .map(|bucket| {
            (
                localized_status(lang, &bucket.reviewer_status),
                format!("{} ({})", localized_proposal_type(lang, &bucket.proposal_type), bucket.count),
                format!(
                    "{} {:.0}% / {} {:.0}% / {} {:.0}%",
                    lang.text("model", "模型"),
                    bucket.average_model_confidence * 100.0,
                    lang.text("validator", "验证"),
                    bucket.average_validator_score * 100.0,
                    lang.text("source", "来源"),
                    bucket.average_source_quality_score * 100.0
                ),
            )
        })
        .collect()
}

#[derive(Properties, Clone, PartialEq)]
struct OnboardingProposalRowProps {
    lang: SemanticLang,
    proposal: OntologyOnboardingProposal,
    on_approve: Callback<String>,
    on_reject: Callback<String>,
}

#[component]
fn OnboardingProposalRow(props: &OnboardingProposalRowProps) -> Html {
    let approve = {
        let id = props.proposal.id.clone();
        let on_approve = props.on_approve.clone();
        Callback::from(move |_| on_approve.emit(id.clone()))
    };
    let reject = {
        let id = props.proposal.id.clone();
        let on_reject = props.on_reject.clone();
        Callback::from(move |_| on_reject.emit(id.clone()))
    };
    let evidence_chips = proposal_evidence_chips(props.lang, &props.proposal);
    html! {
        <article class="ontology-proposal-row">
            <div class="ontology-proposal-head">
                <div>
                    <span>{ format!("{} / {:.0}%", localized_status(props.lang, &props.proposal.review_status), props.proposal.confidence * 100.0) }</span>
                    <strong>{ props.proposal.name.clone() }</strong>
                    <small>{ props.proposal.source_mapping.clone() }</small>
                </div>
                <div class="ontology-onboarding-actions">
                    <button onclick={approve} disabled={props.proposal.review_status == "approved"}>{ props.lang.text("Approve", "批准") }</button>
                    <button onclick={reject} disabled={props.proposal.review_status == "rejected"}>{ props.lang.text("Reject", "拒绝") }</button>
                </div>
            </div>
            <div class="ontology-proposal-evidence-chips">
                { for evidence_chips.iter().map(|(label, value)| html! {
                    <span class="ontology-proposal-chip" key={format!("{label}:{value}")}>
                        <small>{ label }</small>
                        <strong>{ value }</strong>
                    </span>
                }) }
            </div>
            <details class="ontology-proposal-json">
                <summary>{ props.lang.text("Evidence JSON", "证据 JSON") }</summary>
                <JsonPreview value={props.proposal.evidence.clone()} />
            </details>
        </article>
    }
}

fn proposal_evidence_chips(
    lang: SemanticLang,
    proposal: &OntologyOnboardingProposal,
) -> Vec<(String, String)> {
    let mut chips = Vec::new();
    let keys = match proposal.proposal_type.as_str() {
        "object" => [
            "table",
            "seed_ontology_match",
            "primary_key_candidates",
            "row_count",
            "pii_candidates",
            "time_dimensions",
        ]
        .as_slice(),
        "relation" => [
            "source_table",
            "source_field",
            "references_table",
            "references_field",
            "join_success_rate",
            "seed_relation_match",
        ]
        .as_slice(),
        "metric" => [
            "target_object",
            "expression",
            "semantic_model",
            "definition_evidence",
            "domain_scope",
            "tool_namespace",
        ]
        .as_slice(),
        "logic" => [
            "target_object",
            "source_table",
            "primary_key",
            "enum_candidates",
            "pii_candidates",
            "field_null_rates",
        ]
        .as_slice(),
        "action" => [
            "target_object",
            "execution_mode",
            "transaction_profile",
            "approval_required",
            "effect_count",
            "contract_source",
        ]
        .as_slice(),
        _ => ["domain_scope", "industry", "tool_namespace", "source_mode"].as_slice(),
    };

    for key in keys {
        let Some(value) = evidence_string(&proposal.evidence, key) else {
            continue;
        };
        chips.push((
            localized_evidence_key(lang, key).to_string(),
            truncate_text(&value, 80),
        ));
    }

    if chips.is_empty() {
        chips.push((
            lang.text("Recommendation", "建议").to_string(),
            localized_status(lang, label_or(&proposal.recommendation, "review")),
        ));
    }

    chips.into_iter().take(6).collect()
}

fn localized_evidence_key<'a>(lang: SemanticLang, key: &'a str) -> &'a str {
    if lang == SemanticLang::En {
        return key;
    }
    match key {
        "approval_required" => "审批",
        "contract_source" => "动作来源",
        "definition_evidence" => "定义证据",
        "domain_scope" => "领域",
        "effect_count" => "影响数",
        "enum_candidates" => "枚举候选",
        "execution_mode" => "执行模式",
        "expression" => "指标公式",
        "field_null_rates" => "空值率",
        "industry" => "行业",
        "join_success_rate" => "关联成功率",
        "pii_candidates" => "PII 候选",
        "primary_key" => "主键",
        "primary_key_candidates" => "主键候选",
        "references_field" => "目标字段",
        "references_table" => "目标表",
        "row_count" => "样本行",
        "seed_ontology_match" => "种子对象",
        "seed_relation_match" => "种子关系",
        "semantic_model" => "语义模型",
        "source_field" => "来源字段",
        "source_mode" => "来源模式",
        "source_table" => "来源表",
        "table" => "资料表",
        "target_object" => "目标对象",
        "time_dimensions" => "时间维度",
        "tool_namespace" => "工具命名空间",
        "transaction_profile" => "事务策略",
        other => other,
    }
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[derive(Properties, Clone, PartialEq)]
struct SemanticMapProps {
    snapshot: SemanticGraphSnapshot,
    objects: Vec<SemanticObject>,
}

#[component]
fn SemanticMap(props: &SemanticMapProps) -> Html {
    let total = props.objects.len().max(1);
    html! {
        <div class="semantic-map">
            <div class="semantic-canvas">
                <span class="semantic-core">{ props.snapshot.node_count }</span>
                { for props.objects.iter().take(16).enumerate().map(|(index, object)| {
                    let (x, y) = orbit_point(index, total, 50.0, 50.0, 42.0);
                    html! {
                        <b
                            class={classes!("semantic-node", status_tone(&object.status))}
                            title={format!("{} / {}", label_or(&object.title, &object.object_key), object.object_type)}
                            style={position_style(x, y)}
                        />
                    }
                }) }
            </div>
            <div class="semantic-stats">
                <FlowMeter label="Objects" value={props.snapshot.node_count} max={props.snapshot.node_count.max(1)} tone="info" />
                <FlowMeter label="Links" value={props.snapshot.edge_count} max={props.snapshot.edge_count.max(props.snapshot.node_count).max(1)} tone="good" />
                <FlowMeter label="Conflicts" value={props.snapshot.conflicts.len()} max={props.snapshot.conflicts.len().max(1)} tone={if props.snapshot.conflicts.is_empty() { "good" } else { "warn" }} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct RenderedContextPreviewProps {
    lang: SemanticLang,
    rendered: Option<RenderedExecutionContext>,
}

#[component]
fn RenderedContextPreview(props: &RenderedContextPreviewProps) -> Html {
    let Some(rendered) = props.rendered.as_ref() else {
        return html! { <p class="empty">{ props.lang.text("No execution context rendered.", "还没有渲染执行上下文。") }</p> };
    };
    let omissions = &rendered.omitted;
    let budget = &rendered.budget;
    html! {
        <div class="context-preview">
            <div class="context-budget">
                <FlowMeter
                    label={props.lang.text("Prompt tokens", "提示词 token")}
                    value={budget.estimated_tokens_used}
                    max={budget.max_prompt_tokens.max(1)}
                    tone={if budget.estimated_tokens_used > budget.max_prompt_tokens { "warn" } else { "good" }}
                />
                <FlowMeter
                    label={props.lang.text("Objects", "对象")}
                    value={rendered.relevant_objects.len()}
                    max={budget.max_objects.max(1)}
                    tone="info"
                />
                <FlowMeter
                    label={props.lang.text("Fetchable", "可拉取")}
                    value={rendered.fetchable_object_ids.len()}
                    max={rendered.fetchable_object_ids.len().max(rendered.relevant_objects.len()).max(1)}
                    tone="neutral"
                />
            </div>
            <KeyMetrics values={vec![
                (props.lang.text("Packet", "上下文包").to_string(), short_id(&rendered.context_packet_id)),
                (props.lang.text("Role", "角色").to_string(), if props.lang == SemanticLang::Zh && rendered.role == "agent" { "智能体".to_string() } else { label_or(&rendered.role, "agent").to_string() }),
                (props.lang.text("Version", "版本").to_string(), rendered.context_packet_version.to_string()),
                (props.lang.text("Full content", "完整内容").to_string(), rendered.full_content_included.to_string()),
                (props.lang.text("Tools", "工具").to_string(), rendered.available_tools.join(", ")),
                (props.lang.text("Omitted", "省略").to_string(), if props.lang == SemanticLang::Zh {
                    format!(
                        "{} 预算 / {} 对象上限 / {} 来源引用 / {} 正文",
                        omissions.token_budget_exceeded,
                        omissions.object_limit_exceeded,
                        omissions.source_refs_not_rendered,
                        omissions.full_content_not_rendered
                    )
                } else {
                    format!(
                        "{} budget / {} object cap / {} source refs / {} content",
                        omissions.token_budget_exceeded,
                        omissions.object_limit_exceeded,
                        omissions.source_refs_not_rendered,
                        omissions.full_content_not_rendered
                    )
                }),
            ]} />
            <Rows empty={props.lang.text("No policy reminders.", "没有策略提醒。")} rows={rendered.must_follow.iter().take(6).map(|item| {
                ("must_follow".to_string(), item.clone(), "rendered reminder".to_string())
            }).collect::<Vec<_>>()} />
            <div class="context-objects">
                { for rendered.relevant_objects.iter().map(|object| html! {
                    <article class="context-object" key={object.id.clone()}>
                        <div>
                            <span>{ format!("{} / {}", label_or(&object.object_type, "semantic_object"), short_id(&object.id)) }</span>
                            <strong>{ label_or(&object.title, &object.object_key) }</strong>
                        </div>
                        <p>{ label_or(&object.summary, "No summary.") }</p>
                        <small>{ format!("{} / {}", label_or(&object.trust_level, "trust"), label_or(&object.freshness, "freshness")) }</small>
                    </article>
                }) }
            </div>
            <JsonPreview value={rendered.ontology_scope.clone()} />
        </div>
    }
}
