# Tmall Ecommerce Skill Pack

`packs/ecommerce-tmall` is the first Tmall-focused DomainPack for MandoForge.
It is designed around store operations touchpoints, not generic ecommerce AI
features.

## Operating Model

The pack is organized around four operational lanes:

1. Customer service conversations.
2. Reviews, follow-up reviews, comments, and negative-review rescue.
3. After-sales, refunds, returns, logistics, and platform-dispute triage.
4. Product knowledge, VOC-to-content, image briefs, and short-video scripts.

The default posture is draft-first. Buyer-facing messages, review explanations,
refund decisions, QianNiu work orders, media uploads, and product media
associations require approval before the executor agent can call the native
connector.

Customer-service replies are intentionally modeled as approved drafts plus
QianNiu work-order/operator handoff. The pack reads order, refund, review, item,
and work-order context through TOP APIs, but it does not claim a universal
buyer-IM send API; tenants can bind that as an additional connector only when
their shop has an approved channel.

## Pack Contents

- Manifest: `packs/ecommerce-tmall/package.yaml`
- Native connector: `packs/ecommerce-tmall/connectors/tmall-top.yaml`
- Agents:
  - `connector-reader`
  - `touchpoint-analyzer`
  - `response-writer`
  - `content-producer`
  - `tmall-executor`
- Workflows:
  - `connector-readiness`
  - `customer-service-conversation`
  - `review-voc-and-reply`
  - `negative-review-rescue`
  - `question-answer-ops`
  - `after-sales-triage`
  - `product-knowledge-sync`
  - `content-production`
  - `profile-onboarding`
  - `pilot-readiness`

## API Boundary

The `tmall-top` connector declares the Alibaba TOP/Tmall boundary as a native
connector requirement. The pack does not embed credentials. A tenant must bind
store-scoped `TMALL_TOP_APP_KEY`, `TMALL_TOP_APP_SECRET`, and
`TMALL_TOP_SESSION` secrets before live connector quality can pass.

The pack separates account binding from operation contracts. A tenant fills the
`connector-account` profile with tenant/workspace/shop ids, seller nick, TOP
auth mode, secret reference names, probe inputs, and rotation policy. Raw secret
values remain in the runtime secret store and are not embedded in the pack.
`connector-map` maps each workflow lane to required read operations, optional
read operations, and controlled write operations.

Connector readiness is operation-level, not all-or-nothing. A tenant can be
`ready`, `degraded`, or `blocked` per lane depending on credential binding, shop
scope, app permission, and sample reads. For example, missing media upload
permission degrades content production to brief-only, while missing refund read
permission blocks after-sales triage.

Declared read operations include:

- `taobao.trades.sold.get` for order/trade context.
- `tmall.traderate.feeds.get` for reviews, follow-up reviews, and semantic
  review labels.
- `taobao.refunds.receive.get` and `taobao.refund.get` for refund and
  after-sales context.
- `taobao.item.seller.get` for product, item, and SKU context.
- `taobao.qianniu.tasks.get` for QianNiu work-order/todo context.

Declared write operations include:

- `taobao.traderate.explain.add` for approved review explanations.
- `taobao.rp.refunds.agree`, `taobao.refund.refuse`,
  `taobao.rp.returngoods.agree`, and `taobao.rp.returngoods.refuse` for
  approved refund or return actions.
- `taobao.qianniu.task.create` for approved work-order creation.
- `taobao.picture.upload`, `taobao.content.media.upload.secret`,
  `taobao.content.media.upload.pub`, and `taobao.content.video.publishx` for
  approved media upload/publish flows.
- `alibaba.item.edit.fastupdate` for approved product-media association or
  product image updates when the tenant has that permission.

Each declared operation in `connectors/tmall-top.yaml` now includes an operation
id, request contract, response contract, evidence id, and permission boundary.
Write operations additionally require approval and are bound to
`tenant_id`, `workspace_id`, `connector_id`, `api_name`, `operation_id`,
`object_id`, `payload_digest`, and `approval_commit_token`.

Question-answer and comment-area signals are supported as a workflow lane even
when a tenant does not have a stable API source for every signal. In that case,
the workflow accepts operator-imported batches or future browser-assisted
connector evidence, but still treats the input as untrusted data.

## Memory Boundary

Product knowledge and VOC writebacks must remain scoped to:

- `domain_scope=ecommerce`
- `workflow_scope=tmall`
- `share_policy=isolated`

The pack manifest and each workflow declare these semantic scopes explicitly.
Each workflow also adds a `lane_scope` such as `customer-service`,
`review-voc`, `after-sales`, `content-production`, `product-knowledge`, or
`pilot-readiness` so Context OS packets can retrieve lane-specific objects
without sharing raw buyer data across unrelated domains.

The `ontology-seed` profile declares initial Tmall object and relation types for
store, connector account, connector operation, order, refund case, review,
product, Q&A question, content asset, and approval commit semantics.

The pack must not share raw buyer messages, review text, refund records, or
customer-service history with unrelated Legal, Social Media, or other domain
memory. Cross-domain reuse should happen only through approved tenant-common
summaries.

## Approval Boundary

High-risk or critical actions include:

- Sending customer-service messages.
- Submitting review explanations.
- Agreeing to or refusing refunds.
- Creating QianNiu work orders.
- Uploading product images or videos.
- Associating media with a product listing.
- Promising compensation, logistics timing, or product effects.

Those actions are routed through `tmall-executor`, whose manifest grants
`native.connector.call` only as an external write scope. MandoForge must still
enforce TaskGrant scope and ApprovalCommitToken exact binding at runtime.

## Onboarding Boundary

The `profile-onboarding` workflow must produce typed tenant profiles before
release:

- Store profile with shop identity, brand voice, customer-service policy,
  after-sales policy, logistics policy, review policy, content policy, channel
  capabilities, campaign calendar, and escalation owners.
- Approval matrix with workflow owners and approval commit-token bind fields.
- Connector map with readiness probes and degraded-lane behavior.
- Connector account profile with tenant/workspace/shop binding, seller nick,
  TOP auth mode, secret reference names, session rotation policy, and readiness
  probe inputs.
- Ontology seed with tenant-approved object types, relation types, retrieval
  defaults, and prompt boundary.
- Risk policy and output style.

The onboarding readiness output reports each lane as `ready`, `degraded`, or
`blocked`. Missing owners or missing required read lanes block release; missing
optional write/media/comment capabilities degrade only the affected workflow.
Connector readiness must map operation status to lane impact before release.

## Customer-Service And VOC Boundary

Customer-service workflows use `service-playbook` to classify pre-sale, order,
logistics, refund, return, exchange, usage, complaint, invoice, promotion,
harassment, and platform-dispute intents. Every reply draft must list cited
facts, missing facts, forbidden-commitment checks, risk tier, approval need, and
provenance.

Q&A and comment-area workflows use `question-answer-policy`. API reads,
operator-import batches, and browser-assisted captures are allowed input modes,
but all question/comment text is untrusted data. Public answers must cite
product facts and identify feedback targets such as FAQ, detail page,
customer-service playbook, or content production.

Review workflows use `review-voc-taxonomy`. VOC output must include sentiment,
themes, sample window, sample count, confidence, feedback targets, and
provenance. Negative-review rescue separates public merchant explanation,
private recovery plan, and internal root-cause note before approval.

## After-Sales Boundary

After-sales workflows use `after-sales-playbook` to model refund-only,
return-and-refund, exchange, logistics dispute, damaged item, missing item,
counterfeit/legal claim, and platform-intervention states. Every output must
include an evidence checklist, evidence gaps, policy basis, buyer-facing safe
draft, operator review reason, risk tier, approval requirement, and provenance.

Refund refusal, platform intervention, compensation commitment, counterfeit or
legal claims, and policy exceptions are critical gates. The pack can draft
recommended actions such as agree/refuse refund, agree/refuse return goods,
request buyer evidence, or create an internal task, but connector writes remain
blocked until approval binds the exact operation and payload digest.

## Content Production Boundary

Content workflows use `content-production-policy` to produce production-ready
briefs for main images, detail-page images, SKU images, review/VOC explainer
images, promo banners, product demo videos, pain-point explainers, comparison
videos, promo teasers, and post-sale usage guides.

Every content brief must include SKU, audience, scene, selling point, claims,
evidence sources, platform restrictions, required assets, script/storyboard or
visual handoff assets, compliance checks, approval owner, and upload target.
Media upload, product-media association, product-claim changes, and promotion
price/discount claims require approval.

## Audit And Pilot Boundary

The pack includes an observability contract in
`packs/ecommerce-tmall/profiles/observability.yaml`. Buyer-facing and
platform-facing work must emit traceable audit events for connector reads, draft
creation, approval requests, approval decisions, external writes, skipped
writes, and degraded connector paths. The required dashboards are workflow
volume, approval latency, blocked lanes, failure reasons, high-risk case count,
external write traceability, shadow-pilot quality, and degraded connector
impact.

Pilot rollout is governed by `packs/ecommerce-tmall/profiles/pilot_policy.yaml`
and `packs/ecommerce-tmall/policies/pilot_rollout.yaml`.

- Shadow pilot mode disables all external writes. It compares agent drafts
  against human operator decisions across customer service, review/VOC,
  comments/Q&A, after-sales, product knowledge, and content production.
- Controlled pilot mode is approval-only and initially allows only QianNiu task
  creation and review explanations. Refund actions, media upload, video publish,
  and product edits stay blocked until a later production policy explicitly
  enables them.
- Stop conditions include unsupported refund or compensation promises, buyer
  private-data leaks, approval-token mismatch, product claims without evidence,
  cross-domain memory writes, and external writes without trace.

The `pilot-readiness` workflow produces a typed pilot exit report using
`packs/ecommerce-tmall/schemas/pilot_report.schema.json`. The report must list
enabled capabilities, blocked capabilities, degraded connector paths, observed
error classes, human comparison summary, unsafe output review, approval latency,
and follow-up requirements before any rollout promotion.

## Verification

Run:

```bash
cargo test -p mandoforge-api workflow_pack -- --nocapture
cargo test -p mandoforge-api ontology_proposal_review_and_pack_wizard_are_operator_ready -- --nocapture
scripts/verify-workflow-pack-manifest.sh
```

The dedicated validator test is:

```bash
cargo test -p mandoforge-api validates_ecommerce_tmall_domain_pack_fixture -- --nocapture
```

The ecommerce regression gate uses `packs/ecommerce-tmall/evals/golden_cases.jsonl`
and `packs/ecommerce-tmall/policies/eval_quality_gate.yaml`. The golden set must
cover at least 55 representative cases across customer service, Q&A/comment
operations, review VOC, negative-review rescue, after-sales, content production,
connector readiness, onboarding, prompt-injection resistance, approval safety,
evidence citation, memory isolation, content compliance, audit traceability, and
pilot rollout safety.

The pilot gate additionally requires the observability and pilot-rollout
contracts to be present in `packs/ecommerce-tmall/package.yaml` release gates.
