# MandoForge

MandoForge is a Rust-native Managed Agents runtime prototype for enterprise Agent OS work. Stage 1 focuses on one complete commerce demo: a user asks why yesterday's GMV dropped, the runtime records an append-only session timeline, runs governed tools against demo warehouse facts, creates an artifact, and routes risky operating actions to approval.

## Stage 1 Scope

- Rust API server with Axum.
- Session, event, tool, artifact, and approval APIs.
- Static Session Console UI with event timeline and approval queue.
- Demo commerce warehouse schema and seed data.
- YAML governance policy for blocked and approval-required tools.
- Codex CLI adapter stub that runs `codex exec` inside a per-session workspace when invoked.

The current server uses an in-memory store so the UI and API loop can be exercised immediately. The Postgres migrations are included as the durable schema target for the next implementation slice.

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

The API is served on `http://127.0.0.1:8787`. Postgres starts with the core runtime schema and commerce demo tables.

## Important APIs

- `GET /healthz`
- `GET /api/agents`
- `POST /api/sessions`
- `POST /api/sessions/:id/run`
- `GET /api/sessions/:id/events`
- `GET /api/sessions/:id/stream`
- `GET /api/approvals`
- `POST /api/approvals/:id/approve`
- `GET /api/tools`
- `POST /api/tools/:name/execute`

## Demo Prompt

```text
昨天 GMV 为什么下降？请找出主要原因，并生成今天可执行的运营建议。
```

The mock harness currently produces the required Stage 1 acceptance shape:

- GMV decline percentage.
- Top abnormal SKUs.
- Inventory, ad, refund, and customer voice attribution.
- Four operating recommendations.
- One approval request.
- Timeline and artifact events.

## Security Boundary

- Demo warehouse access is intended to be read-only.
- `warehouse.query` rejects non-read SQL commands.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without approval.
- Production secrets must not be passed into Codex workspaces.
- Coupon, price, refund, and bulk-message actions remain draft/approval-only.

## Next Implementation Slice

1. Replace the in-memory store with SQLx-backed Postgres repositories.
2. Add OpenAI-compatible provider calls and tool-call parsing.
3. Persist Codex JSONL events and generated files as artifacts.
4. Add proper tests for SQL safety, approval routing, and session replay.
5. Expand the UI detail panel for tool call arguments, results, and policy decisions.

