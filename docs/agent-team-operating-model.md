# Agent Team Operating Model

This document defines how MandoForge should use parallel agents after Stage 2. The goal is faster delivery without turning the repo into a merge-conflict factory.

## Default Shape

Use five parallel development lanes plus one integration owner.

The rule is simple:

```text
Lane agents own bounded modules and evidence gates.
The integration owner owns shared wiring and mainline health.
```

Avoid having multiple agents edit `crates/mandoforge-api/src/main.rs`, shared `AppState` fields, migration numbering, or global readiness summaries at the same time.

## Lanes

### Lane A: Identity / Tenant / RBAC

Owns:

- `crates/mandoforge-api/src/authorization.rs`
- `crates/mandoforge-api/src/store_governance.rs`
- tenant/RLS migrations
- tenant isolation readiness and evidence gates
- organization, team, project, membership, invitation flows

Good tasks:

- scope enforcement
- membership and invitation APIs
- tenant readiness fields
- tenant evidence scripts

Avoid:

- worker execution
- Remote Computer state sync
- provider rollout

### Lane B: Policy / Approval / Release Control

Owns:

- `crates/mandoforge-api/src/policy.rs`
- `crates/mandoforge-api/src/store_approvals.rs`
- `crates/mandoforge-api/src/store_approval_groups.rs`
- `crates/mandoforge-api/src/store_approval_notification_channels.rs`
- `crates/mandoforge-api/src/store_policy_revisions.rs`
- `crates/mandoforge-api/src/store_releases.rs`
- policy, approval, and eval/release evidence gates

Good tasks:

- approval state machine work
- policy rollout
- release orchestration
- notification escalation

Avoid:

- worker execution internals
- MCP/provider adapter implementation

### Lane C: Execution / Worker / Codex

Owns:

- `crates/mandoforge-api/src/execution.rs`
- `crates/mandoforge-api/src/execution_queue.rs`
- `crates/mandoforge-api/src/execution_queue_broker.rs`
- `crates/mandoforge-api/src/shell_runner.rs`
- `crates/mandoforge-api/src/codex_app_server.rs`
- `crates/mandoforge-api/src/store_codex_app_server.rs`
- `crates/mandoforge-api/src/bin/mandoforge-worker.rs`
- worker and Codex evidence gates

Good tasks:

- queue backends
- lease and retry handling
- worker readiness
- Codex CLI/App Server execution paths

Avoid:

- approval policy semantics
- tenant scope
- Remote Computer state synchronization

### Lane D: Remote Computer

Owns:

- `crates/mandoforge-api/src/remote_computer_runner.rs`
- `crates/mandoforge-api/src/store_remote_computers.rs`
- Remote Computer migrations
- `deploy/k8s/*remote-computer*`
- Remote Computer evidence gates

Good tasks:

- leases
- attachments
- state locks
- sidecar heartbeat/recovery
- warm pool
- state-sync controller boundaries

Avoid:

- generic shell/Codex worker internals
- provider governance

### Lane E: Provider / MCP / Secrets / Ops Signals

Owns:

- `crates/mandoforge-api/src/provider.rs`
- `crates/mandoforge-api/src/mcp_gateway.rs`
- `crates/mandoforge-api/src/secrets.rs`
- `crates/mandoforge-api/src/eval_judge.rs`
- `crates/mandoforge-api/src/observability.rs`
- `crates/mandoforge-api/src/store_secret_records.rs`
- `crates/mandoforge-api/src/store_eval.rs`
- `crates/mandoforge-api/src/store_usage_rollups.rs`
- `crates/mandoforge-api/src/store_cost_alert_routes.rs`
- provider, MCP, secrets, observability, scheduler, and release/eval gates

Good tasks:

- external control-plane boundaries
- health checks
- model/provider governance
- usage, cost, and optional export surfaces
- OTel readiness

Avoid:

- tenant/RBAC core
- worker kernel
- Remote Computer lifecycle

## Shared Hotspots

Treat these as integration-owner territory:

- `crates/mandoforge-api/src/main.rs`
- shared request/response types
- `AppState`
- global readiness summaries
- migration numbering
- cross-lane status enums
- repo-wide CI/evidence workflow changes

Lane agents may propose patches that touch hotspots, but the integration owner should merge and verify those changes serially.

## Branch And Commit Rules

Use branch names:

```text
lane-a/<short-topic>
lane-b/<short-topic>
lane-c/<short-topic>
lane-d/<short-topic>
lane-e/<short-topic>
integration/<date-or-topic>
```

Keep commits atomic. A lane commit should represent one coherent capability, gate, migration, or UI surface.

Do not bundle unrelated cleanup with feature work.

## Verification Gates

### Lane-Local Gate

Every lane runs before handoff:

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api
```

Also run the lane-specific core verifier for the area being changed.

Examples:

```bash
BASE_URL=http://127.0.0.1:8787 scripts/agent-os-core-evidence-gate.sh
scripts/verify-codex-exec-adapter.sh
scripts/verify-remote-computer-k8s-manifests.sh
```

### Integration Gate

The integration owner runs:

```bash
cargo test -p mandoforge-api -- --test-threads=1
scripts/stage1-final-gate.sh
```

Deployment-specific evidence scripts are outside the main integration gate. Run
them only when the change explicitly modifies that script family.

### Mainline Gate

Before pushing main:

```bash
cargo test --workspace --locked --all-targets -- --test-threads=1
scripts/verify-observability-k8s-manifests.sh
scripts/verify-remote-computer-k8s-manifests.sh
kubectl kustomize deploy/k8s >/tmp/mandoforge-kustomize.out
```

## Suitable Parallel Work

- additive APIs inside one lane
- store methods owned by one lane
- lane-specific core verifiers
- K8s manifests owned by one lane
- fail-closed validation within one capability
- docs that describe a specific lane-owned capability

## Unsuitable Parallel Work

- large `main.rs` route rewires
- session event contract changes
- approval status enum changes
- migration renumbering
- shared `AppState` redesign
- repo-wide readiness JSON shape changes
- broad formatting or naming cleanup

## Daily Cadence

1. Pick lane tasks with disjoint file ownership.
2. Let lane agents implement and verify locally.
3. Integration owner reviews lane diffs and resolves hotspots once per batch.
4. Push only after mainline gates or equivalent CI coverage pass.

If a task touches three or more lanes, split it into:

```text
contract patch -> lane implementations -> integration patch
```

Do not let multiple agents independently invent the same shared contract.
