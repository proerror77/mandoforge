# Enterprise Ontology Fast-Onboarding Design

## Summary

Enterprise Ontology Fast-Onboarding turns newly ingested enterprise data into
reviewable ontology proposals and then into agent-usable semantic tools. The
first implementation slice is an ecommerce demo pipeline using eight sample
tables. It proves the product path without depending on live customer
credentials:

```text
demo ecommerce datasets
  -> metadata scan
  -> schema profiling
  -> seed ontology mapping
  -> ontology PR proposals
  -> human review
  -> semantic materialization
  -> agent tool specs
```

The design uses MandoForge's existing ontology-ready substrate:
`semantic_sources`, `semantic_objects`, `semantic_links`, `context_packets`, the
Ontology Builder proposal API, the ontology registry, and the Context Compiler.
It does not introduce a separate data-catalog product or a second ontology
store.

## Goals

- Prove a fast ecommerce onboarding path from raw enterprise-like data to
  approved ontology artifacts.
- Keep AI in proposal mode. AI can draft mappings, metrics, relations, and
  actions, but cannot mutate the durable ontology without review.
- Use seed ontology first. The system maps discovered tables to known ecommerce
  concepts instead of inventing concepts from scratch.
- Produce evidence-backed proposals with confidence, source references, profile
  facts, and review recommendations.
- Materialize approved proposals into the existing semantic store and runtime
  tool-spec layer.
- Keep the path compatible with Tmall connector input without requiring live
  Tmall credentials in the first demo.

## Non-Goals

- No live Tmall, Shopify, SAP, Airbyte, OpenMetadata, or warehouse dependency in
  the first slice.
- No automatic approval or automatic ontology registry release.
- No high-risk external action execution from onboarding.
- No RDF, OWL, TypeDB, or graph database dependency for v1.
- No claim that the ontology engine is customer-grade complete.

## First Slice

The first slice uses eight ecommerce sample datasets:

- `customers`
- `orders`
- `order_items`
- `products`
- `skus`
- `inventory`
- `refunds`
- `tickets`

The pipeline should generate at least:

- 8 object proposals:
  `Customer`, `Order`, `OrderLine`, `Product`, `SKU`, `InventoryItem`,
  `Refund`, `SupportTicket`.
- 7 relation proposals:
  `Customer places Order`, `Order contains OrderLine`,
  `OrderLine references SKU`, `SKU represents Product`,
  `SKU has InventoryItem`, `Order may_have Refund`,
  `Customer creates SupportTicket`.
- 5 metric proposals:
  `GMV`, `AOV`, `Refund Rate`, `Repeat Purchase Rate`,
  `Inventory Turnover`.
- 4 action proposals:
  `refund_order`, `issue_coupon`, `adjust_inventory`, `escalate_ticket`.

Each proposal must include:

- `proposal_type`
- target object, relation, metric, or action
- source mapping
- confidence score
- evidence facts
- recommendation
- review status

## Architecture

### Data Source Adapter

The source adapter normalizes raw input into a common discovery contract. The
first implementation provides `DemoTableSource`.

Future adapters, including Tmall, should implement the same output contract:

```text
DemoTableSource
TmallConnectorSource
CsvUploadSource
WarehouseTableSource
```

All adapters emit:

```text
DiscoveredDataset
DiscoveredField
SampleRows
SourceProvenance
```

The Tmall adapter maps read operations into raw datasets, for example:

```text
taobao.trades.sold.get     -> raw_tmall.trades
taobao.item.seller.get     -> raw_tmall.items
taobao.refunds.receive.get -> raw_tmall.refunds
tmall.traderate.feeds.get  -> raw_tmall.reviews
taobao.qianniu.tasks.get   -> raw_tmall.workorders
```

Tmall remains optional in the first slice. Live mode still requires the existing
approval and connector controls.

### Profiler

The profiler computes deterministic evidence from discovered datasets:

- row count
- null rate
- uniqueness
- primary key candidates
- foreign key candidates
- join success rate
- enum candidates
- time dimensions
- currency-like fields
- PII candidates
- sample values

The profiler output is stored as proposal evidence, not as authoritative
ontology state.

### Seed Mapping Engine

The mapping engine compares discovered datasets and profile results against the
ecommerce seed ontology. It should combine:

- table and field name similarity
- field-set signatures
- primary-key and foreign-key evidence
- sample values
- known ecommerce ontology terms
- existing pack ontology seeds

The first implementation can be deterministic and rules-backed. LLM output may
be accepted as an `agent_draft`, but deterministic profile evidence remains the
validator.

### Ontology Proposal Engine

The proposal engine creates an onboarding run and a list of ontology proposals.
The proposal set should be stored as semantic proposal objects rather than
mutating the registry directly.

Proposal categories:

- object type mapping
- relation mapping
- metric definition
- action type definition
- source-to-ontology mapping
- policy and approval notes

Action proposals must declare:

- `target_object`
- `inputs`
- `reads`
- `effects`
- `policy`
- `executor`
- `audit_event`

Write-like actions stay disabled until separately approved. In the demo, action
proposals create tool specs and policy requirements, not live side effects.

### Human Review

Human review is required before materialization. Review decisions:

- approve
- reject
- request_changes
- merge_into_existing
- needs_more_evidence

The UI should show the proposal, confidence, evidence, source mapping, and risk
notes in a compact review queue. The reviewer should not need to build the
ontology manually; they should only confirm or adjust AI-generated proposals.

### Materialization

Approved proposals materialize into existing MandoForge runtime stores:

- object proposals -> `semantic_objects`
- relation proposals -> `semantic_links`
- metric proposals -> semantic objects with `object_type=metric`
- action proposals -> workflow pack runtime objects or semantic action specs

Materialization appends audit logs and keeps source provenance. Rejected or
changed proposals remain durable review records.

### Semantic Tool Compiler

The compiler turns approved ontology artifacts into agent-usable tool specs.
The first slice should generate tool specs such as:

- `commerce.get_customer`
- `commerce.list_customer_orders`
- `commerce.calculate_gmv`
- `commerce.find_low_inventory_skus`
- `commerce.find_refund_anomalies`
- `commerce.refund_order`
- `commerce.issue_coupon`
- `commerce.adjust_inventory`
- `commerce.escalate_ticket`

Read tools can be marked available in demo mode. Write tools must be marked
approval-gated and non-live unless an approved connector execution path exists.

## API Shape

The first implementation should add a compact API surface:

```text
POST /api/ontology/onboarding/demo-runs
GET  /api/ontology/onboarding/runs
GET  /api/ontology/onboarding/runs/{id}
POST /api/ontology/onboarding/proposals/{id}/review
POST /api/ontology/onboarding/runs/{id}/materialize
GET  /api/ontology/onboarding/runs/{id}/tool-specs
```

The API should require admin or agents-write authorization consistent with the
existing semantic ontology builder routes.

## UI Shape

The existing Semantic view should grow a dedicated Fast-Onboarding section, or a
new focused view can be added if the panel becomes too dense.

Minimum UI:

- start demo onboarding run
- show discovered datasets
- show profile summary
- show proposal counts by type and status
- review proposal list
- approve/reject/request changes
- materialize approved proposals
- preview generated tool specs

The UI should avoid a marketing-style landing page. This is an operator surface
for reviewing evidence and approving ontology changes.

## Runtime And Agent-Native Boundary

The pipeline should preserve MandoForge's agent-native rules:

- UI actions and agent capabilities must have parity.
- Tools are primitive capabilities; onboarding is an outcome achieved by
  composing scan, profile, propose, review, and materialize primitives.
- The agent may create proposal drafts, but review and materialization stay
  governed operations.
- Context packets should be able to include approved ontology objects and links
  after materialization.

## Tmall Connector Compatibility

Tmall can plug into the same source adapter boundary after the demo slice.

Mode boundaries:

- Demo mode uses fixture/sample payloads and requires no secrets.
- Dry-run connector mode uses `native.connector.call` to produce redacted
  request plans and profile-compatible sample structures.
- Live controlled mode requires `MANDOFORGE_NATIVE_CONNECTOR_LIVE_ENABLED`,
  Tmall TOP secrets, connector readiness evidence, and approval commit tokens.

Onboarding may create ActionType proposals from Tmall write operations, but it
must not execute buyer-facing writes during ontology onboarding.

## Data Model Additions

The implementation can start with in-memory/demo records if consistent with the
existing store abstraction, but Postgres support should be designed around these
entities:

- `ontology_onboarding_runs`
- `ontology_onboarding_datasets`
- `ontology_onboarding_profiles`
- `ontology_onboarding_proposals`
- `ontology_onboarding_tool_specs`

If the existing semantic object store is sufficient for proposal persistence,
the new tables can be deferred, but the API response shape should still expose
run/proposal/tool-spec identities explicitly.

## Error Handling

- Missing demo fixture: fail with a clear setup blocker.
- Empty dataset: produce blocked profile evidence rather than panic.
- Ambiguous mapping: lower confidence and recommend `needs_more_evidence`.
- Missing relation evidence: allow object proposals but block relation
  materialization.
- Duplicate approved proposal: merge or no-op with audit evidence.
- Materialization conflict: block and return the conflicting object or relation.
- Write action proposal without policy: block materialization.

## Security And Governance

- Raw samples are treated as untrusted data.
- PII candidates are flagged before review and excluded from prompt rendering
  unless explicitly allowed.
- All materialization writes append audit logs.
- Cross-domain sharing stays default-deny.
- Write actions stay approval-gated and disabled for live execution by default.
- Connector secrets are never copied into ontology proposals or tool specs.

## Testing And Gates

Minimum tests:

- demo run creates expected dataset and profile records
- object proposals include confidence and evidence
- relation proposals require join evidence
- action proposals include reads, effects, policy, and audit metadata
- review decisions update proposal status and audit logs
- materialization writes semantic objects and semantic links
- generated tool specs match approved proposals
- rejected proposals do not materialize

Minimum gate script:

```bash
scripts/verify-enterprise-ontology-fast-onboarding.sh
```

The gate should start from a local API, run the demo onboarding pipeline, review
selected proposals, materialize them, fetch generated tool specs, and write
evidence under:

```text
.mandoforge/enterprise-ontology-fast-onboarding/
```

## Success Criteria

The first slice is complete when:

- A demo onboarding run can be started without external credentials.
- The run discovers 8 ecommerce datasets.
- The run creates at least 8 object proposals, 7 relation proposals, 5 metric
  proposals, and 4 action proposals.
- Every proposal includes confidence and evidence.
- Human review decisions are durable and audited.
- Approved object/relation proposals materialize into the semantic store.
- Tool specs are generated from approved ontology artifacts.
- The UI shows the full run and proposal lifecycle.
- A verification script proves the end-to-end path.

## Implementation Order

1. Add deterministic demo fixtures and profiler.
2. Add onboarding run/proposal API.
3. Add proposal generation for objects and relations.
4. Add review and audit flow.
5. Add materialization into semantic objects and links.
6. Add metric/action proposals and generated tool specs.
7. Add UI review surface.
8. Add verification gate and focused tests.

## Open Boundary

This design intentionally leaves real external catalog integration for a later
slice. The next adapter after demo mode should be Tmall dry-run mode because the
repo already has a real `ecommerce-tmall` DomainPack, `tmall-top` connector
contract, and Alibaba TOP native adapter boundary.
