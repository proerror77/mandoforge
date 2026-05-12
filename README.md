# MandoForge

MandoForge is a Rust-native Generic Agent OS Kernel. It is not a vertical commerce, finance, or support agent. Stage 1 focuses on the runtime kernel that lets any configured agent create sessions, append events, route tools through policy, pause for approval, create artifacts, and replay the timeline.

## Stage 1 Scope

- Rust API server with Axum.
- Postgres-backed session/event/artifact/approval store with in-memory fallback.
- Generic orchestrator agent config.
- Session binding to agent versions with per-version tool allowlist enforcement.
- Static Session Console UI with event timeline and approval queue.
- Generic demo data: `platform_events`, `sample_documents`, and `sample_metrics`.
- YAML governance policy for blocked and approval-required tools.
- Codex CLI adapter stub that runs `codex exec` inside a per-session workspace when invoked.
- Docker Compose and Kubernetes deployment skeleton.

The runtime uses Postgres when `DATABASE_URL` is set and falls back to an in-memory store when it is missing. `session_events` is the durable context object; it is not the model context window.

## Generic Diagnostics Demo

The first demo validates runtime architecture, not business intelligence.

Prompt:

```text
Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md.
```

The mock harness currently demonstrates:

- Agent plan.
- `file.read` result.
- `sql.get_schema` result.
- `sql.query` result over generic platform events.
- `approval.requested` for `shell.exec`.
- `artifact.created` for `diagnostics.md`.
- Final runtime diagnostics summary.
- Replayable event timeline.

## Run Locally

```bash
cargo run -p mandoforge-api
```

Open:

```text
http://127.0.0.1:8787
```

Smoke check:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```

Full Stage 1 approval/artifact demo:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

Static plus in-memory demo final gate:

```bash
./scripts/stage1-final-gate.sh
```

Static UI browser smoke, using `actionbook` CLI instead of Playwright or Chrome DevTools MCP:

```bash
./scripts/verify-static-ui-actionbook.sh
lsof -nP -iTCP:8791 -sTCP:LISTEN || true
lsof -nP -iTCP:9224 -sTCP:LISTEN || true
```

The script starts the API on `127.0.0.1:8791` when needed, launches a headless Chrome CDP endpoint on `9224`, checks the static console through `actionbook`, writes `/tmp/mandoforge-actionbook-smoke.png`, and cleans up both listeners.

Live final gate, with API, Postgres, and Docker available:

```bash
RUN_LIVE=1 BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-final-gate.sh
```

Live final gate with self-started Postgres and host API, when Docker Desktop is running:

```bash
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh
```

Refresh Postgres demo facts:

```bash
DATABASE_URL=postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge ./scripts/seed-platform-events.sh
```

Verify that `sql.query` is executing against live Postgres, not the in-memory fallback:

```bash
DATABASE_URL=postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge \
BASE_URL=http://127.0.0.1:8787 \
./scripts/verify-postgres-sql-query.sh
```

Optional OpenAI-compatible provider transport:

```bash
MANDOFORGE_PROVIDER_BASE_URL=https://api.openai.com \
MANDOFORGE_PROVIDER_API_KEY=... \
MANDOFORGE_PROVIDER_MODEL=gpt-5.4-mini \
cargo run -p mandoforge-api
```

If either provider env var is missing, the runtime uses the deterministic mock provider.

To verify a running API is using the external provider transport instead of the mock:

```bash
RUN_PROVIDER_SMOKE=1 \
MANDOFORGE_PROVIDER_BASE_URL=https://api.openai.com \
MANDOFORGE_PROVIDER_API_KEY=... \
BASE_URL=http://127.0.0.1:8787 \
./scripts/verify-external-provider.sh
```

Optional Docker shell sandbox:

```bash
MANDOFORGE_SHELL_RUNNER=docker \
MANDOFORGE_SHELL_DOCKER_IMAGE=alpine:3.20 \
cargo run -p mandoforge-api
```

`shell.exec` still requires approval. Docker mode runs approved commands with `--network none`, a workspace mount, and basic CPU/memory limits.
Approved shell and Codex execution outputs are truncated before entering `tool_calls`, `session_events`, and artifacts. Set `MANDOFORGE_EXECUTION_OUTPUT_LIMIT_BYTES` to tune the per-field limit.
Approved execution also enters an execution queue facade before it is drained. The queue is Postgres-backed when `DATABASE_URL` is set and in-memory otherwise. Set `MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory|postgres|auto` to choose explicitly. Reserved broker names (`broker`, `redis`, `nats`) fail closed until a real broker-backed backend is implemented. The default worker is `InlineExecutionWorker`; set `MANDOFORGE_EXECUTION_WORKER=queue` to enqueue approved jobs for an external worker loop, then drain a job through `POST /api/execution-jobs/:id/run`. Queue claims record `worker_id` and a short lease so stale `running` jobs can be reclaimed.

Run the external worker loop against an API started with `MANDOFORGE_EXECUTION_WORKER=queue`:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/execution-worker-loop.sh
```

Or run the Rust worker binary:

```bash
BASE_URL=http://127.0.0.1:8787 cargo run -p mandoforge-api --bin mandoforge-worker
```

Verify the handoff path:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/verify-execution-worker-loop.sh
```

Verify Docker shell runner mode against a running API started with `MANDOFORGE_SHELL_RUNNER=docker`:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/verify-docker-shell-runner.sh
```

Optional Codex App Server adapter:

```bash
MANDOFORGE_CODEX_APP_SERVER_URL=http://127.0.0.1:8789 \
MANDOFORGE_CODEX_APP_SERVER_TIMEOUT_SECONDS=30 \
cargo run -p mandoforge-api
```

When configured, Admin-only routes under `/api/codex-app-server/*` can health-check the App Server, create threads, create turns, execute turn-scoped commands, and interrupt turns. When unset, the adapter fails closed and approved `codex.exec` continues to use the Codex CLI path.

Optional provider cost alert webhook:

```bash
MANDOFORGE_COST_ALERT_WEBHOOK_URL=https://alerts.example.internal/mandoforge \
cargo run -p mandoforge-api
```

`GET /api/usage/alerts` lists warning/critical provider budget alerts. `POST /api/usage/alerts/deliver` sends the active alerts to the configured webhook; when no webhook is set, delivery stays reserved and no external request is made.

## Docker

```bash
docker compose up --build
```

The API is served on `http://127.0.0.1:8787`. Postgres starts with the runtime schema and generic demo tables.
Docker Compose also starts `mandoforge-worker`, which drains queued execution jobs through the API when `MANDOFORGE_EXECUTION_WORKER=queue`.

## Kubernetes

The Stage 1 K8s skeleton lives in [deploy/k8s](deploy/k8s).

```bash
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
```

The manifests are a starting point, not a production hardening claim. Before shared-cluster use, split Codex/sandbox workers, replace example secrets, add NetworkPolicy, and move workspaces/artifacts to durable storage.

## Architecture

- [Runtime Architecture](docs/architecture.md)
- [Stage 1 Plan](docs/stage1-plan.md)
- [Stage 1 Completion Audit](docs/stage1-completion-audit.md)
- [Stage 1 Deployment And Demo Guide](docs/deployment-guide.md)
- [Kubernetes Skeleton](deploy/k8s/README.md)

## Important APIs

- `GET /healthz`
- `GET /api/agents`
- `GET /api/agents/:id/versions`
- `GET /api/agents/:id/versions/:version`
- `GET /api/agents/:id/releases`
- `POST /api/agents/:id/releases`
- `POST /api/agents/:id/releases/:release_id/rollback`
- `GET /api/execution-jobs`
- `POST /api/execution-jobs/:id/run`
- `POST /api/sessions`
- `POST /api/sessions/:id/run`
- `GET /api/sessions/:id/events`
- `GET /api/sessions/:id/stream`
- `GET /api/sessions/:id/tool-calls`
- `GET /api/sessions/:id/audit-logs`
- `GET /api/approvals`
- `POST /api/approvals/:id/approve`
- `POST /api/approvals/:id/reject`
- `POST /api/approvals/:id/modify`
- `POST /api/approvals/:id/deliver`
- `GET /api/tool-calls`
- `GET /api/audit-logs`
- `GET /api/tools`
- `POST /api/tools/:name/execute`
- `GET /api/policy`
- `POST /api/policy/simulate`
- `POST /api/policy/test`
- `GET /api/organizations`
- `POST /api/organizations`
- `GET /api/organizations/:id/teams`
- `POST /api/organizations/:id/teams`
- `GET /api/organizations/:id/memberships`
- `POST /api/organizations/:id/memberships`
- `GET /api/teams/:id/projects`
- `POST /api/teams/:id/projects`
- `GET /api/teams/:id/provider-access`
- `POST /api/teams/:id/provider-access`
- `GET /api/eval/datasets`
- `POST /api/eval/datasets`
- `GET /api/eval/datasets/:id/cases`
- `POST /api/eval/datasets/:id/cases`
- `GET /api/eval/datasets/:id/runs`
- `POST /api/eval/datasets/:id/runs`
- `GET /api/eval/runs`
- `GET /api/usage/alerts`
- `POST /api/usage/alerts/ack`
- `POST /api/usage/alerts/deliver`
- `GET /api/codex-app-server/health`
- `POST /api/codex-app-server/threads`
- `POST /api/codex-app-server/threads/:thread_id/turns`
- `POST /api/codex-app-server/turns/:turn_id/commands`
- `POST /api/codex-app-server/turns/:turn_id/interrupt`

## Security Boundary

- Tool Router is the only intended external execution path.
- `sql.query` rejects non-read SQL commands.
- `shell.exec`, `file.write`, `codex.exec`, and `http.request` require approval by Stage 1 policy.
- `mcp.call` executes only when an MCP Gateway is configured and the target server is allowlisted.
- `shell.exec` can run through the optional Docker sandbox runner via `MANDOFORGE_SHELL_RUNNER=docker`.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without extra approval.
- Production secrets must not be passed into prompts, event logs, or Codex workspaces.

## Fit Against PRD v2

Already aligned:

- Generic agent instead of commerce manager.
- Generic diagnostics demo instead of GMV demo.
- Postgres event log with in-memory fallback.
- Tool names moved toward `file.*`, `sql.*`, `shell.exec`, `codex.exec`.
- Harness context builder plus mock OpenAI-compatible provider request/response loop.
- Env-gated OpenAI-compatible HTTP provider transport.
- Provider-emitted tool calls execute through the shared Tool Router, policy, approval, and audit path.
- Approval queue and replayable timeline.
- Tool call records and audit log records for the generic diagnostics path.
- `ToolExecutor` trait and registry for the current read/query tools.
- `artifact.create` and `approval.request` executors.
- Postgres `sql.query` execution path with JSON row normalization.
- YAML policy loading for allowed, blocked, and approval-required tool paths.
- Approval resume execution for approved `file.write` and `shell.exec` tool calls.
- Approved `codex.exec` adapter path with JSONL event ingestion and final-message artifact capture.
- Agent Builder plus artifact, tool-call, and audit detail panels in the static UI.
- Stage 1 demo script and deployment guide.
- Stage 1 final gate script.
- Live Postgres `sql.query` verification script.
- Live Docker shell runner verification script.
- Final gate coverage for approved `codex.exec` JSONL ingestion and final-message artifact capture.
- Repeatable `generic_demo.platform_events` seed generator.
- Docker Compose and K8s skeleton.
- Optional Docker sandbox wrapper for approved `shell.exec`.

Post-Stage 1 hardening:

- Split sandbox and Codex execution into separate worker processes.
- Run a credentialed external provider smoke test when provider credentials are available.
