# Claude Managed Agents Alignment

Date: 2026-05-20

This note records how Claude Managed Agents should reshape the MandoForge plan.
The goal is not to copy the Anthropic API surface exactly. The goal is to adopt
the same product model where it is structurally correct, while keeping
MandoForge self-hostable, provider-neutral, auditable, and governed by its own
policy and worker infrastructure.

## Source Model

Claude Managed Agents is built around five first-class concepts:

| Concept | Claude shape | MandoForge correction |
| --- | --- | --- |
| Agent | Reusable, versioned configuration containing model, system prompt, tools, MCP servers, skills, and optional multiagent coordinator roster. | Keep Managed Agent Registry, but make versioned agent configuration the product entrypoint, not a hidden admin record. |
| Environment | Container configuration referenced by sessions. Each session gets an isolated container instance. | Add first-class `Environment` above runtime profiles and Remote Computers. Remote Computer becomes one environment implementation. |
| Session | Agent instance inside an environment. Creating a session provisions the environment, but work starts only when user events are sent. | Replace the demo mental model of `POST /sessions/:id/run` with event-driven sessions. |
| Events | User events drive work; session, span, and agent events report state and progress. | Make `/api/sessions/:id/events` and streaming the primary control surface and UI model. |
| Threads | Multiagent coordination runs additional context-isolated session threads while the primary stream reports summarized activity. | Turn typed handoffs into session threads with parent/child lineage, not only free-standing assignment rows. |

Important Claude details from the public docs:

- Agents are reusable, versioned resources that bundle model, system prompt,
  tools, MCP servers, skills, and multiagent configuration.
- A session references both an agent and an environment. Session creation
  provisions the container; sending a user event starts actual work.
- Sessions are a state machine with statuses such as idle, running,
  rescheduling, and terminated.
- Communication is event-based: user events go in, and agent/session/span
  events come back for observability.
- Tool confirmations and custom tool calls pause the session with
  `requires_action` until the caller sends a resolution event.
- Idle sessions preserve conversation history and checkpoint container state
  for resume.
- Multiagent sessions use a coordinator agent. Additional agents run in their
  own session threads with isolated conversation history, while sharing the
  same container and filesystem.
- In self-hosted sandbox mode, Anthropic keeps orchestration while a customer
  runs an environment worker. The `self_hosted` environment is effectively a
  work queue: sessions assigned to the environment are enqueued, workers claim
  work, spawn execution contexts, run tool calls locally, and post results back.

Research sources:

- https://platform.claude.com/docs/en/managed-agents/agent-setup
- https://platform.claude.com/docs/en/managed-agents/environments
- https://platform.claude.com/docs/en/managed-agents/sessions
- https://platform.claude.com/docs/en/managed-agents/events-and-streaming
- https://platform.claude.com/docs/en/managed-agents/multi-agent
- https://platform.claude.com/docs/en/managed-agents/self-hosted-sandboxes

## Key Correction

Our previous roadmap over-weighted Remote Computer as the next product center.
Claude's model shows that the product center should be:

```text
Agent -> Environment -> Session -> Events -> Threads
```

Remote Computer remains important, but it is not the top-level product object.
It is one environment substrate for self-hosted or isolated execution.

The corrected MandoForge target is:

```text
Versioned Agent
  -> Environment
  -> Session
  -> user event
  -> orchestrator loop job
  -> model/span events
  -> tool / MCP / custom tool events
  -> policy approval events
  -> environment work queue
  -> worker / Remote Computer / Codex execution
  -> artifacts, audit, usage, and stream updates
```

This means the UI should not start with infrastructure logs. It should start
with "create or resume a session", show the agent and environment, then stream
events and blocking actions.

## Revised Architecture

### 1. Environment As A First-Class Resource

Add `environments` as the canonical execution profile:

- `type`: `local`, `cloud`, `self_hosted`, `remote_computer`, `codex_app_server`.
- Package/runtime metadata.
- Network policy and allowed hosts.
- Workspace/state mount contract.
- Worker pool or queue binding.
- Artifact output paths.
- KMS/Vault and MCP reachability requirements.
- Lifecycle state and release gates.

Existing `agent_runtime_profiles` should either become an implementation detail
of environments or be mapped into environment versions. The user should not
have to reason about raw runtime-profile rows first.

### 2. Event-Driven Session Lifecycle

Make this the main API path:

```text
POST /api/sessions
POST /api/sessions/:id/events
GET  /api/sessions/:id/events
GET  /api/sessions/:id/stream
POST /api/sessions/:id/interrupt
POST /api/sessions/:id/stop
```

`POST /api/sessions/:id/run` should become a compatibility helper that only
adds a `user.message` event and enqueues the session loop.

Session creation should bind:

- agent version
- environment
- tenant/project/team
- policy snapshot
- vault references
- workflow/domain pack versions
- optional initial files or external resources

Session execution should be driven by events, not by a synchronous run button.

### 3. Orchestrator As A Queue-Claimed Session Loop

Do not model the orchestrator as a permanently thinking daemon. Model it as a
versioned coordinator agent whose session loop is claimed by workers.

```text
session event appended
  -> orchestration work item enqueued
  -> orchestrator worker claims session loop
  -> provider call emits span.model_request_start/end
  -> plan/tool/custom-tool/approval events are appended
  -> session returns to idle, waiting_approval, rescheduling, or terminated
```

This keeps the "always available" product feel without a wasteful always-running
LLM process.

### 4. Worker Queue Becomes Environment Work Queue

The current execution job queue is still useful, but the higher-level queue
should be "session work for an environment":

```text
environment_work_items
  session_id
  environment_id
  thread_id
  work_kind: session_loop | tool_call | custom_tool | artifact_sync
  state: queued | claimed | running | waiting_action | completed | failed
```

Tool execution jobs can remain a lower-level queue under this contract.

### 5. Remote Computer Becomes Self-Hosted Environment

Remote Computer should be reframed as the self-hosted sandbox implementation:

```text
Environment(type=remote_computer)
  -> work queue binding
  -> worker claims session work
  -> leases Pod / warm-pool slot
  -> mounts workspace and state
  -> runs tool calls locally
  -> posts events, artifacts, usage, and audit back
```

This preserves the existing Remote Computer plan but moves it under the correct
product abstraction.

### 6. Multiagent Threads

Typed agent handoffs should become `session_threads`:

- primary thread = coordinator/orchestrator stream
- child thread = specialist agent stream
- every child thread has its own conversation/context
- child thread shares session environment unless explicitly isolated
- primary stream shows condensed start/end/blocking events
- drill-down shows full child thread events

This is more user-readable than a flat list of handoff rows.

### 7. UI Correction

The Agent OS UI should have this hierarchy:

1. Sessions
2. Agent
3. Environment
4. Event Stream
5. Blocking Actions
6. Artifacts
7. Threads
8. Infrastructure

Infrastructure remains available, but it is not the first screen.

## Revised Implementation Order

1. Add the Claude-style model document and update roadmap wording.
2. Add first-class `Environment` schema/API and map existing runtime profiles
   and Remote Computer profiles into it.
3. Add `POST /api/sessions/:id/events` as the primary session driver.
4. Make `/run` a compatibility wrapper over user events.
5. Add session statuses and event names aligned with the event-driven model:
   `session.status_running`, `session.status_idle`,
   `session.status_rescheduling`, `session.status_terminated`,
   `span.model_request_start`, `span.model_request_end`,
   `agent.tool_use`, `agent.tool_result`, `agent.custom_tool_use`.
6. Add environment work queue and orchestrator worker claim loop.
7. Reframe Remote Computer execution as a self-hosted environment worker.
8. Add `session_threads` and migrate typed handoffs into thread lifecycle.
9. Rewrite the UI around sessions/events/threads before expanding more admin
   panels.

Implementation update, 2026-05-20:

- The current runtime has first-class `environments`, event-driven
  `/api/sessions/:id/events`, queue-claimed `session_loop_jobs`, and
  `mandoforge-worker` consuming both session-loop and approved execution jobs.
- Provider execution now emits `span.model_request_start`,
  `span.model_request_end`, `agent.tool_use`, and `agent.tool_result` events in
  addition to the legacy `llm.*` and `tool.*` events.
- `session_threads` are durable records. Every session gets a primary thread,
  and manager-to-specialist typed handoffs create child specialist threads with
  parent lineage, specialist session linkage, context, and lifecycle events.
- Remote Computer automatic assignment is now under
  `Environment(type=remote_computer)`: the session environment contract filters
  lease and warm-pool selection by pool, profile, namespace, optional
  `remote_computer_id`, and metadata selectors, then records contract evidence
  on assignment metadata and timeline events.
- Remote Computer environments fail closed when Kubernetes Pod execution
  transport is not enabled, so approved jobs bound to remote environments do
  not silently fall back to local host execution.
- The web console now exposes a Managed Session Workspace that puts Agent,
  Environment, Event Stream, Blocking Actions, Artifacts, and Threads before
  raw worker / Remote Computer / provider infrastructure panels.

## Non-Goals

- Do not turn the orchestrator into an always-running LLM daemon.
- Do not let Remote Computer bypass Tool Router, Policy, Approval, Audit, or
  Event Stream.
- Do not expose worker queue internals as the primary product entrypoint.
- Do not make Workflow Packs the OS. Packs run on top of the managed-session
  runtime.
- Do not claim Claude parity until MandoForge can create/resume a session,
  drive it via events, stream model/tool/session events, pause for approvals,
  and run work through an environment queue.
