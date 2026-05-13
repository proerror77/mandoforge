# Agent Remote Computer Plan

This plan records the target architecture for making MandoForge agents run inside Kubernetes Pod-based remote computers. It is not implemented yet. The current repo has a queue-backed worker, Docker shell sandbox support, Kubernetes API/worker/scheduler skeletons, and a worker readiness gate. It does not yet create a dedicated Pod per agent session or mount a shared distributed state filesystem.

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
  -> Kubernetes Pod lease
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

Not covered today:

- No `remote_computers` store or API.
- No session-to-Pod lease lifecycle.
- No Pod template for an agent remote computer.
- No shared Memory/Notes/Skills mount.
- No distributed filesystem integration such as JuiceFS, CephFS, Longhorn RWX, or object-backed sync.
- No warm pool of prestarted agent Pods.
- No artifact/state sync daemon inside the Pod.
- No KEDA/HPA queue-depth scaling for remote computer pools.

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

Warm pool is a later optimization, not the first implementation.

Responsibilities:

- Keep N ready Pods with base image, tools, and state mount initialized.
- Lease a warm Pod to a session in seconds.
- Recycle or destroy dirty Pods after session completion.
- Refill pool based on queue pressure.

### Scaling

Initial scaling should be conservative:

- HPA for worker Deployment as a skeleton.
- KEDA later for queue depth, stale lease count, and remote computer pool pressure.
- Separate pools for `read-only`, `workspace-write`, `codex`, and `network-limited` profiles.

## Stage 2 Plan

Stage 2 should add this as a governed pilot, not as a full production platform.

1. Document the Remote Computer control-plane contract.
2. Add `remote_computers` and `remote_computer_leases` tables.
3. Add read-only readiness API: `GET /api/remote-computers/readiness`.
4. Add Kubernetes Pod template under `deploy/k8s/agent-remote-computer.yaml`.
5. Add PVC/RWX mount placeholders for state and workspace.
6. Add Admin UI readiness panel showing missing storage, Pod template, NetworkPolicy, warm-pool, and autoscaling blockers.
7. Add event types:
   - `remote_computer.requested`
   - `remote_computer.leased`
   - `remote_computer.started`
   - `remote_computer.heartbeat`
   - `remote_computer.released`
   - `remote_computer.failed`
8. Keep actual tool execution on the current worker path until the Pod lifecycle is observable and testable.

Stage 2 acceptance for this slice:

- Readiness endpoint accurately reports whether Pod template, PVC/RWX storage, service account, NetworkPolicy, and autoscaling manifests exist.
- Kustomize renders the remote computer manifest.
- UI renders readiness without requiring a live cluster.
- Docs clearly state this is a skeleton/pilot boundary, not production remote execution.

## Stage 3 Plan

Stage 3 should make Remote Computer the primary sandbox substrate.

1. Implement session-to-Pod lease allocation.
2. Execute `shell.exec`, `codex.exec`, and selected tool runners inside the leased Pod.
3. Add artifact and event sync from Pod to MandoForge.
4. Add warm pool controller.
5. Add KEDA queue-depth and pool-pressure autoscaling.
6. Add hard sandbox profiles:
   - `read-only`
   - `workspace-write`
   - `network-limited`
   - `codex-workspace`
   - `gvisor-isolated`
   - `firecracker-isolated` later
7. Add distributed state filesystem integration, starting with JuiceFS CSI examples.
8. Add conflict policy for shared Memory/Notes/Skills writes.
9. Add per-team/project Pod security and network policies.
10. Add remote computer replay view in the Session Timeline.

Stage 3 acceptance:

- A session can lease a Pod, execute approved work there, sync artifacts, release the Pod, and replay the full event chain.
- Warm-pool leasing avoids cold-starting every session.
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

After the current worker autoscaling readiness slice is finished, the next coherent implementation slice should be:

```text
Add Remote Computer readiness skeleton
```

Concrete deliverables:

- `docs/agent-remote-computer-plan.md`
- `deploy/k8s/agent-remote-computer.yaml`
- `deploy/k8s/remote-computer-state-pvc.yaml`
- `GET /api/remote-computers/readiness`
- Static UI readiness panel
- API test for readiness output
- Actionbook static UI check
- Honest Stage 2 audit update
