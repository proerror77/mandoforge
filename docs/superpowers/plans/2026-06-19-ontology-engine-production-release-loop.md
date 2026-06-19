# Ontology Engine Production Release Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first customer-grade ontology release loop: approved ontology onboarding proposals become gated, promotable, rollbackable ontology releases, and runtime/readiness surfaces can pin and report the active ontology version.

**Architecture:** Add a durable `OntologyRelease` model on top of the existing semantic kernel. Release lifecycle endpoints stay under the existing Admin authorization boundary and reuse onboarding materialization evidence. Runtime context rendering reads the active ontology release for the packet domain and includes release metadata without expanding provider tool privileges.

**Tech Stack:** Rust, Axum, SQLx/Postgres, in-memory `MemoryStore`, serde/serde_json, bash gate scripts.

---

## File Map

- Create `db/migrations/0061_ontology_releases.sql`: durable Postgres table and indexes.
- Modify `crates/mandoforge-api/src/main.rs`: API structs, route handlers, release gate logic, readiness integration, runtime render metadata, focused tests.
- Modify `crates/mandoforge-api/src/store_backend.rs`: add `ontology_releases` memory map.
- Modify `crates/mandoforge-api/src/store_rows.rs`: add `ontology_release_from_row`.
- Create `crates/mandoforge-api/src/store_ontology_releases.rs`: list/get/create/update helpers for memory and Postgres.
- Modify `scripts/ontology-engine-readiness-gate.sh`: print release-backed readiness details.
- Create `scripts/ontology-release-loop-gate.sh`: local API evidence gate for candidate, gate, promote, rollback, and readiness readback.
- Modify `docs/ontology-builder-usage.md`: document operator commands for the release loop.

## Acceptance Criteria

- `POST /api/ontology/onboarding/runs/{id}/release-candidate` fails closed when the run has no approved materialized proposals.
- A run with approved materialized proposals can create an immutable `candidate` release.
- `POST /api/ontology/releases/{id}/gate` fails when migration policy is missing or write-like action evidence is unsafe.
- A gated candidate can be promoted to `active`; only one active release exists per `domain_scope`.
- A later promoted release supersedes the previous active release and records it as rollback target.
- Rollback restores the previous release pointer without deleting semantic objects or links.
- `POST /api/context-packets/{id}/render` includes active ontology release metadata when a domain-scoped active release exists.
- `/api/ontology/engine-readiness` uses release evidence for `domain-ontology-lifecycle`, `approved-release-materialization`, and `migration-policy`.
- Existing ontology onboarding and runtime-scope tests still pass.

## Task 1: Add Ontology Release Storage

**Files:**
- Create: `db/migrations/0061_ontology_releases.sql`
- Modify: `crates/mandoforge-api/src/main.rs`
- Modify: `crates/mandoforge-api/src/store_backend.rs`
- Modify: `crates/mandoforge-api/src/store_rows.rs`
- Create: `crates/mandoforge-api/src/store_ontology_releases.rs`

- [ ] **Step 1: Add the migration**

Create `db/migrations/0061_ontology_releases.sql` with:

```sql
CREATE TABLE IF NOT EXISTS ontology_releases (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    domain_scope TEXT NOT NULL,
    source_run_id UUID,
    parent_release_id UUID REFERENCES ontology_releases(id),
    rollback_target_release_id UUID REFERENCES ontology_releases(id),
    status TEXT NOT NULL,
    release_class TEXT NOT NULL,
    object_count INTEGER NOT NULL DEFAULT 0,
    relation_count INTEGER NOT NULL DEFAULT 0,
    action_count INTEGER NOT NULL DEFAULT 0,
    migration_policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    gate_result JSONB NOT NULL DEFAULT '{}'::jsonb,
    materialized_object_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    materialized_link_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    promoted_by TEXT,
    promoted_at TIMESTAMPTZ,
    rolled_back_by TEXT,
    rolled_back_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, version)
);

CREATE INDEX IF NOT EXISTS idx_ontology_releases_tenant_domain_status
    ON ontology_releases (tenant_id, domain_scope, status);

CREATE INDEX IF NOT EXISTS idx_ontology_releases_source_run
    ON ontology_releases (tenant_id, source_run_id);
```

- [ ] **Step 2: Define `OntologyRelease`**

In `crates/mandoforge-api/src/main.rs`, near the ontology onboarding structs, add a serde model with fields matching the migration:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OntologyRelease {
    id: Uuid,
    version: String,
    domain_scope: String,
    source_run_id: Option<Uuid>,
    parent_release_id: Option<Uuid>,
    rollback_target_release_id: Option<Uuid>,
    status: String,
    release_class: String,
    object_count: i32,
    relation_count: i32,
    action_count: i32,
    migration_policy: Value,
    gate_result: Value,
    materialized_object_ids: Value,
    materialized_link_ids: Value,
    evidence_refs: Value,
    promoted_by: Option<String>,
    promoted_at: Option<DateTime<Utc>>,
    rolled_back_by: Option<String>,
    rolled_back_at: Option<DateTime<Utc>>,
    archived_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Wire memory store**

In `store_backend.rs`, add `OntologyRelease` to the existing multi-line crate
import block at the top of the file and add this field to `MemoryStore`:

```rust
pub(crate) ontology_releases: HashMap<Uuid, OntologyRelease>,
```

- [ ] **Step 4: Add row converter**

In `store_rows.rs`, import `OntologyRelease` and add:

```rust
pub(crate) fn ontology_release_from_row(row: PgRow) -> Result<OntologyRelease, AppError> {
    Ok(OntologyRelease {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        domain_scope: row.try_get("domain_scope")?,
        source_run_id: row.try_get("source_run_id")?,
        parent_release_id: row.try_get("parent_release_id")?,
        rollback_target_release_id: row.try_get("rollback_target_release_id")?,
        status: row.try_get("status")?,
        release_class: row.try_get("release_class")?,
        object_count: row.try_get("object_count")?,
        relation_count: row.try_get("relation_count")?,
        action_count: row.try_get("action_count")?,
        migration_policy: row.try_get("migration_policy")?,
        gate_result: row.try_get("gate_result")?,
        materialized_object_ids: row.try_get("materialized_object_ids")?,
        materialized_link_ids: row.try_get("materialized_link_ids")?,
        evidence_refs: row.try_get("evidence_refs")?,
        promoted_by: row.try_get("promoted_by")?,
        promoted_at: row.try_get("promoted_at")?,
        rolled_back_by: row.try_get("rolled_back_by")?,
        rolled_back_at: row.try_get("rolled_back_at")?,
        archived_at: row.try_get("archived_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
```

- [ ] **Step 5: Add store helper module**

Create `store_ontology_releases.rs` with `AppState` methods:

```rust
impl AppState {
    pub(crate) async fn list_ontology_releases(&self) -> Result<Vec<OntologyRelease>, AppError>;
    pub(crate) async fn get_ontology_release(&self, id: Uuid) -> Result<OntologyRelease, AppError>;
    pub(crate) async fn active_ontology_release_for_domain(&self, domain_scope: &str) -> Result<Option<OntologyRelease>, AppError>;
    pub(crate) async fn create_ontology_release(&self, release: OntologyRelease) -> Result<OntologyRelease, AppError>;
    pub(crate) async fn update_ontology_release(&self, release: OntologyRelease) -> Result<OntologyRelease, AppError>;
}
```

Use the existing semantic store style: memory branch sorts by `created_at`; Postgres branch filters by `tenant_id = self.current_tenant_id()`.

- [ ] **Step 6: Register the module**

In `main.rs`, add:

```rust
mod store_ontology_releases;
```

- [ ] **Step 7: Verify compile**

Run:

```bash
cargo check --manifest-path crates/mandoforge-api/Cargo.toml
```

Expected: compile succeeds after all Task 1 changes are complete.

- [ ] **Step 8: Commit**

```bash
git add db/migrations/0061_ontology_releases.sql crates/mandoforge-api/src/main.rs crates/mandoforge-api/src/store_backend.rs crates/mandoforge-api/src/store_rows.rs crates/mandoforge-api/src/store_ontology_releases.rs
git commit -m "Add ontology release storage"
```

## Task 2: Create Release Candidate API

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add release candidate tests**

Add two tests:

```rust
#[tokio::test]
async fn ontology_release_candidate_requires_materialized_proposals() {
    let app = test_app().await;
    let headers = [("x-mandoforge-subject", "admin-1"), ("x-mandoforge-roles", "admin")];
    let run: OntologyOnboardingRun = request_json(
        app.clone(),
        json_request_with_headers("POST", "/api/ontology/onboarding/demo-runs", json!({}), &headers),
    ).await;
    let (status, body) = request_value(
        app,
        json_request_with_headers(
            "POST",
            &format!("/api/ontology/onboarding/runs/{}/release-candidate", run.id),
            json!({}),
            &headers,
        ),
    ).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap_or_default().contains("approved materialized"));
}
```

```rust
#[tokio::test]
async fn ontology_release_candidate_captures_materialized_proposals() {
    let state = test_state();
    let run = create_demo_ontology_onboarding_run_for_test(&state).await.expect("run");
    let object = run.proposals.iter().find(|proposal| proposal.proposal_type == "object").expect("object proposal");
    let action = run.proposals.iter().find(|proposal| proposal.proposal_type == "action").expect("action proposal");
    review_ontology_onboarding_proposal_for_test(&state, object.id, "approve", Some("release object")).await.expect("approve object");
    review_ontology_onboarding_proposal_for_test(&state, action.id, "approve", Some("release action")).await.expect("approve action");
    let materialized = materialize_ontology_onboarding_run_for_test(&state, run.id).await.expect("materialize");
    assert!(materialized.semantic_object_count >= 1);
    let release = create_ontology_release_candidate_with_actor(
        &state,
        run.id,
        CreateOntologyReleaseCandidateRequest {
            version: Some("commerce-vtest-001".to_string()),
            migration_policy: Some(default_ontology_release_migration_policy()),
            release_class: None,
        },
        "test",
    ).await.expect("candidate");
    assert_eq!(release.status, "candidate");
    assert_eq!(release.domain_scope, "commerce");
    assert_eq!(release.source_run_id, Some(run.id));
    assert!(release.object_count >= 1);
    assert!(release.action_count >= 1);
    assert!(release.materialized_object_ids.as_array().is_some_and(|ids| !ids.is_empty()));
}
```

- [ ] **Step 2: Add request struct**

```rust
#[derive(Debug, Clone, Deserialize)]
struct CreateOntologyReleaseCandidateRequest {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    migration_policy: Option<Value>,
    #[serde(default)]
    release_class: Option<String>,
}
```

- [ ] **Step 3: Add routes and handlers**

Wire:

```rust
.route("/api/ontology/releases", get(list_ontology_releases))
.route("/api/ontology/releases/{id}", get(get_ontology_release))
.route(
    "/api/ontology/onboarding/runs/{id}/release-candidate",
    post(create_ontology_release_candidate),
)
```

All handlers must call `authorize_request` with `Permission::Admin`, resource
type `ontology_release`, and the release or run id as the resource id when one is
available.

- [ ] **Step 4: Implement candidate helper**

Implement `create_ontology_release_candidate_with_actor`. It must:

- read the onboarding run
- collect materialized onboarding proposal semantic objects for that run
- reject empty materialized proposal sets with `AppError::bad_request("ontology release candidate requires approved materialized proposals")`
- count object, relation, and action proposals
- set `release_class` to `repo_controlled` by default
- store audit action `ontology_release.candidate_created`

- [ ] **Step 5: Run tests and commit**

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release_candidate -- --nocapture
git add crates/mandoforge-api/src/main.rs
git commit -m "Add ontology release candidate API"
```

## Task 3: Add Gate, Promote, Rollback, Archive

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add lifecycle tests**

Add tests:

```rust
#[tokio::test]
async fn ontology_release_gate_requires_migration_policy() {
    let state = test_state();
    let release = ontology_release_candidate_for_test(&state, "commerce-vtest-gate-missing-policy").await;
    let mut release_without_policy = release.clone();
    release_without_policy.migration_policy = json!({});
    state.update_ontology_release(release_without_policy).await.expect("clear policy");
    let gated = gate_ontology_release_with_actor(&state, release.id, "test").await.expect("gate response");
    assert_eq!(gated.status, "failed_gate");
    assert_eq!(gated.gate_result["status"], "failed");
}
```

```rust
#[tokio::test]
async fn ontology_release_promote_supersedes_previous_active_release() {
    let state = test_state();
    let first = ontology_release_candidate_for_test(&state, "commerce-vtest-001").await;
    gate_ontology_release_with_actor(&state, first.id, "test").await.expect("gate first");
    let first_active = promote_ontology_release_with_actor(&state, first.id, "test").await.expect("promote first");
    assert_eq!(first_active.status, "active");
    let second = ontology_release_candidate_for_test(&state, "commerce-vtest-002").await;
    gate_ontology_release_with_actor(&state, second.id, "test").await.expect("gate second");
    let second_active = promote_ontology_release_with_actor(&state, second.id, "test").await.expect("promote second");
    assert_eq!(second_active.status, "active");
    assert_eq!(second_active.rollback_target_release_id, Some(first_active.id));
    let old = state.get_ontology_release(first_active.id).await.expect("old release");
    assert_eq!(old.status, "superseded");
}
```

```rust
#[tokio::test]
async fn ontology_release_rollback_restores_previous_release_without_deleting_semantics() {
    let state = test_state();
    let first = ontology_release_candidate_for_test(&state, "commerce-vtest-rollback-001").await;
    gate_ontology_release_with_actor(&state, first.id, "test").await.expect("gate first");
    let first_active = promote_ontology_release_with_actor(&state, first.id, "test").await.expect("promote first");
    let before_objects = state.list_semantic_objects().await.expect("objects before").len();
    let second = ontology_release_candidate_for_test(&state, "commerce-vtest-rollback-002").await;
    gate_ontology_release_with_actor(&state, second.id, "test").await.expect("gate second");
    let second_active = promote_ontology_release_with_actor(&state, second.id, "test").await.expect("promote second");
    rollback_ontology_release_with_actor(&state, second_active.id, "test").await.expect("rollback");
    let active = state.active_ontology_release_for_domain("commerce").await.expect("active").expect("active release");
    assert_eq!(active.id, first_active.id);
    let after_objects = state.list_semantic_objects().await.expect("objects after").len();
    assert_eq!(before_objects, after_objects);
}
```

- [ ] **Step 2: Add lifecycle routes**

```rust
.route("/api/ontology/releases/{id}/gate", post(gate_ontology_release))
.route("/api/ontology/releases/{id}/promote", post(promote_ontology_release))
.route("/api/ontology/releases/{id}/rollback", post(rollback_ontology_release))
.route("/api/ontology/releases/{id}/archive", post(archive_ontology_release))
```

- [ ] **Step 3: Implement gate helper**

`gate_ontology_release_with_actor` must:

- accept `candidate` or `failed_gate`
- fail if `migration_policy.compatibility` or `migration_policy.rollback` is missing
- fail if materialized object ids are empty
- set rollback target to current active release for the same domain when one exists
- set `gate_result.status` to `passed` or `failed`
- append `ontology_release.gated`

- [ ] **Step 4: Implement promotion helper**

`promote_ontology_release_with_actor` must:

- require candidate status
- require `gate_result.status == "passed"`
- mark current active same-domain release as `superseded`
- set candidate status to `active`
- set `promoted_by` and `promoted_at`
- append `ontology_release.promoted`

- [ ] **Step 5: Implement rollback/archive helpers**

Rollback must activate `rollback_target_release_id`, mark current release `rolled_back`, and append `ontology_release.rolled_back`. Archive must reject active releases, set status `archived`, set `archived_at`, and append `ontology_release.archived`.

- [ ] **Step 6: Run tests and commit**

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release -- --nocapture
git add crates/mandoforge-api/src/main.rs
git commit -m "Add ontology release lifecycle"
```

## Task 4: Wire Runtime Ontology Release Pinning

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`

- [ ] **Step 1: Add render test**

Add:

```rust
#[tokio::test]
async fn context_packet_render_includes_active_ontology_release() {
    let app = test_app().await;
    let headers = [("x-mandoforge-subject", "admin-1"), ("x-mandoforge-roles", "admin")];
    let state = app_state_from_test_app(&app);
    let release = ontology_release_candidate_for_test(&state, "commerce-vtest-render").await;
    gate_ontology_release_with_actor(&state, release.id, "test").await.expect("gate");
    let active = promote_ontology_release_with_actor(&state, release.id, "test").await.expect("promote");
    let packet = create_context_packet_for_test(&state, json!({"domain_scope": "commerce", "memory_scope": "commerce"})).await;
    let rendered: RenderedExecutionContext = request_json(
        app,
        json_request_with_headers("POST", &format!("/api/context-packets/{}/render", packet.id), json!({}), &headers),
    ).await;
    assert_eq!(rendered.ontology_scope["ontology_release"]["id"], json!(active.id));
    assert_eq!(rendered.ontology_scope["ontology_release"]["status"], "active");
}
```

If `app_state_from_test_app` or `create_context_packet_for_test` do not exist, implement equivalent private test helpers using existing test app/state setup patterns.

- [ ] **Step 2: Add async render wrapper**

Patch `render_context_packet` so it calls an async wrapper that:

- renders existing context fields through the existing renderer
- reads active ontology release by `domain_scope`, then `memory_scope`, then `workflow_scope`
- inserts release metadata into `rendered.ontology_scope["ontology_release"]`

- [ ] **Step 3: Verify provider filtering still holds**

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml workflow_step_run_endpoint_claims_and_executes_session_loop -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add crates/mandoforge-api/src/main.rs
git commit -m "Pin ontology release in rendered context"
```

## Task 5: Update Readiness And Gates

**Files:**
- Modify: `crates/mandoforge-api/src/main.rs`
- Modify: `scripts/ontology-engine-readiness-gate.sh`
- Create: `scripts/ontology-release-loop-gate.sh`

- [ ] **Step 1: Make readiness state-aware**

Change `get_ontology_engine_readiness` to call an async `build_ontology_engine_readiness(&state)`. Keep the empty-store baseline blocked. When at least one active release exists:

- `domain-ontology-lifecycle` is `ready`
- `approved-release-materialization` is `ready` if active release has non-empty materialized ids and `gate_result.status == "passed"`
- `migration-policy` is `ready` if active release has `migration_policy.compatibility` and `migration_policy.rollback`
- `conflict-trust-runtime-gates` stays `pilot_ready`

- [ ] **Step 2: Add readiness test**

Add:

```rust
#[tokio::test]
async fn ontology_engine_readiness_uses_promoted_release_evidence() {
    let state = test_state();
    let release = ontology_release_candidate_for_test(&state, "commerce-vtest-readiness").await;
    gate_ontology_release_with_actor(&state, release.id, "test").await.expect("gate");
    promote_ontology_release_with_actor(&state, release.id, "test").await.expect("promote");
    let readiness = build_ontology_engine_readiness(&state).await.expect("readiness");
    assert_eq!(ontology_readiness_check_status(&readiness, "domain-ontology-lifecycle"), "ready");
    assert_eq!(ontology_readiness_check_status(&readiness, "approved-release-materialization"), "ready");
    assert_eq!(ontology_readiness_check_status(&readiness, "migration-policy"), "ready");
    assert_eq!(ontology_readiness_check_status(&readiness, "conflict-trust-runtime-gates"), "pilot_ready");
    assert_eq!(readiness.status, "blocked");
}
```

- [ ] **Step 3: Add release loop script**

Create executable `scripts/ontology-release-loop-gate.sh` that follows the auth pattern from `scripts/ontology-engine-readiness-gate.sh`, writes evidence under `.mandoforge/ontology-release-loop/`, and exercises:

1. demo run creation
2. proposal approvals
3. materialization
4. release candidate creation
5. gate
6. promote
7. second candidate/gate/promote
8. rollback
9. readiness readback

- [ ] **Step 4: Update readiness script output**

In `scripts/ontology-engine-readiness-gate.sh`, print non-ready and release-backed checks with:

```bash
jq -r '.checks[]? | "- \(.id)=\(.status) evidence=\(.current_evidence_class)"' "$readiness_file"
```

- [ ] **Step 5: Run tests and commit**

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_engine_readiness -- --nocapture
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release -- --nocapture
git add crates/mandoforge-api/src/main.rs scripts/ontology-engine-readiness-gate.sh scripts/ontology-release-loop-gate.sh
git commit -m "Report ontology release readiness"
```

## Task 6: Documentation And Final Verification

**Files:**
- Modify: `docs/ontology-builder-usage.md`

- [ ] **Step 1: Document operator flow**

Add a `## Release Loop` section after the materialization/tool-spec commands. Include commands for:

- `POST /api/ontology/onboarding/runs/$run_id/release-candidate`
- `POST /api/ontology/releases/$release_id/gate`
- `POST /api/ontology/releases/$release_id/promote`
- `POST /api/ontology/releases/$release_id/rollback`
- `GET /api/ontology/releases`

- [ ] **Step 2: Run full targeted verification**

```bash
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_release -- --nocapture
cargo test --manifest-path crates/mandoforge-api/Cargo.toml ontology_onboarding -- --nocapture
cargo test --manifest-path crates/mandoforge-api/Cargo.toml workflow_step_run_endpoint_claims_and_executes_session_loop -- --nocapture
cargo check --manifest-path web-ui/Cargo.toml
git diff --check
```

- [ ] **Step 3: Commit docs**

```bash
git add docs/ontology-builder-usage.md
git commit -m "Document ontology release loop usage"
```

- [ ] **Step 4: Final status**

Report commits created, tests run, remaining dirty files, and whether `/api/ontology/engine-readiness` is still expected to be blocked because conflict/trust remains pilot-ready.

## Risks And Mitigations

- Risk: `main.rs` is very large and release code could worsen it. Mitigation: keep store code in `store_ontology_releases.rs`; only route/handler and test glue stays in `main.rs` for this slice.
- Risk: readiness could overstate enterprise completion. Mitigation: keep `conflict-trust-runtime-gates` pilot-ready unless customer-grade workflow evidence exists.
- Risk: release promotion could imply business action execution. Mitigation: release promotion only changes ontology version metadata; provider tools and high-risk actions remain governed by existing TaskGrant and approval policy.
- Risk: Postgres/in-memory behavior could drift. Mitigation: store helper methods mirror existing semantic store style; tests exercise memory and migration is included for Postgres.
