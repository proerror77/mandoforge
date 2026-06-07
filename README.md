# MandoForge

[简体中文 README](README.zh-CN.md)

## What Is It?

MandoForge is an **Agent middleware platform and Agent OS kernel**.

It is not a single chatbot. It is not a vertical commerce, finance, support, or coding agent. It is the runtime layer that lets many different business agents run in a controlled, auditable, and extensible way.

In simpler terms:

```text
Business agents solve domain problems.
MandoForge runs, governs, audits, and replays those agents.
```

The goal is to build a reusable operating layer for agents across industries. A commerce agent, finance agent, internal operations agent, code execution agent, legal review agent, or data analysis agent should not each rebuild its own runtime, approval system, audit log, sandbox, cost tracking, and release gate. MandoForge provides that shared foundation.

## Agent OS Stack

MandoForge is organized as an Agent OS, not as a production-evidence checklist:

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

The Managed Runtime Layer can borrow the useful parts of Claude Managed Agents:
Agent, Environment, Session, Events, and Threads. That model does not define the
whole Agent OS. The higher product layers remain MandoForge-owned.

The runtime boundary is explicit:

```text
MandoForge Agent Runtime
  -> Codex CLI / Claude Code CLI / Codex App Server runtime adapters
  -> normalized events, tool calls, artifacts, and audit logs
```

MandoForge manages sessions, context, policy, approvals, audit, artifacts,
resume cursors, streaming, and worker leases. Codex CLI and Claude Code CLI are
execution backends that MandoForge calls and supervises. Manager Agents are
managed agents running on this runtime; they coordinate WorkItems and
Assignments, but they do not own a separate execution stack.

For delegated runtime workflows, MandoForge owns the outer run envelope:
WorkflowRun identity, Workflow Pack binding, TaskGrant, memory scope, approval
policy, audit, artifacts, and UI observability. Claude Code, Codex App Server,
or Codex CLI can own the inner dynamic multi-agent execution, including
subagent fan-out/fan-in, as long as events and artifacts are streamed back into
the MandoForge runtime record.

## Why Agent Middleware?

A useful enterprise agent is more than a model call. It usually needs:

- Identity and permissions: who can create, run, approve, and inspect agents.
- Session lifecycle: every agent job needs a traceable start, state, and end.
- Tool execution: files, SQL, shell, MCP, Codex, and other controlled tools.
- Risk control: high-risk actions pause for human approval.
- Auditability: model messages, tool calls, approvals, artifacts, and failures are recorded.
- Replay: the full timeline can be reviewed after the run.
- Isolation: agents should not freely pollute each other's workspace.
- Governance: providers, budgets, evals, releases, rollbacks, and cost reports need control surfaces.

MandoForge is the common layer for those capabilities. Domain agents sit above it.

## What It Is Not

- It is not a finished vertical SaaS product.
- It is not limited to one industry or one agent type.
- It is not only a prompt management tool.
- It is not a thin OpenAI API wrapper.
- It is not a complete production platform yet.

The current repo is best understood as a **Rust-native Agent OS kernel prototype**. The Managed Runtime Layer is in place for the repo-controlled pilot; the Collaboration, Manager Agent, and Semantic layers are the next productization surface.

## Core Runtime Loop

The Managed Runtime Layer is designed around a Claude Managed Agents-style runtime model:

```text
Agent -> Environment -> Session -> Events -> Threads
```

The generic governed execution loop still matters, but it sits underneath that
managed-agent surface:

```text
Create or resume Session
-> Append user/tool/approval Event
-> Claim session-loop work through a worker queue
-> Call Provider
-> Parse Tool Call
-> Check Policy
-> Pause For Approval When Needed
-> Human Approves, Rejects, Or Provides Tool Result
-> Execute Tool In Workspace, Sandbox, Codex, MCP, Or Remote Computer Environment
-> Create Artifact
-> Persist Events, Threads, And Audit
-> Stream And Replay Timeline
```

Once this loop is stable, different business agents can reuse it by changing agent configuration, tools, data sources, policies, and approval rules.

The current implementation status is tracked in
[docs/runtime-truth-audit.md](docs/runtime-truth-audit.md): runtime facts,
remaining core gaps, and the evidence required for managed agent actions.

## Current State

Stage 1 implements the generic runtime kernel:

- Rust + Axum API server.
- Postgres-backed runtime store with in-memory fallback when `DATABASE_URL` is missing.
- Agents, agent versions, sessions, events, tool calls, approvals, artifacts, and audit logs.
- Tool Router for all controlled external execution.
- Policy Engine for allow, deny, and approval-required decisions.
- Approval Queue for high-risk actions.
- Session Timeline for replaying agent runs.
- Static Web Console for agents, sessions, timeline, approvals, artifacts, tool calls, and audit logs.
- Docker Compose and Kubernetes skeletons.

Stage 2 adds the governed middleware pilot layer:

- Organization, team, and project scopes.
- RBAC.
- Workflow Pack / Domain Pack contracts for installable industry workflow packages.
- Provider governance, model allowlists, budgets, and health checks.
- Secret-reference and credential boundaries.
- Worker queues, Redis/NATS handoff, and worker readiness.
- Approval v2: argument modification, delegated approval, expiry, escalation, and notifications.
- MCP Gateway governance.
- Codex App Server adapter.
- Eval, release gates, rollback, and drift checks.
- Observability, usage, and cost tracking.
- Scheduler due-runs.
- Remote Computer readiness skeleton.

Stage 2 is complete for the repo-controlled pilot. The core completion evidence is the runtime action record: session events, tool calls, approvals, artifacts, and audit logs.

## Main Design Direction

The next important direction is not another vertical demo. It is building the
full Agent OS stack around the runtime kernel:

```text
Work Surfaces -> Collaboration -> Manager Agent / Work Coordination -> Managed Runtime -> Semantic Layer -> Data Foundation
```

The most important near-term slice is still **Managed Session Runtime**, because
the upper layers depend on a reliable event-driven runtime:

```text
Create or resume a session, send user events, run the runtime session loop through a governed environment queue, call the selected CLI/runtime adapter, and stream model/tool/session events back to the UI.
```

This is a correction to the earlier Remote Computer-first framing. Remote
Computer remains critical, but it should be modeled as one Environment
implementation, not as the top-level product object.

The immediate slice is event-driven session state:

- Add first-class Environment resources above runtime profiles and Remote Computer profiles.
- Make `POST /api/sessions/:id/events` the primary way to drive work.
- Treat `POST /api/sessions/:id/run` as a compatibility wrapper over user events.
- Move runtime session-loop execution out of the API request path and into a queue-claimed worker path.
- Stream session status, model spans, tool use, approvals, artifacts, and child threads back to the UI.

Current managed-agent baseline:

- `GET/POST /api/environments` and `GET/PATCH/DELETE /api/environments/:id` manage first-class Environment records.
- `POST /api/sessions` accepts `environment_id` and records `session.environment_bound` in the session event log.
- `Environment.runtime_profile_id` is the canonical managed runtime-adapter binding for the session. `agent_cli.exec` remains a compatibility facade for CLI-backed adapters, but requested profiles must match the bound environment profile before falling back to handoff or agent runtime profiles. The legacy env-var allowlist only applies when no managed binding exists.
- `POST /api/sessions/:id/events` enqueues a lease-claimed `session_loop_job`; `mandoforge-worker` claims it and runs the runtime session loop outside the API request path.
- Session execution emits managed-agent style `session.status_*`, `span.model_request_*`, `agent.tool_use`, `agent.tool_result`, and `thread.*` timeline events.
- `GET /api/sessions/:id/threads` exposes durable `session_threads`; typed manager-to-specialist handoffs create child specialist threads linked to the parent session.
- `Environment(type=remote_computer)` now owns automatic Remote Computer assignment: approved execution jobs only auto-claim leases or warm-pool resources that match the session environment contract, and remote environments fail closed when the Remote Computer execution transport is not enabled.
- The UI start form loads environments and binds new sessions to the selected environment.
- The UI run view is organized around the managed-session objects first: Agent, Environment, Event Stream, Blocking Actions, Artifacts, and Threads. Raw worker, Remote Computer, provider, secret, MCP, and tenant infrastructure remain in system and advanced panels.
- Dynamic Workflow Plans are now first-class review envelopes: `POST /api/dynamic-workflow-plans` validates phases, agent fleet limits, governance, validation, and materialization policy; review approval gates execution; materialization creates a normal `WorkflowDefinition`, `WorkflowRun`, primary session, root `TaskGrant`, and start steps. Delegated plans target adapters such as Claude Code or Codex App Server without letting the external runtime bypass MandoForge policy and audit.
- Semantic Ontology Builder is proposal-only: `POST /api/semantic-ontology/builder` accepts operator/AI first-draft context, normalizes object and relation candidates, records source refs and review gates, and creates an `ontology_expansion` semantic object for review. It does not directly mutate the ontology registry or durable organizational memory.

Runtime alignment is tracked in
[Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md) and
[Agent OS Product Roadmap](docs/stage2-stage3-roadmap.md). The core runtime
contract now centers on resumable idle sessions, event-cursor loop processing,
live streaming, Environment-bound worker claims, and lease-fenced job
finalization. The first WorkItem intake, assignment-routing, review, Activity
Feed, Agent Teammate/Squad, and Manager Plan binding slice now persists
collaboration work and audit evidence; the next product work should continue
upward into Workflow Pack-defined manager roles, UI workflow surfaces, and
Semantic Objects rather than sideways into platform-owned manager loops or
deployment-evidence tracks.

## Run Locally

Start the API:

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1 \
cargo run -p mandoforge-api
```

Open the console:

```text
http://127.0.0.1:8787
```

Basic smoke check:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```

UI/API truth gate:

```bash
node scripts/verify-ui-api-truth-gate.mjs
BASE_URL=http://127.0.0.1:8787 node scripts/verify-ui-api-truth-gate.mjs
```

Full Stage 1 approval and artifact demo:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

Final gate:

```bash
./scripts/stage1-final-gate.sh
```

Live final gate with Docker Desktop:

```bash
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh
```

Postgres-backed restart/resume core evidence:

```bash
START_POSTGRES=1 ./scripts/managed-session-restart-resume-core-gate.sh
```

This gate requires Docker Desktop or an existing `DATABASE_URL`. It writes
session-event, tool-call, audit-log, restart/resume, cursor, thread-lineage, and
runtime-turn evidence under
`.mandoforge/managed-session-restart-resume-core-evidence/`.

Static UI smoke:

```bash
./scripts/verify-static-ui-actionbook.sh
```

## Optional Runtime Modes

Postgres:

```bash
DATABASE_URL=postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge \
cargo run -p mandoforge-api
```

OpenAI-compatible provider:

```bash
MANDOFORGE_PROVIDER_BASE_URL=https://api.openai.com \
MANDOFORGE_PROVIDER_API_KEY=... \
MANDOFORGE_PROVIDER_MODEL=gpt-5.4-mini \
cargo run -p mandoforge-api
```

Docker shell sandbox:

```bash
MANDOFORGE_SHELL_RUNNER=docker \
MANDOFORGE_SHELL_DOCKER_IMAGE=alpine:3.20 \
cargo run -p mandoforge-api
```

Queue-backed execution worker:

```bash
MANDOFORGE_EXECUTION_WORKER=queue \
MANDOFORGE_DEV_ADMIN_TOKEN=local-worker-token \
MANDOFORGE_WORKER_TOKEN=local-worker-token \
cargo run -p mandoforge-api

BASE_URL=http://127.0.0.1:8787 \
MANDOFORGE_WORKER_TOKEN=local-worker-token \
WORKER_POOL=managed-agent \
cargo run -p mandoforge-api --bin mandoforge-worker
```

The same worker drains both session-loop jobs and approved execution jobs. It is
the local entrypoint for the always-available runtime session loop. `WORKER_ENVIRONMENT_ID`
binds a worker to one Environment id; `WORKER_POOL` or `WORKER_QUEUE` binds it
to Environments whose `worker_queue_binding` names the same pool.

Governed coding-agent CLI profiles:

```bash
# Codex CLI profile
curl -sS -X POST "$BASE_URL/api/agent-runtime-profiles" \
  -H 'content-type: application/json' \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' \
  -d '{
    "name": "codex-cli-worker",
    "runtime_type": "codex_cli",
    "command": "/usr/bin/codex",
    "default_args": ["exec", "--json"],
    "remote_computer_required": true
  }'

# Claude Code CLI profile
curl -sS -X POST "$BASE_URL/api/agent-runtime-profiles" \
  -H 'content-type: application/json' \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' \
  -d '{
    "name": "claude-code-worker",
    "runtime_type": "claude_code",
    "command": "/usr/local/bin/claude",
    "default_args": ["--print"],
    "remote_computer_required": true
  }'
```

Bind one profile to an Environment, specialist agent, or handoff assignment.
MandoForge Agent Runtime then calls the selected CLI/runtime adapter through the
session-loop worker path. `agent_cli.exec` can still be used with the matching
`profile` and `task` as a compatibility facade, and the approved execution job
is drained by `mandoforge-worker`.

Managed `codex_cli`, `claude_code`, Gemini, OpenCode, and Aider profiles are
treated as runtime adapters: their JSONL or stream-json output is ingested into
`runtime_adapter.event` session events with basic secret-key redaction and
event-count limits. Codex CLI and Claude Code CLI output also maps into
normalized runtime turn records for turn start, items/tool calls, usage, final
messages, artifacts, and completion; Codex App Server turn APIs emit the same
taxonomy with thread/turn lineage.

This keeps CLI-backed agents inside the same Tool Router, Policy Engine,
Approval Engine, worker lease, Remote Computer, event log, and audit path while
moving the product semantics toward Environment-owned runtime adapters. The
target Managed Agents model is:

```text
Agent -> Environment -> Session -> runtime adapter -> Events
```

Codex App Server adapter:

```bash
MANDOFORGE_CODEX_APP_SERVER_URL=http://127.0.0.1:8789 \
MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS=30 \
MANDOFORGE_CODEX_EXECUTION_STRATEGY=auto \
cargo run -p mandoforge-api
```

If provider or App Server configuration is missing, the runtime uses deterministic mock or fail-closed reserved behavior instead of silently pretending production integration exists.

## Docker

```bash
docker compose up --build
```

The API is served on:

```text
http://127.0.0.1:8787
```

## Kubernetes

Kubernetes skeletons live in [deploy/k8s](deploy/k8s).

```bash
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
```

These manifests are a self-hosted pilot starting point, not a production hardening claim. Before shared-cluster use, split Codex and sandbox workers, replace example secrets, add NetworkPolicy, move workspaces and artifacts to durable storage, and finish the real Remote Computer execution path.

## Key Documents

- [Runtime Architecture](docs/architecture.md)
- [Stage 1 Plan](docs/stage1-plan.md)
- [Stage 1 Completion Audit](docs/stage1-completion-audit.md)
- [Stage 2 Completion Audit](docs/stage2-completion-audit.md)
- [Enterprise Product Completion Contract](docs/enterprise-product-completion-contract.md)
- [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md) - runtime-layer reference only
- [MandoForge Roadmap v2](docs/mandoforge-roadmap-v2.md)
- [Agent OS Product Roadmap](docs/stage2-stage3-roadmap.md)
- [Workflow Pack Adaptation Plan](docs/workflow-pack-adaptation-plan.md)
- [WorkflowPack Manifest Contract](docs/workflow-pack-manifest-contract.md)
- [Agent Remote Computer Plan](docs/agent-remote-computer-plan.md)
- [Deployment And Demo Guide](docs/deployment-guide.md)
- [Kubernetes Skeleton](deploy/k8s/README.md)

## Key APIs

Runtime:

- `GET /healthz`
- `GET /api/agents`
- `GET /api/agents/:id/versions`
- `POST /api/sessions`
- `POST /api/sessions/:id/events`
- `POST /api/sessions/:id/run` (compatibility wrapper for the demo-era run flow)
- `GET /api/sessions/:id/events`
- `GET /api/sessions/:id/stream` (SSE replay with `?after_seq=` / `Last-Event-ID`, then live push for newly appended events)
- `GET /api/sessions/:id/tool-calls`
- `GET /api/sessions/:id/audit-logs`

Tools and approvals:

- `GET /api/approvals`
- `POST /api/approvals/:id/approve`
- `POST /api/approvals/:id/reject`
- `POST /api/approvals/:id/modify`
- `POST /api/tools/:name/execute`
- `GET /api/tool-calls`
- `GET /api/audit-logs`

Governance and operations:

- `GET /api/policy`
- `POST /api/policy/simulate`
- `GET /api/stage2/readiness`
- `GET /api/enterprise-product/readiness`
- `GET /api/enterprise-security/admin-readiness`
- `GET /api/native-connectors/production-readiness`
- `GET /api/ontology/engine-readiness`
- `GET /api/providers/policy-gate`
- `GET /api/vault/readiness`
- `GET /api/execution-jobs/worker-readiness`
- `GET /api/remote-computers/readiness`
- `GET /api/remote-computers/runner/readiness`
- `GET /api/remote-computers/production-path`
- `POST /api/scheduler/run-due`
- `GET /api/usage`
- `GET /api/observability/collector-readiness`

## Security Boundary

- Tool Router is the only intended external execution path.
- `sql.query` rejects non-read SQL commands.
- `shell.exec`, `file.write`, `codex.exec`, and `http.request` require approval by policy.
- `mcp.call` executes only when an MCP Gateway is configured and the target server/tool is allowlisted.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without extra approval.
- Production secrets must not be passed into prompts, event logs, artifacts, or Codex workspaces.

## One-Sentence Summary

```text
MandoForge is a reusable Agent middleware kernel for running, governing,
approving, auditing, and replaying agent jobs across different industries.
```
