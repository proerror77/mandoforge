# Agent Remote Computer Plan

This plan records the architecture for running MandoForge agents in isolated Kubernetes environments. The production-default manifests now select Kubernetes SIG Agent Sandbox for `SandboxClaim`, Sandbox, Pod, per-sandbox PVC, and warm-pool lifecycle. MandoForge remains the authority for WorkflowRun, TaskGrant, policy, approval, context, execution jobs, leases, events, artifacts, and audit. Kubernetes `pods/exec` remains the execution transport behind explicit fail-closed gates. The repository still lacks production-target storage, network, load, state-sync, and rollback evidence, so selecting the substrate is not the same as enabling live execution.

## Managed Agents Alignment

This plan is now subordinate to the managed-agent resource model in
[Claude Managed Agents Alignment](claude-managed-agents-alignment.md).

Remote Computer remains the target isolated execution substrate, but it should
not be the top-level product object. The top-level chain is:

```text
Agent -> Environment -> Session -> Events -> Threads
```

Remote Computer should become `Environment(type=remote_computer)`: a
self-hosted sandbox implementation claimed by an environment worker. The Remote
Computer manager still owns Pod lease, state mount, artifact sync, heartbeat,
and cleanup, but sessions should reach it through the Environment queue and
session event stream rather than through a separate product path.

## Objective

Add an Agent Remote Computer layer that gives each approved agent run a leaseable, isolated execution environment with:

- Operating-system tools and runtime dependencies.
- A session workspace and filesystem.
- Mounted Memory, Notes, Skills, and tool configuration state.
- Shell, Codex, script, and MCP-adjacent execution capability.
- Event, artifact, stdout, stderr, and state synchronization back into MandoForge.
- Kubernetes-native scaling, replacement, and warm-pool operations.

The core runtime shape becomes:

```text
Agent Session
  -> Harness / Tool Router / Policy Engine
  -> Execution Job Queue
  -> Remote Computer Manager
  -> Agent SandboxClaim / warm-pool lease
  -> Controller-managed Sandbox / Kubernetes Pod
  -> Mounted state filesystem
  -> Sandbox / Codex / shell / tool execution
  -> Artifact + Event + Audit sync
```

## Why This Belongs In MandoForge

The current worker model proves the governance path: approval, queueing, lease, drain, retry, audit, and replay. The Remote Computer layer is the next execution substrate. It moves high-risk or long-running work out of the API process and out of generic workers into an environment that can be:

- Created per session or leased from a pool.
- Destroyed after completion.
- Replayed through durable events and artifacts.
- Governed by pod security, network policy, resource limits, and tool policy.
- Scaled horizontally through Kubernetes.

This matches the Generic Agent OS goal: agents are not just chat sessions; they are governed, auditable execution objects with durable state and isolated compute.

## Current State

Covered today:

- Execution jobs can be queued and drained by `mandoforge-worker`.
- Worker leases, retry counts, stale lease detection, and worker readiness are API-visible.
- Docker Compose and Kubernetes skeletons exist.
- Approved `shell.exec` can run through an optional Docker sandbox runner.
- Codex App Server and Codex CLI execution remain governed by approval and queue paths.
- Remote Computer readiness is API/UI-visible.
- The default K8s bundle includes the pinned Agent Sandbox controller contract, SandboxTemplate, SandboxWarmPool, per-sandbox workspace PVC, project cache PVC, runtime service account, and ingress/egress NetworkPolicy.
- A mounted state-contract ConfigMap now defines the `/agent-state` layout for Memory, Notes, Skills, artifacts, lock files, and manifests. The contract records the current conflict rule: one active writer per session workspace, with shared Memory/Notes/Skills kept read-mostly until a lock-aware sync manager is configured.
- A JuiceFS CSI example manifest documents the target shared `/agent-state` provider shape, but it is not included in the default kustomization and must be configured explicitly before use.
- A warm-pool Deployment example documents the cold-start mitigation shape, but it is not included in the default kustomization and does not yet assign sessions to prestarted Pods. It now keeps the same fail-closed artifact discovery sidecar shape as the regular Remote Computer Pod template so prewarmed Pods do not diverge from the eventual session Pod contract.
- `deploy/agent-sandbox-pilot` is retained as a compatibility alias for the default bundle. The separate `deploy/agent-sandbox-smoke` overlay is the only tracked bundle that creates a live `SandboxClaim` example.
- KEDA scales MandoForge queue workers from queue pressure. It does not own Workflow decisions, approvals, governance, or Sandbox lifecycle.
- The production-state gate is currently green only for the single-node local-hostpath Whiskey pilot; multi-node distributed Memory/Notes/Skills promotion still needs a shared filesystem and a live state-sync runner proof.
- `remote_computers` and `remote_computer_leases` persist control-plane lease state.
- Lease lifecycle APIs write `remote_computer.*` session events and audit logs without executing tools.
- `RemoteComputerRunner` exists as a reserved/fail-closed boundary with Admin-only readiness, dry-run, and explicit mutate endpoints.
- The production configuration selects `MANDOFORGE_REMOTE_COMPUTER_RUNNER=agent-sandbox`. The API creates a `SandboxClaim`, resolves the controller-managed Pod from the `agents.x-k8s.io/pod-name` annotation, persists the claim identity in Remote Computer metadata, and reuses Kubernetes `pods/exec`. The legacy direct-Pod mode remains in code as an explicit non-default fallback and has no create/delete RBAC in the default bundle.
- Kubernetes dry-runs return the planned API path, namespace, pod name, and template path so future live calls have an auditable request plan before mutation is enabled.
- Kubernetes live mutation is gated by both `MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED` and `MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED`; it remains Admin-only.
- `remote_computer_session_attachments` persists session-to-lease attach/release state and stale attach detection without moving tool execution into Pods.
- `remote_computer_job_assignments` persists approved execution-job-to-lease handoff plans. Worker runs acknowledge handoff, emit a Pod exec transport plan, and can execute approved `file.write`, `shell.exec`, `codex.exec`, and `agent_cli.exec` inside the assigned Pod when `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes`, `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED=true`, and the Kubernetes runner live mutation gates are enabled.
- Remote execution uses a session-scoped workspace under `/workspace/sessions/{session_id}` so multiple jobs in the same session reuse the same lease and directory, while concurrent jobs for the same session do not claim a second Pod.
- `POST /api/remote-computers/reclaim-stale` reclaims stale attachments and expired leases with event/audit records and no tool execution.
- `/api/scheduler/due-plan` and `/api/scheduler/run-due` include Remote Computer stale reclaim in the aggregate operations path.
- **On-demand environment provisioning** (`provision_remote_computer_pod_for_job`): when no persisted lease is available, Agent Sandbox mode creates a `SandboxClaim`, waits for the bound Pod, persists both identities, and leases the environment to the job. Requires the Kubernetes exec transport plus the triple mutation gate. The function name remains for compatibility with the legacy direct-Pod adapter.
- Stale reclaim deletes the owning SandboxClaim in Agent Sandbox mode, allowing the controller to clean up the generated Sandbox, Pod, and private PVC.
- The dedicated Agent Sandbox runtime image is built from a clean Git-index
  context and includes pinned Rust, Node, pnpm, uv, sccache, Codex CLI, Claude
  Code, and the fixed MandoForge launcher. The runtime is non-root, has a
  read-only root filesystem, mounts only `/workspace/sessions` from the private
  workspace PVC, and mounts dependency caches under `/cache/project`.
- `GET /api/remote-computers/readiness` now separates static Agent Sandbox
  contracts from live lifecycle evidence. Static readiness without a fresh,
  complete `.mandoforge/agent-sandbox-runtime-evidence/summary.json` remains
  `pilot_only`; malformed, stale, wrong-controller, or failed evidence remains
  production-blocking.
- A live Docker Desktop drill against Agent Sandbox controller `v0.5.1` proved
  WarmPool Claim binding, generated Sandbox/Pod resolution, runtime versions,
  same-session lease/workspace reuse, cross-Sandbox workspace isolation,
  project-cache persistence, NetworkPolicy behavior, idempotent retry, TTL
  cleanup, and terminal-session cleanup. It also proved the governed MandoForge
  path from pending approval through Postgres-backed execution job, Pod exec,
  durable events, artifact, and audit records. This is local pilot evidence,
  not multi-node or production storage evidence.
- Out-of-cluster API processes can use
  `MANDOFORGE_REMOTE_COMPUTER_CA_CERT_PATH` with a short-lived
  `mandoforge-api` ServiceAccount token. Runtime Pods and queue workers keep token automount
  disabled and do not receive runner permissions.

Not covered today:

- Session attachment APIs remain control-plane state; worker execution uses job assignments and session-scoped lease reuse as the runtime attach path.
- Pod exec transport is implemented but remains fail-closed until Agent Sandbox target-cluster evidence is accepted and the execution, mutation, and live-mutation gates are explicitly enabled.
- No real distributed Memory/Notes/Skills mount in the default deployment; the default deployment has only the mounted state layout and conflict contract.
- No production distributed filesystem integration yet; JuiceFS exists as an example manifest only, while CephFS, Longhorn RWX, cloud file storage, or object-backed sync remain future provider options.
- Agent Sandbox supplies the warm-pool controller, but production still needs target-cluster upgrade/rollback ownership and evidence that dirty workspaces are not reused across claims.
- The artifact discovery sidecar is present but disabled by default; there is still no production artifact/state sync daemon running against real leased Pods.
- No production KEDA/HPA queue-depth scaling for remote computer pools; the KEDA manifest is an opt-in example only.
- Agent Sandbox has a successful local lifecycle drill, but production
  promotion still requires the drill on the target cluster, a real RWX/shared
  cache provider, distributed state-sync proof, target-CNI enforcement, load
  evidence, and an operator-owned controller upgrade/rollback process.
- Standard Kubernetes `NetworkPolicy` cannot express an FQDN allowlist and may
  not block traffic to node or host-network endpoints. The pilot allows
  external TCP 443 and relies on an uncredentialed runtime Pod; production
  deployments that require domain or API-server egress enforcement need a
  Cilium FQDN/host policy, egress proxy, or equivalent.

## Target Components

### Remote Computer Manager

Responsibilities:

- Allocate a Pod or warm-pool slot for a session.
- Bind the session, workspace, tenant, project, agent version, and policy snapshot.
- Record lease ownership, heartbeat, expiration, and terminal state.
- Fail closed when policy, secret, storage, or network prerequisites are missing.
- Emit `remote_computer.*` events and audit logs.

### Pod Runner

The Pod is the actual remote computer. It should contain:

- Base OS image with approved tools.
- Runtime sidecar or agent entrypoint.
- Workspace volume.
- State volume for Memory/Notes/Skills.
- Artifact output path.
- Optional Codex CLI or App Server bridge.
- Restricted service account.
- Resource requests/limits.
- NetworkPolicy boundary.

### State Filesystem

Start with a pluggable interface, not a hard dependency:

- Stage 2 skeleton: PVC/RWX abstraction and readiness checks.
- Stage 2 pilot option: JuiceFS CSI for shared state mounts.
- Enterprise options: CephFS/Rook, Longhorn RWX, cloud file storage, or object-store-backed sync.

State categories:

- `memory/`
- `notes/`
- `skills/`
- `tool-config/`
- `workspace/`
- `artifacts/`
- `.mandoforge/manifest.json`
- `.mandoforge/events.jsonl`

Concurrency rule: first version should avoid multi-writer conflicts by assigning one active writer per session workspace. Shared Memory/Notes/Skills should be read-mostly, with writes routed through explicit runtime APIs or lock-aware sync jobs.

### Warm Pool

The default Agent Sandbox bundle declares a one-replica `SandboxWarmPool` to
reduce claim latency. It is an allocation optimization only; it does not own
MandoForge queueing, governance, or session authority.

Responsibilities:

- Keep N controller-managed Sandboxes ready with the runtime image and cache mount initialized.
- Bind a warm Sandbox to a claim without sharing the private workspace PVC across sessions.
- Destroy or reset dirty environments according to the controller lifecycle contract.
- Refill the pool independently from MandoForge queue-worker scaling.

### Scaling

Initial scaling should be conservative:

- HPA/KEDA scale queue workers from CPU or execution queue depth.
- `SandboxWarmPool.spec.replicas` controls prewarmed runtime capacity; future automation may tune it from measured claim latency and pool pressure.
- Separate pools for `read-only`, `workspace-write`, `codex`, and `network-limited` profiles.

## Stage 2 Status

Stage 2 established the governed control plane and local Agent Sandbox proof. It
is complete as a repository slice, but its local evidence is not production
evidence.

Completed Stage 2 readiness skeleton:

- Document the Remote Computer control-plane contract.
- Add read-only readiness API: `GET /api/remote-computers/readiness`.
- Add Kubernetes Pod template under `deploy/k8s/agent-remote-computer.yaml`.
- Add PVC/RWX mount placeholders for state and workspace.
- Add Admin UI readiness panel showing storage, Pod template, NetworkPolicy, warm-pool, and autoscaling blockers.
- Add planned event types to readiness output:
   - `remote_computer.requested`
   - `remote_computer.leased`
   - `remote_computer.started`
   - `remote_computer.heartbeat`
   - `remote_computer.attached`
   - `remote_computer.execution_handoff_planned`
   - `remote_computer.execution_handoff_acknowledged`
   - `remote_computer.execution_transport_planned`
   - `remote_computer.runner_dry_run`
   - `remote_computer.detached`
   - `remote_computer.attachment_reclaimed`
   - `remote_computer.lease_reclaimed`
   - `remote_computer.released`
   - `remote_computer.failed`
- Add reserved runner boundary:
   - `GET /api/remote-computers/runner/readiness`
   - `POST /api/remote-computers/runner/dry-run`
   - UI `RUNNER BOUNDARY` section
   - audit action `remote_computer.runner_dry_run`
   - tests proving dry-runs do not create leases, execution jobs, or tool calls
- Add persisted session attachment control plane:
   - `POST /api/remote-computer-leases/:id/attach`
   - `GET /api/remote-computer-attachments`
   - `GET /api/remote-computer-attachments/stale`
   - `POST /api/remote-computer-attachments/:id/release`
   - audit actions `remote_computer.attached` and `remote_computer.detached`
   - tests proving attachments do not enqueue jobs or execute tools
- Add execution job handoff planning:
   - `POST /api/execution-jobs/:id/remote-computer-lease`
   - `GET /api/remote-computer-job-assignments`
   - event `remote_computer.execution_handoff_planned`
   - event `remote_computer.execution_handoff_acknowledged`
   - event `remote_computer.execution_transport_planned`
   - audit actions `remote_computer.execution_handoff_planned`, `remote_computer.execution_handoff_acknowledged`, and `remote_computer.execution_transport_planned`
   - tests proving handoff planning, worker acknowledgement, and transport planning do not start Pod execution or create another job
- Add Admin-only stale reclaim:
   - `POST /api/remote-computers/reclaim-stale`
   - releases stale attachments
   - fails expired leases
   - audit actions `remote_computer.attachment_reclaimed`, `remote_computer.lease_reclaimed`, and `remote_computer.reclaim_stale_run`
   - tests proving reclaim does not enqueue jobs or execute tools

Remaining production-promotion work:

1. Run the lifecycle and governed-execution drill on the intended production cluster.
2. Prove the real RWX/project-cache provider, distributed state synchronization, target-CNI enforcement, load envelope, and controller rollback.
3. Partition cache PVCs by project/tenant and keep all session context in private workspace PVCs.
4. Enable the three execution/mutation gates only through a reviewed deployment change backed by fresh evidence.

Stage 2 acceptance for this slice:

- Readiness endpoint accurately reports whether Pod template, PVC/RWX storage, service account, NetworkPolicy, and autoscaling manifests exist.
- Kustomize renders the Agent Sandbox template, warm pool, NetworkPolicy, API identity, and scoped RBAC by default.
- The opt-in warm-pool render keeps artifact discovery sidecar parity with the regular Remote Computer Pod template.
- UI renders readiness without requiring a live cluster.
- Runner readiness and dry-run routes are Admin-only, audited, and fail closed without mutating Kubernetes.
- Session attachment APIs persist attach/release/stale evidence without creating jobs or tool calls.
- Stale reclaim is Admin-only, audited, and does not create execution jobs or tool calls.
- Docs and gates distinguish selecting the substrate from enabling production remote execution.

## Stage 3 Plan

Stage 3 should productionize Agent Sandbox as the primary environment substrate.

1. Promote target-cluster Agent Sandbox evidence into the production readiness gate.
2. Execute `shell.exec`, `codex.exec`, `file.write`, and `agent_cli.exec` inside the claimed Sandbox behind explicit transport gates. The implementation exists; production hardening remains around state sync, artifact supervision, and incident runbooks.
3. Add artifact and event sync from Pod to MandoForge.
4. Add claim cleanup sweeps and prove terminal, stale, retry, and controller-upgrade behavior.
5. Harden the Agent Sandbox runner adapter with state-sync proof and production lifecycle validation.
6. Validate KEDA worker scaling and tune SandboxWarmPool capacity from separate metrics.
7. Add hard sandbox profiles:
   - `read-only`
   - `workspace-write`
   - `network-limited`
   - `codex-workspace`
   - `gvisor-isolated`
   - `firecracker-isolated` later
8. Select and validate a production cache/state provider once secret handling, namespace selection, state conflict rules, cache partitioning, and Pod/Sandbox assignment are proven.
9. Add conflict policy for shared Memory/Notes/Skills writes.
10. Add per-team/project Pod security and network policies.
11. Add remote computer replay view in the Session Timeline.

Stage 3 acceptance:

- A session can claim a Sandbox, execute approved work there, sync artifacts, release the claim, and replay the full event chain.
- Warm-pool claim binding avoids cold-starting every session without mixing private workspaces.
- Memory/Notes/Skills state is mounted or synced before execution.
- Failed Pods can be replaced without losing session audit history.
- Queue pressure can scale workers or remote computer pools.

## Non-Goals

- Do not bypass Tool Router, Policy Engine, or Approval Engine.
- Do not allow direct production secret mounts into agent Pods.
- Do not treat K8s Pod isolation alone as sufficient multi-tenant security.
- Do not claim production autoscaling until queue-depth scaling and load validation exist.
- Do not introduce a distributed filesystem as a hard dependency for local Docker Compose.

## Immediate Next Slice

After selecting Agent Sandbox as the default substrate, the next coherent slice is:

```text
Prove Agent Sandbox on the production target and enable it through evidence gates
```

Concrete deliverables:

- Capture production-target claim latency, warm-pool behavior, cleanup, retry, load, and controller rollback evidence.
- Validate per-project cache PVCs and distributed Memory/Notes/Skills synchronization without cross-session context leakage.
- Promote artifact discovery from disabled sidecar skeleton to assignment-aware session sidecar injection.
- Prove session terminal cleanup deletes the owning Claim and all controller-generated resources.
- Enable the execution/mutation gates in a separate reviewed deployment only after the production readiness evidence is green.
