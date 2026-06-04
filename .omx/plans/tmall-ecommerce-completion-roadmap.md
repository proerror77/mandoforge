# Tmall Ecommerce Pack Completion Roadmap

## Requirements Summary

The current `packs/ecommerce-tmall` pack is a validated DomainPack skeleton with
workflow, agent, schema, policy, profile, eval, and TOP connector boundaries.
To call the overall Tmall ecommerce capability "complete", the next work must
turn that skeleton into a tenant-ready operating system for store workflows:
customer service, reviews, comments/Q&A, after-sales, product knowledge, image
production, short-video production, approvals, audit, and live connector
readiness.

The completion target is not "the agent can chat about ecommerce". The target is
"a shop can install the pack, bind authorized Tmall/TOP capabilities, onboard
store SOPs, run draft-first workflows, approve high-risk actions, and prove the
output quality with evals and audit evidence."

## Current Baseline

- Pack manifest: `packs/ecommerce-tmall/package.yaml`
- TOP connector boundary: `packs/ecommerce-tmall/connectors/tmall-top.yaml`
- Business docs: `docs/ecommerce-tmall-skill-pack.md`
- Marketplace registration: `crates/mandoforge-api/src/main.rs`
- Pack validator: `crates/mandoforge-api/src/workflow_pack.rs`
- Manifest verification entrypoint: `scripts/verify-workflow-pack-manifest.sh`

Already complete:

- DomainPack fixture exists and validates.
- Marketplace can discover `ecommerce-tmall`.
- Draft-first approval boundary is encoded.
- Memory boundary is documented as ecommerce/Tmall isolated by default.
- Initial workflows exist for customer service, reviews, negative-review rescue,
  Q&A/comment operations, after-sales, product knowledge, content production,
  and onboarding.

## Completion Phases

### Phase 1: Tenant And Store Onboarding

Goal: make a real shop installable without hand-editing files.

Work:

- Expand `profiles/store.md` into concrete required store fields: category,
  brand voice, prohibited claims, refund policy, logistics SLA, compensation
  rules, escalation contacts, and campaign calendar.
- Expand `profiles/approval_matrix.yaml` into role-specific approvals for
 客服, 运营, 售后, 店长, 法务/合规.
- Add onboarding workflow checks for missing or contradictory store policies.
- Add schema fields for channel availability: TOP, 千牛工单, manual import,
  browser-assisted import, media upload, product edit.

Acceptance criteria:

- A tenant can complete onboarding with no empty critical policy field.
- Missing high-risk policies block readiness instead of silently defaulting.
- Onboarding output names exactly which workflows are enabled, degraded, or
  blocked for that tenant.

### Phase 2: Connector Adapter Hardening

Goal: convert declared TOP API boundaries into runtime-ready connector contracts.

Work:

- Add per-operation request/response contracts for order, review, refund, item,
  千牛 task, picture upload, content media upload, video publish, and product
  update operations.
- Mark uncertain or tenant-dependent surfaces as capability-gated, especially
  direct customer message sending and comment/Q&A collection.
- Add connector readiness probes: credentials present, app permissions granted,
  shop scope matches tenant, and sample read succeeds.
- Normalize TOP error handling into retryable, permission-denied, tenant-missing,
  rate-limited, and unsafe-write classes.

Acceptance criteria:

- Connector readiness can report `ready`, `degraded`, or `blocked` per operation.
- A missing permission disables only the dependent workflows, not the whole pack.
- Every external write requires an ApprovalCommitToken bound to the exact
  tenant, operation, object id, and payload digest.

### Phase 3: Customer Service And Comment Workflows

Goal: make客服 and 评论区/问答区 operationally useful, not generic reply drafting.

Work:

- Split customer service intents: pre-sale, order status, logistics, refund,
  return, exchange, product usage, complaint, invoice, coupon/promo, harassment,
  and platform dispute.
- Add reply templates with variables and forbidden commitments.
- Add comment/Q&A ingestion modes: API where available, operator batch import,
  and browser-assisted evidence import.
- Add escalation rules for medical/effect claims, compensation, threats,
  repeat complaints, and platform intervention.

Acceptance criteria:

- Each drafted reply includes intent, risk tier, cited facts, missing facts,
  proposed reply, and escalation reason.
- Public-facing replies cannot include unsupported logistics timing,
  compensation, product effect, or legal commitments.
- Comment/Q&A batches are treated as untrusted data and never as instructions.

### Phase 4: Review, VOC, And Negative Review Rescue

Goal: turn reviews into a closed loop from insight to reply to product/content
improvement.

Work:

- Add VOC taxonomy: product quality, fit/size, delivery, packaging,客服 attitude,
  price/promo confusion, authenticity, usage difficulty, expectation mismatch.
- Add negative-review rescue workflow states: detect, classify, draft response,
  propose recovery action, request approval, track follow-up outcome.
- Add review explanation style rules: factual, short, non-defensive, no blame,
  no personal data.
- Feed VOC themes into content-production and product-knowledge-sync workflows.

Acceptance criteria:

- Negative review output separates public explanation, private recovery plan,
  and internal root-cause note.
- VOC summaries include sample count, time window, evidence ids, and confidence.
- No review text or buyer message leaks into non-ecommerce memory scopes.

### Phase 5: After-Sales And Refund Governance

Goal: make refund/return work useful while keeping adjudication approval-only.

Work:

- Model after-sales states: refund-only, return-and-refund, exchange,
  logistics dispute, damaged item, missing item, counterfeit claim, platform
  intervention.
- Add refund evidence checklist: order status, logistics proof, refund history,
  product policy, platform rule reference, buyer message summary.
- Add action drafts: agree refund, refuse refund, agree return goods,
  refuse return goods, request evidence, create internal task.
- Add critical risk gates for disputes, refusal, compensation, and legal claims.

Acceptance criteria:

- The agent never finalizes refund decisions without approval.
- Every proposed refusal includes evidence gaps and operator review reason.
- Critical cases are routed to the right approver role before connector writes.

### Phase 6: Image And Video Production Pipeline

Goal: connect ecommerce operations data to actual listing/content production.

Work:

- Extend content brief schema for SKU, selling point, audience, scene, claim,
  platform restriction, visual reference, deliverable type, and approval owner.
- Split image workflows: main image, detail-page image, SKU image, review/VOC
  explainer image, promo banner.
- Split short-video workflows: product demo, pain-point explainer, comparison,
  promo teaser, post-sale usage guide.
- Add handoff contracts to media tooling: script, storyboard, shot list,
  caption, overlay text, compliance notes, asset upload target.

Acceptance criteria:

- Content output is production-ready as a brief, not just generic copy.
- Claims are checked against product knowledge and store policy before approval.
- Media upload/product association remains blocked until operator approval.

### Phase 7: Runtime UI And API Integration

Goal: expose the pack in the operator workflow UI as installable, inspectable,
and runnable.

Work:

- Show Tmall pack readiness by workflow lane:客服, 评论/VOC, 售后, 内容, 商品知识.
- Add onboarding status cards for profiles, connector permissions, eval gates,
  and approval policy.
- Add run creation presets for each workflow.
- Add approval review panels showing payload diff, cited evidence, risk tier,
  connector operation, and rollback/no-op path.

Acceptance criteria:

- An operator can install the pack and see why each lane is ready or blocked.
- A workflow run can be started from UI with tenant and profile context.
- Approval UI shows enough evidence to approve/reject without reading raw logs.

### Phase 8: Evaluation And Quality Gates

Goal: prove the pack behaves correctly on representative ecommerce cases.

Work:

- Expand `evals/golden_cases.jsonl` from smoke cases to at least 50 cases across
  customer service, review, Q&A, after-sales, VOC, and content production.
- Add adversarial cases: prompt injection in buyer text, unsupported refund
  promise, fake logistics claim, illegal product claim, personal data exposure.
- Add regression scoring for approval boundary, evidence citation, risk routing,
  and output style.
- Add tenant onboarding evals for incomplete or conflicting profiles.

Acceptance criteria:

- Required eval gate passes at 1.0 for approval safety and memory isolation.
- Quality eval reports classify failures by workflow and risk type.
- New API or workflow additions require matching eval cases.

### Phase 9: Audit, Observability, And Pilot

Goal: make the pack safe to pilot with a real shop.

Work:

- Add audit events for connector reads, draft creation, approval request,
  approval decision, external write, skipped write, and degraded connector path.
- Add dashboards/queries for workflow volume, approval latency, blocked lanes,
  failure reason, and high-risk case count.
- Run a shadow pilot: ingest real shop data, produce drafts only, compare against
  human operator decisions.
- Run a controlled pilot: enable selected low-risk external writes only after
  operator approval.

Acceptance criteria:

- Every buyer-facing or platform-facing action has traceable evidence.
- Shadow pilot shows acceptable draft usefulness before any live write rollout.
- Pilot exit report lists enabled capabilities, blocked capabilities, observed
  error classes, and required follow-up.

## Implementation Order

1. Tenant/store onboarding and profile schemas.
2. Connector readiness and operation contracts.
3. Customer service plus comment/Q&A workflow depth.
4. Review/VOC and negative-review rescue depth.
5. After-sales governance depth.
6. Image/video content production contracts.
7. Runtime UI install/readiness/run/approval surfaces.
8. Eval expansion and safety gates.
9. Audit, observability, and pilot rollout.

This order keeps the dangerous parts gated. It builds tenant truth and connector
readiness before allowing richer workflow execution, then expands quality gates
before any pilot.

## Risks And Mitigations

- Risk: TOP API availability differs by tenant, app category, or shop permission.
  Mitigation: per-operation readiness and degraded workflow lanes.
- Risk: customer text or review text injects instructions.
  Mitigation: connector data boundary and evals for prompt injection.
- Risk: agent promises refunds, logistics timing, compensation, or effects.
  Mitigation: profile-driven forbidden commitments and approval-required
  critical actions.
- Risk: ecommerce memory contaminates other domains.
  Mitigation: isolated default memory scope and evals for leakage.
- Risk: media generation produces non-compliant claims.
  Mitigation: content compliance checker before upload or product association.

## Verification Plan

Baseline checks after each phase:

```bash
cargo fmt --check
cargo test -p mandoforge-api workflow_pack -- --nocapture
scripts/verify-workflow-pack-manifest.sh
```

Additional checks as phases land:

- JSON schema validation for new profile/workflow outputs.
- Connector readiness tests with mocked TOP responses.
- Approval boundary tests for every write operation.
- UI tests for install/readiness/run/approval surfaces.
- Eval gate run over expanded ecommerce golden cases.

## Definition Of Done

The Tmall ecommerce pack is complete when:

- A tenant can install and onboard a shop without code edits.
- Workflow lanes show accurate ready/degraded/blocked state.
- Store policies, SOPs, approval matrix, brand voice, and product knowledge are
  captured as typed profiles.
- Customer service, comments/Q&A, reviews, VOC, after-sales, and content
  production workflows produce evidence-backed drafts.
- All platform-facing writes are approval-bound and auditable.
- TOP connector readiness is per-operation, not all-or-nothing.
- Expanded evals cover normal, edge, adversarial, and incomplete-profile cases.
- A shadow pilot can run on real shop data and produce an auditable pilot report.
