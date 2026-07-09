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

- First-class Environment resources.
- Sessions bound to agents and environments.
- `/api/sessions/:id/events` as the event-driven session input path.
- Queue-claimed `session_loop_jobs` with event cursor windows.
- Runtime adapter normalization for Codex CLI, Claude Code CLI, and Codex App
  Server turn output.
- Durable session threads for manager-to-specialist handoff.
- Tool Router, Policy Engine, Approval Engine, Artifact Store, and Audit Logger.
- SSE event streaming with reconnect cursors.
- Session-first UI shell for Agent, Environment, Event Stream, Blocking
  Actions, Artifacts, and Threads.

The first Collaboration Layer slice is now in place:

- `/api/work-items` creates and lists first-class WorkItems.
- WorkItem creation persists source, priority, status, scope, and metadata.
- `/api/work-items/:id/assignments` routes WorkItems to users, agents, squads,
  or teams.
- `/api/work-items/:id/reviews` records user, agent, squad, or team review
  decisions.
- `/api/work-items/:id/activity` exposes the human-readable Activity Feed for
  intake, routing, and review.
- `/api/agent-teammates`, `/api/squads`, and `/api/squads/:id/members` expose
  collaboration identities for runtime agents and squads without adding a
  second runtime orchestrator.
- `/api/work-items/:id/manager-plans` exposes Manager Agent planning records
  bound to the WorkItem without starting specialist runtime execution.
- WorkItems with `metadata.semantic_scopes` are projected into `semantic_objects`
  as `work_item:*` records so context packets can retrieve Collaboration Layer
  work as runtime context.
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

### S4: Semantic Layer / Ontology Service

Goal: give agents stable business context instead of loose prompt stuffing.

Scope:

- Keep semantic sources, objects, links, and context packets as the first
  semantic contract.
- Model business objects, metrics, relations, actions, permissions, tool
  bindings, retrieval context, and data contracts explicitly.
- Keep context packet replay in the timeline so operators know what context an
  agent saw before acting.
- Keep memory writeback approval-gated.

Acceptance:

- Agents can retrieve and act on business objects with provenance, freshness,
  and permission checks.
- Operators can inspect why a context packet was built and which source objects
  contributed to it.

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
