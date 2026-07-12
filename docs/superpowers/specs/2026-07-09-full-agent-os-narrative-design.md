# Full Agent OS Narrative Design

## Product Boundary

MandoForge should be a runtime-centered Enterprise Agent OS. It should not be a
Claude Managed Agents clone, a Palantir AIP clone, or a general enterprise data
platform.

The product center is the Manager Runtime and Managed Runtime:

```text
Who started the work?
Who decomposed it?
Who was assigned?
Which agent ran?
Which environment ran it?
Which context did it see?
Which tool did it call?
Was the call authorized?
Who approved it?
What happened?
Can the run be replayed?
Can the capability be released or rolled back?
```

Claude Managed Agents is a runtime reference for:

```text
Agent -> Environment -> Session -> Events -> Threads
```

Palantir AIP is an enterprise operation reference for:

```text
Context Engineering
Ontology-backed actions
Purpose-based governance
Package, release, and deploy
Human and AI application surfaces
Operational automation
```

MandoForge owns the runtime truth and authority boundary:

```text
MandoForge turns agent work into governed, replayable enterprise operations.
```

## Layered Architecture

The Agent OS narrative has eight layers. Each layer has one job and hands off to
the next layer through durable objects.

```text
Work Surfaces
  Feishu / Slack / GitHub / Jira / Linear / Email / Browser

Collaboration Layer
  WorkItem / Project / Assignment / Review / Activity Feed / Squad

Manager Agent Layer
  Intake / Plan / Decompose / Route / Delegate / Escalate / Review

Managed Runtime Layer
  Agent / Environment / Session / Events / Threads / Runtime Turns

Governance Layer
  Policy / Approval / RBAC / TaskGrant / Audit / Eval / Release / Rollback

Ontology Action Contract Layer
  Business Object / Rule / Relation / Metric / Action / Tool Binding / Validation

Environment Scheduling Layer
  Environment Work Queue / K Agent / Worker Lease / Sandbox Lifecycle / CLI Dispatch

Execution Substrate
  Codex / Claude Code / CMA / MCP / SQL / Shell / Remote Computer / APIs
```

### Work Surfaces

Work Surfaces ingest real work from external systems. They should not directly
start business execution.

They are responsible for:

- Capturing external events and source URLs.
- Preserving source identity and timestamps.
- Creating or updating Collaboration Layer objects.

They are not responsible for:

- Task decomposition.
- Business authorization.
- Tool execution.

### Collaboration Layer

The Collaboration Layer turns external events into human-visible work.

It is responsible for:

- WorkItems, Projects, Assignments, Reviews, Activity Feed entries, and Squads.
- Shared state that both humans and agents can inspect.
- Source, owner, reviewer, assignee, blocker, due date, priority, and status.

It is not responsible for:

- Running agent loops.
- Deciding tool authorization.
- Hiding work inside prompts.

### Manager Agent Layer

Manager Agents coordinate work while running on the same Managed Runtime as all
other agents.

They are responsible for:

- Intake interpretation.
- Planning and decomposition.
- Specialist routing.
- Escalation.
- Result review.
- Creating ManagerPlan, Assignment, Review, child thread, or WorkflowRun
  materialization requests.

They are not responsible for:

- Owning a second runtime orchestrator.
- Executing tools directly.
- Bypassing policy, approval, audit, TaskGrant, or event replay.

### Managed Runtime Layer

The Managed Runtime Layer is the action truth layer. It follows the CMA-style
contract where it fits MandoForge:

```text
Agent -> Environment -> Session -> Events -> Threads
```

It is responsible for:

- Versioned agents.
- First-class environments.
- Resumable sessions.
- Durable event streams.
- SessionLoopJobs with event cursor windows.
- Primary and specialist SessionThreads.
- Runtime turn normalization.
- Streaming and replay.

It is not responsible for:

- Business policy ownership.
- External system authority.
- Long-lived always-thinking daemons.

### Governance Layer

The Governance Layer decides whether a requested action may proceed.

It is responsible for:

- Policy decisions.
- RBAC.
- TaskGrant scoping.
- Approval and ApprovalCommitToken gates.
- Audit logs.
- Eval gates.
- Release and rollback controls.

It is not responsible for:

- Defining business semantics.
- Running CLI or sandbox processes.
- Replacing runtime event evidence.

### Ontology Action Contract Layer

Ontology is not the product center. It is the business action contract.

It is responsible for:

- Business objects.
- Relations.
- Rules.
- Metrics.
- Action contracts.
- Permission contracts.
- Tool bindings.
- Validation rules.
- Risk classes.

It is not responsible for:

- Granting execution authority by itself.
- Mutating production state without review.
- Becoming a full data platform or digital twin.

### Environment Scheduling Layer

The Environment Scheduling Layer runs work in the right place without becoming
the authority model.

It is responsible for:

- Environment work queues.
- K Agent worker claims.
- Worker leases.
- Sandbox, Pod, or warm-pool selection.
- Workspace and state mount setup.
- CLI adapter dispatch.
- Heartbeats, timeout, retry, cleanup, and artifact sync.

It is not responsible for:

- ManagerPlan ownership.
- Policy decisions.
- Approval decisions.
- TaskGrant expansion.
- Ontology validation.
- WorkflowPack release.

### Execution Substrate

The Execution Substrate is the set of hands that perform work.

It is responsible for:

- Running Codex, Claude Code, CMA, MCP tools, SQL, shell, Remote Computer, and
  native APIs through approved adapters.
- Returning runtime events, tool results, artifacts, usage, and errors.

It is not responsible for:

- Owning business authorization.
- Becoming the source of replay truth.
- Writing external state outside Tool Router and governance controls.

## Core Data Flow

The main flow for real work is:

```text
External Work Surface
  -> WorkItem
  -> ManagerPlan
  -> Assignment / Review
  -> WorkflowRun or Session
  -> TaskGrant
  -> ContextPacket
  -> RuntimeTurn
  -> Tool / Approval / Artifact
  -> Audit / Timeline / Release Evidence
```

Detailed flow:

1. Work Surface intake captures an external event and creates a WorkItem.
2. Collaboration state records source, owner, status, priority, due date,
   assignee, reviewer, and activity.
3. Manager Agent planning produces a reviewable ManagerPlan.
4. Reviewed plans materialize into WorkflowRuns, WorkflowStepRuns, Sessions,
   SessionThreads, or DynamicWorkflowPlans.
5. Each executable unit receives a TaskGrant.
6. ContextPacket construction uses TaskGrant and Ontology Action Contract
   boundaries.
7. Managed Runtime execution processes events through SessionLoopJobs.
8. Tool Router checks Policy, TaskGrant, Approval, ConnectorScope, secret
   boundaries, and Ontology Action Contract validity.
9. Environment Scheduling claims the work and dispatches the selected sandbox or
   CLI adapter.
10. Execution Substrate returns runtime events, tool results, artifacts, and
    errors.
11. Durable records power replay, audit, activity feeds, and release evidence.

Design invariants:

```text
External work does not directly enter agent execution.
Agent plans do not directly execute.
Tool calls do not bypass policy.
Approvals do not bypass the event loop.
Execution results do not only exist in stdout.
Business actions are not constrained only by prompts.
```

## Learning Boundaries

### Learn From Claude Managed Agents

MandoForge should learn CMA's runtime contract:

- Versioned Agent configuration.
- First-class Environment placement.
- Resumable Session lifecycle.
- Event-driven input and streaming output.
- `requires_action` pause and resume.
- Multi-agent SessionThread isolation.
- Self-hosted environment worker boundaries.
- Runtime turn event normalization.

MandoForge should not learn:

- Claude as the only runtime.
- Anthropic API parity as a product goal.
- Hosted-only execution assumptions.
- Opaque multi-agent execution that bypasses MandoForge events, approvals, or
  audit.

### Learn From Palantir AIP

MandoForge should learn AIP's enterprise operation contract:

- Context Engineering as a product surface.
- Ontology-backed business actions.
- Purpose-based controls.
- Approvals and checkpoints.
- Package, release, deploy, and rollback.
- Human and AI applications.
- Operational automation.

MandoForge should not learn:

- Ontology as the product center.
- Foundry-style data platform ownership.
- A full enterprise digital twin as a prerequisite.
- Business action execution directly from ontology validity.
- High-risk automation as default external write execution.

## K Agent Boundary

K Agent is the execution-side controller for sandbox and CLI dispatch. It belongs
under Environment Scheduling, not above Manager Runtime.

K Agent owns:

- Claiming Environment-bound work.
- Selecting sandbox, Pod, warm-pool slot, or Remote Computer lease.
- Preparing workspace, state mounts, artifact paths, network policy, and allowed
  secret references.
- Launching runtime adapters such as Codex CLI, Claude Code, CMA worker, or
  other CLI families.
- Streaming stdout, JSONL, stream-json, status, and heartbeat data back.
- Syncing artifacts.
- Handling timeout, retry, cleanup, and checkpoint mechanics.

K Agent does not own:

- ManagerPlan.
- WorkItem routing authority.
- TaskGrant creation or expansion.
- Policy decisions.
- Approval decisions.
- Ontology validity.
- WorkflowPack release or rollback.
- Audit truth.

The boundary is:

```text
MandoForge decides whether work may run and what authority it has.
K Agent decides where and how the approved runtime turn is executed.
```

## Ontology Action Contract

Ontology validates business meaning. It does not grant execution authority by
itself.

Correct execution chain:

```text
Ontology says the action is semantically valid
+ TaskGrant says the actor has scoped authority
+ Policy says allow or requires approval
+ ApprovalCommitToken authorizes a concrete high-risk commit
+ Tool Router verifies exact args, connector, secret, and target binding
-> execute
```

For low-risk actions:

```text
Ontology valid + TaskGrant allow + Policy allow -> may execute automatically.
```

For high-risk actions:

```text
Ontology valid + TaskGrant allow + Policy requires approval + ApprovalCommitToken -> may execute.
```

For denied actions:

```text
Ontology valid does not override TaskGrant, Policy, Approval, connector, or
secret boundaries.
```

This keeps Ontology as business rule and action contract, not implicit root
permission.

## Implementation Phases

### Phase 1: Runtime Contract

Goal: make CMA-style runtime the only main execution path.

Scope:

- Keep `/api/sessions/:id/events` as the primary input path.
- Keep `/run` and `/messages` as compatibility wrappers.
- Route user messages, approval decisions, custom tool results, worker
  completions, and runtime finals through session events.
- Make Environment runtime binding the runtime adapter selection entrypoint.
- Continue reducing `agent_cli.exec` and `codex.exec` to compatibility facades.

Evidence gate:

- A session starts from a user event, enqueues a SessionLoopJob, calls a runtime
  adapter, emits `runtime.turn.*`, enters `requires_action`, resumes through an
  approval event, and returns to `idle` without losing event cursor state.

### Phase 2: Environment Scheduling And K Agent

Goal: isolate sandbox and CLI dispatch from business runtime authority.

Scope:

- Define Environment Work Queue semantics above current session-loop and
  execution-job APIs.
- Bind K Agent claim paths to Environment, worker pool, and lease.
- Make sandbox, Pod, warm-pool, Remote Computer, and CLI selection explicit.
- Normalize K Agent output into runtime events and artifacts.

Evidence gate:

- The same SessionLoopJob can be run by an Environment-bound K Agent.
- K Agent failure does not lose the event window.
- Retry does not reprocess completed event sequences.
- Runtime output is normalized into `runtime.turn.*`.

### Phase 3: Manager Runtime

Goal: make Manager Agents operate on WorkItems and ManagerPlans rather than
ad-hoc prompts or direct tools.

Scope:

- Keep ManagerPlan as a reviewable object.
- Connect ManagerPlan to WorkItem, Assignment, Review, WorkflowRun,
  WorkflowStepRun, SessionThread, and TaskGrant.
- Keep high-risk routing and downstream execution behind governance.

Evidence gate:

- A WorkItem creates a ManagerPlan.
- The ManagerPlan is reviewed.
- Approval materializes a WorkflowRun or child SessionThread.
- Each executable unit has a scoped TaskGrant.

### Phase 4: Ontology Action Contract

Goal: make Ontology the business action specification layer.

Scope:

- Represent BusinessObject, Rule, Relation, Metric, ActionContract,
  PermissionContract, ToolBinding, ValidationRule, and RiskClass.
- Require action validity before tool execution.
- Keep TaskGrant, Policy, Approval, and Tool Router as execution authority.

Evidence gate:

- Low-risk valid action can execute automatically.
- High-risk valid action enters `requires_action`.
- Ontology-valid action cannot bypass TaskGrant, Policy, Approval, or connector
  scope.

### Phase 5: Pack, Release, And Evidence

Goal: make business capabilities installable, releasable, observable, and
rollbackable.

Scope:

- WorkflowPack.
- DomainPack.
- AgentVersion.
- EnvironmentProfile.
- OntologyActionContract.
- ToolSpec.
- EvalGate.
- Release.
- Rollback.

Evidence gate:

- A pack can install, stage, release, run, observe, and rollback.
- Released workflows cannot bypass runtime, policy, audit, or TaskGrant.
- Rollback disables future entrypoints while preserving historical evidence.

## Non-Goals

- Do not rebuild Foundry.
- Do not clone the Claude Managed Agents API.
- Do not let Manager Agents become a second runtime orchestrator.
- Do not let K Agent own business authorization.
- Do not let Ontology become implicit root permission.
- Do not put Remote Computer above Environment.
- Do not expose worker queue internals as the primary product model.
- Do not make WorkflowPacks the Agent OS itself.
- Do not treat provider stdout as the replay source of truth.

## Spec Acceptance Criteria

This design is accepted when:

- The product center is clearly Manager Runtime and Managed Runtime.
- CMA is referenced only as a runtime contract.
- AIP is referenced only as an enterprise operation contract.
- K Agent is bounded to Environment Scheduling and Execution Substrate.
- Ontology is bounded to action validity, not execution authority.
- The implementation phases are independently testable.
- Each phase has an evidence gate tied to durable runtime records.
