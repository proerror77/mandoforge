# Ontology Builder IR Implementation Plan

## Requirements Summary

Implement the next ontology builder slice from
`docs/superpowers/specs/2026-06-14-ontology-builder-ir-design.md`.

The goal is to turn the current demo-oriented onboarding flow into a reusable,
reviewable Ontology Builder pipeline:

```text
Pipeline Mapping v2 / Simple LLM Extraction v1
  -> OntologyBuilderDag
  -> DatasetSnapshot / CuratedDatasetDraft
  -> OntologyPromptPacket
  -> OntologyProposalDraft
  -> OntologyReviewGraph
  -> human review
  -> semantic materialization
  -> tool specs
```

The visual review requirement is first-class: operators must be able to inspect
a run-scoped graph of datasets, business objects, relations, metrics, logic
rules, actions, and compiled tools before approving or materializing ontology
changes.

## Acceptance Criteria

- `cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture`
  passes.
- `cargo check --manifest-path crates/mandoforge-api/Cargo.toml` passes.
- `cargo check --manifest-path web-ui/Cargo.toml` passes after UI changes.
- `trunk build --release` passes from `web-ui` after UI changes.
- Existing fast-onboarding behavior remains compatible.
- Non-ecommerce runs rehydrate their original industry/source metadata after
  list/get.
- The API exposes a structured `OntologyPromptPacket`.
- The API exposes an `OntologyBuilderDag` with topological execution levels and
  cycle rejection.
- The API exposes a bounded `OntologyReviewGraph` with node and edge status,
  confidence, evidence, risk, and source proposal IDs.
- The Semantic UI includes a review graph panel that lets users inspect whether
  business relationships, logic, and actions are attached to the correct
  objects before approval.
- Write-like action tools remain approval-gated.

## Implementation Steps

### 1. Extract Ontology Builder IR Types

Files:

- `crates/mandoforge-api/src/main.rs`
- optional new module: `crates/mandoforge-api/src/ontology_builder.rs`

Work:

- Extract or introduce typed structs for:
  `OntologyBuilderRun`, `OntologyBuilderDag`, `OntologyBuilderNode`,
  `OntologyBuilderEdge`, `OntologyBuilderExecutionLevel`,
  `DatasetSnapshot`, `DatasetFieldSnapshot`, `DatasetProfile`,
  `DatasetQualityReport`, `CuratedDatasetDraft`, `OntologyPromptPacket`,
  `OntologyReviewGraph`, `OntologyReviewGraphNode`,
  `OntologyReviewGraphEdge`, `OntologyProposalDraft`, and
  `CompiledToolSpec`.
- Keep existing API responses compatible by bridging old
  `OntologyOnboarding*` structs to the new IR during the first slice.

Verification:

- Existing ontology onboarding tests still pass.

### 2. Add DAG Validation And Execution Levels

Files:

- `crates/mandoforge-api/src/main.rs`
- optional new module: `crates/mandoforge-api/src/ontology_builder.rs`

Work:

- Build default DAG definitions for:
  - `pipeline_mapping_v2`
  - `simple_llm_extraction_v1`
- Implement cycle detection using a topological sort.
- Compute execution levels so same-level nodes can be shown as parallelizable.
- Compute affected downstream subgraphs for future rerun support.
- Store or reconstruct DAG status in the run response.

Tests:

- cyclic graph returns a validation error before execution.
- independent nodes appear in the same execution level.
- changed upstream node marks only downstream nodes stale.

### 3. Fix Run Metadata Rehydration

Files:

- `crates/mandoforge-api/src/main.rs`

Work:

- Stop hardcoding `ontology_demo_source_bundle()` in run reconstruction.
- Persist industry, source mode, seed metadata, and dataset snapshot metadata in
  proposal content or a run envelope semantic object.
- Make ecommerce and insurance runs list/get with their original metadata.

Tests:

- create insurance demo run, list/get it, assert `source_mode=demo_insurance`
  and insurance seed counts are preserved.

### 4. Compile OntologyPromptPacket

Files:

- `crates/mandoforge-api/src/main.rs`

API:

- `GET /api/ontology/onboarding/runs/{id}/prompt-packet`

Work:

- Compile seed objects, relations, metrics, logic/action patterns, curated
  dataset snapshots, profiles, allowed ontology triples, evidence rules, and
  policy reminders into structured JSON.
- Keep sample rows bounded.

Tests:

- prompt packet includes seed ontology, dataset evidence, relation triples,
  evidence rules, and action/PII policy reminders.

### 5. Add CuratedDatasetDraft Review Boundary

Files:

- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`
- `web-ui/src/views/semantic.rs`

API:

- `POST /api/ontology/onboarding/curated-datasets/{id}/review`

Work:

- Add curated dataset drafts with quality score, issues, schema version, review
  status, and reviewer metadata.
- Use curated review status to influence proposal recommendation:
  approved datasets can generate approve recommendations; rejected or low
  quality datasets produce `needs_more_evidence`.

Tests:

- low-quality or rejected curated dataset causes proposals to require more
  evidence.

### 6. Add Logic Rule Proposals

Files:

- `crates/mandoforge-api/src/main.rs`

Work:

- Generate `logic_rule` proposals from:
  - non-null/uniqueness evidence
  - enum/state fields
  - PII fields
  - mapping constraints
  - relation evidence
- Materialize approved logic rules as `semantic_objects` with
  `object_type=ontology_logic_rule`.
- Keep logic disabled until a later publish policy enables it.

Tests:

- demo run generates at least one validation or mapping logic rule.
- approved logic proposal materializes idempotently.

### 7. Add OntologyReviewGraph Projection API

Files:

- `crates/mandoforge-api/src/main.rs`
- `web-ui/src/api.rs`

API:

- `GET /api/ontology/onboarding/runs/{id}/review-graph`

Work:

- Project the current run into bounded graph JSON.
- Include node types:
  `dataset`, `object`, `metric`, `logic`, `action`, `tool`.
- Include edge types:
  `maps_to`, `relates_to`, `depends_on`, `validates`, `acts_on`,
  `compiles_to`, `uses_metric`.
- Include review status, materialization status, confidence, risk tone,
  evidence, source refs, and source proposal ID.
- Cap graph size deterministically and return truncation metadata.

Tests:

- review graph includes expected node and edge types for the ecommerce demo.
- graph nodes point back to proposal IDs.
- truncation metadata is present when the cap is exceeded.

### 8. Update Tool Spec Metadata

Files:

- `crates/mandoforge-api/src/main.rs`

Work:

- Add read/write risk, approval requirement, target object, and source proposal
  refs to compiled tool specs.
- Keep write-like tools approval-gated.

Tests:

- `commerce.refund_order` remains approval-gated and has audit metadata.

### 9. Add Semantic UI Review Graph Panel

Files:

- `web-ui/src/api.rs`
- `web-ui/src/main.rs`
- `web-ui/src/views/semantic.rs`
- `web-ui/src/styles.css`

Work:

- Add API structs for `OntologyReviewGraph`, nodes, edges, and truncation
  metadata.
- Fetch review graph after run creation, proposal review, and materialization.
- Add an `Ontology review graph` panel inside the Semantic view.
- Render a dense, operator-oriented graph:
  - nodes grouped by type
  - edges shown as relationship rows or simple SVG/canvas projection
  - selected node details show evidence, mapping, risk, confidence, and review
    action when tied to a proposal
  - filters for node type and review status
- Do not build a freeform drag/drop graph editor in this slice.

Verification:

- `cargo check --manifest-path web-ui/Cargo.toml`
- `rm -rf web-ui/target`
- `trunk build --release` from `web-ui`

### 10. Extend Verification Gate

Files:

- `scripts/verify-enterprise-ontology-fast-onboarding.sh`

Work:

- Capture:
  - run JSON
  - DAG JSON
  - prompt packet JSON
  - review graph JSON
  - reviewed proposal JSON
  - materialization JSON
  - tool specs JSON
- Assert the review graph contains object, relation, logic/action, and tool
  evidence when the relevant proposals exist.

Verification:

- Start memory API and run
  `./scripts/verify-enterprise-ontology-fast-onboarding.sh`.
- Clean `.mandoforge/enterprise-ontology-fast-onboarding` after verifying.

## Risks And Mitigations

- Risk: graph UI becomes a complex modeling canvas.
  Mitigation: first slice is read/review oriented only; approval still uses
  existing proposal actions.
- Risk: graph payload grows too large for the browser.
  Mitigation: backend returns deterministic bounded graph with truncation
  metadata.
- Risk: logic/action proposals blur into executable automation.
  Mitigation: materialized logic stays disabled and write-like tools stay
  approval-gated.
- Risk: adding DAG mechanics changes existing demo behavior.
  Mitigation: bridge existing run creation first, preserve old API fields, then
  add new DAG and graph endpoints.

## Verification Commands

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture
cargo check --manifest-path crates/mandoforge-api/Cargo.toml
cargo check --manifest-path web-ui/Cargo.toml
rm -rf web-ui/target
(cd web-ui && trunk build --release)
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_STORE_BACKEND=memory \
MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
cargo run --manifest-path crates/mandoforge-api/Cargo.toml
./scripts/verify-enterprise-ontology-fast-onboarding.sh
```
