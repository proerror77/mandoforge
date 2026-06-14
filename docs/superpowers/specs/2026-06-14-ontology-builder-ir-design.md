# Ontology Builder IR Design

## Summary

MandoForge already has the right runtime substrate for enterprise ontology
onboarding: semantic sources, semantic objects, semantic links, context packets,
ontology registry validation, policy, approval, audit, and tool calls. The next
architecture step is not to add a separate graph product. It is to extract the
current demo-driven ontology onboarding code into an industry-neutral Ontology
Builder IR that can support ecommerce, insurance, Tmall, and future industries
through the same reviewable pipeline.

The design references `jingw2/nano-ontoprompt` at commit `41acb1a`. The useful
ideas are its explicit pipeline stages, curated dataset review, mapping drafts,
link inference, and separate logic/action review lifecycles. The parts not
adopted are its FastAPI service split, Neo4j/Chroma operational stack, and
visual pipeline canvas. Those are product choices outside the current
MandoForge Agent OS boundary.

Target flow:

```text
Source adapter
  -> DatasetSnapshot
  -> profile and quality report
  -> CuratedDataset review envelope
  -> OntologyPromptPacket
  -> object / relation / metric / logic / action drafts
  -> Ontology PR human review
  -> semantic materialization
  -> tool spec compiler
  -> governed agent runtime
```

Both the Pipeline Mapping path and the Simple LLM Extraction path are workflow
DAGs, not hardcoded sequential scripts. Each stage produces artifacts consumed
by downstream stages. The builder must validate dependency edges before
execution, reject cycles, compute topological execution levels, and support
rerunning only the affected downstream subgraph when an upstream artifact
changes.

## Goals

- Make ontology onboarding industry-neutral instead of demo-specific.
- Preserve MandoForge's proposal-first governance: generated ontology changes
  do not mutate published semantic memory until reviewed.
- Add a curated dataset boundary between profiling and ontology proposals.
- Add a promptable, typed IR that AI and deterministic validators can both use.
- Add first-class logic rule drafts alongside object, relation, metric, and
  action proposals.
- Keep actions governed: write-like action tools compile with approval and audit
  metadata, not direct external execution.
- Reuse existing semantic stores and audit logs instead of introducing a new
  ontology database for this slice.

## Non-Goals

- No Neo4j, Chroma, RDF, OWL, or TypeDB dependency in this slice.
- No visual pipeline builder.
- No live Tmall credential handling or external Tmall write execution.
- No automatic publishing of AI-generated ontology changes.
- No general ETL product. Source adapters only emit normalized snapshots needed
  by ontology onboarding.
- No replacement of `/api/ontology/registry` or existing semantic-link
  validation.

## Reference Assessment

`nano-ontoprompt` has two useful architectural patterns:

- Pipeline Mapping:
  `Data Connection -> Raw Storage -> Transform -> Curated Dataset -> Ontology Mapping`.
- Ontology building blocks:
  Entity/Object, Relation/Link, Logic Rule, and Action.

Its implementation also shows practical mechanics MandoForge should adapt:

- Curated datasets carry schema, quality score, status, version, and review
  records.
- Mapping drafts map one curated dataset to an entity class with field mapping,
  primary key, status, and confidence.
- Link mapping drafts capture source dataset, target dataset, relation type,
  source key, target key, status, and inferred confidence.
- Relation inference combines naming, exact FK matching, normalized ID matching,
  value overlap, and alternate-key matching.
- Logic and action artifacts stay in `draft` or review states until published.

The main anti-pattern to avoid is a single `build_all` service that maps
entities, writes relations, discovers logic, creates actions, writes vectors,
and mutates graph state in one pass. In MandoForge, those steps must remain
separate proposal compilers connected through review, policy, and audit.

## Architecture

### Builder IR Types

The first implementation should define internal Rust structs near the current
ontology onboarding code, then move them into a dedicated module once the shape
stabilizes.

Core IR:

```text
OntologyBuilderRun
OntologyBuilderDag
OntologyBuilderNode
OntologyBuilderEdge
OntologyBuilderExecutionLevel
OntologySourceBundle
DatasetSnapshot
DatasetFieldSnapshot
DatasetProfile
DatasetQualityReport
CuratedDatasetDraft
OntologyPromptPacket
OntologyReviewGraph
OntologyReviewGraphNode
OntologyReviewGraphEdge
ObjectMappingDraft
RelationMappingDraft
MetricDraft
LogicRuleDraft
ActionTypeDraft
OntologyProposalDraft
CompiledToolSpec
```

`OntologyBuilderDag` is the execution contract for both v1 and v2 onboarding
paths. It must include:

- `nodes`: typed builder stages with stable IDs, stage kind, input artifact
  refs, output artifact refs, status, and retry policy
- `edges`: dependency edges where `from` must finish before `to` starts
- `levels`: topologically sorted batches that can run in parallel
- `critical_path`: the longest dependency chain for operator diagnostics
- `affected_subgraph`: downstream nodes to rerun when a node input changes

`DatasetSnapshot` replaces demo-only dataset assumptions. It must include:

- `dataset_id`
- `table_name`
- `source_system`
- `source_object`
- `source_mode`
- `schema`
- bounded `sample_rows`
- `provenance`

`CuratedDatasetDraft` is the reviewable envelope between profile and mapping. It
must include:

- `dataset_id`
- `quality_score`
- `quality_issues`
- `schema_version`
- `status`: `pending_review`, `approved`, `rejected`, or `changes_requested`
- `review`: reviewer, reason, decided_at

`OntologyPromptPacket` is the prompt-native contract. It is structured JSON, not
free-form prose. It must include:

- seed ontology objects, properties, relations, metrics, logic patterns, and
  action patterns
- curated dataset snapshots
- profile and quality evidence
- allowed relation triples from `/api/ontology/registry`
- evidence rules for confidence
- policy reminders for actions and PII
- output schema for proposals

The prompt packet is useful even before a live LLM integration because it gives
deterministic builders, tests, and future coding agents the same contract.

The follow-on intelligence layer is documented in
`docs/superpowers/specs/2026-06-14-ontology-builder-intelligence-engine-design.md`.
This IR remains the governed review/control plane. The intelligence engine feeds
it better schema understanding, subgraph proposals, entity resolution, and
confidence calibration records, but it does not bypass proposal review or
materialization policy.

### Workflow DAG Execution Model

Ontology onboarding has correctness dependencies. A relation proposal cannot be
generated before object mapping candidates exist. A tool spec cannot compile
before an action proposal is approved and materialized. A Simple LLM Extraction
run cannot validate references before the extraction result exists.

The builder should therefore represent every run as a DAG and execute it with a
topological scheduler:

```text
Pipeline Mapping v2:
connect_source
  -> snapshot_raw_dataset
  -> transform_dataset
  -> profile_curated_dataset
  -> review_curated_dataset
  -> compile_prompt_packet
  -> generate_mapping_proposals
  -> review_ontology_pr
  -> materialize_ontology
  -> compile_agent_tools

Simple LLM Extraction v1:
upload_documents
  -> convert_to_markdown
  -> compile_prompt_packet
  -> extract_ontology_json
  -> validate_extraction
  -> calibrate_confidence
  -> infer_missing_relations
  -> generate_ontology_pr
  -> review_ontology_pr
  -> materialize_ontology
  -> compile_agent_tools
```

The scheduler must enforce:

- producers finish before consumers start
- cycles are rejected before any node runs
- nodes in the same topological level may run concurrently
- fan-in nodes wait for all required upstream artifacts
- downstream-only reruns when an upstream dataset, prompt, seed pack, or review
  decision changes

Cycle detection is a definition-time error. The API should return a clear
validation error that names the offending edge path instead of starting a run
that later deadlocks.

The run record should store the DAG definition, execution levels, node statuses,
artifact refs, and audit refs. This makes the operator view explain why a node
is blocked, which stages ran in parallel, and which downstream artifacts were
invalidated by a change.

### Ontology Review Graph

The builder needs a visual review surface so operators can confirm whether the
proposed business objects, relations, metrics, logic rules, actions, and tools
match the actual business process. This is a review graph, not an editing canvas
and not a separate graph database.

`OntologyReviewGraph` is a bounded JSON projection derived from the current run:

- dataset nodes: raw or curated datasets used as evidence
- object nodes: proposed or materialized business objects
- relation edges: proposed or materialized business links
- metric nodes: metric definitions and their source objects
- logic nodes: validation, state, mapping, security, or automation rules
- action nodes: action types with risk and approval metadata
- tool nodes: compiled agent tool specs

Each graph node must include:

- stable `id`
- `node_type`: `dataset`, `object`, `metric`, `logic`, `action`, or `tool`
- display `label`
- review/materialization `status`
- `confidence`
- `risk_tone`: `neutral`, `info`, `warn`, or `bad`
- `source_refs`
- compact `evidence`
- source proposal ID when applicable

Each graph edge must include:

- stable `id`
- `from`
- `to`
- `edge_type`: `maps_to`, `relates_to`, `depends_on`, `validates`,
  `acts_on`, `compiles_to`, or `uses_metric`
- review/materialization `status`
- `confidence`
- `label`

The first UI should support:

- graph filters by node type and review status
- visual distinction between pending, approved, rejected, and materialized
  proposals
- click-to-inspect side panel with evidence, source mapping, confidence, risk,
  and review actions
- highlighting of logic/action dependencies so users can validate whether
  business logic is attached to the right objects and relations
- no freeform drag/drop editing in the first slice

The graph must be bounded for browser reliability. The first slice should cap
the response to a deterministic subset such as 80 nodes and 160 edges, with a
`truncated=true` flag and omitted counts when the run is larger.

### Proposal Categories

Generated proposals should use one lifecycle while keeping type-specific
content:

```text
pending_review -> approved | rejected | changes_requested | merge_into_existing | needs_more_evidence
approved -> materialized
```

Proposal types:

- `object_mapping`
- `relation_mapping`
- `metric_definition`
- `logic_rule`
- `action_type`

Each proposal must include:

- source dataset references
- source mapping
- confidence
- evidence facts
- risk notes
- recommendation
- review status
- materialization status

### Logic Rule Drafts

Logic rules should be generated from schema/profile/quality/relation evidence,
not only from seed names.

Supported first-slice logic types:

- `validation`: non-null, uniqueness, type consistency, positive amount
- `state`: enum-backed status fields and allowed transitions
- `mapping`: technical-to-business field mapping constraints
- `security`: PII or sensitive field handling
- `automation`: low-risk suggestions that still compile to review-only actions

Logic rules materialize as semantic objects with `object_type=ontology_logic_rule`
and stay disabled unless explicitly published by a later policy-aware flow.

### Action Type Drafts

Action drafts must model what an agent could do without granting execution by
default.

Required action fields:

- `target_object`
- `inputs`
- `reads`
- `effects`
- `submission_criteria`
- `permission_rules`
- `executor`
- `approval_required`
- `audit_event`

Materialized write-like actions compile into tool specs with
`approval_required=true` unless a future production policy proves otherwise.

### Materialization

Approved proposals materialize through existing stores:

- object mappings -> `semantic_objects` with `object_type=business_object`
- metric definitions -> `semantic_objects` with `object_type=business_metric`
- logic rules -> `semantic_objects` with `object_type=ontology_logic_rule`
- action types -> `semantic_objects` with `object_type=ontology_action_type`
- relation mappings -> `semantic_links` after registry triple validation

The current materialization helpers should remain idempotent by object key or
link identity. Every materialization writes an audit log.

### API Shape

Extend the current `/api/ontology/onboarding/*` surface instead of adding a
separate service:

```text
GET  /api/ontology/onboarding/seed-packs
POST /api/ontology/onboarding/runs
GET  /api/ontology/onboarding/runs
GET  /api/ontology/onboarding/runs/{id}
GET  /api/ontology/onboarding/runs/{id}/dag
GET  /api/ontology/onboarding/runs/{id}/review-graph
POST /api/ontology/onboarding/curated-datasets/{id}/review
GET  /api/ontology/onboarding/runs/{id}/prompt-packet
POST /api/ontology/onboarding/proposals/{id}/review
POST /api/ontology/onboarding/runs/{id}/materialize
GET  /api/ontology/onboarding/runs/{id}/tool-specs
```

The existing demo endpoint can remain as a compatibility alias.

### UI Shape

The first UI pass should stay inside the Semantic view. It should not become a
pipeline canvas.

Minimum additions:

- show curated dataset quality and review status before proposals
- show the DAG levels and blocked/runnable/completed node status for the current
  onboarding run
- show an Ontology Review Graph that lets reviewers inspect object, relation,
  metric, logic, action, and tool dependencies
- show prompt packet summary: seed pack, datasets, allowed relation triples,
  evidence rules
- group proposals by object, relation, metric, logic, action
- show action risk and approval requirement beside generated tool specs

## Known Current Architecture Gaps

The implementation should address these while extracting the IR:

- `ontology_onboarding_run_from_objects()` reconstructs runs from
  `ontology_demo_source_bundle()`, which loses the original industry/source
  bundle for non-ecommerce runs. Store run metadata in proposal content or a
  run envelope object and rehydrate from that metadata.
- The current seed and source structs live inline in `main.rs`. This is enough
  for the demo but too coupled for an industry-generic builder.
- Current proposals cover object, relation, metric, and action, but not logic
  rule drafts.
- Current profiling is useful but skips the curated review boundary from the
  product model.
- Tool specs are generated only for materialized action proposals. This should
  remain, but the spec should also expose read/write risk and approval policy.
- The current Semantic graph is a general semantic map. It does not yet provide
  a run-scoped ontology review graph with proposal evidence and review actions.

## Testing

Backend tests should prove:

- invalid DAG definitions with cycles are rejected before execution
- topological levels place independent nodes in the same runnable batch
- affected-subgraph recomputation marks only downstream nodes stale
- review graph projection includes datasets, objects, relations, metrics, logic,
  actions, tools, statuses, confidence, evidence, and truncation metadata
- review graph nodes link back to source proposal IDs when applicable
- ecommerce and insurance runs rehydrate the original source mode and seed
  metadata correctly
- prompt packet generation includes seed objects, relations, datasets, profiles,
  allowed relation triples, evidence rules, and policy reminders
- curated dataset review gates proposal generation or marks proposals as
  `needs_more_evidence`
- object, relation, metric, logic, and action proposal counts meet expected demo
  floors
- relation inference handles exact FK and normalized alternate-key evidence
- materialization remains idempotent
- write-like actions compile with approval and audit metadata

Verification should extend the existing
`scripts/verify-enterprise-ontology-fast-onboarding.sh` gate to capture:

- run JSON
- DAG JSON with levels and node statuses
- review graph JSON with expected node and edge types
- curated dataset review JSON
- prompt packet JSON
- reviewed proposal JSON
- materialization JSON
- tool spec JSON

## Rollout Plan

This design should be implemented in narrow commits:

1. Extract IR structs and preserve existing behavior.
2. Add DAG definitions, cycle detection, topological execution levels, and
   affected-subgraph invalidation.
3. Store and rehydrate run metadata without ecommerce hardcoding.
4. Add prompt packet compiler and tests.
5. Add curated dataset drafts and review lifecycle.
6. Add logic rule proposal generation and materialization.
7. Add review graph projection API.
8. Update tool spec compiler metadata for action risk and approval.
9. Update Semantic UI with the run-scoped review graph and verification script.

## Success Criteria

- Existing onboarding tests still pass.
- The verification script still proves the ecommerce demo flow.
- A non-ecommerce seed run rehydrates correctly after list/get.
- The API can return a structured `OntologyPromptPacket`.
- The API can return the run DAG with topological levels and node statuses.
- The API can return a bounded ontology review graph for a run.
- A cyclic onboarding definition fails before execution starts.
- A changed source artifact invalidates only downstream nodes.
- The Semantic UI lets reviewers inspect business-object, relation, logic,
  action, metric, and tool dependencies before approval.
- Approved logic and action proposals materialize into semantic objects without
  bypassing review.
- Generated write tools stay approval-gated.
- No new external graph, vector, or ETL service is required.
