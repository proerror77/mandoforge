# Agent OS Product Roadmap

This roadmap tracks the product architecture for MandoForge as an Agent OS.

## Architecture

The product architecture follows the Full Agent OS narrative:

```text
Work Surfaces
  Feishu / Slack / GitHub / Jira / Linear / Email / Browser
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review / Activity Feed / Squad
        |
Manager Agent Layer
  Intake / Plan / Decompose / Route / Delegate / Escalate / Review
        |
Managed Runtime Layer
  Agent / Environment / Session / Events / Threads / Runtime Turns
        |
Governance Layer
  Policy / Approval / RBAC / TaskGrant / Audit / Eval / Release / Rollback
        |
Ontology Action Contract Layer
  Business Object / Rule / Relation / Metric / Action / Tool Binding / Validation
        |
Environment Scheduling Layer
  Environment Work Queue / K Agent / Worker Lease / Sandbox Lifecycle / CLI Dispatch
        |
Execution Substrate
  Codex / Claude Code / CMA / MCP / SQL / Shell / Remote Computer / APIs
```

Claude Managed Agents remains a useful reference for the Managed Runtime Layer:
Agent, Environment, Session, Events, and Threads. It is not the complete
MandoForge architecture.

Palantir AIP remains a useful reference for enterprise operation patterns:
context engineering, purpose-based controls, package/release/deploy, and
Human+AI operation surfaces. MandoForge does not copy AIP's ontology-centered
data-platform boundary.

## Current Baseline

The Managed Runtime Layer has a strong baseline:

- First-class Agent records for managed runtime identity, role, model, tool,
  skill, WorkflowPack, semantic-scope, and runtime-profile binding.
- First-class Environment resources.
- Sessions bound to agents and environments.
- `/api/sessions/:id/events` as the event-driven session input path.
- Queue-claimed `session_loop_jobs` with event cursor windows.
- `k_agent.claimed` session events for Environment-bound session-loop claims,
  recording the worker pool, worker id, lease expiry, attempt count, cursor
  window, dispatch surface, and Environment Scheduling authority boundary.
- `k_agent.heartbeat` session events for running session-loop jobs, extending
  the active worker lease while preserving the same Environment Scheduling
  authority boundary.
- `k_agent.completed` and `k_agent.failed` session events for session-loop K
  Agent return evidence across direct session-loop and workflow-step run
  surfaces, preserving the worker identity, worker pool, cursor window,
  processed sequence, job/session status, and Environment Scheduling authority
  boundary.
- `k_agent.claimed`, `k_agent.completed`, and `k_agent.failed` session events
  for `/api/execution-jobs/:id/run`, recording the execution job id, approval
  id, tool call id, tool, Environment, worker identity, worker pool, attempt
  count, retry status, dispatch surface, and Environment Scheduling authority
  boundary.
- Runtime adapter normalization for Codex CLI, Claude Code CLI, and Codex App
  Server turn output.
- Runtime Turn evidence is recorded through existing session-scoped surfaces:
  `runtime.turn.started`, `runtime.item`, `runtime.tool_call`,
  `runtime.usage`, `runtime.final`, `runtime.turn.completed`,
  `artifact.created`, and `execution.completed`, with session events,
  tool-call readback, artifacts, and audit logs as the source of truth rather
  than provider stdout replay.
- `agent_cli.exec` remains a compatibility facade and records the session-bound
  runtime binding source (`environment`, `handoff`, `agent`, or legacy
  `requested`) in execution results, session events, and audit evidence.
- Durable SessionThread records for primary and specialist runtime lanes,
  manager-to-specialist handoff, status readback, and event/audit evidence.
- Tool Router, Policy Engine, Approval Engine, Artifact Store, and Audit Logger.
- SSE event streaming with reconnect cursors.
- Session-first UI shell for Agent, Environment, Event Stream, Blocking
  Actions, Artifacts, and Threads.

The first Collaboration Layer slice is now in place:

- `/api/work-surface-events` ingests external Work Surface events into
  WorkItems without starting runtime execution, preserving surface identity,
  event type, external id, source URL, actor, timestamp, metadata, Activity Feed
  evidence, and audit evidence.
- The shared Work Surface intake now records webhook-authentication evidence,
  verifies configured HMAC signatures, preserves cursor and delivery ids, and
  treats repeated cursor or delivery submissions as replay evidence
  on the existing WorkItem rather than starting a duplicate runtime path.
- The same intake path normalizes Feishu, Slack, GitHub, Jira, Linear, and
  Email events into canonical surface adapters and preserves owner, reviewer,
  assignee, blocker, due date, source URL, and status metadata for operator
  review, plus extracted object, repository, channel, thread, project, label,
  and mention fields where the platform payload exposes them.
- Email Work Surface intake preserves inbound mail authentication evidence from
  relay headers and payload metadata, including Authentication-Results,
  Received-SPF, DKIM, ARC, Message-ID, provider, and SPF/DKIM/DMARC verdicts,
  while marking that evidence as observed rather than MandoForge-enforced.
- `/api/work-items` creates and lists first-class WorkItems.
- WorkItem creation persists source, priority, status, scope, and metadata.
- `/api/work-items/:id/assignments` routes WorkItems to users, agents, squads,
  or teams.
- `/api/work-items/:id/reviews` records user, agent, squad, or team review
  decisions.
- `/api/work-items/:id/activity` exposes the human-readable Activity Feed for
  intake, routing, and review.
- `/api/capability-discovery` now exposes an Agent OS product capability catalog
  for WorkItem, ManagerPlan, AgentHandoff, WorkflowPack, DomainPack, Agent,
  AgentVersion, Environment, EnvironmentProfile, ManagedSession, SessionThread,
  RuntimeTurn, ContextPacket, ArtifactStore, AuditLogger, RemoteComputer,
  KAgent, TaskGrant, PolicyEngine, Approval, ToolRouter,
  OntologyActionContract, ToolSpec, EvalGate, Release, and Rollback, including
  the existing API surfaces, lifecycle actions, audit/evidence events, and
  authority boundaries for each capability.
- The Agent entry exposes existing managed runtime identity, version, agent
  card, runtime-profile binding, tool/skill/workflow-pack, and semantic-scope
  records while preserving the boundary that Agent cannot execute work, release
  itself, issue TaskGrant scope, approve actions, bypass Policy or Tool Router,
  or replace session events and audit evidence.
- The Environment entry exposes existing placement and binding records for
  runtime profiles, worker queues, RemoteComputer profiles, state mounts,
  network policy, vault requirements, and MCP requirements while preserving the
  boundary that Environment cannot execute work, issue TaskGrant scope, approve
  actions, bypass Policy or Tool Router, or replace session events and audit
  evidence.
- The SessionThread entry exposes existing primary/specialist thread readback,
  handoff assignment, session-loop status-change evidence, and thread events
  while preserving the boundary that threads cannot execute tools, create
  TaskGrant scope, approve actions, bypass Policy or Tool Router checks, or
  replace session events and audit evidence.
- The RuntimeTurn entry exposes existing runtime adapter turn creation, polling,
  item normalization, tool-call and usage recording, final artifact persistence,
  completion, and session-loop resume surfaces while preserving the boundary
  that RuntimeTurn cannot authorize work, replace the Session event log, bypass
  Manager Runtime, TaskGrant, Policy, Approval, Tool Router, or Environment
  scheduling, or treat provider stdout replay as truth.
- The ContextPacket entry exposes existing session-scoped generation,
  versioned readback, rendered execution context, semantic evidence, policy
  reminders, runtime profile, and replay source references while preserving the
  boundary that context construction cannot issue TaskGrant scope, approve
  actions, bypass Tool Router checks, or grant access outside session
  visibility.
- The ArtifactStore and AuditLogger entries in `/api/capability-discovery`
  expose existing session-scoped artifact readback, RemoteComputer artifact
  sync/discovery evidence, collection/session audit-log readback, common audit
  actions, and explicit boundaries showing that evidence storage cannot
  authorize tools, approve business actions, or replace Manager Runtime,
  TaskGrant, Policy, Approval, or Tool Router checks.
- The PolicyEngine entry exposes existing policy runtime, simulation, test,
  revision, rollout, rollback, and orchestration-readiness surfaces while
  preserving the boundary that policy can require approval or deny work but
  cannot issue TaskGrant scope, approve business actions, bypass exact tool
  routing, or replace audit evidence.
- The KAgent entry exposes existing execution-job queue, session-loop worker,
  heartbeat, worker-readiness, worker-load-validation, RemoteComputer lease
  assignment, and `k_agent.*` return-evidence surfaces while preserving the
  boundary that K Agent schedules and dispatches already-authorized work but
  does not own ManagerPlan, TaskGrant, Policy, Approval, Ontology validity,
  WorkflowPack release, or audit truth.
- `/api/workflow-packs/installations/:id/capabilities` provides read-only
  per-installation capability evidence for WorkflowPack and DomainPack
  installations, summarizing manifest capabilities, gate evidence, bindings,
  runtime objects, workflow definition release states, and audit trace without
  creating any execution side channel.
- `/api/work-surface-events/capability-readback` provides read-only Work
  Surface connector evidence from existing WorkItem metadata, including
  webhook verification, replay detection, observed rate-limit evidence,
  observed live-readback evidence, and Email authentication evidence without
  starting runtime execution.
- `/api/agents/:id/versions/:version/capability-readback` provides read-only
  AgentVersion capability evidence from existing version, runtime contract,
  tool contract, policy contract, EvalRun evidence, and AgentRelease evidence
  without turning a version record into release or execution authority.
- `/api/agent-runtime-profiles/:id/capability-readback` provides read-only
  EnvironmentProfile capability evidence from existing runtime profile,
  release gate, environment bindings, and agent bindings without turning an
  environment profile into execution authority.
- `/api/ontology/onboarding/runs/:id/tool-specs/capability-readback` provides
  read-only ToolSpec capability evidence from existing ontology onboarding run
  state and materialized tool specs without granting TaskGrant, Policy,
  Approval, connector scope, or Tool Router execution authority.
- `/api/eval/runs/:id/capability-readback` provides read-only EvalGate
  capability evidence from existing EvalRun, EvalCase, gate decision, and drift
  decision state without turning eval gates into execution authority.
- `/api/agent-teammates`, `/api/squads`, and `/api/squads/:id/members` expose
  collaboration identities for runtime agents and squads without adding a
  second runtime orchestrator.
- `/api/work-items/:id/manager-plans` exposes Manager Agent planning records
  bound to the WorkItem without starting specialist runtime execution.
- `/api/manager-plans/:id/materialize-handoff` turns a reviewed or approved
  ManagerPlan into an accepted handoff, specialist assignment, child
  SessionThread, and, when a parent WorkflowRun TaskGrant is active, the
  downstream WorkflowStepRun and child TaskGrant. High-risk or approval-required
  handoffs still fail closed instead of being auto-accepted.
- `/api/manager-plans/:id/materialize-workflow-run` turns a reviewed or
  approved non-high-risk ManagerPlan, or an approval-gated high-risk
  ManagerPlan, into a WorkflowRun through the shared WorkflowDefinition runtime
  path, preserving the source WorkItem, creating the
  primary Session, issuing the root TaskGrant, materializing start
  WorkflowStepRuns, and recording ManagerPlan evidence in input/runtime
  envelopes, session events, WorkItem Activity Feed, and audit logs. Repeated
  materialization for the same ManagerPlan and WorkflowDefinition returns the
  existing WorkflowRun without replacing its input payload or duplicating
  materialization evidence. High-risk ManagerPlans still fail closed unless the
  request includes an approved, session-bound Approval for the same ManagerPlan
  and WorkflowDefinition.
- WorkItems with `metadata.semantic_scopes` are projected into `semantic_objects`
  as `work_item:*` records so context packets can retrieve Collaboration Layer
  work as runtime context.
- Ontology Action Contract gates accept both legacy `ontology_action_type`
  objects and richer `ontology_action_contract` objects with explicit
  BusinessObject, Rule, Relation, Metric, PermissionContract, ToolBinding,
  ValidationRule, and RiskClass evidence in the policy decision.
- WorkItem intake, routing, review, activity, Agent Teammate/Squad, and Manager
  Plan binding, and WorkItem semantic projection write `work_item.created`,
  `work_item.assignment_created`, `work_item.review_created`,
  `agent_teammate.created`, `squad.created`, `squad.member_added`,
  `manager_plan.created`, `manager_plan.reviewed`, and
  `work_item.semantic_object_projected` audit/activity evidence and are covered by
  `scripts/work-item-collaboration-evidence-gate.sh`.

## Near-Term Priority

The next product slice is to preserve Agent OS runtime correctness while moving
up the stack.

1. Keep the event-driven session loop as the single execution path.
2. Keep every agent action recorded in `session_events`, `tool_calls`, and
   `audit_logs`.
3. Keep approval decisions and execution completions flowing back through
   durable events rather than direct provider resume paths.
4. Keep worker claims bound to Environment and worker queue bindings.
5. Keep SSE reconnect semantics so UI clients can resume without missing
   events.

## Full Agent OS Implementation Phases

1. Runtime Contract: keep event-driven sessions, runtime turns, `requires_action`,
   and environment-bound runtime adapters on the single session-loop path.
2. Environment Scheduling + K Agent: isolate sandbox and CLI dispatch from
   business authority while preserving worker lease, event, artifact, and audit
   evidence.
3. Manager Runtime: make Manager Agents operate on WorkItems, ManagerPlans,
   Assignments, Reviews, WorkflowRuns, SessionThreads, and TaskGrants.
4. Ontology Action Contract: use Ontology for action validity, rules, tool
   bindings, and validation; keep TaskGrant, Policy, Approval, and Tool Router
   as execution authority.
5. Pack / Release / Evidence: make WorkflowPack, DomainPack, AgentVersion,
   EnvironmentProfile, OntologyActionContract, ToolSpec, EvalGate, Release, and
   Rollback installable and auditable.

## Product Workstreams

### S1: Collaboration Layer

Goal: make external work surfaces converge into first-class Agent OS objects.

Scope:

- Add or harden WorkItem, Project, Assignment, Review, Agent Teammate, Squad,
  and Activity Feed objects.
- Map Slack, Feishu, GitHub, Jira, Linear, and Email events into WorkItems and
  Activity Feed entries.
- Preserve human-visible state: owner, reviewer, assignee, blocker, due date,
  source URL, and current status.

Acceptance:

- A user can understand what work exists without reading raw session logs.
- An agent can create, update, route, and review the same work objects the UI
  exposes.

### S2: Manager Agent / Work Coordination Layer

Goal: make Manager Agents operate on WorkItems instead of ad hoc prompts while
running on the same Managed Runtime as every other agent.

Scope:

- Persist task intake, decomposition, specialist routing, escalation, and result
  review as structured manager-plan records.
- Connect manager plans to WorkItems, Assignments, specialist sessions, and
  reviews.
- Keep high-risk routing and downstream execution behind policy and approval.
- Keep runtime orchestration in the Managed Runtime Layer. Manager Agents produce
  work-coordination decisions; they do not own tool execution, queues, approval,
  audit, or session resume.

Acceptance:

- Manager decisions are inspectable before and after execution.
- Specialist work is represented as child threads or assignments, not hidden
  model context.

### S3: Managed Runtime Layer

Goal: finish long-running session correctness.

Scope:

- Keep `POST /api/sessions/:id/events` as the main driver.
- Keep `/run` as a compatibility wrapper that appends a user event.
- Keep explicit lifecycle states: `idle`, `running`, `requires_action`,
  `rescheduling`, `terminated`, and `failed`.
- Preserve session-loop cursor windows: pending sequence start/end plus
  processed high-water mark.
- Route user messages, approvals, custom tool results, and worker completions
  through the same session-loop path.
- Call selected CLI/runtime adapters through Environment runtime profiles.
- Keep runtime turn metadata for resume handles, usage, timing, final messages,
  collected items, and tool calls.

Acceptance:

- A worker can restart and resume without reprocessing already consumed events.
- The UI can explain the run from the event stream alone.
- Every tool action has a tool-call row and audit trail.

### S4: Ontology Action Contract

Goal: give agents stable business context instead of loose prompt stuffing.

Scope:

- Keep semantic sources, objects, links, and context packets as the first
  semantic contract.
- Model business objects, metrics, relations, actions, permissions, tool
  bindings, retrieval context, and data contracts explicitly.
- Keep context packet replay in the timeline so operators know what context an
  agent saw before acting.
- Require Ontology Action Contract validity before governed connector tool
  execution when the TaskGrant declares that boundary.
- Keep TaskGrant, Policy, Approval, and Tool Router as the execution authority;
  Ontology only validates the business action shape, rule binding, and tool
  binding.
- Keep memory writeback approval-gated.

Acceptance:

- Agents can retrieve and act on business objects with provenance, freshness,
  and permission checks.
- Operators can inspect why a context packet was built and which source objects
  contributed to it.
- A read-only valid action can execute without approval when TaskGrant,
  connector scope, and policy allow it.
- A high-risk valid action still enters approval / `requires_action`.
- An ontology-valid action cannot bypass TaskGrant, Policy, Approval, or
  connector scope.

### S5: WorkflowPack / DomainPack Platform

Goal: let domain workflows run on top of the Agent OS without becoming the OS.

Scope:

- Keep WorkflowPack and DomainPack manifests installable, staged, evaluated,
  released, and rolled back.
- Keep connector provenance, tenant scope, write gating, and prompt-injection
  boundaries in pack contracts.
- Keep pack eval fixtures and release gates.

Acceptance:

- A domain pack can add business behavior without bypassing runtime policy,
  approval, event logging, audit, or semantic context rules.

### S6: Remote Computer As Environment Substrate

Goal: keep Remote Computer under Environment, not above Agent OS.

Scope:

- Treat Remote Computer as `Environment(type=remote_computer)`.
- Keep approved `file.write`, `shell.exec`, and `codex.exec` behind Tool Router,
  Policy Engine, Approval Engine, event log, artifact store, and audit logs.
- Keep artifact sync and sidecar/lease state as runtime implementation details.

Acceptance:

- Remote execution is visible as ordinary session events, tool calls, artifacts,
  and audit entries.
- Remote Computer never becomes a side channel around the Agent OS kernel.

## Non-Goals

- Do not let Claude Managed Agents replace the broader Agent OS stack.
- Do not let Manager Agents become a separate runtime orchestrator.
- Do not expose worker queue internals as the primary product model.
- Do not model the runtime session loop as an always-running LLM daemon.

## Follow-Up Implementation Plans

The Full Agent OS narrative should be implemented through separate focused
plans:

1. Work Surface connectors: the shared `/api/work-surface-events` intake now
   normalizes Feishu, Slack, GitHub, Jira, Linear, and Email into canonical
   WorkItem adapter metadata without starting runtime execution, verifies
   configured webhook HMAC signatures, records cursor/delivery replay evidence,
   extracts richer platform object metadata, accepts GitHub, Slack, Linear, and
   Jira native webhook signature headers plus Feishu/Lark `X-Lark-Signature`
   as connector-specific verification variants, and
   preserves platform rate-limit, live-readback, and Email-specific
   authentication evidence supplied by connector headers or payload metadata.
   `/api/work-surface-events/capability-readback` now exposes that observed
   connector evidence without claiming execution authority. Platform-specific
   OAuth/token lifecycle, active live API fetch/readback, and
   MandoForge-enforced rate-limit scheduling remain open.
2. Runtime adapter consolidation: make Environment runtime binding the only
   product entrypoint for managed runtime execution. `agent_cli.exec` and
   `codex.exec` remain compatibility facades, but session-bound Environment
   runtime profiles are now authoritative for `agent_cli` profile selection and
   `codex_cli` / `codex_app_server` strategy selection, with binding evidence in
   results, events, audit, and K Agent dispatch records. `codex.exec` no longer
   executes from request/env strategy without a session Environment binding, and
   `agent_cli.exec` no longer accepts the requested-profile escape hatch for
   unbound sessions or falls back to agent/handoff runtime profiles outside the
   session Environment binding.
3. Environment Scheduling + K Agent: complete the Environment Work Queue,
   sandbox dispatch, and full runtime event/artifact return contract.
   Session-loop K Agent claim, lease, heartbeat, completion, and failure
   evidence are already recorded as `k_agent.claimed`, `k_agent.heartbeat`,
   `k_agent.completed`, and `k_agent.failed`; completion/failure return
   evidence is shared across direct session-loop and workflow-step run
   surfaces. Execution-job K Agent claim and return evidence is also recorded
   for `/api/execution-jobs/:id/run`, including runtime artifact return counts
   and final artifact IDs from the completed tool result, plus artifact lineage
   from the session `artifact.created` chain. K Agent execution-job events also
   record dispatch evidence for sandbox mode, runtime profile, execution
   strategy, and Remote Computer assignment. K Agent execution-job events now
   also carry `k_agent_sandbox_lifecycle_dispatch` evidence that summarizes the
   Remote Computer handoff/acknowledgement/transport/terminal lifecycle stages
   from session events. Remote Computer readiness now includes sidecar
   supervision, sidecar recovery validation, and production state-sync
   controller evidence. Production-grade Pod lifecycle automation, continuous
   sidecar artifact sync, and customer-grade multi-node state synchronization
   evidence remain open.
4. Manager Runtime materialization: expand the reviewed ManagerPlan
   materialization policy with richer WorkflowRun selection. The current
   baseline already supports WorkflowRun-first materialization for reviewed
   non-high-risk ManagerPlans, approval-gated materialization for reviewed
   high-risk ManagerPlans, idempotent reuse for the same ManagerPlan and
   WorkflowDefinition, released WorkflowDefinition selection by explicit id,
   request selector, or ManagerPlan specialist-selection entrypoint/name, and
   handoff materialization into Assignment, SessionThread, and child TaskGrant
   evidence when a parent WorkflowRun grant is active.
5. Ontology Action Contract object model: deepen BusinessObject, Rule,
   Relation, Metric, ActionContract, PermissionContract, ToolBinding,
   ValidationRule, and RiskClass representation beyond the current checked
   `ontology_action_type` contract gate. The gate already accepts richer
   `ontology_action_contract` semantic objects with nested ToolBinding and
   model evidence. Ontology release candidates now package each materialized
   action object's id/type/key and action contract model evidence into
   `evidence_refs`. Standalone `OntologyActionContract` objects now have a
   first-class read/list product API at `/api/ontology/action-contracts` with an
   explicit governance boundary showing that the contract grants action
   validity only, while TaskGrant, Policy, Approval, connector scope, and the
   Tool Router remain execution authority. Standalone
   `OntologyActionContract` objects can now be packaged into ontology release
   candidates through `/api/ontology/action-contracts/:id/release-candidate`;
   the release evidence includes the action object id/type/key, contract model,
   transaction profile, execution mode, and existing ontology release gates.
6. Pack / Release / Evidence: `/api/capability-discovery` now lists
   WorkItem, ManagerPlan, AgentHandoff, WorkflowPack, DomainPack, Agent,
   AgentVersion, Environment, EnvironmentProfile, ManagedSession,
   SessionThread, RuntimeTurn, ContextPacket, ArtifactStore, AuditLogger,
   RemoteComputer, KAgent, TaskGrant, PolicyEngine, Approval, ToolRouter,
   OntologyActionContract, ToolSpec, EvalGate, Release, and Rollback as
   auditable product capabilities with their existing routes, lifecycle actions,
   evidence events, and authority boundaries. ContextPacket is a scoped context
   construction/readback surface, not execution authority. Agent is the managed
   runtime identity and contract surface, not execution or release authority.
   Environment is the
   placement and binding surface for runtime profiles, worker queues, and
   substrate requirements, not execution authority. SessionThread is the
   readback and isolation surface for primary/specialist runtime lanes, not a
   tool execution or authorization surface. RuntimeTurn is the
   session-scoped adapter-output normalization surface for runtime events,
   tool-call evidence, usage, final artifacts, and audit readback, not
   execution authority or provider-stdout truth. ArtifactStore and AuditLogger
   are evidence/readback surfaces, not execution or
   business-authorization shortcuts; PolicyEngine can gate or deny work but does
   not issue TaskGrant scope or replace Tool Router and audit checks. KAgent is
   the Environment Scheduling worker/dispatch surface, not business authority.
   WorkflowPack and DomainPack
   installations now also expose read-only per-installation
   capability readback through
   `/api/workflow-packs/installations/:id/capabilities`, including gate
   evidence, bindings, runtime objects, workflow definition release states, and
   audit trace. AgentVersion now exposes read-only capability readback through
   `/api/agents/:id/versions/:version/capability-readback`, including runtime,
   tool, policy, eval, and release evidence. EnvironmentProfile now exposes
   read-only capability readback through
   `/api/agent-runtime-profiles/:id/capability-readback`, including runtime
   profile state, release gate, environment bindings, and agent bindings.
   ToolSpec now exposes read-only capability readback through
   `/api/ontology/onboarding/runs/:id/tool-specs/capability-readback`,
   including run state, materialized specs, approval requirements, execution
   modes, and risk distribution. EvalGate now exposes read-only capability
   readback through
   `/api/eval/runs/:id/capability-readback`, including run state, case ids,
   gate decision, drift decision, and authority boundary.
