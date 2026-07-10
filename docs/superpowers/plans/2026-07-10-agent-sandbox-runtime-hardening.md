# Agent Sandbox Runtime Hardening Implementation Plan

> **For agentic workers:** Use `superpowers:subagent-driven-development` and
> complete each task with an implementer review and a task-scoped code review.

**Goal:** Make the opt-in Agent Sandbox path a correct, governed MandoForge
execution substrate without moving session, policy, approval, lease, event, or
audit truth into Kubernetes.

**Architecture:** Keep the existing execution queue, Environment contract,
Remote Computer records, leases, assignments, Tool Router, and audit flow. Add
one versioned runtime identity in existing Remote Computer metadata, correct the
Claim -> Sandbox -> Pod binding, use a fixed stdin-driven launcher for Pod exec,
and route every cleanup through the persisted substrate. Keep the pilot behind
the existing mutation/execution gates and keep production readiness blocked
until a live-cluster evidence bundle exists.

**Tech stack:** Rust 2024, Axum/SQLx, reqwest, tokio-tungstenite, serde,
Kubernetes Agent Sandbox v1beta1, Kustomize, Bash, Docker.

**Approved design:**
`docs/superpowers/specs/2026-07-10-agent-sandbox-runtime-hardening-design.md`

## Global Constraints

- MandoForge API/DB remains authoritative for authorization, sessions, events,
  tool calls, approvals, leases, assignments, artifacts, and audit.
- Agent Sandbox is only an Environment-selected execution substrate.
- Keep the existing four runtime gates fail closed. Do not enable Agent Sandbox
  in the default ConfigMap.
- Do not add a second control plane, RPC service, database migration, or new
  dependency unless the current Rust/Kubernetes stack cannot express the need.
- Preserve legacy Kubernetes Pod mode and legacy Remote Computer metadata.
- No prompt, file content, shell script, token, or managed runtime environment
  value may appear in a Pod exec URI, event payload, or audit projection.
- Shared storage may contain dependency and content-addressed caches only.
  `CARGO_TARGET_DIR` and agent state remain session-private.
- High-risk business writes remain approval-gated.
- Static renders prove wiring only. Production readiness remains blocked until
  the live Agent Sandbox drill succeeds and its artifacts are recorded.

## Acceptance Criteria

1. A `SandboxClaim` resolves `status.sandbox.name` (or the documented claim
   annotation fallback), then resolves the Sandbox Pod annotation; generated
   warm-pool Sandbox names work.
2. Claim requests contain propagated tracking metadata and a bounded lifecycle
   deadline; create 409 and delete 404/410 converge idempotently.
3. `remote_computers.metadata.runtime_identity` records schema version,
   substrate, namespace, claim/Sandbox/Pod names, and lifecycle deadline.
4. Delete dispatch is based on persisted identity, even when the process-level
   runner mode later changes.
5. Cancel, expired-lease reclaim, terminal-session cleanup, repeated cleanup,
   and later re-provisioning converge Remote Computer, lease, assignment,
   Kubernetes resource, event, and audit state.
6. Pod exec URIs contain only
   `/usr/local/bin/mandoforge-sandbox-runtime execute-json`; one bounded JSON
   envelope is sent through WebSocket stdin.
7. The launcher rejects oversized input, paths outside
   `/workspace/sessions/<uuid>`, invalid environment keys, unsupported
   operations, and invalid file-write paths.
8. The dedicated image contains the launcher and pinned CLI/toolchain profile,
   seeds a clean source snapshot once per session, shares dependency/sccache
   stores, and keeps Cargo target output in the session workspace.
9. Agent Sandbox egress is deny-by-default plus DNS, MandoForge API TCP 8787,
   and external TCP 443 excluding private/link-local/metadata CIDRs.
10. Rust tests, formatting, shell syntax, manifest verifiers, Kustomize renders,
    static production preflight, and the available live-cluster drill pass.

## Task 1: Correct Agent Sandbox Binding And Runtime Identity

**Files:**

- Modify: `crates/mandoforge-api/src/remote_computer_runner.rs:695-811`
- Modify: `crates/mandoforge-api/src/remote_computer_runner.rs:1329-1407`
- Modify: `crates/mandoforge-api/src/remote_computer_runtime.rs:1-47`
- Modify: `crates/mandoforge-api/src/execution.rs:1200-1609`
- Modify: `crates/mandoforge-api/src/store_environments.rs:452-487`
- Test: `crates/mandoforge-api/src/remote_computer_runner.rs:1692`
- Test: `crates/mandoforge-api/src/main_tests/environment_tests.rs`
- Test: `crates/mandoforge-api/src/main_tests/remote_computer_execution_tests.rs`

- [ ] Add failing tests for generated Sandbox names, claim annotation fallback,
      controller terminal conditions, create 409, delete 404/410, TTL fields,
      additional Pod metadata, and legacy/runtime-identity decoding.
- [ ] Add the smallest shared `RemoteComputerRuntimeIdentity` model in
      `remote_computer_runtime.rs`; serialize it into existing metadata under a
      versioned key and retain legacy `sandbox_claim_name` fallback parsing.
- [ ] Change binding discovery to GET the Claim, resolve the Sandbox name, GET
      the Sandbox, resolve its Pod, then use the existing Pod Running poll.
- [ ] Add claim lifecycle deadline and propagated labels/annotations. Source
      namespace, warm pool, cache scope, and workspace seed from the bound
      Environment profile with validated global fallbacks.
- [ ] Persist the complete identity before lease creation and select create and
      delete API paths from identity/substrate, not mutable global mode.
- [ ] Run:
      `cargo test -p mandoforge-api remote_computer_runner::tests -- --nocapture`
- [ ] Run:
      `cargo test -p mandoforge-api environment_tests -- --nocapture`
- [ ] Commit only Task 1 files with message
      `Harden Agent Sandbox resource identity`.

## Task 2: Converge Lease, Assignment, And Runtime Cleanup

**Files:**

- Modify: `crates/mandoforge-api/src/remote_computer_runtime.rs`
- Modify: `crates/mandoforge-api/src/store_remote_computers.rs:19-380`
- Modify: `crates/mandoforge-api/src/execution.rs:996-1613`
- Modify: `crates/mandoforge-api/src/handlers/execution_jobs.rs:500-592`
- Modify: `crates/mandoforge-api/src/remote_computer_supervision_runtime.rs:59-192`
- Modify as required by the existing terminal-session route:
  `crates/mandoforge-api/src/handlers/sessions.rs`
- Test: `crates/mandoforge-api/src/main_tests/remote_computer_lease_tests.rs`
- Test: `crates/mandoforge-api/src/main_tests/remote_computer_lifecycle_tests.rs`
- Test: `crates/mandoforge-api/src/main_tests/remote_computer_execution_tests.rs`

- [ ] Add failing tests for cancel, repeated cancel, delete failure retry,
      expired reclaim, terminal-session cleanup, global mode changes, and
      rebind of an `attention` on-demand record.
- [ ] Reuse one identity-based cleanup helper from provisioning rollback,
      cancel, stale reclaim, and terminal-session cleanup.
- [ ] On cleanup success, transition the lease and assignment and mark the
      backing Remote Computer `attention`; preserve history and emit sanitized
      lifecycle evidence.
- [ ] On cleanup failure, record the failure and leave state retryable instead
      of swallowing it or claiming convergence.
- [ ] Allow deterministic on-demand records whose backing resource is absent to
      be rebound after a replacement Pod reaches Running.
- [ ] Ensure lease-creation failure removes only a resource created by the
      current attempt and preserves primary plus cleanup error details.
- [ ] Run:
      `cargo test -p mandoforge-api remote_computer_lease_tests -- --nocapture`
- [ ] Run:
      `cargo test -p mandoforge-api remote_computer_lifecycle_tests -- --nocapture`
- [ ] Run:
      `cargo test -p mandoforge-api remote_computer_execution_tests -- --nocapture`
- [ ] Commit only Task 2 files with message
      `Converge Remote Computer runtime cleanup`.

## Task 3: Move Pod Exec Payloads To A Fixed Stdin Protocol

**Files:**

- Create: `crates/mandoforge-api/src/sandbox_runtime_protocol.rs`
- Create: `crates/mandoforge-api/src/bin/mandoforge-sandbox-runtime.rs`
- Modify: `crates/mandoforge-api/src/remote_computer_runner.rs:842-1110`
- Modify: `crates/mandoforge-api/src/remote_computer_runtime.rs:9-41`
- Modify: `crates/mandoforge-api/src/execution.rs:1783-2730`
- Test: `crates/mandoforge-api/src/remote_computer_runner.rs`
- Test: colocated tests in `mandoforge-sandbox-runtime.rs` or the shared
  protocol module
- Test: `crates/mandoforge-api/src/main_tests/remote_computer_execution_tests.rs`

- [ ] Add failing tests proving sensitive sentinel strings are absent from the
      exec URI and audit response while the WebSocket receives them on stdin.
- [ ] Define one bounded serde envelope for `shell`, `file_write`, `codex`, and
      `agent_cli` execution using existing tool-call inputs; avoid a generic
      extensibility layer.
- [ ] Make the exec URI use fixed launcher argv and `stdin=true`; send one
      channel-0 binary frame containing newline-terminated JSON.
- [ ] Implement the launcher with standard library path checks and Tokio
      process execution. Validate the workspace UUID path, operation, timeout,
      output/input limits, environment names, and file-write relative path.
- [ ] Seed `/workspace/sessions/<session_id>` atomically from
      `/opt/mandoforge-source` when absent, then reuse it for the session.
- [ ] Redact the request metadata and exec output from runner responses used by
      events/audit while retaining lengths, status, and identifiers.
- [ ] Run:
      `cargo test -p mandoforge-api sandbox_runtime -- --nocapture`
- [ ] Run:
      `cargo test -p mandoforge-api remote_computer_execution_tests -- --nocapture`
- [ ] Commit only Task 3 files with message
      `Secure Agent Sandbox exec transport`.

## Task 4: Add The Dedicated Runtime Image And Isolation Manifests

**Files:**

- Create: `Dockerfile.agent-sandbox`
- Modify: `.dockerignore` only if required to exclude local state/build output
- Modify: `deploy/k8s/agent-sandbox-runtime.yaml:1-163`
- Create: `deploy/k8s/agent-sandbox-egress-networkpolicy.yaml`
- Modify: `deploy/agent-sandbox-pilot/kustomization.yaml:1-6`
- Modify: `deploy/agent-sandbox-smoke/sandbox-claim.yaml:1-11`
- Modify: `scripts/verify-remote-computer-k8s-manifests.sh`

- [ ] Pin the Rust, Node, pnpm, uv, sccache, Codex, and Claude Code versions in
      the dedicated image using upstream release sources; do not reuse the API
      image or install tools at Pod startup.
- [ ] Copy the launcher and a clean source seed into the image. Confirm target,
      credentials, `.mandoforge`, and host `.git` state are excluded; initialize
      a fresh non-credentialed Git snapshot for agent diff inspection.
- [ ] Keep Cargo registry/git, sccache, pnpm, and uv stores on the project cache
      PVC. Remove shared `CARGO_TARGET_DIR`; let the launcher set it under the
      session workspace and set `RUSTC_WRAPPER=sccache`.
- [ ] Align all workspace/artifact paths to `/workspace/sessions/<session_id>`
      or disable a sidecar path that cannot be safely session-bound.
- [ ] Add additive Agent Sandbox egress policy for cluster DNS, MandoForge API,
      and external HTTPS with private/link-local/metadata exclusions. Keep the
      existing deny-by-default policy.
- [ ] Extend the existing manifest verifier instead of creating another static
      gate. Assert image separation, launcher presence, private Cargo target,
      cache scope, network rules, lifecycle, and absence of a default live
      claim.
- [ ] Run `bash -n scripts/verify-remote-computer-k8s-manifests.sh`.
- [ ] Run `bash scripts/verify-remote-computer-k8s-manifests.sh`.
- [ ] Build and smoke the runtime image when Docker is available; otherwise
      record the unavailable runtime as missing evidence, not a pass.
- [ ] Commit only Task 4 files with message
      `Add dedicated Agent Sandbox runtime image`.

## Task 5: Complete Readiness, Documentation, And Evidence Gates

**Files:**

- Modify: `crates/mandoforge-api/src/remote_computer_readiness.rs`
- Modify: `crates/mandoforge-api/src/main_tests.rs` readiness assertions
- Modify: `docs/agent-remote-computer-plan.md`
- Create: `docs/runbooks/agent-sandbox-runtime-drill.md`
- Modify: `scripts/production-launch-preflight.sh` only if its current output
  could incorrectly promote this lane
- Modify: `scripts/verify-stage2-evidence-k8s-manifests.sh` only for new static
  contract assertions that are not already covered

- [ ] Make readiness distinguish runtime image/static contract readiness from
      live Agent Sandbox lifecycle evidence. Keep status blocked or pilot-only
      while the live evidence bundle is absent.
- [ ] Document Environment profile fields, cache/tenant scope, FQDN NetworkPolicy
      limitation, TTL behavior, cleanup/retry semantics, and rollback.
- [ ] Add a live drill runbook that records controller version, cluster context,
      generated Claim/Sandbox/Pod names, startup timing, workspace reuse,
      cross-session isolation, approved exec, cancel, expiry, retry, network,
      cache, event, artifact, and audit evidence.
- [ ] Run the full required static suite:

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api --bins
cargo test -p mandoforge-api
bash -n scripts/verify-remote-computer-k8s-manifests.sh
bash -n scripts/verify-stage2-evidence-k8s-manifests.sh
bash -n scripts/production-launch-preflight.sh
bash scripts/verify-remote-computer-k8s-manifests.sh
bash scripts/verify-stage2-evidence-k8s-manifests.sh
STATIC_ONLY=1 bash scripts/production-launch-preflight.sh
git diff --check
```

- [ ] If a configured cluster with Agent Sandbox CRDs is reachable, execute the
      runbook and save tenant-safe evidence under the repo's existing evidence
      conventions. If not reachable, keep the goal and readiness blocked and
      report the exact missing external evidence.
- [ ] Commit Task 5 with message
      `Document Agent Sandbox runtime evidence`.

## Deep Review And Completion Audit

- [ ] Generate a branch review package from base `4f87d1b9` through HEAD.
- [ ] Run an independent whole-branch code review covering correctness,
      authorization, secret handling, failure ordering, retries, compatibility,
      manifest isolation, and test adequacy.
- [ ] Fix every Critical or Important finding and re-review.
- [ ] Compare current files and command outputs against all ten Acceptance
      Criteria; treat missing live-cluster evidence as incomplete.
- [ ] Use `superpowers:finishing-a-development-branch` only after the static
      suite and independent review are clean.

## Risks And Mitigations

- **Kubernetes API drift:** Assert only current v1beta1 fields used by upstream
  Agent Sandbox and preserve raw HTTP status in errors.
- **Cleanup destroys reusable capacity:** Delete only records marked on-demand;
  pooled resources release leases without deleting shared backing capacity.
- **Config drift deletes the wrong resource:** Reconstruct runner mode and
  namespace from persisted runtime identity.
- **Input leaks through diagnostics:** Use a fixed URI, redact request metadata,
  and test with sentinel secrets.
- **Shared caches become an isolation claim:** Label them project-scoped, keep
  mutable targets private, and keep multi-tenant readiness blocked.
- **Static green is mistaken for production:** Require explicit live evidence
  fields and keep the promotion gate blocked when absent.
