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

## Current Alignment State

This is the authoritative status snapshot for the Claude Managed Agents
alignment. The target model is intentionally close to Claude Managed Agents, but
MandoForge should not claim exact Claude API parity. It should claim an
equivalent product contract where the implementation is actually wired through.

Implementation baseline, 2026-05-20:

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
- Managed CLI runtimes now have a first runtime-adapter event ingestion slice:
  `codex_cli` JSONL and `claude_code` stream-json output from governed
  `agent_cli.exec` profiles is parsed into `runtime_adapter.event` session
  events with basic secret-key redaction and event-count limits, while the
  legacy tool result payload remains available for compatibility.
- New Codex CLI releases reinforce this direction: features such as richer turn
  results and `codex exec resume --output-schema` mean the adapter contract
  should capture structured turn metadata, usage/timing, resumable run handles,
  and schema-constrained final output instead of treating CLI stdout as the only
  source of truth.

Remaining alignment work:

| Claude-style contract | Current MandoForge baseline | Required next correction |
| --- | --- | --- |
| Sessions remain resumable and normally return to idle after a loop. | Runtime emits managed-agent-style status events, but the persisted session enum still uses demo-era terminal `completed` for a successful pass. | Add explicit persisted `idle`, `running`, `requires_action`, `rescheduling`, and `terminated` semantics; normal loop completion should return to idle, and terminated should require an explicit stop/final-close path. |
| User, approval, and custom tool result events are the durable input contract. | `/api/sessions/:id/events` persists events, wakes the loop, and session-loop jobs now store `pending_event_seq_start`, `pending_event_seq_end`, and `processed_event_seq` so each worker owns a concrete event window. Provider context is scoped to that pending range for user/custom-tool inputs, and approval decisions can trigger the loop through their durable event ids when no worker job is required. | Keep expanding typed event payloads for approval and custom-tool results so provider context can avoid legacy compatibility fallbacks. |
| Session loop continuation is the single orchestration path. | Initial user events, custom-tool results, approval decisions, and approved execution completion now enqueue `session_loop_jobs`; worker completion first writes `execution.completed` and then uses that event as the loop trigger. | Route any remaining non-worker tool-result continuation sources through the same event-windowed path so retry, lease, metrics, audit, and tracing stay on one path. |
| Environment owns placement for session work. | Environment records and Remote Computer policies exist, and workers can now bind polling/claiming to `x-mandoforge-environment-id` for both session-loop and execution-job endpoints. Direct run attempts for mismatched Environment jobs fail as not claimable. | Promote `Environment.worker_queue_binding` into named pool selection and scheduler/autoscaler evidence, beyond the current Environment-id filter. |
| CLI-backed runtimes are Environment runtime adapters, not opaque tools. | `agent_cli.exec` can run managed `codex_cli` and `claude_code` runtime profiles, ingests their JSON/stream events into `runtime_adapter.event`, and now treats `Environment.runtime_profile_id` as the canonical managed-session adapter binding before handoff or agent runtime profile fallback. Managed `codex_cli` and `claude_code` output also maps into normalized runtime turn events. | Keep `agent_cli.exec` as a compatibility facade while routing App Server output through the same runtime-adapter event taxonomy with normalized turn state. |
| Runtime adapters preserve structured turn state. | Codex CLI JSONL is normalized into runtime turn records for resume handles, structured output schema metadata, timing, usage, collected items, tool calls, final messages, and final-message artifacts. Claude Code stream-json now maps `system`, `assistant`, and terminal `result` events into the same turn-start, item, usage, final, and completed taxonomy. Session-loop cursors make resumable event windows explicit. App Server still needs parity with that model. | Extend the normalized turn metadata model across Codex App Server. |
| Streaming is live progress. | `/api/sessions/:id/stream` exposes session events through SSE, emits each event sequence as the SSE `id`, and supports reconnect replay with `?after_seq=` or `Last-Event-ID`. | Back streaming with a live broadcaster, DB tail, or poll-and-push contract beyond the current cursor-safe replay snapshot. |
| Thread APIs show each participating session's thread view. | Primary and specialist thread rows are durable, and lifecycle events are emitted. | Ensure specialist sessions can enumerate their own child thread membership, not only receive thread lifecycle events. |
| Production readiness proves restart/resume behavior. | Stage 2 evidence gates cover many external controllers and readiness endpoints. | Add managed-session runtime evidence: enqueue events, drain jobs, restart API/worker, prove resumed session state, thread lineage, and lease-fenced finalization. |

## Revised Implementation Order

1. Keep the Claude-style resource chain as the product contract:
   `Agent -> Environment -> Session -> Events -> Threads`.
2. Preserve the landed baseline: Environment API, session event API,
   session-loop jobs, managed-session UI, Remote Computer environment policy,
   and durable session threads.
3. Replace demo-era terminal session completion with explicit resumable session
   states.
4. Make every continuation path enqueue `session_loop_jobs`.
5. Promote environment queue binding into session-loop worker placement.
6. Promote Environment queue binding or add an environment work queue above
   low-level execution jobs.
7. Promote managed CLI execution from the `agent_cli.exec` compatibility facade
   into Environment-owned runtime adapters.
8. Harden live event streaming and thread membership views.
9. Add production evidence gates for managed-session restart and recovery.
10. Then expand Workflow Packs, scheduler, Codex traces, and production Remote
   Computer execution on top of the managed-session runtime.

## Non-Goals

- Do not turn the orchestrator into an always-running LLM daemon.
- Do not let Remote Computer bypass Tool Router, Policy, Approval, Audit, or
  Event Stream.
- Do not expose worker queue internals as the primary product entrypoint.
- Do not make Workflow Packs the OS. Packs run on top of the managed-session
  runtime.
- Do not claim exact Claude API parity. Claim MandoForge parity only for the
  product contract that has been implemented and verified.
- Do not call a deployment production-ready for managed sessions until it can
  create/resume a session, drive it via events, stream model/tool/session
  events, pause for approvals, run work through an environment queue, and
  recover after API/worker restart with evidence.
