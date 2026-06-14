# Ontology Builder Intelligence Engine Design

## Summary

The current Ontology Builder slice gives MandoForge a governed control plane:
seed packs, dataset profiles, curated dataset review, prompt packets, ontology
proposals, review graph, human review, materialization, and compiled agent tool
specs.

The next architecture step is the intelligence engine behind those proposals.
It must not replace the review/control plane. It should feed it with better
object, relation, metric, logic, action, and entity-resolution proposals.

Research reviewed for this slice points to one hard constraint:

```text
LLM-only ontology construction is not reliable enough for enterprise dark data.
The correct product shape is LLM recommendation + deterministic validation
+ human correction + continuous learning.
```

Public and benchmark-oriented results can look strong, but private enterprise
data, undocumented tables, internal acronyms, and hidden business process
semantics create a large accuracy drop. Therefore MandoForge should treat AI as
a proposal generator and validator assistant, never as the source of truth.

Target flow:

```text
DatasetSnapshot / DocumentSnapshot / ApiSnapshot
  -> SchemaUnderstandingCandidate
  -> SubgraphProposalDraft
  -> EntityResolutionCandidate
  -> ConfidenceCalibrationRecord
  -> OntologyProposalDraft
  -> OntologyReviewGraph
  -> human review
  -> semantic materialization
  -> tool specs and action execution envelopes
```

## Design Principles

- Keep AI in proposal mode.
- Prefer seed ontology mapping before free invention.
- Model business subgraphs, not isolated extraction tasks.
- Combine LLM semantics with deterministic evidence and embedding similarity.
- Store every confidence score with evidence, reviewer outcome, and later
  runtime outcome.
- Keep Action Type transaction semantics explicit; do not assume a vendor-like
  behavior that is not implemented in MandoForge.
- Reuse `semantic_sources`, `semantic_objects`, `semantic_links`,
  `context_packets`, audit logs, approvals, and the existing ontology registry.

## Research Claims Adopted

### End-To-End Subgraph Modeling

Ontology construction should not be decomposed into a brittle linear extractor
that first recognizes entities, then extracts relationships, then guesses
hierarchy. That shape compounds errors and grows poorly when relation
candidates approach quadratic scale.

MandoForge should add a `SubgraphProposalEngine` that generates one bounded
business subgraph at a time:

```text
objects + properties + links + metrics + logic rules + action candidates
```

The engine returns a reviewable `SubgraphProposalDraft`; it never writes
published ontology state directly.

### Schema Understanding From Samples

Undocumented enterprise tables need a GeTT-style schema understanding path:

```text
sample rows
  -> infer candidate entity type name
  -> infer field meaning and business category
  -> build taxonomy layer candidates
  -> emit object/property hierarchy proposals
```

The first production shape should sample a bounded number of rows per table,
attach profile evidence, and ask the model for typed JSON. The output is still
checked against deterministic rules:

- primary key uniqueness
- null rates
- enum and state candidates
- join success rates
- PII and currency markers
- seed ontology match
- existing registry constraints

### LLM Plus Embedding, Not Either-Or

The engine should split responsibilities:

```text
LLM:
  concept naming
  business-language interpretation
  action and policy draft generation
  duplicate explanation

Embedding:
  taxonomy coherence
  nearest ontology candidates
  canonical object suggestions
  reviewer history retrieval

Deterministic validators:
  key uniqueness
  join evidence
  registry-allowed relation triples
  action approval and audit requirements
```

### Graphiti-Style Entity Resolution

Entity resolution should use a hybrid retrieval path:

```text
new candidate entity
  -> vector search top-k
  -> BM25 search top-k
  -> merge candidates
  -> LLM duplicate decision
  -> review if confidence is low or risk is high
```

The output contract:

```json
{
  "is_duplicate": true,
  "canonical_name": "Customer",
  "existing_node_uuid": "uuid-if-duplicate",
  "confidence": 0.91,
  "evidence": {
    "vector_candidates": [],
    "bm25_candidates": [],
    "llm_rationale": "..."
  }
}
```

Unlike Graphiti's free-form relation type strings, MandoForge must constrain
relationship proposals through the ontology registry and review graph. Free-form
labels can be draft evidence, but published link types must be registry-backed.

### Palantir-Style Object, Link, Action Boundary

MandoForge should keep the same conceptual split:

```text
Object Type: business entity
Link Type: relationship between object instances
Property: typed object attribute
Action Type: governed business operation
Function: reusable business logic
Audit: immutable execution and review trace
```

Rejected assumptions:

- Do not assume Action Type execution is an atomic transaction unless
  MandoForge implements it.
- Do not assume writes immediately propagate everywhere unless MandoForge
  implements propagation, invalidation, and conflict handling.

### DeepOnto Placement

DeepOnto/BERTMap-style ontology alignment belongs after multiple ontology
fragments already exist. It is useful for aligning concepts such as
`CRM.Customer`, `ERP.Client`, and `Support.Requester`. It is not the primary
engine for building an ontology from raw tables, documents, or APIs.

## Architecture

### Schema Understanding Engine

Inputs:

- `DatasetSnapshot`
- `DatasetProfile`
- bounded sample rows
- source metadata and lineage
- seed ontology pack
- existing ontology registry

Outputs:

- `SchemaUnderstandingCandidate`
- object type candidates
- property candidates
- taxonomy layer candidates
- confidence and evidence

Responsibilities:

- infer business object names from poorly documented tables
- propose property semantics
- identify over-specific or duplicate object candidates
- mark low confidence or high-risk candidates for human review

Non-responsibilities:

- no durable ontology mutation
- no action execution
- no source-system writes

### Subgraph Proposal Engine

Inputs:

- schema understanding candidates
- curated dataset drafts
- profile evidence
- existing object/link/action types
- prompt packet policy reminders
- optional SOP, OpenAPI, and document snapshots

Outputs:

- `SubgraphProposalDraft`
- linked `OntologyProposalDraft` records
- review graph nodes and edges

The engine should produce bounded subgraphs. For example, an ecommerce order
subgraph may include:

```text
Customer
Order
OrderLine
SKU
Customer places Order
Order contains OrderLine
GMV
Refund Rate
refund_order
Order identity rule
```

This keeps the model context small and gives reviewers a business-meaningful
unit to accept, reject, or edit.

### Entity Resolution Engine

Inputs:

- new object, property, link, metric, action, or entity candidates
- existing semantic objects and ontology registry records
- reviewer history
- source refs and profile evidence

Outputs:

- `EntityResolutionCandidate`
- duplicate decision draft
- canonical name recommendation
- merge target recommendation

Review behavior:

- high confidence, low risk: auto-attach recommendation to proposal
- medium confidence: show in review graph as a merge edge
- low confidence or high risk: block materialization until reviewed

### Confidence Calibration Loop

Confidence must become empirical over time. Each proposal should record:

- model confidence
- deterministic validator score
- retrieval similarity score
- source quality score
- reviewer decision
- later runtime success or failure when applicable

The calibration loop should emit:

- threshold recommendations per industry and source mode
- proposal-type-specific precision estimates
- reviewer disagreement hotspots
- seed ontology gaps

Initial threshold policy:

```text
>= 0.90: draft-ready, still visible in review history
0.70-0.89: quick review queue
0.50-0.69: detailed review queue
< 0.50: retry or discard
```

These thresholds are defaults, not universal truth. They must be tuned on
customer data.

### Action Transaction Model

Action Types require an explicit consistency model. The engine should compile
action proposals with one of these transaction profiles:

```text
local_serializable:
  single MandoForge/Postgres transaction

event_sourced:
  immutable action event, projected state, replayable audit

saga:
  multi-system action with step state, compensation, timeout, and operator
  repair path

proposal_only:
  no execution; review and modeling only
```

MVP policy:

- read tools may execute after normal authorization
- write-like tools require approval
- cross-system writes default to `proposal_only` until a transaction profile and
  compensation policy are configured
- every action proposal must declare reads, effects, policy, executor, audit
  event, and transaction profile

## Runtime Placement In MandoForge

The intelligence engine should be introduced as a layer above the existing
semantic primitives:

```text
semantic_sources:
  source, dataset, document, API, and provenance records

semantic_objects:
  proposal records, object types, logic rules, metrics, action specs,
  calibration records

semantic_links:
  registry-backed links, proposal evidence links, duplicate/merge suggestions

context_packets:
  bounded prompt and execution snapshots

approvals and audit logs:
  review decisions, action approvals, materialization trace
```

Do not create a parallel graph store for v1. A graph database or RDF/TypeDB
sync can be added later as a projection from the governed semantic store.

## API Surface

Proposed future endpoints:

```text
POST /api/ontology/intelligence/schema-understanding
POST /api/ontology/intelligence/subgraph-proposals
POST /api/ontology/intelligence/entity-resolution
GET  /api/ontology/intelligence/runs/{id}/calibration
POST /api/ontology/intelligence/action-transaction-profiles
```

These endpoints should return proposal artifacts and review graph updates, not
publish ontology state.

## UI Requirements

The Semantic view should extend the current review graph with:

- low-confidence queue
- duplicate and merge suggestions
- taxonomy layer warnings
- over-fragmentation warnings
- action transaction profile badges
- reviewer feedback capture
- calibration evidence panel

The UI must help the operator answer:

- Did the model invent a new object when it should have mapped to an existing
  one?
- Is the object hierarchy too granular?
- Are relationships backed by join, lineage, document, or API evidence?
- Does an action have explicit reads, effects, policy, transaction profile, and
  audit event?
- Which proposal types are repeatedly rejected for this customer or industry?

## Risks

- LLM hallucination can produce polished but wrong business concepts.
- Free-form relation labels can bypass ontology constraints if not gated.
- Entity resolution mistakes can merge objects that should remain separate.
- Confidence thresholds are not portable across industries or customers.
- Action Type execution can create inconsistent state without an explicit
  transaction model.
- Review UI can become noisy if every low-value candidate is surfaced.

## Mitigations

- Every generated artifact remains proposal-first.
- Published relation types must pass ontology registry validation.
- Duplicate and merge suggestions require evidence and review when risky.
- Confidence calibration is per industry, source mode, and proposal type.
- Write actions are approval-gated and require transaction profiles.
- The review graph is bounded and grouped by business subgraph.

## Open Questions

- Which vector store should be used for the first entity-resolution prototype:
  Postgres pgvector, an embedded local index, or an external service?
- Should confidence calibration be stored as semantic objects or a dedicated
  append-only table?
- What is the minimum transaction profile needed before enabling a live Tmall
  or ERP write action?
- How should reviewer feedback be weighted when different users disagree?
- When should MandoForge introduce a graph projection store instead of relying
  only on semantic objects and links?
