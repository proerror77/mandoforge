# Ontology Engine Production Release Loop Design

## Status

Approved first-stage scope from the operator on 2026-06-19.

This design turns the current Ontology Builder demo and proposal flow into a
customer-grade release loop for ontology state. It does not enable direct
production business writes. High-risk business actions remain draft-only or
approval-only until a later production policy explicitly enables execution.

## Problem

The repo already has an Ontology Builder pipeline:

```text
seed pack / source profile
  -> onboarding run
  -> schema understanding
  -> subgraph and action proposals
  -> operator review
  -> semantic object/link materialization
  -> compiled semantic tools for agents
```

That is enough for a reviewed demo loop and agent-readable semantic context, but
the enterprise completion contract requires more:

- Core and domain ontology objects must be versioned and queryable.
- Approved ontology changes must promote into immutable releases.
- Domain ontology releases must support promotion, rollback, archive, and
  migration evidence.
- Context packets and runtime tool access must pin the ontology version they
  rely on.
- Readiness must distinguish repo-controlled evidence, pilot evidence, and
  customer-grade release evidence.

## Goals

1. Add a first-class ontology release model that can represent release
   candidates, active releases, rolled-back releases, archived releases, and
   migration state.
2. Promote approved Ontology Builder proposals into immutable ontology release
   records instead of treating semantic materialization as the end of the flow.
3. Add release gates that verify review evidence, materialization evidence,
   relation constraints, migration policy, rollback target, and runtime pinning.
4. Make `/api/ontology/engine-readiness` report the release loop as ready only
   when customer-grade evidence exists.
5. Keep managed-agent runtime access bounded by `ContextPacket`, `TaskGrant`,
   trust/freshness gates, and provider tool filtering.

## Non-Goals

- Do not enable live writes to Tmall, Xiaohongshu, TikTok Shop, Amazon, Feishu,
  or other business systems.
- Do not replace the existing semantic store or Context OS primitives.
- Do not introduce a separate graph database in this slice.
- Do not mark the whole enterprise product as complete. This slice only targets
  the `ontology-engine` lane.
- Do not bypass human review for AI-generated ontology changes.

## Current Surfaces

The implementation should reuse these existing surfaces:

- `GET /api/ontology/registry`
- `GET /api/ontology/engine-readiness`
- `/api/ontology/onboarding/*`
- `/api/ontology/intelligence/*`
- `semantic_objects` and `semantic_links`
- `context_packets`
- `audit_logs`
- `scripts/ontology-engine-readiness-gate.sh`
- `scripts/verify-enterprise-ontology-fast-onboarding.sh`

The current readiness builder has eight checks. The first implementation should
focus on turning these blocked checks into release-backed checks:

- `domain-ontology-lifecycle`
- `approved-release-materialization`
- `migration-policy`

The existing `conflict-trust-runtime-gates` check can remain `pilot_ready` until
customer-specific high-risk workflow policy evidence exists.

## Data Model

Add a durable ontology release table through a migration.

Suggested table:

```text
ontology_releases
  id uuid primary key
  tenant_id uuid not null
  version text not null
  domain_scope text not null
  source_run_id uuid null
  parent_release_id uuid null
  rollback_target_release_id uuid null
  status text not null
  release_class text not null
  object_count integer not null
  relation_count integer not null
  action_count integer not null
  migration_policy jsonb not null
  gate_result jsonb not null
  materialized_object_ids jsonb not null
  materialized_link_ids jsonb not null
  evidence_refs jsonb not null
  promoted_by text null
  promoted_at timestamptz null
  rolled_back_by text null
  rolled_back_at timestamptz null
  archived_at timestamptz null
  created_at timestamptz not null
  updated_at timestamptz not null
```

The unique constraint on version must be scoped to the owning tenant and domain: `UNIQUE (tenant_id, domain_scope, version)`. A bare `UNIQUE` on `version` alone would cause cross-tenant collisions.

Allowed statuses:

- `candidate`
- `active`
- `superseded`
- `rolled_back`
- `archived`
- `failed_gate`

Allowed release classes:

- `repo_controlled`
- `production_like_pilot`
- `customer_grade`

The first release class should remain `repo_controlled` unless gate evidence
explicitly proves customer-grade criteria. Do not inflate evidence class.

## API Design

Add ontology release endpoints under the existing admin boundary:

```text
GET  /api/ontology/releases
GET  /api/ontology/releases/{id}
POST /api/ontology/onboarding/runs/{id}/release-candidate
POST /api/ontology/releases/{id}/gate
POST /api/ontology/releases/{id}/promote
POST /api/ontology/releases/{id}/rollback
POST /api/ontology/releases/{id}/archive
```

`release-candidate` creates an immutable candidate from one onboarding run. It
must require at least one approved and materialized proposal.

`gate` validates the candidate and stores a deterministic gate result. The gate
must fail closed when:

- approved proposals are missing review evidence
- materialized semantic objects or links are missing
- a semantic link violates ontology relation constraints
- a write-like action lacks a transaction profile or approval policy
- migration policy is missing
- rollback target is missing when the candidate supersedes an active release

`promote` activates a gated candidate. It must supersede the previous active
release for the same `domain_scope` in the same transaction where the backend
supports transactions, or with a fail-closed update order for the in-memory
store.

`rollback` activates the rollback target or previous active release and records
audit evidence. It must not delete semantic objects or links. Rollback changes
the active ontology version pointer and marks the failed release as
`rolled_back`.

`archive` is allowed only for non-active releases.

## Runtime Contract

Context packets should carry explicit ontology release metadata:

```json
{
  "ontology_release": {
    "id": "...",
    "version": "commerce-v2026.06.19-001",
    "domain_scope": "commerce",
    "release_class": "repo_controlled",
    "status": "active"
  }
}
```

Provider tool schemas must stay filtered by the existing task grant and rendered
context packet. Runtime tools may read only objects visible to the packet unless
`scoped_lookup` is explicitly allowed by the grant.

High-risk actions must still use the existing policy and approval path. Ontology
release promotion does not grant live business execution.

## Readiness Contract

`GET /api/ontology/engine-readiness` should remain blocked until all required
checks have customer-grade evidence. In this first stage:

- `domain-ontology-lifecycle` becomes `ready` only when release records support
  candidate, promote, rollback, archive, and active-version query.
- `approved-release-materialization` becomes `ready` only when approved
  proposals can become a gated release candidate and promotion records durable
  audit evidence.
- `migration-policy` becomes `ready` only when the release gate validates a
  migration policy and rollback target.
- `conflict-trust-runtime-gates` remains `pilot_ready` unless customer-grade
  high-risk policy binding evidence exists.

This means the ontology lane can become materially stronger without pretending
the whole product is enterprise-complete.

## UI Design

Keep the first slice API-first. The existing Semantic page can add a compact
release summary after the API is stable:

- active ontology version
- latest candidate status
- gate result
- promote / rollback / archive actions behind admin token checks

No new canvas or visual redesign is required for this slice.

## Audit And Authorization

All write endpoints require `Permission::Admin` or the same admin-grade boundary
used by current ontology readiness operations.

Required audit actions:

- `ontology_release.candidate_created`
- `ontology_release.gated`
- `ontology_release.promoted`
- `ontology_release.rolled_back`
- `ontology_release.archived`

Audit records must include subject, release id, version, domain scope, source run
id, previous active release id when relevant, gate status, and evidence refs.

## Testing

Add targeted API tests before broad UI work:

- creating a candidate from a run with no materialized proposals fails closed
- approved and materialized onboarding proposals create a release candidate
- gate fails when migration policy is missing
- gate passes when review, materialization, relation constraints, migration
  policy, and rollback target are present
- promotion marks one active release per domain and supersedes the previous one
- rollback restores the previous active release without deleting semantic state
- readiness reflects release-backed lifecycle and materialization evidence
- provider runtime still receives only filtered ontology tools

Verification commands:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release -- --nocapture
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture
cargo test --manifest-path crates/mandoforge-api/Cargo.toml workflow_step_run_endpoint_claims_and_executes_session_loop -- --nocapture
cargo check --manifest-path web-ui/Cargo.toml
```

After API implementation, extend `scripts/ontology-engine-readiness-gate.sh` or
add a focused `scripts/ontology-release-loop-gate.sh` that exercises candidate,
gate, promote, rollback, and readiness readback against a running local API.

## Implementation Shape

Keep the first implementation slice narrow:

1. Add migration and store methods for ontology releases.
2. Add release candidate, gate, promote, rollback, archive handlers.
3. Wire routes into the existing API router.
4. Update readiness builder to consume release evidence instead of static
   blocked states.
5. Add focused tests and a local evidence script.
6. Add only minimal UI readback after API correctness is proven.

Do not refactor the large `main.rs` beyond the minimum needed for this slice
unless the release code becomes impossible to reason about in place. If handler
extraction is required, extract only ontology-release code and keep unrelated
route cleanup out of this change.
