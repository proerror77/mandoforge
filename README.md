# MandoForge

MandoForge is a Rust-native Generic Agent OS Kernel. It is not a vertical commerce, finance, or support agent. Stage 1 focuses on the runtime kernel that lets any configured agent create sessions, append events, route tools through policy, pause for approval, create artifacts, and replay the timeline.

## Stage 1 Scope

- Rust API server with Axum.
- Postgres-backed session/event/artifact/approval store with in-memory fallback.
- Generic orchestrator agent config.
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

## Docker

```bash
docker compose up --build
```

The API is served on `http://127.0.0.1:8787`. Postgres starts with the runtime schema and generic demo tables.

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
- [Kubernetes Skeleton](deploy/k8s/README.md)

## Important APIs

- `GET /healthz`
- `GET /api/agents`
- `POST /api/sessions`
- `POST /api/sessions/:id/run`
- `GET /api/sessions/:id/events`
- `GET /api/sessions/:id/stream`
- `GET /api/sessions/:id/tool-calls`
- `GET /api/sessions/:id/audit-logs`
- `GET /api/approvals`
- `POST /api/approvals/:id/approve`
- `GET /api/tool-calls`
- `GET /api/audit-logs`
- `GET /api/tools`
- `POST /api/tools/:name/execute`

## Security Boundary

- Tool Router is the only intended external execution path.
- `sql.query` rejects non-read SQL commands.
- `shell.exec`, `file.write`, `codex.exec`, and `http.request` require approval by Stage 1 policy.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without extra approval.
- Production secrets must not be passed into prompts, event logs, or Codex workspaces.

## Fit Against PRD v2

Already aligned:

- Generic agent instead of commerce manager.
- Generic diagnostics demo instead of GMV demo.
- Postgres event log with in-memory fallback.
- Tool names moved toward `file.*`, `sql.*`, `shell.exec`, `codex.exec`.
- Approval queue and replayable timeline.
- Tool call records and audit log records for the generic diagnostics path.
- Docker Compose and K8s skeleton.

Still incomplete:

- Real provider/harness loop and tool-call parsing.
- Real Tool trait implementations for file, SQL, shell, artifact, approval.
- Resume semantics after approval.
- Docker sandbox wrapper and split worker processes.
- Agent Builder form and detailed Tool/Audit panels.
