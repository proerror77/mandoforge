use crate::components::{KeyMetrics, PackCardModel, PackMosaic, Panel, Rows, StatusLogo};
use crate::state::{ConsoleData, UiLang};
use crate::{
    json_array_len, label_or, pack_blocker_summary, pack_card_models, pack_connector_rows,
    pack_has_external_writes, pack_lifecycle_steps, pack_metric_count, pack_requires_approval,
    pack_string_list, semantic_scope_summary, status_tone,
};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct PacksProps {
    pub(crate) data: ConsoleData,
    pub(crate) lang: UiLang,
}

#[component]
pub(crate) fn PacksView(props: &PacksProps) -> Html {
    let cards = pack_card_models(
        &props.data.workflow_pack_installations.data,
        &props.data.workflow_pack_marketplace.data,
    );
    let discovery = &props.data.capability_discovery.data;
    let lang = props.lang;
    html! {
        <div class="page-stack">
            <section class="page-purpose">
                <p class="eyebrow">{ lang.text("Capabilities / 能力包", "能力包 / Capabilities") }</p>
                <h2>{ lang.text("Package connectors, templates, actions, and governance boundaries for managed agents.", "把行业连接器、模板、动作和治理边界打包给托管智能体使用。") }</h2>
                <p>{ lang.text(
                    "Capabilities are not a separate business-process page. They declare which connectors, semantic objects, actions, and approval boundaries managed agents can use.",
                    "能力包不是一个独立业务流程页面。它声明 Managed Agents 可以调用什么连接器、读取什么语义对象、触发哪些动作，以及哪些动作必须先进入审批。"
                ) }</p>
            </section>
            <section class="pack-product-grid">
                { for cards.into_iter().map(|card| html! { <PackProductCard card={card} lang={lang} /> }) }
            </section>
            <div class="page-grid">
                <Panel title={lang.text("Capability Map", "能力地图")}>
                    <PackMosaic
                        installations={props.data.workflow_pack_installations.data.clone()}
                        marketplace={props.data.workflow_pack_marketplace.data.clone()}
                    />
                </Panel>
            <Panel title={lang.text("Installed Packs", "已安装能力包")}>
                <Rows empty={lang.text("No pack installations.", "还没有安装能力包。")} rows={props.data.workflow_pack_installations.data.iter().take(12).map(|pack| {
                    (
                        pack.status.clone(),
                        label_or(&pack.pack_id, "pack").to_string(),
                        format!("{} / {} / {}", label_or(&pack.kind, "kind"), label_or(&pack.version, "version"), semantic_scope_summary(&pack.manifest["semantic_scopes"]))
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Available Packs", "可用能力包")}>
                <KeyMetrics values={vec![
                    (lang.text("Status", "状态").to_string(), props.data.workflow_pack_marketplace.data.status.clone()),
                    (lang.text("Packs", "能力包").to_string(), props.data.workflow_pack_marketplace.data.packs.len().to_string()),
                    (lang.text("Bindings", "绑定").to_string(), "via /api/workflow-packs/installations/{id}/bindings".to_string()),
                    (lang.text("Runtime objects", "运行时对象").to_string(), "via /api/workflow-packs/installations/{id}/runtime-objects".to_string()),
                ]} />
                <Rows empty={lang.text("No available packs.", "没有可用能力包。")} rows={props.data.workflow_pack_marketplace.data.packs.iter().map(|pack| {
                    (
                        pack.status.clone(),
                        label_or(&pack.name, &pack.id).to_string(),
                        format!("{} / {} / {}", label_or(&pack.kind, "kind"), label_or(&pack.version, "version"), pack.description.clone())
                    )
                }).collect::<Vec<_>>()} />
            </Panel>
            <Panel title={lang.text("Agent Capability Discovery", "能力发现")}>
                <h3>{ lang.text("Agent cards", "能力卡片") }</h3>
                <div class="rows">
                    { for discovery.agent_cards.iter().map(|card| {
                        html! {
                            <article class="row" key={card.agent_id.clone()}>
                                <StatusLogo status={card.release_state.clone()} />
                                <div>
                                    <strong>{ format!("{} / {}", label_or(&card.name, "agent"), label_or(&card.kind, "kind")) }</strong>
                                    <span>{ format!("{} / {} / {}", label_or(&card.provider, "provider"), label_or(&card.model, "model"), label_or(&card.primary_action, "primary action")) }</span>
                                </div>
                                <small>{ label_or(&card.agent_role, "role") }</small>
                            </article>
                        }
                    }) }
                </div>
                <h3>{ lang.text("Suggested prompts", "推荐提示") }</h3>
                <Rows empty={lang.text("No suggested prompts.", "没有推荐提示。")} rows={discovery.suggested_prompts.iter().map(|prompt| {
                    (label_or(&prompt.target_view, "view").to_string(), label_or(&prompt.title, "title").to_string(), label_or(&prompt.prompt, "prompt").to_string())
                }).collect::<Vec<_>>()} />
                <h3>{ lang.text("Onboarding steps", "引导步骤") }</h3>
                <Rows empty={lang.text("No onboarding steps.", "没有引导步骤。")} rows={discovery.onboarding_steps.iter().map(|step| {
                    (label_or(&step.key, "step").to_string(), label_or(&step.title, "title").to_string(), label_or(&step.description, "description").to_string())
                }).collect::<Vec<_>>()} />
                <h3>{ lang.text("Empty states", "空态") }</h3>
                <Rows empty={lang.text("No empty states.", "没有空态建议。")} rows={discovery.empty_states.iter().map(|state| {
                    (label_or(&state.view, "view").to_string(), label_or(&state.title, "title").to_string(), label_or(&state.action, "action").to_string())
                }).collect::<Vec<_>>()} />
            </Panel>
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PackProductCardProps {
    card: PackCardModel,
    lang: UiLang,
}

#[component]
fn PackProductCard(props: &PackProductCardProps) -> Html {
    let card = &props.card;
    let workflow_count = pack_metric_count(&card.manifest, "workflows", "workflow_count");
    let agent_count = pack_metric_count(&card.manifest, "agents", "agent_count");
    let connector_count = pack_metric_count(&card.manifest, "connectors", "connector_count");
    let action_count = pack_metric_count(&card.manifest, "actions", "action_count");
    let release_gate_count =
        pack_metric_count(&card.manifest, "release_gates", "required_eval_gate_count");
    let validated_file_count =
        pack_metric_count(&card.manifest, "validated_files", "validated_file_count");
    let external_writes = pack_has_external_writes(&card.manifest);
    let approval_required = pack_requires_approval(&card.manifest);
    let safe_action = if external_writes {
        props.lang.text(
            "Draft + approval before external write",
            "外部写入前先草稿并审批",
        )
    } else {
        props
            .lang
            .text("Read and draft surfaces only", "只开放读取和草稿界面")
    };
    html! {
        <article class={classes!("pack-product-card", status_tone(&card.status))} key={card.id.clone()}>
            <header class="pack-product-header">
                <StatusLogo status={card.status.clone()} />
                <div>
                    <span>{ format!("{} / {}", label_or(&card.kind, "WorkflowPack"), label_or(&card.version, "version")) }</span>
                    <strong>{ label_or(&card.name, &card.id) }</strong>
                </div>
                <em>{ label_or(&card.status, &card.source) }</em>
            </header>
            <p>{ label_or(&card.description, props.lang.text("No pack description reported.", "没有能力包说明。")) }</p>
            <div class="pack-lifecycle" aria-label="WorkflowPack lifecycle">
                { for pack_lifecycle_steps(&card.status, &card.source).into_iter().map(|(label, tone)| html! {
                    <span class={classes!("pack-lifecycle-step", tone)}>{ label }</span>
                }) }
            </div>
            <div class="pack-card-metrics">
                <PackMetric label={props.lang.text("Workflows", "流程")} value={workflow_count} />
                <PackMetric label={props.lang.text("Agents", "智能体")} value={agent_count} />
                <PackMetric label={props.lang.text("Connectors", "连接器")} value={connector_count} />
                <PackMetric label={props.lang.text("Actions", "动作")} value={action_count} />
                <PackMetric label={props.lang.text("Gates", "闸门")} value={release_gate_count} />
                <PackMetric label={props.lang.text("Files", "文件")} value={validated_file_count} />
            </div>
            <div class="pack-capabilities">
                { for pack_string_list(&card.manifest, "capabilities", 6).into_iter().map(|capability| html! {
                    <span>{ capability }</span>
                }) }
                { if json_array_len(&card.manifest["capabilities"]) == 0 {
                    html! { <span>{ props.lang.text("No capabilities declared", "没有声明能力") }</span> }
                } else {
                    html! {}
                }}
            </div>
            <div class="pack-connector-list">
                { for pack_connector_rows(&card.manifest).into_iter().map(|row| html! {
                    <div class="pack-connector-row" key={row.0.clone()}>
                        <strong>{ row.0 }</strong>
                        <span>{ row.1 }</span>
                    </div>
                }) }
            </div>
            <div class="pack-gate-strip">
                <span>{ if props.lang == UiLang::En { format!("{release_gate_count} release gates") } else { format!("{release_gate_count} 个发布闸门") } }</span>
                <span>{ if approval_required { props.lang.text("approval required", "需要审批") } else { props.lang.text("approval not declared", "未声明审批") } }</span>
                <span>{ safe_action }</span>
            </div>
            <div class="pack-card-footer">
                <small>{ pack_blocker_summary(card, external_writes, release_gate_count) }</small>
                <b>{ semantic_scope_summary(&card.manifest["semantic_scopes"]) }</b>
            </div>
        </article>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct PackMetricProps {
    label: &'static str,
    value: usize,
}

#[component]
fn PackMetric(props: &PackMetricProps) -> Html {
    html! {
        <div class="pack-metric">
            <span>{ props.label }</span>
            <strong>{ props.value }</strong>
        </div>
    }
}
