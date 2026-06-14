# Ontology Builder Intelligence Engine Implementation Plan

## Requirements Summary

Implement the next phase described in
`docs/superpowers/specs/2026-06-14-ontology-builder-intelligence-engine-design.md`.

The current Ontology Builder has a governed review/control plane. This plan
adds the first intelligence layer while preserving proposal-first governance:

```text
SchemaUnderstandingCandidate
  -> SubgraphProposalDraft
  -> EntityResolutionCandidate
  -> ConfidenceCalibrationRecord
  -> OntologyProposalDraft
  -> OntologyReviewGraph
```

This is not a rewrite of the semantic store. The implementation must reuse the
existing `semantic_sources`, `semantic_objects`, `semantic_links`,
`context_packets`, review, approval, and audit primitives.

## Acceptance Criteria

- Existing ontology onboarding tests continue to pass.
- The new intelligence spec and plan are checked into the repo.
- New code, when added, does not bypass human review or ontology registry
  validation.
- All generated intelligence outputs are persisted as proposals or calibration
  records, not published ontology state.
- Write-like action proposals remain approval-gated and must declare a
  transaction profile.
- The Semantic UI can surface duplicate/merge, low-confidence, taxonomy, and
  transaction-profile warnings through the review graph.

## Phase 1: Architecture And Contract Documentation

Files:

- `docs/superpowers/specs/2026-06-14-ontology-builder-intelligence-engine-design.md`
- `docs/superpowers/plans/2026-06-14-ontology-builder-intelligence-engine-implementation.md`

Work:

- Document the adopted research claims:
  - end-to-end subgraph modeling
  - GeTT-style schema understanding from samples
  - LLM plus embedding hybrid scoring
  - Graphiti-style entity resolution
  - Palantir-style Object/Link/Action boundaries
  - DeepOnto as alignment, not raw ontology construction
- Document rejected assumptions:
  - Action Type atomicity is not assumed
  - immediate write propagation is not assumed
  - free-form LLM relation names are not published without registry validation
- Map the intelligence engine to existing MandoForge semantic primitives.

Verification:

- `rg -n "SubgraphProposalEngine|EntityResolutionCandidate|transaction profile" docs/superpowers`
- `git diff --check`

## Phase 2: Schema Understanding Contracts

Files:

- `crates/mandoforge-api/src/main.rs`
- optional later extraction:
  `crates/mandoforge-api/src/ontology_builder_intelligence.rs`

Work:

- Add typed structs:
  - `SchemaUnderstandingCandidate`
  - `PropertyUnderstandingCandidate`
  - `TaxonomyLayerCandidate`
  - `SchemaUnderstandingRequest`
  - `SchemaUnderstandingResponse`
- Compile candidates from existing dataset snapshots and profiles.
- Add deterministic evidence fields:
  - profile score
  - seed ontology match
  - primary key evidence
  - PII/currency/time markers
  - sample row refs
- Keep any LLM output stubbed or deterministic in this slice.

API:

- `POST /api/ontology/intelligence/schema-understanding`

Tests:

- ecommerce `orders` produces an `Order` candidate with field evidence.
- insurance run produces insurance-specific candidates and preserves
  `source_mode`.
- low evidence candidates are marked `needs_review`.

## Phase 3: Subgraph Proposal Drafts

Files:

- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`
- `web-ui/src/views/semantic.rs`

Work:

- Add `SubgraphProposalDraft` that groups object, relation, metric, logic, and
  action proposals into a bounded business subgraph.
- Link subgraph drafts into the existing `OntologyReviewGraph`.
- Ensure subgraph drafts are reviewable units, not materialized state.

API:

- `POST /api/ontology/intelligence/subgraph-proposals`

Tests:

- ecommerce order subgraph includes `Customer`, `Order`, `OrderLine`, `SKU`,
  `Customer places Order`, `Order contains OrderLine`, `GMV`, and
  `refund_order`.
- generated subgraph nodes point back to proposal IDs.
- rejecting a subgraph leaves all child proposals unmaterialized.

## Phase 4: Entity Resolution Proposal Queue

Files:

- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`
- `web-ui/src/views/semantic.rs`

Work:

- Add typed structs:
  - `EntityResolutionCandidate`
  - `EntityResolutionRetrievalHit`
  - `EntityResolutionDecisionDraft`
- Implement deterministic first-pass retrieval over existing semantic objects:
  - exact normalized name match
  - token overlap
  - object type compatibility
  - optional future vector/BM25 hooks
- Emit duplicate/merge suggestions into review graph edges.
- Require human review for risky merges.

API:

- `POST /api/ontology/intelligence/entity-resolution`

Tests:

- `Customer` and `Client` can produce a merge suggestion when evidence is high.
- incompatible object types do not auto-merge.
- risky duplicate decisions block materialization until reviewed.

## Phase 5: Confidence Calibration Records

Files:

- `crates/mandoforge-api/src/store_semantic_kernel.rs`
- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`
- `web-ui/src/views/semantic.rs`

Work:

- Add semantic object type `ontology_confidence_calibration`.
- Record:
  - model confidence
  - deterministic validator score
  - retrieval similarity score
  - source quality score
  - reviewer decision
  - optional runtime outcome
- Expose calibration summary per run and proposal type.

API:

- `GET /api/ontology/intelligence/runs/{id}/calibration`

Tests:

- approve/reject proposal review appends calibration evidence.
- calibration summary reports counts by proposal type and outcome.
- thresholds remain configurable and are not hardcoded as universal truth.

## Phase 6: Action Transaction Profiles

Files:

- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`
- `web-ui/src/views/semantic.rs`

Work:

- Add action transaction profile enum:
  - `proposal_only`
  - `local_serializable`
  - `event_sourced`
  - `saga`
- Require action proposals to declare one profile.
- Keep cross-system writes as `proposal_only` until a transaction profile,
  compensation policy, and approval gate are configured.
- Surface transaction profile in tool specs and review graph risk badges.

Tests:

- `commerce.refund_order` remains approval-gated and declares a transaction
  profile.
- cross-system action without transaction profile cannot compile as executable.
- read-only tools are not incorrectly marked as write approval required.

## Phase 7: Semantic UI Intelligence Review

Files:

- `web-ui/src/api.rs`
- `web-ui/src/main.rs`
- `web-ui/src/views/semantic.rs`
- `web-ui/src/styles.css`
- `web/` generated Trunk assets

Work:

- Extend the review graph panel with:
  - duplicate/merge warning rows
  - low-confidence queue
  - taxonomy over-fragmentation warnings
  - action transaction profile badges
  - calibration evidence summary
- Keep the UI dense and operator-oriented.
- Do not add a drag/drop ontology editor in this slice.

Verification:

- `cargo check --manifest-path web-ui/Cargo.toml`
- `rm -rf web-ui/target`
- `(cd web-ui && env -u NO_COLOR trunk build --release)`
- Chrome DevTools smoke on `http://127.0.0.1:8787/#semantic`

## Phase 8: End-To-End Gate

Files:

- `scripts/verify-enterprise-ontology-fast-onboarding.sh`
- optional new script:
  `scripts/verify-ontology-builder-intelligence-engine.sh`

Work:

- Extend or add a gate that proves:
  - schema understanding candidates are generated
  - subgraph proposal drafts are generated
  - entity resolution suggestions are reviewable
  - calibration records are captured
  - action transaction profiles are present
  - review graph exposes intelligence warnings

Verification:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture
cargo check --manifest-path crates/mandoforge-api/Cargo.toml
cargo check --manifest-path web-ui/Cargo.toml
rm -rf web-ui/target
(cd web-ui && env -u NO_COLOR trunk build --release)
./scripts/verify-enterprise-ontology-fast-onboarding.sh
git diff --check
```

## Implementation Order

1. Documentation and contracts.
2. Deterministic schema understanding contracts.
3. Subgraph proposal draft grouping.
4. Entity resolution proposal queue.
5. Confidence calibration records.
6. Action transaction profiles.
7. UI intelligence review.
8. Verification gate.

Do not start with live LLM calls. The first implementation must be deterministic
and testable, with LLM integration added behind the same typed contracts later.
