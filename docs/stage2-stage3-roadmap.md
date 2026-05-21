# Stage 2 Production Adoption And Stage 3 Roadmap

This roadmap separates two different kinds of work:

- Stage 2 production adoption proves the completed repo-controlled Governed Runtime Pilot against real external targets.
- Stage 3 turns the runtime into a productized Enterprise Agent OS with external scheduling, richer traces, production-grade Remote Computer execution, workflow packs, and typed agent-team handoffs.

Do not treat Stage 2 production adoption as a reason to reopen the repo-controlled Stage 2 completion decision. The Stage 2 pilot is complete in this repository. Production adoption is environment-specific evidence work.

## Claude Managed Agents Alignment Update

Reference: [Claude Managed Agents Alignment](claude-managed-agents-alignment.md).

The Stage 3 plan is revised around the managed-agent product model:

```text
Agent -> Environment -> Session -> Events -> Threads
```

The prior roadmap correctly identified Remote Computer, worker queues, traces,
workflow packs, and typed handoffs as important. The correction is ordering and
abstraction:

- Remote Computer is not the top-level product object. It is an Environment
  substrate for isolated or self-hosted execution.
- The Orchestrator should not be a permanently running LLM daemon. It should be
  a versioned coordinator agent whose session loop is claimed from a queue when
  user events arrive.
- `POST /api/sessions/:id/run` is a demo-era convenience. The product API should
  be event-driven: create a session, append user events, stream session events,
  pause for approvals or custom tool results, then resume.
- Typed handoffs should become session threads with parent/child lineage,
  isolated conversation history, shared or isolated environments, and streamable
  thread events.
- The UI should start with sessions, events, blocking actions, artifacts, and
  threads. Infrastructure panels remain available, but they should not be the
  operator's first mental model.

Status update, 2026-05-20:

- The first managed-agent baseline is now present: Environment CRUD, sessions
  bound to environments, `/api/sessions/:id/events`, queue-claimed
  `session_loop_jobs`, `mandoforge-worker`, managed-agent-style timeline events,
  durable `session_threads`, and a session-first UI shell.
- This roadmap should therefore be read as a refinement plan, not as a list of
  completely absent features.
- The remaining gap is making the Claude-style contract complete end to end:
  resumable non-terminal session states, event-cursor processing, single-path
  session-loop continuation, environment queue binding, live streaming,
  specialist thread membership, lease-fenced finalization, and production
  restart/recovery evidence.

## Stage 2 Production Adoption

### Goal

Run the existing fail-closed evidence gates against real deployment targets and archive the resulting proof.

The output should answer:

- Which environment was validated.
- Which controller targets were used.
- Which evidence artifacts were captured.
- Which gates stayed blocked.
- Which adoption backlog items remain for that environment.

### Workstream S2-A: Evidence Baseline

Owner: integration owner.

Deliverables:

- Production adoption runbook: [Stage 2 Production Adoption Runbook](stage2-production-adoption-runbook.md).
- Controller environment matrix in the adoption runbook.
- Secret rendering checklist.
- Evidence PVC and archive procedure.
- Required artifact map for each production target.

Verification:

```bash
scripts/verify-stage2-controller-env-template.sh
scripts/verify-stage2-evidence-k8s-manifests.sh
scripts/verify-stage2-evidence-archive.sh --self-test
```

### Workstream S2-B: Cluster And Isolation Proof

Primary lanes: Lane A, Lane C, Lane D, Lane E.

Deliverables:

- Tenant routing evidence against a real multi-tenant deployment.
- Worker load-validation evidence against a real queue and cluster profile.
- Remote Computer state-sync and sidecar-recovery evidence against real distributed state storage.
- OTel collector deployment, rollout, and remediation evidence.

Required evidence gates:

```bash
scripts/tenant-isolation-evidence-gate.sh
scripts/worker-evidence-gate.sh
scripts/remote-computer-evidence-gate.sh
scripts/observability-collector-evidence-gate.sh
```

### Workstream S2-C: Governance Control Plane Proof

Primary lanes: Lane B and Lane E.

Deliverables:

- Policy rollout orchestration proof.
- Provider gate, rollout, and rollback proof.
- MCP connector deployment, rollout, and rollback proof.
- Eval/release rollout, orchestration, and rollback proof.

Required evidence gates:

```bash
scripts/policy-rollout-evidence-gate.sh
scripts/provider-governance-evidence-gate.sh
scripts/mcp-gateway-evidence-gate.sh
scripts/eval-release-evidence-gate.sh
```

### Workstream S2-D: Business Safety Proof

Primary lanes: Lane B, Lane C, and Lane E.

Deliverables:

- Vault/KMS/HSM rotation and recovery evidence.
- Approval notification delivery evidence against real delivery targets.
- Codex App Server deployment and production-ops evidence.
- Finance close, export delivery, and reconciliation evidence.

Required evidence gates:

```bash
scripts/vault-evidence-gate.sh
scripts/approval-notification-evidence-gate.sh
scripts/codex-app-server-evidence-gate.sh
scripts/finance-evidence-gate.sh
```

### Workstream S2-E: Adoption Closeout

Owner: integration owner.

Deliverables:

- Strict production evidence bundle run.
- Evidence archive, checksum, and manifest.
- Updated completion audit section naming the validated environment.
- Residual adoption backlog for anything still blocked.

Verification:

```bash
scripts/stage2-production-evidence-gate.sh
scripts/stage2-completion-audit-gate.sh
scripts/archive-stage2-production-evidence.sh
scripts/verify-stage2-evidence-archive.sh <archive>
```

## Stage 3 Product Roadmap

### Goal

Build the product layer above the governed runtime: first-class managed
environments, event-driven sessions, streamable session loops, environment
workers, session threads, operator-grade traces, portable workflow packs, and
governed execution substrates.

### Workstream S3-0: Managed Session Resource Model

Primary owner: integration owner.

Status: baseline landed; continue hardening the contract.

Scope:

- Keep first-class Environment as the product abstraction above runtime profiles,
  Remote Computer, Codex App Server, and future hosted runtimes.
- Keep the canonical resource chain: Agent Version -> Environment -> Session
  -> Events -> Threads.
- Continue mapping Agent Runtime Profiles and Remote Computer profiles into
  Environment versions without breaking current APIs.
- Record release gates, policy snapshots, vault reachability, MCP reachability,
  state mounts, and worker queue bindings on the Environment contract.

Acceptance criteria:

- Operators can see which Agent and Environment a Session is running under.
- The product UI no longer presents Remote Computer as the primary entrypoint.
- Existing runtime profile and Remote Computer readiness remains visible through
  the Environment view.

### Workstream S3-1: Event-Driven Session Lifecycle

Primary lanes: Lane A and Lane C.

Status: baseline landed; the remaining work is correctness and resumability.

Scope:

- Keep `POST /api/sessions/:id/events` as the main session driver.
- Keep `/api/sessions/:id/run` as a compatibility wrapper that appends a user event
  and enqueues the session loop.
- Replace demo-era persisted terminal completion with explicit idle, running,
  requires_action, rescheduling, and terminated semantics.
- Preserve the session-loop event cursor: each job records its pending event
  sequence window and advances `processed_event_seq` on completion.
- Preserve `/api/sessions/:id/stream` reconnect semantics: emitted SSE ids are
  session event sequences, and `?after_seq=` / `Last-Event-ID` replay only
  events after the caller's cursor. Keep live tailing attached after replay so
  connected clients receive newly appended session events without polling; the
  remaining hardening is production fan-out/backpressure evidence.

Acceptance criteria:

- Creating a session does not imply work starts until a user event is appended.
- A session can move through running, idle, waiting approval, rescheduling, and
  terminated states through durable events.
- The UI can explain what happened from the event stream alone.

### Workstream S3-2: Orchestrator Worker Loop And Environment Queue

Primary lanes: Lane C and Lane D.

Status: session-loop jobs exist with event cursor windows; approved execution
completion now re-enters through `execution.completed` events; worker polling can
bind to an Environment id for session-loop and execution jobs; named pool
selection and any remaining non-worker continuation sources remain.

Scope:

- Promote environment queue binding into the session-loop and execution-job
  claim path, then extend it from Environment-id filtering to named worker pools
  and autoscaler routing.
- Keep orchestrator workers claiming session-loop work when user events arrive.
- Keep the LLM/provider call outside the API request path.
- Keep approval resolution and execution-job completion routed through
  `session_loop_jobs` instead of directly resuming the provider inline.
- Route any remaining non-worker tool-result continuation sources through the
  same event-windowed loop path.
- Preserve Tool Router, Policy Engine, Approval Engine, event log, artifact, and
  audit paths as authoritative.
- Keep lower-level execution jobs for actual file, shell, Codex, MCP, and
  artifact-sync work.

Acceptance criteria:

- The Orchestrator feels always available without being a permanently running
  LLM daemon.
- The API can enqueue work and return promptly.
- Worker progress is visible as session events, not only queue rows.

### Workstream S3-A: External Scheduler Integration

Primary lane: Lane E.

Scope:

- Promote scheduler due-plan and run-due APIs into a real external scheduler contract.
- Add idempotency keys, run windows, retry policy, and run ownership metadata.
- Persist scheduler execution plans and outcomes as first-class audit evidence.
- Keep mutable actions fail-closed unless their readiness gates pass.

Acceptance criteria:

- Operators can preview, trigger, replay, and audit scheduled work.
- Scheduler runs are safe to retry.
- Stage 2 evidence jobs can use the same scheduler contract instead of bespoke orchestration.

### Workstream S3-B: Codex App Server Trace Dashboard

Primary lane: Lane C.

Scope:

- Expand per-turn traces across thread, turn, command, poll, interrupt, artifact sync, worker lease, and retry events.
- Add timeline views for terminal state, stuck turns, stale polls, and fallback to CLI.
- Surface artifact lineage from App Server response to MandoForge artifact row.

Acceptance criteria:

- An operator can answer why a Codex turn is stuck or failed without reading raw logs.
- Trace views distinguish provider failure, worker lease failure, non-terminal polling, and artifact sync failure.
- Long-running turn supervision has evidence suitable for scheduler and production ops gates.

### Workstream S3-C: Production Remote Computer Execution

Primary lane: Lane D.

Scope:

- Productize Remote Computer as `Environment(type=remote_computer)`, the
  self-hosted sandbox substrate behind managed sessions.
- Productize the assigned-Pod execution transport that Stage 2 keeps behind explicit gates.
- Keep Tool Router, Policy Engine, Approval Engine, event log, and audit paths authoritative.
- Support cancellation through assigned Pod deletion or command interruption.
- Capture stdout/stderr bounds, final artifacts, sidecar heartbeats, state locks, and assignment lifecycle.

Acceptance criteria:

- Approved `file.write`, `shell.exec`, and `codex.exec` can run through an assigned Remote Computer without bypassing policy or audit.
- Failed or stale Remote Computer assignments are recoverable through worker lease retry.
- Artifact discovery and push sync produce replayable evidence.

### Workstream S3-D: WorkflowPack / DomainPack Platform

Primary lanes: Lane A, Lane B, and Lane E.

Scope:

- Define the `WorkflowPack` and `DomainPack` manifest contract, starting with `schemas/workflow-pack-manifest.schema.json`.
- Validate pack layout, connectors, schemas, policies, evals, profiles, and worker roles.
- Add profile onboarding contracts for company, department, approval matrix, connector map, risk policy, and output style.
- Ship the first AI Governance Pack slice.

Acceptance criteria:

- A pack can be installed, validated, staged, evaluated, and released without silently changing runtime safety.
- Connectors declare provenance, tenant scope, write gating, and prompt-injection boundaries.
- Pack behavior has eval fixtures and release gates.

### Workstream S3-E: Typed Agent Team Handoffs

Primary lanes: Lane A, Lane B, and Lane C.

Status: durable `session_threads` exist; thread membership and handoff
semantics need tightening.

Scope:

- Keep `session_threads` as the primary multiagent execution surface.
- Continue migrating typed `agent_handoff_events` into thread lifecycle events where the
  handoff is the request and the thread is the durable execution object.
- Ensure specialist sessions can enumerate their own child/specialist thread
  membership, not only receive lifecycle events emitted by the source session.
- Enforce allowlisted source agent, target agent, intent enum, schema version, risk level, and approval requirement.
- Persist request, accept, reject, fail, and complete transitions in timeline and audit logs.
- Support Reader / Analyzer / Writer role boundaries for untrusted input workflows.

Acceptance criteria:

- Agent-to-agent routing is not free-form natural language.
- Child agent work is visible as a thread under the parent session.
- High-risk handoffs require approval before downstream work starts.
- Handoff chains are replayable and visible in the operator console.

### Workstream S3-G: Session-First UI

Primary lanes: Lane A and Lane C.

Status: the first managed-session workspace shell exists; continue making it
operationally exact.

Scope:

- Keep the first screen centered on Session intake, Agent selection, Environment
  selection, current stream, blocking actions, artifacts, and child threads.
- Move raw infrastructure panels behind an Infrastructure tab.
- Show "what runs where" explicitly: API harness, orchestrator worker,
  environment worker, Remote Computer, Codex App Server, or MCP.
- Expose approvals and custom tool requests as resumable session blockers.

Acceptance criteria:

- A new operator can explain the flow without reading infra logs.
- The UI distinguishes demo harness mode from queue-backed environment-worker
  mode.
- The UI makes it obvious whether the session is waiting for an approval, a
  worker, a tool result, or the model.

### Workstream S3-F: Product Hardening

Primary owner: integration owner.

Scope:

- Split remaining global hotspots before they block parallel work.
- Keep readiness summaries consistent across lanes.
- Add migration numbering discipline for parallel branches.
- Preserve strict CI gates and add targeted Stage 3 verification scripts.

Acceptance criteria:

- Multiple lane agents can develop Stage 3 features without repeatedly colliding in `main.rs`.
- Every Stage 3 feature has a lane-local verification path and an integration gate.

## Recommended Execution Order

1. Keep Stage 2 production adoption evidence work separate from product roadmap work.
2. Preserve the landed managed-agent baseline: Environment, Session, Events,
   session-loop jobs, Threads, Remote Computer environment policy, and
   session-first UI.
3. Fix resumable session state semantics and event-cursor processing.
4. Route every continuation through `session_loop_jobs` and add environment
   queue binding.
5. Harden live streaming, specialist thread membership, and lease-fenced job
   finalization.
6. Run the managed-session runtime evidence gate for API/worker restart and
   recovery against a real production target.
7. Then expand WorkflowPack, scheduler, Codex traces, and production Remote
   Computer execution on top of the managed-session runtime.

## Non-Goals

- Do not claim a production environment is validated from local mock controller evidence.
- Do not allow Workflow Packs to bypass tenant scope, policy, approval, or audit.
- Do not let Remote Computer execution become a side channel around the Tool Router.
- Do not add free-form agent handoff messages as the primary routing contract.
- Do not expose worker queue internals as the primary product model.
- Do not model the Orchestrator as an always-running LLM daemon.
- Do not mix Stage 2 production adoption evidence changes with unrelated Stage 3 product work in one commit.
