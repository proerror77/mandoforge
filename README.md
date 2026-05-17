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

MandoForge is designed around this generic agent job loop:

```text
Create Agent
-> Create Session
-> Call Provider
-> Parse Tool Call
-> Check Policy
-> Pause For Approval When Needed
-> Human Approves Or Rejects
-> Execute Tool In Workspace Or Sandbox
-> Create Artifact
-> Persist Events And Audit
-> Replay Timeline
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

The next important direction is not another vertical demo. It is hardening the Agent middleware foundation.

The most important near-term slice is **Remote Computer**:

```text
Let an agent session attach to a leaseable, isolated, auditable remote execution environment.
```

The immediate slice is session attach state:

- Attach a session to a selected Remote Computer lease.
- Persist attach and release status.
- Detect stale attachments.
- Keep `shell.exec` and `codex.exec` on the existing approved worker path for now.
- Prove that attach state does not bypass the Tool Router, Policy Engine, or Approval Engine.

The longer-term direction is to make Remote Computer the main sandbox substrate: agents execute approved work inside isolated Pods or workspaces, sync artifacts and timeline events back to MandoForge, and preserve the full audit chain.

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
MANDOFORGE_EXECUTION_WORKER=queue cargo run -p mandoforge-api
BASE_URL=http://127.0.0.1:8787 cargo run -p mandoforge-api --bin mandoforge-worker
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
- [Stage 2 Gap Audit](docs/stage2-gap-audit.md)
- [Stage 2 Completion Audit](docs/stage2-completion-audit.md)
- [Stage 2 / Stage 3 Roadmap](docs/stage2-stage3-roadmap.md)
- [Workflow Pack Adaptation Plan](docs/workflow-pack-adaptation-plan.md)
- [Agent Remote Computer Plan](docs/agent-remote-computer-plan.md)
- [Deployment And Demo Guide](docs/deployment-guide.md)
- [Kubernetes Skeleton](deploy/k8s/README.md)

## Key APIs

Runtime:

- `GET /healthz`
- `GET /api/agents`
- `GET /api/agents/:id/versions`
- `POST /api/sessions`
- `POST /api/sessions/:id/run`
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
