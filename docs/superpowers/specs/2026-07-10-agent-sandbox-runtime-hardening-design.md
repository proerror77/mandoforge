# Agent Sandbox Runtime Hardening Design

## Purpose

This design closes the gap between the current Agent Sandbox adapter pilot and
a usable, governed remote CLI runtime.

MandoForge remains the runtime truth and authority layer. It owns sessions,
event cursors, tool calls, policy, approvals, TaskGrants, execution jobs,
Remote Computer leases, artifacts, and audit logs. Kubernetes SIG Agent
Sandbox owns only the lower-level isolated environment lifecycle.

The target result is:

```text
approved execution job
  -> Environment-bound sandbox allocation
  -> isolated session workspace
  -> fixed in-pod runtime launcher
  -> CLI or tool execution
  -> normalized output, artifacts, events, and audit
  -> idempotent cancel, expiry, retry, and cleanup
```

The code path may become production-capable, but it remains opt-in and must not
be reported as production-ready until live-cluster evidence proves the full
lifecycle.

## Repository Boundary

The implementation must preserve these existing product invariants:

- `session_events` is the durable execution timeline.
- `tool_calls` records every governed tool action.
- `audit_logs` records decisions and side effects.
- `Environment` selects execution placement and runtime profile.
- `TaskGrant`, Policy, Approval, and Tool Router decide whether work may run.
- K Agent, Remote Computer, Kubernetes Pod, and Agent Sandbox are execution
  substrates. They do not own business authority or replay truth.
- High-risk business writes remain approval-gated.
- Static manifests and mocks prove wiring only. Customer-grade readiness needs
  live evidence from the target cluster.

## Current Failure Modes

The current pilot has ten connected problems:

1. It assumes a `Sandbox` has the same name as its `SandboxClaim`, which is not
   true when a warm-pool Sandbox with a generated name is adopted.
2. The configured image is the API image and does not contain Codex, Claude,
   Git, Cargo, pnpm, uv, sccache, or a project workspace bootstrap.
3. Full shell scripts, prompts, file contents, and managed runtime environment
   values are encoded into the Kubernetes `pods/exec` request URI.
4. The matching Remote Computer NetworkPolicy denies all sandbox egress.
5. Cancel and reclaim can delete Kubernetes resources without converging the
   assignment, lease, and Remote Computer state.
6. Claims have no lifecycle deadline, so a worker crash before DB persistence
   can leak a Sandbox indefinitely.
7. A shared mutable `CARGO_TARGET_DIR` crosses sandbox boundaries.
8. Delete retries treat an already-absent resource as failure.
9. Cleanup dispatch depends on the current global runner mode instead of the
   substrate persisted with the resource.
10. Template workspace paths differ from the session-scoped paths used by the
    execution engine.

These are one lifecycle and execution-boundary problem, not ten independent
patches.

## Selected Approach

Harden the existing Kubernetes execution adapter instead of introducing a
second control plane or replacing it with an external sandbox RPC service.

This approach keeps the current queue, lease, approval, event, and audit paths.
It adds typed resource identity, correct Agent Sandbox discovery, a fixed
stdin-driven in-pod launcher, a dedicated runtime image, and convergent cleanup.

The alternatives were rejected for this slice:

- A minimal manifest and polling patch would leave the CLI, credential,
  workspace, and lifecycle paths incomplete.
- A new sandbox RPC/router control plane would duplicate authentication,
  service discovery, protocol, and lifecycle state before the current adapter
  has been made correct.

## Architecture

```text
MandoForge execution worker
  -> validate Session / Environment / TaskGrant / Approval
  -> claim or create Remote Computer lease
  -> AgentSandboxRunner.create_claim
  -> GET SandboxClaim until status.sandbox.name is present
  -> GET Sandbox by resolved name
  -> read agents.x-k8s.io/pod-name
  -> persist RuntimeIdentity
  -> pods/exec fixed launcher argv
  -> send execution envelope through WebSocket stdin
  -> ingest stdout / stderr / status
  -> persist events, tool result, artifact, and audit
  -> release or retain lease by session policy
```

The Kubernetes API is never the source of business authorization. It is a
reconciled execution substrate beneath the MandoForge state machine.

## Runtime Identity

Introduce one internal typed identity shared by provisioning, execution,
cancel, stale reclaim, and terminal-session cleanup:

```rust
enum RemoteComputerSubstrate {
    KubernetesPod,
    AgentSandbox,
}

struct RemoteComputerRuntimeIdentity {
    substrate: RemoteComputerSubstrate,
    namespace: String,
    resource_name: String,
    claim_name: Option<String>,
    sandbox_name: Option<String>,
    pod_name: String,
    lifecycle_deadline: Option<DateTime<Utc>>,
}
```

The identity is persisted in `remote_computers.metadata` with a schema version.
Existing `pod_name` remains the direct exec target for backward compatibility.
Legacy records that only contain `sandbox_claim_name` continue to deserialize.

All destructive operations are selected from the persisted substrate. A global
environment-variable change must not cause a SandboxClaim name to be sent to
the Pod delete API or a Pod name to be sent to the SandboxClaim API.

## Agent Sandbox Allocation

### Claim Creation

The claim request includes:

- `warmPoolRef.name` selected by the bound Environment.
- Tenant, project, environment, session, and Remote Computer tracking labels.
- Claim annotations for non-label tracking fields. The pinned `v0.5.1`
  controller rejects custom `mandoforge.io/*` fields in
  `spec.additionalPodMetadata`, so that map remains empty and MandoForge DB plus
  the Claim remain the tracking sources.
- `spec.lifecycle.shutdownTime` calculated from the configured sandbox TTL.
- `spec.lifecycle.shutdownPolicy: Delete`.

The TTL must outlive the initial lease and readiness timeout. It is a final
orphan guard, not a replacement for normal cleanup.

### Binding Discovery

Binding discovery follows the upstream resource contract:

1. GET the `SandboxClaim` by claim name.
2. Read `status.sandbox.name`.
3. Fall back to the claim annotation `agents.x-k8s.io/sandbox-name` during
   controller convergence.
4. GET the resolved `Sandbox` name.
5. Read the Sandbox annotation `agents.x-k8s.io/pod-name`.
6. Poll the resolved Pod until Running.

Claim and Sandbox Ready/Finished conditions are inspected so controller
failures terminate early instead of waiting for the full timeout. A 404 while
the controller is converging is retryable.

### Idempotency

- Create HTTP 409 is treated as a recoverable existing allocation and proceeds
  to discovery.
- Delete HTTP 404 or 410 is treated as successful convergence.
- Other non-2xx responses remain failures with status codes preserved.
- A retry uses the same deterministic claim name for the same active session
  generation.

## Environment Contract

`Environment(type=remote_computer).remote_computer_profile` remains the
placement contract. The Agent Sandbox slice adds validated optional fields:

```json
{
  "profile": "agent-sandbox",
  "namespace": "agent-os",
  "sandbox_warm_pool": "mandoforge-agent-runtime",
  "cache_scope": "mandoforge",
  "workspace_seed": "mandoforge"
}
```

The Environment namespace and warm pool override global defaults for normal
session execution. Global values remain available only for explicit admin smoke
operations and backward-compatible single-environment deployments.

The initial pilot supports one MandoForge project cache. Broad multi-tenant use
requires separate namespaces or warm pools and cache claims per tenant/project.
Labels alone are never described as an isolation boundary.

## Secure Pod Exec Transport

Kubernetes requires exec argv in query parameters. Therefore query argv must be
fixed and contain no user or credential material:

```text
/usr/local/bin/mandoforge-sandbox-runtime execute-json
```

The exec WebSocket enables stdin. MandoForge sends one bounded, newline-
terminated JSON envelope on stdin. The launcher reads exactly one envelope, so
the client does not need to close the stdin channel before receiving output.

The envelope contains the requested working directory, shell command or fixed
tool operation, arguments, and runtime environment. It is carried inside the
TLS-protected WebSocket body and is not included in the Kubernetes request URI,
runner response, event payload, or audit summary.

The launcher validates:

- Envelope size.
- Absolute workspace path under `/workspace/sessions/`.
- Environment variable names.
- Operation allowlist.
- File-write relative paths.
- Process timeout and output bounds.

The existing Tool Router, approval, and TaskGrant checks remain outside the
launcher. The launcher is an execution mechanism, not an authorization layer.

## Dedicated Sandbox Runtime

Add a dedicated runtime image instead of reusing the API image.

The image contains:

- `mandoforge-sandbox-runtime`.
- Bash, Git, CA certificates, curl, and jq.
- The pinned Rust toolchain, Cargo, and sccache.
- A pinned Node runtime and pnpm.
- A pinned uv binary.
- Pinned Codex and Claude Code CLI versions used by released runtime profiles.

The MandoForge pilot image contains a clean source snapshot under
`/opt/mandoforge-source`. It excludes host build output, `.mandoforge` state,
credentials, and the host `.git` directory. Image construction creates a fresh
snapshot repository so coding agents can inspect diffs without receiving host
Git credentials or unrelated history.

On first execution for a session, `mandoforge-sandbox-runtime` copies the seed
into `/workspace/sessions/{session_id}`. Later jobs in the same session reuse
that workspace and lease. Different sessions receive different PVC-backed
directories and agent state.

Future projects should build project-specific Environment images or use a
reviewed source-bootstrap adapter. Dynamic arbitrary Git credential injection
is not part of this slice.

## Cache Isolation

The shared project cache contains only reusable package or content-addressed
caches:

- Cargo registry and Git dependency cache.
- sccache storage.
- pnpm store.
- uv cache.

`CARGO_TARGET_DIR` is moved to the session workspace PVC. The runtime sets
`RUSTC_WRAPPER=sccache`, so compiler outputs can be reused without sharing the
mutable Cargo target tree.

The current static PVC is explicitly MandoForge-project scoped. A different
tenant or project must use a different SandboxTemplate, WarmPool, and cache PVC
before the feature can be enabled for that scope.

## Network Policy

The deny-by-default policy remains the base. A separate Agent Sandbox egress
policy permits only:

- UDP/TCP DNS to the cluster DNS pods.
- TCP 8787 to the MandoForge API for artifact/state callbacks.
- External TCP 443 while excluding private, link-local, and metadata ranges.

Vanilla NetworkPolicy cannot enforce DNS names. Production environments that
need provider-specific egress allowlists must use an egress proxy or a CNI with
FQDN policy support. The pilot must document this limitation and must not claim
strong multi-tenant network isolation from the base policy alone.

## Lifecycle Convergence

Provisioning, cancel, stale reclaim, and terminal-session cleanup use shared
helpers rather than duplicated delete-name logic.

### Provisioning

- Reuse an existing record only when its lease and runtime identity are usable.
- If a deterministic session record is `attention` or its resource is gone,
  create a new claim and rebind the existing Remote Computer record after the
  replacement Pod is Running.
- Persist the runtime identity before creating the lease.
- If lease creation fails, clean up a resource created by this attempt and
  preserve both the primary and cleanup errors.

### Cancel

1. Delete or converge the persisted runtime resource.
2. Transition the active lease to failed for on-demand resources or released
   for reusable pool resources.
3. Mark the assignment canceled.
4. Emit resource, lease, assignment, and audit evidence.

Repeating the operation must converge to the same result.

### Stale And Terminal Reclaim

- Use the same runtime-identity delete helper as cancel.
- Record cleanup failures instead of silently discarding them.
- Keep historical lease and assignment records for replay.
- Mark the Remote Computer `attention` when its backing resource is absent.
- Allow later provisioning to rebind that record rather than permanently
  failing lease creation.
- Claim TTL remains the final cleanup path if the worker dies between external
  mutation and DB persistence.

## Events And Audit

The implementation adds or enriches evidence without storing sensitive
execution envelopes:

- `remote_computer.sandbox_claim_created`
- `remote_computer.sandbox_bound`
- `remote_computer.runtime_rebound`
- `remote_computer.runtime_cleanup_completed`
- `remote_computer.runtime_cleanup_failed`

Evidence includes tenant-safe resource names, namespace, substrate, status
code, lease id, session id, execution job id, and timing. It excludes bearer
tokens, runtime environment values, prompts, file contents, and stdin payloads.

## Compatibility And Migration

- No database migration is required; the versioned runtime identity is stored
  in existing JSON metadata.
- Existing Kubernetes Pod mode continues to use its Pod template.
- Existing Agent Sandbox records with only `sandbox_claim_name` remain
  readable.
- The feature stays behind the current execution, mutation, live-mutation, and
  Kubernetes transport gates.
- The default ConfigMap remains on the Kubernetes Pod runner until operators
  explicitly select `agent-sandbox`.

## Verification

### Unit Tests

- Claim creation includes lifecycle and propagated tracking metadata.
- A claim whose Sandbox has a generated warm-pool name resolves the correct
  Pod.
- Claim Ready/Finished failures stop polling with a useful error.
- Create 409 and delete 404/410 are idempotent.
- Runtime identity dispatches delete to the correct API despite global mode
  changes.
- Exec request URI contains only fixed launcher argv.
- Sensitive envelope data is sent on stdin and absent from the URI and audit
  projection.
- Cache paths keep `CARGO_TARGET_DIR` private and enable sccache.

### Store And Runtime Tests

- Cancel releases or fails the lease and cancels the assignment after resource
  convergence.
- Retry after cancel does not reuse a dead Pod.
- Expired and terminal-session leases use the same cleanup state machine.
- An attention record can be rebound to a replacement Sandbox Pod.
- Cleanup failure is audited and remains retryable.

### Manifest And Image Tests

- Kustomize renders the pilot without a live claim.
- The smoke overlay is the only static live-claim example.
- The runtime image is distinct from the API image.
- The shared cache does not contain `CARGO_TARGET_DIR`.
- The egress policy allows required DNS/API/HTTPS paths and keeps private
  ranges denied.
- A runtime-image smoke verifies launcher, Git, Cargo, sccache, Node, pnpm, uv,
  and configured CLI binaries when Docker is available.

### Live Cluster Evidence

A production promotion requires an installed Agent Sandbox controller and a
real cluster drill that proves:

1. Warm-pool claim startup and generated Sandbox-name resolution.
2. Workspace seed initialization.
3. Approved CLI execution through stdin transport.
4. Runtime event, artifact, and audit ingestion.
5. Session workspace reuse without cross-session state reuse.
6. Cancel, expiry, worker-crash TTL, and retry convergence.
7. Network and cache isolation for the declared tenant/project scope.

Static verification must continue to report this lane as pilot-only when those
artifacts are absent.

## Implementation Slices

1. Correct Claim discovery, resource identity, lifecycle deadline, and
   idempotent mutations.
2. Converge cancel, stale reclaim, terminal cleanup, and runtime rebind.
3. Add the stdin exec protocol and `mandoforge-sandbox-runtime` binary.
4. Add the dedicated runtime image, workspace seed, cache layout, and egress
   policy.
5. Update docs, readiness wording, static gates, and live-cluster runbook.

Each slice must be an atomic commit with targeted tests. A deep review is
required after the runtime, security, and orchestration changes and before any
merge.

## Non-Goals

- Replacing MandoForge leases or events with Kubernetes CRDs.
- Adding a second business workflow orchestrator.
- Supporting arbitrary dynamic Git credentials in SandboxClaims.
- Claiming gVisor, Kata, or VM-grade isolation from a standard container
  runtime.
- Enabling Agent Sandbox by default.
- Treating a passing static Kustomize render as production evidence.

## Acceptance Criteria

The hardening is complete when:

- A current Agent Sandbox warm-pool claim resolves its generated Sandbox and
  Pod names correctly.
- The default sandbox runtime can start the configured CLI in a seeded,
  session-isolated workspace.
- No prompt, file content, or runtime environment value appears in the Pod exec
  URI or audit projection.
- Cancel, repeated cancel, expiry, terminal session, and retry converge across
  Kubernetes resources and MandoForge lease/assignment state.
- Claim TTL prevents unbounded orphan resources.
- Shared caches are project-scoped and contain no mutable Cargo target tree.
- Network egress is explicit and private cluster ranges remain denied.
- All Rust, manifest, image-contract, and targeted lifecycle tests pass.
- Production readiness remains blocked until the live-cluster evidence bundle
  is present.
