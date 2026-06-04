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
  - `customer-service-conversation`
  - `review-voc-and-reply`
  - `negative-review-rescue`
  - `question-answer-ops`
  - `after-sales-triage`
  - `product-knowledge-sync`
  - `content-production`
  - `profile-onboarding`

## API Boundary

The `tmall-top` connector declares the Alibaba TOP/Tmall boundary as a native
connector requirement. The pack does not embed credentials. A tenant must bind
store-scoped `TMALL_TOP_APP_KEY`, `TMALL_TOP_APP_SECRET`, and
`TMALL_TOP_SESSION` secrets before live connector quality can pass.

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

Question-answer and comment-area signals are supported as a workflow lane even
when a tenant does not have a stable API source for every signal. In that case,
the workflow accepts operator-imported batches or future browser-assisted
connector evidence, but still treats the input as untrusted data.

## Memory Boundary

Product knowledge and VOC writebacks must remain scoped to:

- `domain_scope=ecommerce`
- `workflow_scope=tmall`
- `share_policy=isolated`

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
