# Ontology Builder Usage

Ontology Builder turns enterprise data context into reviewable ontology changes
and then into agent-usable semantic tools. It is not a direct "AI writes the
ontology" path.

The implemented contract is:

```text
seed ontology + source data/profile evidence
  -> ontology onboarding run
  -> schema understanding and subgraph proposals
  -> review graph and prompt packet
  -> human proposal review
  -> semantic materialization
  -> ontology release candidate
  -> gate, promote, and rollback
  -> compiled tool specs for agents
```

The important product rule is proposal-first governance. AI and deterministic
validators can draft object, relation, metric, logic, action, and merge
proposals. Published semantic state changes only after review.

Run identity, lifecycle status, and counters are stored in
`ontology_onboarding_runs`. Proposal payloads remain in `semantic_objects` and
link back through `run_id`; older proposal-only runs remain readable during the
migration period.

## Current Surfaces

The current implementation exposes two related paths.

### Pipeline Mapping V2

Use this for enterprise data onboarding.

```text
data connection
  -> raw or demo source bundle
  -> metadata and profile scan
  -> curated dataset review
  -> ontology proposal run
  -> review graph
  -> materialization
  -> tool specs
```

This is the main path for ecommerce, insurance, Tmall-style commerce packs, and
future industries. The current demo modes are ecommerce and insurance.

### LLM Extraction V1

Use this for quick operator text or document-derived first drafts.

`POST /api/semantic-ontology/builder` accepts free-form source text and creates
an `ontology_expansion` semantic object for review. It does not mutate the
ontology registry or durable organizational memory directly.

This path is useful for "upload notes, draft graph candidates, then review."
It should still flow through ontology review before anything is trusted by
agents.

## Console Flow

Start the local API:

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_STORE_BACKEND=memory \
MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
cargo run --manifest-path crates/mandoforge-api/Cargo.toml
```

Open the product console:

```text
http://127.0.0.1:8787/#semantic
```

Then use the Semantic tab:

1. Check `Ontology readiness` in the overview/status surfaces.
2. Open `Semantic`.
3. For quick text extraction, paste source text into `Ontology builder` and run
   `Preview ontology proposal`.
4. For the full onboarding path, click `Start ecommerce demo run`.
5. Inspect the run summary: dataset count, proposal count, approved count,
   rejected count, materialized count, and compiled tool spec count.
6. Inspect the Ontology Review Graph. It shows object, relation, metric, logic,
   action, merge, and tool nodes with confidence and risk.
7. Use `Approve` or `Reject` on proposals. Low-confidence and high-risk items
   should not be approved blindly.
8. Click `Materialize approved` only after proposal review.
9. Inspect generated tool specs. Write-like business actions should stay
   `proposal_only` or `write_approval_required` unless a later policy enables
   execution.

The console is an operator review surface, not a pipeline canvas and not a
marketing page. The user job is to decide whether the proposed business logic is
correct.

## API Flow

For local development without a bearer token, use dev identity headers:

```bash
export BASE_URL=http://127.0.0.1:8787
AUTH_HEADERS=(-H "x-mandoforge-subject: ontology-onboarding-local" -H "x-mandoforge-roles: admin")
```

List seed packs:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/seed-packs" | jq
```

Create an ecommerce demo run:

```bash
run_id="$(
  curl -sS -X POST "${AUTH_HEADERS[@]}" \
    "$BASE_URL/api/ontology/onboarding/demo-runs" | jq -r '.id'
)"
echo "$run_id"
```

Create a non-demo onboarding run for a supported source mode:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d '{"industry":"insurance","source_mode":"demo_insurance"}' \
  "$BASE_URL/api/ontology/onboarding/runs" | jq
```

Run schema understanding:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d "{\"run_id\":\"$run_id\",\"max_sample_rows\":5}" \
  "$BASE_URL/api/ontology/intelligence/schema-understanding" | jq
```

Run a subgraph proposal for a target object:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d "{\"run_id\":\"$run_id\",\"target_object\":\"Order\"}" \
  "$BASE_URL/api/ontology/intelligence/subgraph-proposals" | jq
```

Inspect execution structure and prompt context:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/runs/$run_id/dag" | jq

curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/runs/$run_id/prompt-packet" | jq
```

Inspect the review graph:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/runs/$run_id/review-graph" | jq
```

Approve or reject a proposal:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d '{"decision":"approve","reason":"reviewed evidence and accepted mapping"}' \
  "$BASE_URL/api/ontology/onboarding/proposals/$proposal_id/review" | jq

curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d '{"decision":"reject","reason":"mapping is too weak"}' \
  "$BASE_URL/api/ontology/onboarding/proposals/$proposal_id/review" | jq
```

Materialize approved proposals:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/runs/$run_id/materialize" | jq
```

List compiled tool specs:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/onboarding/runs/$run_id/tool-specs" | jq
```

## Release Loop

Materialization proves reviewed proposals can become semantic objects and links.
The production loop then turns that materialized evidence into a versioned
ontology release.

Create a release candidate from a materialized run:

```bash
release_id="$(
  curl -sS -X POST "${AUTH_HEADERS[@]}" \
    -H "content-type: application/json" \
    -d '{"version":"commerce-v1","release_class":"customer_grade"}' \
    "$BASE_URL/api/ontology/onboarding/runs/$run_id/release-candidate" | jq -r '.id'
)"
echo "$release_id"
```

Gate and promote the candidate:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/releases/$release_id/gate" | jq

curl -sS -X POST "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/releases/$release_id/promote" | jq
```

List or inspect releases:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/releases" | jq

curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/releases/$release_id" | jq
```

Rollback a newer active release to its previous active release:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/releases/$new_release_id/rollback" | jq
```

Rollback does not delete semantic objects or links. It changes which release is
active for the domain, so runtime context rendering can pin the active ontology
version while preserving audit history.

Render a context packet and check the pinned release metadata:

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d '{"allow_on_demand_fetch":true}' \
  "$BASE_URL/api/context-packets/$context_packet_id/render" \
  | jq '.ontology_scope.ontology_release'
```

Read release-backed readiness:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/engine-readiness" | jq
```

After at least one active release exists, `domain-ontology-lifecycle`,
`approved-release-materialization`, and `migration-policy` should report
`ready`. `conflict-trust-runtime-gates` remains `pilot_ready` until customer
workflow policy evidence binds trust and freshness downgrade behavior to every
high-risk lane, so overall readiness is still expected to be `blocked`.

Inspect confidence calibration:

```bash
curl -sS "${AUTH_HEADERS[@]}" \
  "$BASE_URL/api/ontology/intelligence/runs/$run_id/calibration" | jq
```

Entity resolution uses existing semantic objects as candidates. The gate seeds a
`Client` semantic object, then verifies that a `Customer` candidate can produce a
merge suggestion with review required.

```bash
curl -sS -X POST "${AUTH_HEADERS[@]}" \
  -H "content-type: application/json" \
  -d '{"candidate_name":"Customer","candidate_object_type":"Customer","domain_scope":"commerce"}' \
  "$BASE_URL/api/ontology/intelligence/entity-resolution" | jq
```

## Verification

Run the end-to-end gate against a running local API:

```bash
BASE_URL=http://127.0.0.1:8787 \
./scripts/verify-enterprise-ontology-fast-onboarding.sh
```

The gate writes evidence under:

```text
.mandoforge/enterprise-ontology-fast-onboarding/
```

Run the release-loop gate against a running local API:

```bash
BASE_URL=http://127.0.0.1:8787 \
./scripts/ontology-release-loop-gate.sh
```

The release-loop gate writes candidate, gate, promote, rollback, release list,
and readiness evidence under:

```text
.mandoforge/ontology-release-loop/
```

Expected evidence includes:

- seed packs for ecommerce and insurance
- ecommerce demo run with 8 datasets and at least 32 proposals
- schema understanding evidence for tables such as `orders`
- an `Order` business subgraph with Customer, Order, OrderLine, SKU, GMV, and
  `refund_order`
- DAG levels for pipeline mapping
- prompt packet with seed ontology and allowed relation triples
- curated dataset review evidence
- review graph before and after materialization
- confidence calibration records
- materialized semantic objects and semantic links
- compiled tool specs including `commerce.refund_order`
- insurance run evidence with `Claim` and `approve_claim`

The recent verified console path produced 8 datasets, 33 proposals, 19 semantic
objects, 8 semantic links, and 4 tool specs. Treat those as demo evidence, not a
production benchmark.

## Extension Pattern

To add a new industry, keep the same control plane:

1. Add or load a seed ontology pack with canonical object types, relation
   triples, metric patterns, logic patterns, and allowed action types.
2. Add a source bundle or connector output shape. Raw data should keep source
   metadata such as source system, source object, source primary key, extraction
   batch, and raw payload.
3. Generate profiles: row counts, null rates, uniqueness, key candidates, join
   success rates, enum candidates, time dimensions, currency fields, and PII
   candidates.
4. Generate proposals with evidence and confidence. Do not publish directly.
5. Render the run-scoped review graph so a reviewer can check the business
   logic visually.
6. Materialize approved proposals into `semantic_objects` and `semantic_links`.
7. Compile agent tool specs from approved action and metric proposals.

For Tmall-style connector work, the connector should supply source data and
metadata. Ontology Builder should map that evidence into commerce objects and
actions. Connector secrets must not be copied into proposals, prompt packets,
tool specs, audit logs, or semantic objects.

## Boundaries

- Ontology Builder is not a complete external data catalog.
- It does not replace the ontology registry.
- It does not let AI publish business semantics without review.
- Materialization writes reviewed semantic objects and links, not buyer-facing
  source-system writes.
- Generated business actions are governed tool specs. Risky writes remain
  approval-gated and proposal-only until production policy explicitly enables
  execution.
- Confidence thresholds are customer/domain policy, not a universal benchmark.

## Related Design Docs

- [Enterprise Ontology Fast-Onboarding Design](superpowers/specs/2026-06-12-enterprise-ontology-fast-onboarding-design.md)
- [Ontology Builder IR Design](superpowers/specs/2026-06-14-ontology-builder-ir-design.md)
- [Ontology Builder Intelligence Engine Design](superpowers/specs/2026-06-14-ontology-builder-intelligence-engine-design.md)
