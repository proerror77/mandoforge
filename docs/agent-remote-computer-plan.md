# Agent Remote Computer Plan

This plan records the target architecture for making MandoForge agents run inside Kubernetes Pod-based remote computers. The current repo has a control-plane skeleton plus an active assigned-Pod execution path behind explicit gates. It has a queue-backed worker, Docker shell sandbox support, Kubernetes API/worker/scheduler skeletons, worker readiness, a Remote Computer Pod template, restricted service account, RWX PVC placeholder, JuiceFS CSI example, warm-pool example, KEDA scaling example, deny-by-default NetworkPolicy, `GET /api/remote-computers/readiness`, `GET /api/remote-computers/runner/readiness`, fail-closed runner dry-run/mutate routes, persisted Remote Computer lease lifecycle APIs, persisted session attachment state, warm-pool claim, on-demand Pod creation when no warm-pool Remote Computer is available, and Kubernetes `pods/exec` transport for approved `file.write`, `shell.exec`, `codex.exec`, and `agent_cli.exec` jobs when all runtime gates are enabled. It does not yet mount a real shared distributed state filesystem or provide a production warm-pool controller.

## Full Agent OS Alignment

This plan is subordinate to the Full Agent OS narrative in
[Full Agent OS Narrative Design](superpowers/specs/2026-07-09-full-agent-os-narrative-design.md)
and the managed-agent resource model in
[Claude Managed Agents Alignment](claude-managed-agents-alignment.md).

Remote Computer remains the target isolated execution substrate, but it is not
the top-level product object. The top-level runtime chain remains:

```text
Agent -> Environment -> Session -> Events -> Threads
```

Remote Computer should be reached through:

```text
Managed Runtime
  -> Environment Scheduling Layer
  -> K Agent
  -> Remote Computer lease / Pod / warm-pool slot
  -> CLI adapter or approved tool execution
  -> runtime events, artifacts, usage, and audit
```

K Agent is the execution-side controller for sandbox and CLI dispatch. It may
claim work, select Pods, prepare workspace/state mounts, launch Codex or Claude
Code, stream runtime output, sync artifacts, and clean up leases. It does not
own ManagerPlan, WorkItem routing authority, TaskGrant creation or expansion,
Policy decisions, Approval decisions, Ontology validity, WorkflowPack release,
or audit truth.

## Objective

Add an Agent Remote Computer layer that gives each approved agent run a leaseable, isolated execution environment with:

- Operating-system tools and runtime dependencies.
- A session workspace and filesystem.
- Mounted Memory, Notes, Skills, and tool configuration state.
- Shell, Codex, script, and MCP-adjacent execution capability.
- Event, artifact, stdout, stderr, and state synchronization back into MandoForge.
- Kubernetes-native scaling, replacement, and warm-pool operations.

The core execution shape becomes:

```text
WorkItem / ManagerPlan / WorkflowRun / Session
  -> TaskGrant / Policy / Approval
  -> Session Event / SessionLoopJob / ExecutionJob
  -> Environment Scheduling
  -> K Agent worker claim
  -> Remote Computer lease or Pod claim
  -> Mounted workspace and state filesystem
  -> Sandbox / Codex / Claude Code / shell / tool execution
  -> Runtime events + Artifact + Audit sync
```

## Why This Belongs In MandoForge

The current worker model proves the governance path: approval, queueing, lease, drain, retry, audit, and replay. The Remote Computer layer is the next execution substrate. It moves high-risk or long-running work out of the API process and out of generic workers into an environment that can be:

- Created per session or leased from a pool.
- Destroyed after completion.
- Replayed through durable events and artifacts.
- Governed by pod security, network policy, resource limits, and tool policy.
- Scaled horizontally through Kubernetes.

This matches the Generic Agent OS goal: agents are not just chat sessions; they are governed, auditable execution objects with durable state and isolated compute.

## K Agent Guardrail

K Agent must stay an Environment Scheduling component:

- It may claim approved work for an Environment.
- It may select or create an execution substrate.
- It may dispatch CLI adapters and approved tool runners.
- It may report heartbeats, runtime events, artifacts, usage, and cleanup state.
- It must not grant authority, expand TaskGrants, approve high-risk actions,
  decide Ontology validity, release WorkflowPacks, or become the audit source of
  truth.

If an implementation needs K Agent to make a business decision, that
decision belongs in Manager Runtime, Governance, or Ontology Action Contract
first, and K Agent should receive only the approved execution envelope.

## Current State

Covered today:

- Execution jobs can be queued and drained by `mandoforge-worker`.
- Worker leases, retry counts, stale lease detection, and worker readiness are API-visible.
- Docker Compose and Kubernetes skeletons exist.
- Approved `shell.exec` can run through an optional Docker sandbox runner.
- Codex App Server and Codex CLI execution remain governed by approval and queue paths.
- Remote Computer readiness is API/UI-visible.
- K8s Remote Computer manifests exist for the Pod template, service account, state PVC placeholder, and NetworkPolicy skeleton.
- A mounted state-contract ConfigMap now defines the `/agent-state` layout for Memory, Notes, Skills, artifacts, lock files, and manifests. The contract records the current conflict rule: one active writer per session workspace, with shared Memory/Notes/Skills kept read-mostly until a lock-aware sync manager is configured.
- A JuiceFS CSI example manifest documents the target shared `/agent-state` provider shape, but it is not included in the default kustomization and must be configured explicitly before use.
- A warm-pool Deployment example documents the cold-start mitigation shape, but it is not included in the default kustomization and does not yet assign sessions to prestarted Pods. It now keeps the same fail-closed artifact discovery sidecar shape as the regular Remote Computer Pod template so prewarmed Pods do not diverge from the eventual session Pod contract.
- A KEDA ScaledObject example documents the queue-pressure scaling shape for the warm pool, but it is not included in the default kustomization and depends on production metrics work.
- The production-state gate is currently green only for the single-node local-hostpath Whiskey pilot; multi-node distributed Memory/Notes/Skills promotion still needs a shared filesystem and a live state-sync runner proof.
- `remote_computers` and `remote_computer_leases` persist control-plane lease state.
- Lease lifecycle APIs write `remote_computer.*` session events and audit logs without executing tools.
- `RemoteComputerRunner` exists as a reserved/fail-closed boundary with Admin-only readiness, dry-run, and explicit mutate endpoints.
- `KubernetesRemoteComputerRunner` exists as an explicit `MANDOFORGE_REMOTE_COMPUTER_RUNNER=kubernetes` adapter skeleton. It validates template/client config, including kubeconfig or API-server-plus-bearer-token inputs, can perform a read-only `/version` probe, reports Pod create/delete intent, and can call the Kubernetes Pod create/delete API only when both mutation gates are explicitly enabled.
- Kubernetes dry-runs return the planned API path, namespace, pod name, and template path so future live calls have an auditable request plan before mutation is enabled.
- Kubernetes live mutation is gated by both `MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED` and `MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED`; it remains Admin-only.
- `remote_computer_session_attachments` persists session-to-lease attach/release state and stale attach detection without moving tool execution into Pods.
- `remote_computer_job_assignments` persists approved execution-job-to-lease handoff plans. Worker runs acknowledge handoff, emit a Pod exec transport plan, and can execute approved `file.write`, `shell.exec`, `codex.exec`, and `agent_cli.exec` inside the assigned Pod when `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes`, `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED=true`, and the Kubernetes runner live mutation gates are enabled.
- Remote execution uses a session-scoped workspace under `/workspace/sessions/{session_id}` so multiple jobs in the same session reuse the same lease and directory, while concurrent jobs for the same session do not claim a second Pod.
- `POST /api/remote-computers/reclaim-stale` reclaims stale attachments and expired leases with event/audit records and no tool execution.
- `/api/scheduler/due-plan` and `/api/scheduler/run-due` include Remote Computer stale reclaim in the aggregate operations path.
- **On-demand Pod provisioning** (`provision_remote_computer_pod_for_job`): when no warm-pool Remote Computer is available, the execution engine creates a Kubernetes Pod, polls until Running phase, persists the DB record with a unique pod-name index (`remote_computers(tenant_id, pod_name)`), and leases it to the job. Requires `MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes` plus the triple mutation gate. Pod-ready polling is configurable via `MANDOFORGE_REMOTE_COMPUTER_POD_READY_TIMEOUT_SECONDS` (default 60s) and `MANDOFORGE_REMOTE_COMPUTER_POD_READY_POLL_INTERVAL_MS` (default 2000ms).
- Stale reclaim now issues a `live_delete` mutation for expired on-demand Pod leases, preventing orphaned Pods.

Not covered today:

- Session attachment APIs remain control-plane state; worker execution uses job assignments and session-scoped lease reuse as the runtime attach path.
- Pod exec transport is implemented but remains fail-closed until the explicit Kubernetes runner, execution, mutation, and live-mutation gates are enabled.
- No real distributed Memory/Notes/Skills mount in the default deployment; the default deployment has only the mounted state layout and conflict contract.
- No production distributed filesystem integration yet; JuiceFS exists as an example manifest only, while CephFS, Longhorn RWX, cloud file storage, or object-backed sync remain future provider options.
- Warm-pool Remote Computer records can be claimed by the worker when present, but there is not yet a production controller that registers and recycles prewarmed Pods automatically.
- The artifact discovery sidecar is present but disabled by default; there is still no production artifact/state sync daemon running against real leased Pods.
- No production KEDA/HPA queue-depth scaling for remote computer pools; the KEDA manifest is an opt-in example only.

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

Remaining Stage 2 pilot work:

1. Add a real Kubernetes API client implementation behind the existing adapter boundary, still fail-closed by policy and configuration.
2. Keep actual tool execution on the current worker path until the Pod lifecycle is observable and testable.

Stage 2 acceptance for this slice:

- Readiness endpoint accurately reports whether Pod template, PVC/RWX storage, service account, NetworkPolicy, and autoscaling manifests exist.
- Kustomize renders the remote computer manifest.
- The opt-in warm-pool render keeps artifact discovery sidecar parity with the regular Remote Computer Pod template.
- UI renders readiness without requiring a live cluster.
- Runner readiness and dry-run routes are Admin-only, audited, and fail closed without mutating Kubernetes.
- Session attachment APIs persist attach/release/stale evidence without creating jobs or tool calls.
- Stale reclaim is Admin-only, audited, and does not create execution jobs or tool calls.
- Docs clearly state this is a skeleton/pilot boundary, not production remote execution.

## Stage 3 Plan

Stage 3 should make Remote Computer the primary sandbox substrate.

1. Implement session-to-Pod lease allocation.
2. Execute `shell.exec`, `codex.exec`, `file.write`, and `agent_cli.exec` inside the leased Pod behind explicit Kubernetes transport gates. The initial implementation is present; hardening remains around state sync, artifact sidecar session injection, and live-cluster operational runbooks.
3. Add artifact and event sync from Pod to MandoForge.
4. Add warm pool controller.
5. Wire the KEDA queue-depth and pool-pressure example to real Prometheus metrics and an opt-in overlay.
6. Add hard sandbox profiles:
   - `read-only`
   - `workspace-write`
   - `network-limited`
   - `codex-workspace`
   - `gvisor-isolated`
   - `firecracker-isolated` later
7. Promote the JuiceFS CSI and warm-pool examples into opt-in overlays once secret handling, namespace selection, state sync conflict rules, and Pod assignment are implemented.
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

After assigned-Pod execution, the next coherent implementation slice should be:

```text
Harden Remote Computer session lifecycle and state/artifact sync
```

Concrete deliverables:

- Add a warm-pool controller that registers prewarmed Pods as available Remote Computers, resets dirty workspaces, and refills the pool.
- Promote artifact discovery from disabled sidecar skeleton to assignment-aware session sidecar injection.
- Add session terminal cleanup that releases or deletes Pods based on on-demand vs warm-pool origin.
- Add live-cluster runbooks and tests proving Pod replacement, stale cleanup, and artifact sync do not bypass Tool Router, Policy Engine, or Approval Engine.
