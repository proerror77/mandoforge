# Enterprise Ontology Fast-Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first ecommerce demo slice of Enterprise Ontology Fast-Onboarding, from eight sample datasets to reviewed ontology proposals, materialized semantic objects/links, and generated agent tool specs.

**Architecture:** Add a deterministic demo onboarding pipeline inside `mandoforge-api` that reuses the existing semantic store and audit log. The API creates demo runs, profiles datasets, generates evidence-backed proposals, records human review decisions, materializes approved proposals into `semantic_objects` and `semantic_links`, and exposes generated tool specs. The Yew Semantic view gets a compact operator surface for starting runs, reviewing proposals, materializing approved items, and previewing tool specs.

**Tech Stack:** Rust, Axum, serde/serde_json, existing `AppState` store methods, Yew/Trunk frontend, shell verification script with `curl` and `jq`.

---

## File Structure

- Modify `crates/mandoforge-api/src/main.rs`
  - Add onboarding request/response structs near the existing ontology structs.
  - Add demo dataset fixtures, profiler, mapping/proposal generation, review, materialization, and tool spec helpers near the existing semantic ontology builder functions.
  - Add Axum routes under `/api/ontology/onboarding/*`.
  - Add focused unit tests inside the existing `mod tests`.
- Modify `web-ui/src/api.rs`
  - Add frontend structs for onboarding runs, datasets, proposals, and tool specs.
- Modify `web-ui/src/main.rs`
  - Add polling state and mutation callbacks for onboarding run, review, materialization, and tool specs.
  - Pass the new state and callbacks into `SemanticView`.
- Modify `web-ui/src/views/semantic.rs`
  - Add the Fast-Onboarding panel and proposal review list.
- Modify `web-ui/src/styles.css`
  - Add dense operator styles for onboarding cards, proposal rows, evidence, and tool specs.
- Create `scripts/verify-enterprise-ontology-fast-onboarding.sh`
  - End-to-end local API gate that starts a demo run, reviews proposals, materializes, fetches tool specs, and writes evidence.
- Modify `docs/superpowers/specs/2026-06-12-enterprise-ontology-fast-onboarding-design.md`
  - Add a short implementation-status note only after code is complete.

## Task 1: Backend Demo Model And Profiler

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add failing unit test for demo profiling**

Add this test inside `#[cfg(test)] mod tests` in `crates/mandoforge-api/src/main.rs`:

```rust
#[test]
fn ontology_onboarding_demo_profiles_have_expected_tables_and_evidence() {
    let datasets = ontology_demo_datasets();
    let profiles = ontology_profile_demo_datasets(&datasets);
    let table_names = profiles
        .iter()
        .map(|profile| profile.table_name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(profiles.len(), 8);
    assert!(table_names.contains(&"customers"));
    assert!(table_names.contains(&"orders"));
    assert!(table_names.contains(&"order_items"));

    let orders = profiles
        .iter()
        .find(|profile| profile.table_name == "orders")
        .expect("orders profile");
    assert_eq!(orders.row_count, 4);
    assert!(orders.primary_key_candidates.contains(&"id".to_string()));
    assert!(orders.foreign_key_candidates.iter().any(|candidate| {
        candidate.field == "customer_id"
            && candidate.references_table == "customers"
            && candidate.references_field == "id"
            && candidate.join_success_rate >= 0.99
    }));
    assert!(orders.currency_fields.contains(&"total_price".to_string()));
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_profiles_have_expected_tables_and_evidence -- --nocapture
```

Expected: FAIL with missing `ontology_demo_datasets` or `ontology_profile_demo_datasets`.

- [ ] **Step 3: Add backend structs and demo fixtures**

Add near existing ontology structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingField {
    name: String,
    field_type: String,
    sample_values: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingDataset {
    table_name: String,
    source_system: String,
    source_object: String,
    fields: Vec<OntologyOnboardingField>,
    rows: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyForeignKeyCandidate {
    field: String,
    references_table: String,
    references_field: String,
    join_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyDatasetProfile {
    table_name: String,
    row_count: usize,
    primary_key_candidates: Vec<String>,
    foreign_key_candidates: Vec<OntologyForeignKeyCandidate>,
    enum_candidates: Vec<String>,
    time_dimensions: Vec<String>,
    currency_fields: Vec<String>,
    pii_candidates: Vec<String>,
    field_null_rates: Value,
    field_uniqueness: Value,
}
```

Add `ontology_demo_datasets()` that returns the eight datasets with four rows each. Use table names exactly:

```rust
fn ontology_demo_datasets() -> Vec<OntologyOnboardingDataset> {
    vec![
        ontology_demo_dataset(
            "customers",
            "demo_commerce",
            "customers",
            vec![
                ("id", "string"),
                ("email", "string"),
                ("name", "string"),
                ("created_at", "timestamp"),
            ],
            vec![
                json!({"id":"cus_1","email":"a@example.com","name":"Ada","created_at":"2026-06-01T00:00:00Z"}),
                json!({"id":"cus_2","email":"b@example.com","name":"Ben","created_at":"2026-06-02T00:00:00Z"}),
                json!({"id":"cus_3","email":"c@example.com","name":"Cy","created_at":"2026-06-03T00:00:00Z"}),
                json!({"id":"cus_4","email":"d@example.com","name":"Dee","created_at":"2026-06-04T00:00:00Z"}),
            ],
        ),
        ontology_demo_dataset(
            "orders",
            "demo_commerce",
            "orders",
            vec![
                ("id", "string"),
                ("customer_id", "string"),
                ("status", "string"),
                ("total_price", "decimal"),
                ("created_at", "timestamp"),
            ],
            vec![
                json!({"id":"ord_1","customer_id":"cus_1","status":"paid","total_price":120.0,"created_at":"2026-06-08T00:00:00Z"}),
                json!({"id":"ord_2","customer_id":"cus_1","status":"refunded","total_price":80.0,"created_at":"2026-06-09T00:00:00Z"}),
                json!({"id":"ord_3","customer_id":"cus_2","status":"paid","total_price":210.0,"created_at":"2026-06-10T00:00:00Z"}),
                json!({"id":"ord_4","customer_id":"cus_3","status":"fulfilled","total_price":45.0,"created_at":"2026-06-11T00:00:00Z"}),
            ],
        ),
        ontology_demo_dataset(
            "order_items",
            "demo_commerce",
            "order_items",
            vec![
                ("id", "string"),
                ("order_id", "string"),
                ("sku_id", "string"),
                ("quantity", "integer"),
                ("line_total", "decimal"),
            ],
            vec![
                json!({"id":"oli_1","order_id":"ord_1","sku_id":"sku_1","quantity":1,"line_total":120.0}),
                json!({"id":"oli_2","order_id":"ord_2","sku_id":"sku_2","quantity":2,"line_total":80.0}),
                json!({"id":"oli_3","order_id":"ord_3","sku_id":"sku_3","quantity":1,"line_total":210.0}),
                json!({"id":"oli_4","order_id":"ord_4","sku_id":"sku_1","quantity":1,"line_total":45.0}),
            ],
        ),
        ontology_demo_dataset(
            "products",
            "demo_commerce",
            "products",
            vec![("id", "string"), ("title", "string"), ("category", "string")],
            vec![
                json!({"id":"prd_1","title":"Running Shoe","category":"footwear"}),
                json!({"id":"prd_2","title":"Trail Jacket","category":"apparel"}),
                json!({"id":"prd_3","title":"Water Bottle","category":"accessory"}),
                json!({"id":"prd_4","title":"Yoga Mat","category":"fitness"}),
            ],
        ),
        ontology_demo_dataset(
            "skus",
            "demo_commerce",
            "skus",
            vec![("id", "string"), ("product_id", "string"), ("sku_code", "string"), ("price", "decimal")],
            vec![
                json!({"id":"sku_1","product_id":"prd_1","sku_code":"SHOE-8","price":120.0}),
                json!({"id":"sku_2","product_id":"prd_2","sku_code":"JACKET-M","price":40.0}),
                json!({"id":"sku_3","product_id":"prd_3","sku_code":"BOTTLE-1L","price":210.0}),
                json!({"id":"sku_4","product_id":"prd_4","sku_code":"MAT-BLUE","price":45.0}),
            ],
        ),
        ontology_demo_dataset(
            "inventory",
            "demo_commerce",
            "inventory",
            vec![("id", "string"), ("sku_id", "string"), ("warehouse_id", "string"), ("available_quantity", "integer")],
            vec![
                json!({"id":"inv_1","sku_id":"sku_1","warehouse_id":"wh_1","available_quantity":5}),
                json!({"id":"inv_2","sku_id":"sku_2","warehouse_id":"wh_1","available_quantity":2}),
                json!({"id":"inv_3","sku_id":"sku_3","warehouse_id":"wh_2","available_quantity":50}),
                json!({"id":"inv_4","sku_id":"sku_4","warehouse_id":"wh_2","available_quantity":1}),
            ],
        ),
        ontology_demo_dataset(
            "refunds",
            "demo_commerce",
            "refunds",
            vec![("id", "string"), ("order_id", "string"), ("amount", "decimal"), ("reason", "string"), ("status", "string")],
            vec![
                json!({"id":"ref_1","order_id":"ord_2","amount":80.0,"reason":"size_issue","status":"approved"}),
                json!({"id":"ref_2","order_id":"ord_3","amount":20.0,"reason":"late_delivery","status":"requested"}),
                json!({"id":"ref_3","order_id":"ord_1","amount":10.0,"reason":"coupon_adjustment","status":"closed"}),
                json!({"id":"ref_4","order_id":"ord_4","amount":5.0,"reason":"minor_defect","status":"requested"}),
            ],
        ),
        ontology_demo_dataset(
            "tickets",
            "demo_commerce",
            "tickets",
            vec![("id", "string"), ("customer_id", "string"), ("order_id", "string"), ("status", "string"), ("topic", "string")],
            vec![
                json!({"id":"tic_1","customer_id":"cus_1","order_id":"ord_1","status":"open","topic":"shipping"}),
                json!({"id":"tic_2","customer_id":"cus_2","order_id":"ord_3","status":"escalated","topic":"refund"}),
                json!({"id":"tic_3","customer_id":"cus_3","order_id":"ord_4","status":"closed","topic":"coupon"}),
                json!({"id":"tic_4","customer_id":"cus_4","order_id":null,"status":"open","topic":"product_question"}),
            ],
        ),
    ]
}
```

Add `ontology_demo_dataset(...)`, `ontology_profile_demo_datasets(...)`, uniqueness, null-rate, join-rate, and field-classification helpers. The helpers must treat `id` as a primary key candidate when all non-null values are unique, mark fields ending in `_at` as time dimensions, fields containing `price`, `amount`, or `total` as currency fields, and fields containing `email`, `phone`, or `address` as PII candidates.

- [ ] **Step 4: Run test and verify it passes**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_profiles_have_expected_tables_and_evidence -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mandoforge-api/src/main.rs
git commit -m "Add ontology onboarding demo profiler"
```

## Task 2: Proposal Generation

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add failing proposal generation test**

Add:

```rust
#[test]
fn ontology_onboarding_demo_generates_required_proposals() {
    let datasets = ontology_demo_datasets();
    let profiles = ontology_profile_demo_datasets(&datasets);
    let proposals = ontology_generate_demo_proposals(&datasets, &profiles);

    let object_count = proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "object")
        .count();
    let relation_count = proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "relation")
        .count();
    let metric_count = proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "metric")
        .count();
    let action_count = proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == "action")
        .count();

    assert!(object_count >= 8);
    assert!(relation_count >= 7);
    assert!(metric_count >= 5);
    assert!(action_count >= 4);
    assert!(proposals.iter().all(|proposal| proposal.confidence >= 0.70));
    assert!(proposals.iter().all(|proposal| proposal.evidence.is_object()));
    assert!(proposals.iter().any(|proposal| {
        proposal.proposal_type == "relation"
            && proposal.name == "Customer places Order"
            && proposal.evidence["join_success_rate"].as_f64().unwrap_or_default() >= 0.99
    }));
    assert!(proposals.iter().any(|proposal| {
        proposal.proposal_type == "action"
            && proposal.name == "refund_order"
            && proposal.content["policy"]["approval_required"].as_bool() == Some(true)
    }));
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_generates_required_proposals -- --nocapture
```

Expected: FAIL with missing `OntologyOnboardingProposalDraft` or `ontology_generate_demo_proposals`.

- [ ] **Step 3: Add proposal draft model and deterministic proposal generation**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingProposalDraft {
    id: Uuid,
    run_id: Uuid,
    proposal_type: String,
    name: String,
    source_mapping: String,
    confidence: f64,
    evidence: Value,
    recommendation: String,
    review_status: String,
    content: Value,
}
```

Add `ontology_generate_demo_proposals(datasets, profiles)` to create deterministic proposals. Use helper constructors:

- `ontology_object_proposal(...)`
- `ontology_relation_proposal(...)`
- `ontology_metric_proposal(...)`
- `ontology_action_proposal(...)`

Required object mappings:

```text
customers -> Customer
orders -> Order
order_items -> OrderLine
products -> Product
skus -> SKU
inventory -> InventoryItem
refunds -> Refund
tickets -> SupportTicket
```

Required action proposals:

```text
refund_order: target_object=Order, approval_required=true
issue_coupon: target_object=Customer, approval_required=true
adjust_inventory: target_object=InventoryItem, approval_required=true
escalate_ticket: target_object=SupportTicket, approval_required=false
```

Each action `content` must include `inputs`, `reads`, `effects`, `policy`,
`executor`, and `audit_event`.

- [ ] **Step 4: Run test and verify it passes**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_generates_required_proposals -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mandoforge-api/src/main.rs
git commit -m "Generate ontology onboarding proposals"
```

## Task 3: Onboarding API, Review, And Materialization

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add failing async API lifecycle test**

Add an async test inside `mod tests` using existing test-state helpers:

```rust
#[tokio::test]
async fn ontology_onboarding_demo_review_and_materialize_flow() {
    let state = test_app_state();
    let run = create_demo_ontology_onboarding_run_for_test(&state)
        .await
        .expect("demo run");

    assert_eq!(run.dataset_count, 8);
    assert!(run.proposal_count >= 24);

    let object_proposal = run
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_type == "object" && proposal.name == "Customer")
        .expect("customer proposal")
        .id;
    let relation_proposal = run
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_type == "relation" && proposal.name == "Customer places Order")
        .expect("customer order relation")
        .id;

    review_ontology_onboarding_proposal_for_test(
        &state,
        object_proposal,
        "approve",
        Some("seed mapping and profile evidence match"),
    )
    .await
    .expect("approve object");
    review_ontology_onboarding_proposal_for_test(
        &state,
        relation_proposal,
        "approve",
        Some("join evidence is above threshold"),
    )
    .await
    .expect("approve relation");

    let materialized = materialize_ontology_onboarding_run_for_test(&state, run.id)
        .await
        .expect("materialize");
    assert!(materialized.semantic_object_count >= 1);
    assert!(materialized.semantic_link_count >= 1);

    let semantic_objects = state.list_semantic_objects().await.expect("objects");
    assert!(semantic_objects.iter().any(|object| {
        object.object_type == "business_object" && object.object_key == "commerce.customer"
    }));
    let semantic_links = state.list_semantic_links().await.expect("links");
    assert!(semantic_links.iter().any(|link| {
        link.relation_type == "places"
    }));
}
```

If `test_app_state()` does not exist, use the local test helper that creates an in-memory `AppState`; name it consistently with nearby tests.

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_review_and_materialize_flow -- --nocapture
```

Expected: FAIL with missing lifecycle helpers.

- [ ] **Step 3: Add response structs and in-memory run shape**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingRun {
    id: Uuid,
    status: String,
    source_mode: String,
    dataset_count: usize,
    profile_count: usize,
    proposal_count: usize,
    approved_count: usize,
    materialized_count: usize,
    datasets: Vec<OntologyOnboardingDataset>,
    profiles: Vec<OntologyDatasetProfile>,
    proposals: Vec<OntologyOnboardingProposalDraft>,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ReviewOntologyOnboardingProposalRequest {
    decision: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingMaterializationResult {
    run_id: Uuid,
    status: String,
    semantic_object_count: usize,
    semantic_link_count: usize,
    tool_spec_count: usize,
    semantic_object_ids: Vec<Uuid>,
    semantic_link_ids: Vec<Uuid>,
}
```

For the first slice, reconstruct deterministic demo runs from semantic proposal objects instead of adding migrations. Persist proposals as `semantic_objects` with `object_type="ontology_onboarding_proposal"` and `object_key="ontology:onboarding:{run_id}:{proposal_id}"`. Store `run_id`, proposal content, review status, and materialization status in `content`.

- [ ] **Step 4: Add routes**

Add routes near existing ontology routes:

```rust
.route("/api/ontology/onboarding/demo-runs", post(create_demo_ontology_onboarding_run))
.route("/api/ontology/onboarding/runs", get(list_ontology_onboarding_runs))
.route("/api/ontology/onboarding/runs/{id}", get(get_ontology_onboarding_run))
.route(
    "/api/ontology/onboarding/proposals/{id}/review",
    post(review_ontology_onboarding_proposal),
)
.route(
    "/api/ontology/onboarding/runs/{id}/materialize",
    post(materialize_ontology_onboarding_run),
)
.route(
    "/api/ontology/onboarding/runs/{id}/tool-specs",
    get(list_ontology_onboarding_tool_specs),
)
```

- [ ] **Step 5: Implement route handlers**

Implement handlers with `Permission::AgentsWrite` for mutations and `Permission::AgentsRead` or `Permission::Admin` for reads, matching existing semantic route authorization style.

`create_demo_ontology_onboarding_run` should:

1. Generate demo datasets.
2. Profile them.
3. Generate proposal drafts.
4. Persist each proposal as a semantic object.
5. Append `ontology_onboarding.demo_run_created` audit log.
6. Return the run.

`review_ontology_onboarding_proposal` should:

1. Require decision in `approve`, `reject`, `request_changes`, `merge_into_existing`, `needs_more_evidence`.
2. Update proposal content review status.
3. Append `ontology_onboarding.proposal_reviewed` audit log.

`materialize_ontology_onboarding_run` should:

1. Load approved, unmaterialized proposals.
2. Create semantic objects for object and metric proposals.
3. Create semantic links for relation proposals.
4. Create semantic action specs for action proposals.
5. Mark proposals as materialized.
6. Append `ontology_onboarding.run_materialized` audit log.

- [ ] **Step 6: Run lifecycle test and verify it passes**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_demo_review_and_materialize_flow -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/mandoforge-api/src/main.rs
git commit -m "Add ontology onboarding API lifecycle"
```

## Task 4: Tool Spec Compiler

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add failing tool spec test**

Add:

```rust
#[tokio::test]
async fn ontology_onboarding_generates_agent_tool_specs() {
    let state = test_app_state();
    let run = create_demo_ontology_onboarding_run_for_test(&state)
        .await
        .expect("demo run");

    for proposal in run.proposals.iter().filter(|proposal| proposal.proposal_type == "action") {
        review_ontology_onboarding_proposal_for_test(
            &state,
            proposal.id,
            "approve",
            Some("action contract has policy and audit metadata"),
        )
        .await
        .expect("approve action");
    }

    let materialized = materialize_ontology_onboarding_run_for_test(&state, run.id)
        .await
        .expect("materialize actions");
    assert!(materialized.tool_spec_count >= 4);

    let specs = ontology_onboarding_tool_specs_for_run(&state, run.id)
        .await
        .expect("tool specs");
    let names = specs.iter().map(|spec| spec.name.as_str()).collect::<Vec<_>>();
    assert!(names.contains(&"commerce.refund_order"));
    assert!(names.contains(&"commerce.issue_coupon"));
    assert!(names.contains(&"commerce.adjust_inventory"));
    assert!(names.contains(&"commerce.escalate_ticket"));
    assert!(specs.iter().any(|spec| spec.name == "commerce.refund_order" && spec.approval_required));
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_generates_agent_tool_specs -- --nocapture
```

Expected: FAIL with missing `OntologyOnboardingToolSpec`.

- [ ] **Step 3: Add tool spec model and compiler**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyOnboardingToolSpec {
    id: Uuid,
    run_id: Uuid,
    name: String,
    description: String,
    tool_kind: String,
    target_object: String,
    read_only: bool,
    approval_required: bool,
    input_schema: Value,
    effects: Value,
    policy: Value,
    audit_event: String,
    source_proposal_id: Uuid,
}
```

Implement `ontology_onboarding_tool_specs_for_run(state, run_id)` by reading materialized action proposal semantic objects and compiling names:

```text
refund_order -> commerce.refund_order
issue_coupon -> commerce.issue_coupon
adjust_inventory -> commerce.adjust_inventory
escalate_ticket -> commerce.escalate_ticket
```

Return write-like tools as `approval_required=true` and `read_only=false`.

- [ ] **Step 4: Run tool spec test and verify it passes**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding_generates_agent_tool_specs -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mandoforge-api/src/main.rs
git commit -m "Compile ontology onboarding tool specs"
```

## Task 5: UI Fast-Onboarding Panel

**Files:**
- Modify: `web-ui/src/api.rs`
- Modify: `web-ui/src/main.rs`
- Modify: `web-ui/src/views/semantic.rs`
- Modify: `web-ui/src/styles.css`

- [ ] **Step 1: Add frontend API types**

In `web-ui/src/api.rs`, add:

```rust
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingRun {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source_mode: String,
    #[serde(default)]
    pub dataset_count: usize,
    #[serde(default)]
    pub profile_count: usize,
    #[serde(default)]
    pub proposal_count: usize,
    #[serde(default)]
    pub approved_count: usize,
    #[serde(default)]
    pub materialized_count: usize,
    #[serde(default)]
    pub proposals: Vec<OntologyOnboardingProposal>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingProposal {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub proposal_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source_mapping: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub review_status: String,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub content: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct OntologyOnboardingToolSpec {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target_object: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub approval_required: bool,
}
```

- [ ] **Step 2: Add state and callbacks in `web-ui/src/main.rs`**

Import the new types. Add `use_state(|| None::<OntologyOnboardingRun>)` and `use_state(Vec::<OntologyOnboardingToolSpec>::new)`. Add callbacks:

- `start_ontology_onboarding`
- `approve_ontology_proposal`
- `reject_ontology_proposal`
- `materialize_ontology_onboarding`

Use `api_post` to call:

```text
/api/ontology/onboarding/demo-runs
/api/ontology/onboarding/proposals/{id}/review
/api/ontology/onboarding/runs/{id}/materialize
/api/ontology/onboarding/runs/{id}/tool-specs
```

- [ ] **Step 3: Extend `SemanticProps`**

In `web-ui/src/views/semantic.rs`, add props:

```rust
pub(crate) onboarding_run: Option<OntologyOnboardingRun>,
pub(crate) onboarding_tool_specs: Vec<OntologyOnboardingToolSpec>,
pub(crate) on_start_onboarding: Callback<MouseEvent>,
pub(crate) on_approve_onboarding_proposal: Callback<String>,
pub(crate) on_reject_onboarding_proposal: Callback<String>,
pub(crate) on_materialize_onboarding: Callback<MouseEvent>,
```

- [ ] **Step 4: Render Fast-Onboarding panel**

Add a panel titled `Enterprise ontology fast-onboarding` before the existing `Ontology builder` panel. It must show:

- start demo button
- run id/status/source mode
- dataset/profile/proposal/approved/materialized counts
- proposal rows grouped by proposal type
- evidence preview with `JsonPreview`
- approve/reject buttons
- materialize button
- generated tool specs list

Use existing `Panel`, `Rows`, `KeyMetrics`, and `JsonPreview`.

- [ ] **Step 5: Add CSS**

In `web-ui/src/styles.css`, add classes:

```css
.ontology-onboarding { display: grid; gap: 12px; }
.ontology-onboarding-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.ontology-proposal-list { display: grid; gap: 8px; max-height: 520px; overflow: auto; }
.ontology-proposal-row { display: grid; gap: 8px; padding: 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--panel-muted); }
.ontology-proposal-row header { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
.ontology-proposal-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.ontology-tool-specs { display: grid; gap: 6px; }
```

Use existing CSS variables. If a variable is not present, use existing panel/card colors already used in the file.

- [ ] **Step 6: Run frontend check**

Run:

```bash
cargo check --manifest-path web-ui/Cargo.toml
```

Expected: PASS.

- [ ] **Step 7: Clean generated target**

Run:

```bash
rm -rf web-ui/target
```

- [ ] **Step 8: Commit**

```bash
git add web-ui/src/api.rs web-ui/src/main.rs web-ui/src/views/semantic.rs web-ui/src/styles.css
git commit -m "Add ontology onboarding review UI"
```

## Task 6: Verification Gate

**Files:**
- Create: `scripts/verify-enterprise-ontology-fast-onboarding.sh`
- Modify: `docs/superpowers/specs/2026-06-12-enterprise-ontology-fast-onboarding-design.md`

- [ ] **Step 1: Create verification script**

Create `scripts/verify-enterprise-ontology-fast-onboarding.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_SUBJECT:-ontology-onboarding-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_ONBOARDING_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-ontology-fast-onboarding}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise ontology fast-onboarding gate requires $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

mkdir -p "$EVIDENCE_DIR"

headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

run_file="$EVIDENCE_DIR/demo-run.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/demo-runs" | tee "$run_file" >/dev/null
run_id="$(jq -r '.id // empty' "$run_file")"
if [[ -z "$run_id" ]]; then
  echo "demo onboarding run did not return id" >&2
  exit 1
fi

jq -e '.dataset_count == 8 and .proposal_count >= 24' "$run_file" >/dev/null

proposal_ids=($(jq -r '.proposals[] | select(.proposal_type == "object" or .proposal_type == "relation" or .proposal_type == "action") | .id' "$run_file" | head -12))
for proposal_id in "${proposal_ids[@]}"; do
  curl -sS -X POST "${headers[@]}" \
    -H 'content-type: application/json' \
    -d '{"decision":"approve","reason":"gate-approved demo proposal"}' \
    "$BASE_URL/api/ontology/onboarding/proposals/$proposal_id/review" \
    >"$EVIDENCE_DIR/review-$proposal_id.json"
done

materialized_file="$EVIDENCE_DIR/materialized.json"
curl -sS -X POST "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/materialize" | tee "$materialized_file" >/dev/null
jq -e '.semantic_object_count >= 1 and .semantic_link_count >= 1 and .tool_spec_count >= 1' "$materialized_file" >/dev/null

tool_specs_file="$EVIDENCE_DIR/tool-specs.json"
curl -sS "${headers[@]}" "$BASE_URL/api/ontology/onboarding/runs/$run_id/tool-specs" | tee "$tool_specs_file" >/dev/null
jq -e 'any(.tool_specs[]?; .name == "commerce.refund_order")' "$tool_specs_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "run_id=$run_id"
  echo "dataset_count=$(jq -r '.dataset_count' "$run_file")"
  echo "proposal_count=$(jq -r '.proposal_count' "$run_file")"
  echo "semantic_object_count=$(jq -r '.semantic_object_count' "$materialized_file")"
  echo "semantic_link_count=$(jq -r '.semantic_link_count' "$materialized_file")"
  echo "tool_spec_count=$(jq -r '.tool_spec_count' "$materialized_file")"
  echo "run_file=$run_file"
  echo "materialized_file=$materialized_file"
  echo "tool_specs_file=$tool_specs_file"
} >"$summary_file"

cat "$summary_file"
echo "enterprise ontology fast-onboarding gate ok"
```

- [ ] **Step 2: Make script executable**

Run:

```bash
chmod +x scripts/verify-enterprise-ontology-fast-onboarding.sh
```

- [ ] **Step 3: Update design implementation status**

Append to `docs/superpowers/specs/2026-06-12-enterprise-ontology-fast-onboarding-design.md`:

```markdown
## Implementation Status

The first ecommerce demo slice is implemented by the API routes under
`/api/ontology/onboarding/*`, the Semantic console fast-onboarding panel, and
`scripts/verify-enterprise-ontology-fast-onboarding.sh`. The first slice remains
demo-source only; Tmall connector input is the next compatible adapter.
```

- [ ] **Step 4: Commit**

```bash
git add scripts/verify-enterprise-ontology-fast-onboarding.sh docs/superpowers/specs/2026-06-12-enterprise-ontology-fast-onboarding-design.md
git commit -m "Add ontology onboarding verification gate"
```

## Task 7: End-to-End Verification And Static Assets

**Files:**
- Modify generated static assets under `web/` only if `trunk build --release` changes them.

- [ ] **Step 1: Run backend targeted tests**

Run:

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run API check**

Run:

```bash
cargo check --manifest-path crates/mandoforge-api/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run frontend check**

Run:

```bash
cargo check --manifest-path web-ui/Cargo.toml
rm -rf web-ui/target
```

Expected: PASS and no `web-ui/target` remains.

- [ ] **Step 4: Build static UI assets**

Run:

```bash
NO_COLOR=false trunk build --release --manifest-path web-ui/Cargo.toml
```

Expected: `web/index.html` and hashed `web/mandoforge-web-ui-*` assets update if frontend code changed.

- [ ] **Step 5: Run static UI gates**

Run:

```bash
./scripts/verify-static-ui-assets.sh
node scripts/verify-ui-api-truth-gate.mjs
```

Expected: PASS.

- [ ] **Step 6: Run onboarding gate against local API**

Start the API in another shell:

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_STORE_BACKEND=memory \
MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
cargo run --manifest-path crates/mandoforge-api/Cargo.toml
```

Then run:

```bash
./scripts/verify-enterprise-ontology-fast-onboarding.sh
```

Expected: PASS and evidence under `.mandoforge/enterprise-ontology-fast-onboarding/`.

- [ ] **Step 7: Clean local evidence and generated build directories**

Run:

```bash
rm -rf .mandoforge/enterprise-ontology-fast-onboarding web-ui/target
```

- [ ] **Step 8: Run diff checks**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; only intended source, script, doc, and generated web asset changes remain.

- [ ] **Step 9: Commit static assets if changed**

If `web/` changed, run:

```bash
git add web web-ui
git commit -m "Build ontology onboarding static UI"
```

If `web/` did not change, do not create an empty commit.

## Spec Coverage Self-Review

- Demo data source and eight datasets: Task 1.
- Profiling evidence: Task 1.
- Seed mapping and proposal generation: Task 2.
- Human review and audit: Task 3.
- Materialization into semantic objects/links: Task 3.
- Tool spec compiler: Task 4.
- UI operator surface: Task 5.
- Verification gate and evidence output: Task 6.
- End-to-end validation and static assets: Task 7.
- Tmall compatibility boundary: covered in design and preserved by keeping adapters behind common discovery contracts; implementation remains demo-source only in this slice.
