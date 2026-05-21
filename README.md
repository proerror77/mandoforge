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

The current repo is best understood as a **Rust-native Agent OS kernel prototype** with Stage 1 and the repo-controlled Stage 2 governed-runtime pilot complete. Real production deployments still require environment-specific adoption evidence before they can be called validated.

## Core Runtime Loop

MandoForge is designed around a Claude Managed Agents-style product model:

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
- Vault secret-reference boundaries.
- Worker queues, Redis/NATS handoff, and worker readiness.
- Approval v2: argument modification, delegated approval, expiry, escalation, and notifications.
- MCP Gateway governance.
- Codex App Server adapter.
- Eval, release gates, rollback, and drift checks.
- Observability, usage, cost, and finance operations.
- Scheduler due-runs.
- Remote Computer readiness skeleton.

Stage 2 is complete for the repo-controlled pilot. The strict audit and the remaining external production adoption backlog are in [Stage 2 Completion Audit](docs/stage2-completion-audit.md).

## Main Design Direction

The next important direction is not another vertical demo. It is making the
Agent middleware foundation closely track the Claude Managed Agents product
model while staying self-hostable, provider-neutral, and policy-governed:

```text
Agent -> Environment -> Session -> Events -> Threads
```

The most important near-term slice is **Managed Session Runtime**:

```text
Create or resume a session, send user events, run the orchestrator loop through a governed environment queue, and stream model/tool/session events back to the UI.
```

This is a correction to the earlier Remote Computer-first framing. Remote
Computer remains critical, but it should be modeled as one Environment
implementation, not as the top-level product object.

The immediate slice is event-driven session state:

- Add first-class Environment resources above runtime profiles and Remote Computer profiles.
- Make `POST /api/sessions/:id/events` the primary way to drive work.
- Treat `POST /api/sessions/:id/run` as a compatibility wrapper over user events.
- Move orchestrator execution out of the API request path and into a queue-claimed session loop.
- Stream session status, model spans, tool use, approvals, artifacts, and child threads back to the UI.

Current managed-agent baseline:

- `GET/POST /api/environments` and `GET/PATCH/DELETE /api/environments/:id` manage first-class Environment records.
- `POST /api/sessions` accepts `environment_id` and records `session.environment_bound` in the session event log.
- `Environment.runtime_profile_id` is the canonical managed runtime-adapter binding for the session. `agent_cli.exec` remains a compatibility facade for CLI-backed adapters, but requested profiles must match the bound environment profile before falling back to handoff or agent runtime profiles. The legacy env-var allowlist only applies when no managed binding exists.
- `POST /api/sessions/:id/events` enqueues a lease-claimed `session_loop_job`; `mandoforge-worker` claims it and runs the orchestrator loop outside the API request path.
- Session execution emits managed-agent style `session.status_*`, `span.model_request_*`, `agent.tool_use`, `agent.tool_result`, and `thread.*` timeline events.
- `GET /api/sessions/:id/threads` exposes durable `session_threads`; typed manager-to-specialist handoffs create child specialist threads linked to the parent session.
- `Environment(type=remote_computer)` now owns automatic Remote Computer assignment: approved execution jobs only auto-claim leases or warm-pool resources that match the session environment contract, and remote environments fail closed when the Remote Computer execution transport is not enabled.
- The UI start form loads environments and binds new sessions to the selected environment.
- The UI run view is organized around the managed-session objects first: Agent, Environment, Event Stream, Blocking Actions, Artifacts, and Threads. Raw worker, Remote Computer, provider, Vault, MCP, and tenant infrastructure remain in system and advanced panels.

Current alignment gaps are tracked in
[Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md) and
[Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md). The most important
remaining work is to make the Claude-style contract complete end to end:
resumable non-terminal idle sessions, event-cursor based loop processing, live
streaming, environment queue binding, lease-fenced job finalization, and
production evidence that proves worker restart and session recovery.

The remaining production hardening work is also cluster evidence:
`Environment(type=remote_computer)` policy is enforced by the runtime, while real
Kubernetes Pod execution still depends on the configured Remote Computer
transport and the external state-sync / sidecar / worker-pool evidence gates.

## Run Locally

Start the API:

```bash
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
cargo run -p mandoforge-api

BASE_URL=http://127.0.0.1:8787 \
MANDOFORGE_WORKER_TOKEN=local-worker-token \
WORKER_POOL=managed-agent \
cargo run -p mandoforge-api --bin mandoforge-worker
```

The same worker drains both session-loop jobs and approved execution jobs. It is
the local entrypoint for the always-on orchestrator loop. `WORKER_ENVIRONMENT_ID`
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

Bind one profile to a specialist agent or handoff assignment, then call
`agent_cli.exec` with the matching `profile` and `task`. The approved execution
job is drained by `mandoforge-worker`, and the result records legacy
`profile`, `runtime_type`, `stdout`, `stderr`, truncation flags, and exit
status fields for compatibility. Managed `codex_cli`, `claude_code`, Gemini,
OpenCode, and Aider profiles are treated as runtime adapters: their JSONL or
stream-json output is ingested into `runtime_adapter.event` session events with
basic secret-key redaction and event-count limits. Codex CLI and Claude Code
CLI output also maps into normalized runtime turn records for turn start,
items/tool calls, usage, final messages, artifacts, and completion; Codex App
Server turn APIs emit the same taxonomy with thread/turn lineage. This keeps CLI-backed agents
inside the same Tool Router, Policy Engine, Approval Engine, worker lease,
Remote Computer, event log, and audit path while moving the product semantics
toward Environment-owned runtime adapters. `agent_cli.exec` remains the
compatibility facade; the target Managed Agents model is
`Agent -> Environment -> Session -> runtime adapter -> Events`.

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
- [Stage 2 Gap Audit](docs/stage2-gap-audit.md)
- [Stage 2 Completion Audit](docs/stage2-completion-audit.md)
- [Stage 2 Production Adoption Runbook](docs/stage2-production-adoption-runbook.md)
- [Claude Managed Agents Alignment](docs/claude-managed-agents-alignment.md)
- [Whiskey Adoption Runbook](docs/whiskey-adoption-runbook.md)
- [Whiskey Adoption Status](docs/whiskey-adoption-status.md)
- [MandoForge Roadmap v2](docs/mandoforge-roadmap-v2.md)
- [Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md)
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
- `GET /api/sessions/:id/stream`
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
- `GET /api/providers/policy-gate`
- `GET /api/vault/readiness`
- `GET /api/execution-jobs/worker-readiness`
- `GET /api/remote-computers/readiness`
- `GET /api/remote-computers/runner/readiness`
- `POST /api/scheduler/run-due`
- `GET /api/usage/finance-operations/summary`
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
