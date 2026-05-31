# Agent OS Architecture

MandoForge is an Agent OS kernel and middleware layer. It is not a vertical
agent app, and it is not an enterprise production-evidence checklist.

## Product Stack

```text
Existing Work Surfaces
  Slack / Feishu / GitHub / Jira / Linear / Email
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review
  Agent Teammate / Squad / Activity Feed
        |
Manager Agent / Work Coordination Layer
  Task decomposition / routing / escalation / review
        |
Managed Runtime Layer
  Session / Event Log / Tool Router / Policy
  Approval / Audit / Artifact / Eval
        |
Semantic Layer / Ontology Service
  Business Objects / Metrics / Relations
  Actions / Permissions / Tool Bindings
  Retrieval Context / Data Contracts
        |
Enterprise Data Foundation
  Warehouse / Lakehouse / Postgres / Vector
  Graph / Docs / APIs / Event Streams
```

The current repository is strongest in the Managed Runtime Layer. Claude Managed
Agents is a useful reference for that layer only: Agent, Environment, Session,
Events, and Threads. It does not define the whole Agent OS.

The workflow-level target architecture is defined in
[Managed Agent Workflow Architecture](managed-agent-workflow-architecture.md).
That document covers WorkflowRun, WorkflowStepRun, TaskGrant, MemoryScope,
ConnectorScope, pack materialization, worker-agent boundaries, and workflow UI
observability.

Core ownership:

- MandoForge Agent Runtime owns sessions, event logs, policy, approval, audit,
  artifacts, threads, cursor/resume, streaming, and worker leases.
- Codex CLI, Claude Code CLI, Codex App Server, and future agent runtimes are
  runtime adapters called by MandoForge.
- Manager Agents are managed agents running on MandoForge. They coordinate work
  through WorkItems, Assignments, Reviews, and child threads; they do not own a
  separate runtime orchestrator.

## Core Boundary

Agent OS core completion is proven by runtime action evidence:

- `session_events` is the ordered action timeline.
- `tool_calls` is the durable tool-action table.
- `audit_logs` is the operator decision and side-effect trail.

Every user input, model turn, policy decision, approval request/decision, tool
call/result, runtime adapter turn, artifact, thread transition, and session
state change must be reconstructable from those records.

The implementation-level status is tracked in
[runtime-truth-audit.md](runtime-truth-audit.md).

## Runtime Model

```text
Agent Version
  -> Environment
  -> Session
  -> Event
  -> Session Loop Job
  -> Runtime Turn
  -> Tool / Approval / Artifact / Thread
  -> Stream + Replay
```

Rules:

- Creating a session does not imply work starts.
- `POST /api/sessions/:id/events` is the primary driver.
- `POST /api/sessions/:id/run` is a compatibility wrapper that appends a user
  event.
- Session-loop jobs process explicit event sequence windows and advance a
  processed high-water mark.
- UI replay must come from durable event/tool/audit/artifact/thread state, not
  transient provider state.

The runtime session loop calls an LLM or CLI-backed runtime adapter to execute
an agent turn. The adapter may run its own local agent loop, but MandoForge stays
the supervisor and evidence layer:

```text
MandoForge Agent Runtime
  -> runtime profile
  -> runtime adapter
      -> Codex CLI
      -> Claude Code CLI
      -> Codex App Server
      -> future hosted/self-hosted runtimes
  -> normalized session events, tool calls, artifacts, and audit logs
```

MandoForge should not reimplement the Codex or Claude Code agent loop. It should
prepare context, call the selected CLI/runtime adapter, ingest structured output,
enforce policy/approval, and persist the evidence.

## Store Boundary

The runtime supports two store backends:

- Postgres when `DATABASE_URL` is set.
- In-memory fallback for local development.

The public API shape should stay identical across both. Route handlers should
go through `AppState` and the `store_*` modules instead of touching storage
directly.

Current core store groups:

- `store_entities`: agents, agent versions, environments, and sessions.
- `store_events`: append-only session events.
- `store_tool_calls`: tool-call persistence and status updates.
- `store_artifacts`: artifact persistence.
- `store_approvals`: approval persistence and decisions.
- `store_audit`: audit-log persistence.
- `store_manager_plans`: manager-agent planning records.
- `store_semantic`: semantic sources, objects, links, and context packets.

## Event Contract

`session_events` is the durable context object. It is not the model context
window.

Required properties:

- Events are append-only.
- `seq` is session-local and monotonic.
- Tool calls, tool results, policy decisions, approvals, artifacts, runtime
  turns, thread transitions, and final reports link back to event sequence and
  payload references.
- SSE uses event `seq` as the reconnect cursor.

Important event families:

- `user.*`
- `session.*`
- `span.model_*`
- `runtime.turn.*`
- `runtime.item`
- `runtime.tool_call`
- `runtime.usage`
- `agent.tool_use`
- `agent.tool_result`
- `approval.*`
- `policy.*`
- `execution.*`
- `artifact.*`
- `thread.*`

## Tool Boundary

Tools are primitive capabilities. Product features should be outcomes achieved
by agents operating in a loop, not workflow logic hidden inside tools.

Core tools:

- `file.read`
- `file.write`
- `sql.query`
- `shell.exec`
- `codex.exec`
- `agent_cli.exec`
- `mcp.call`
- `approval.request`
- `artifact.create`

Rules:

- All tool execution goes through the Tool Router.
- Tool policy is evaluated before execution.
- High-risk tools pause for approval.
- Tool input, result or error, status, and session linkage are persisted in
  `tool_calls`.
- Tool side effects append session events and audit entries.
- `codex.exec` and `agent_cli.exec` can remain compatibility or lower-level
  execution facades, but the product model for managed sessions is
  Environment-owned runtime adapters.

## Policy And Approval Boundary

Policy controls whether a tool is allowed, denied, or requires approval.
Approval controls human authorization for high-risk actions.

Rules:

- Policy decisions append events.
- Approval requests append events and audit logs.
- Approval decisions append events and audit logs.
- Approval or tool-result continuation must re-enter through the session loop,
  not a direct provider resume path.

## Runtime Adapter Boundary

Runtime adapters are the execution backends for MandoForge Agent Runtime. They
normalize external agent runtimes into the Agent OS event model.

Targets:

- Codex CLI.
- Claude Code CLI.
- Codex App Server.
- Future hosted or self-hosted runtimes.

Normalized turn model:

- `runtime.turn.started`
- `runtime.item`
- `runtime.tool_call`
- `runtime.usage`
- `runtime.final`
- `runtime.turn.completed`

Adapters should preserve resume handles, usage, timing, schema validation,
collected items, final messages, artifacts, and runtime lineage.

Adapter rules:

- CLI-backed runtimes are not opaque shell tools in the product model.
- `Environment.runtime_profile_id` selects the adapter for a session.
- `agent_cli.exec` remains a compatibility facade while managed sessions move
  toward direct Environment-owned runtime adapters.
- The adapter can call the model and run its local loop, but session state,
  policy, approval, audit, artifacts, streaming, and replay remain owned by
  MandoForge.

## Collaboration Layer

The next product layer above runtime is not another demo agent. It is durable
work coordination:

- WorkItem.
- Project.
- Assignment.
- Review.
- Agent Teammate.
- Squad.
- Activity Feed.

External work surfaces such as Slack, Feishu, GitHub, Jira, Linear, and Email
should map into these objects before manager agents act on them.

## Manager Agent / Work Coordination Layer

Manager Agents are pack-defined managed agents that run on MandoForge Agent
Runtime. They perform work orchestration inside a Workflow Pack, not runtime
orchestration and not platform-owned business control.

They operate on WorkItems and Assignments:

- Intake.
- Decomposition.
- Specialist selection.
- Routing.
- Escalation.
- Result review.

Manager decisions must be structured records, visible in timeline/audit, and
linked to child session threads.

Rules:

- Workflow Packs own manager behavior: task intake, decomposition, routing,
  review checkpoints, SLA policy, and escalation policy.
- Manager Agents use runtime tools to create plans, assignments, reviews,
  escalations, and child specialist threads.
- Manager Agents may choose which specialist/runtime profile should handle a
  task, but execution still goes through the Managed Runtime Layer.
- Manager Agents must not bypass Tool Router, Policy Engine, Approval Engine,
  `session_events`, `tool_calls`, `audit_logs`, or artifacts.
- The platform must not expose a hard-coded manager closed-loop API that
  bypasses pack/workflow definitions.

## Delegated Runtime Workflow Envelope

MandoForge should not reimplement every inner multi-agent workflow that Codex or
Claude Code can already execute. The platform owns the outer enterprise
control plane:

- WorkItem and WorkflowRun identity.
- Workflow Pack binding.
- tenant, domain, memory, and task-grant scope.
- approval and commit gates.
- event ingestion, audit, artifacts, and UI observability.

`WorkflowDefinition.execution_strategy` selects the control boundary:

- `native_steps`: MandoForge materializes and schedules the declared step graph.
- `delegated_runtime`: MandoForge creates one governed runtime envelope and
  delegates inner planning, fan-out/fan-in, and subagent execution to a runtime
  adapter such as `claude_code`, `codex_app_server`, or `codex_cli`.
- `native_dynamic`: reserved for a future MandoForge-owned dynamic orchestrator.

The delegated path records `runtime_adapter`, `runtime_mode`,
`runtime_capability_contract`, and `event_ingestion_policy` on the definition.
Each `WorkflowRun` snapshots those values with `delegation_status`,
`external_run_ref`, `runtime_event_cursor`, and `runtime_envelope`. Workers may
call the external runtime, but they still receive a TaskGrant and must stream or
normalize runtime output back into MandoForge session events, artifacts, and
audit records. Runtime scripts or external agents must not become a side channel
around policy, approval, memory scope, connector scope, or worker leases.

## Dynamic Workflow Plan

`DynamicWorkflowPlan` is the reviewable planning envelope for large multi-agent
runs. It is not a JavaScript or Rust workflow runtime. It is the platform-owned
proposal that captures:

- objective.
- phases, prompts, agent counts, dependencies, and phase-level validation.
- agent fleet limits such as total agents, max parallel agents, timeout, and
  retry budget.
- governance scope for memory, tools, connectors, external effects, and
  approvals.
- materialization target: `delegated_runtime` for Claude Code or Codex-managed
  inner workflows, or `native_steps` when MandoForge should materialize the
  phase graph itself.

Lifecycle:

1. `POST /api/dynamic-workflow-plans/compile` deterministically compiles a
   natural-language objective into a reviewable phase graph request. This is a
   compiler for governed workflow plans, not a side-channel script runtime.
2. `POST /api/dynamic-workflow-plans` validates phases and fleet policy, then
   stores an analyzable proposal with audit evidence.
3. `POST /api/dynamic-workflow-plans/:id/review` records the human or
   pack-defined review decision. Only approved plans can be materialized.
4. `POST /api/dynamic-workflow-plans/:id/materialize` creates a
   `WorkflowDefinition`, `WorkflowRun`, primary session, root `TaskGrant`, and
   start steps through the same managed runtime path as normal workflows.
5. `POST /api/dynamic-workflow-plans/:id/adjudicate` evaluates materialized
   step outputs against the plan's voting threshold and records session/audit
   evidence. Missing vote evidence is not treated as approval.
6. `POST /api/dynamic-workflow-plans/:id/pressure-test` records a large-fleet
   control-plane pressure simulation. It is evidence for batching and
   backpressure limits, not a claim of live provider execution at that scale;
   the response status is `control_plane_passed`.

This keeps the Claude Code Dynamic Workflow product pattern, where many
subagents may be planned behind one run, but preserves MandoForge's enterprise
boundary: stable identity, approval, memory scope, connector scope, audit,
artifacts, and UI observability stay outside the delegated runtime.

## Semantic Layer

The semantic layer gives agents stable business context:

- Semantic sources.
- Semantic objects.
- Semantic links.
- Ontology registry.
- Context packets.
- Memory writeback candidates.

The current ontology contract is `/api/ontology/registry` version `core-v0.1`.
It exposes the canonical object catalog and the allowed semantic relation
triples. New semantic links must match a declared
`from_entity_type/relation_type/to_entity_type` triple before they can be
persisted. This keeps memory and evidence relations inspectable instead of
letting workers mint arbitrary relation names at write time.

Workers and managed agents should submit normalized semantic material through
`POST /api/semantic-ingestion/batches` when they need to persist multiple
objects and links from the same source. The batch contract creates one
`SemanticSource`, materializes temp-ref addressed `SemanticObject` records, then
creates ontology-governed `SemanticLink` records after all relation triples have
been validated. This gives ingestion, reflection, and future dreaming workflows
one audited write path instead of a collection of ad hoc object/link writes.

Context packets should be replayable so operators can inspect what an agent saw
before it acted. Memory writeback remains approval-gated.

Reflection and dreaming use the same governed writeback path. Workers submit
structured synthesis results to
`POST /api/sessions/:id/semantic-synthesis-runs` after a session, goal,
workflow, or handoff checkpoint. The API writes a
`semantic_reflection_report` or `semantic_dreaming_report` artifact and turns
durable lessons into pending `MemoryWritebackCandidate` rows. It does not
promote those lessons into durable semantic memory until a reviewer approves the
candidate.

Scheduled reflection/dreaming is scheduler-owned. Released or active Workflow
Pack runtime objects with `runtime_kind=semantic_synthesis_schedule` are counted
in `/api/scheduler/due-plan` and executed by `/api/scheduler/run-due` when their
schedule policy is due. The scheduler still only records report artifacts and
pending writeback candidates; one-shot schedules are de-duplicated by audit
evidence, and recurring schedules require an explicit interval policy.

Workflow Pack workflows may declare `semantic_synthesis_schedule` directly in
their workflow YAML. During stage/release materialization MandoForge turns that
declaration into a schedule runtime object bound to the staged
`workflow_definition_id`. If the schedule uses `session_selector.source:
completed_workflow_runs`, the scheduler selects completed workflow runs for that
definition and runs synthesis against each run's primary session. The selected
session must still have a TaskGrant whose `memory_scope.writeback_allowed` is
true, so pack-authored schedules cannot bypass memory governance.

## Environment And Remote Computer Boundary

Environment is the runtime placement contract. Remote Computer is one
Environment implementation, not the top-level product object.

Rules:

- `Environment.runtime_profile_id` is the canonical runtime-adapter binding.
- Worker polling and job claims should respect Environment id and worker queue
  binding.
- Remote Computer execution must still go through Tool Router, Policy Engine,
  Approval Engine, session events, artifacts, and audit logs.
- Remote Computer must not become a side channel around the Agent OS kernel.

## Verification

Focused local checks:

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api --bins
cargo test -p mandoforge-api -- --test-threads=1
bash -n scripts/stage1-demo.sh scripts/agent-os-core-evidence-gate.sh scripts/stage1-final-gate.sh
shellcheck scripts/stage1-demo.sh scripts/agent-os-core-evidence-gate.sh scripts/stage1-final-gate.sh
git diff --check
```

Against a running API:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/agent-os-core-evidence-gate.sh
```
