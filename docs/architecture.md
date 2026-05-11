# MandoForge Runtime Architecture

## Runtime Layers

MandoForge is structured around five runtime boundaries:

1. API layer: Axum routes expose agents, sessions, events, tools, artifacts, approvals, and SSE.
2. Store layer: `AppState` owns the runtime store and hides whether data is backed by Postgres or in-memory demo state.
3. Harness layer: the current mock harness writes a deterministic GMV diagnosis timeline; the next slice replaces that with provider/tool-call turns.
4. Tool layer: tool execution is routed through named tool handlers and records `tool.call`, `tool.result`, `tool.error`, Codex, artifact, and approval events.
5. Execution layer: warehouse and Codex execution stay behind policy checks and session-scoped workspace boundaries.

## Store Boundary

The code now supports two backends:

- Postgres backend: enabled when `DATABASE_URL` is set. Startup connects through SQLx, executes Stage 1 migrations, seeds the demo tenant, and inserts the Commerce Manager Agent.
- Memory backend: enabled when `DATABASE_URL` is missing. This keeps local UI/API demos fast and avoids requiring Docker for every small change.

The public API shape is identical for both backends. Handlers call `AppState` methods instead of touching storage directly:

- `list_agents`
- `create_agent`
- `create_session`
- `get_session`
- `set_session_status`
- `append_event`
- `list_events`
- `insert_artifact`
- `list_artifacts`
- `insert_approval`
- `list_approvals`
- `decide_approval`

This is the repository boundary for Stage 1. Future SQLx query code should stay behind these methods or move into a dedicated `store` module without changing route handlers.

## Event Log Contract

`session_events` is the durable context object. It is not the model context window.

Rules:

- Events are append-only.
- `seq` is session-local and monotonic.
- Tool calls, tool results, artifacts, approvals, and final reports are linked by event sequence and payload references.
- UI replay must be derived from the event log, not from transient harness state.

Current event types used by the prototype:

- `user.message`
- `manager.plan`
- `tool.call`
- `tool.result`
- `approval.requested`
- `approval.approved`
- `approval.rejected`
- `artifact.created`
- `llm.response`
- `codex.task.started`
- `codex.task.completed`
- `codex.task.failed`
- `session.completed`

## Policy Boundary

Stage 1 policy is defined in `config/policy.stage1.yaml`.

Current enforced checks:

- `warehouse.query` rejects non-read SQL.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without approval.
- High-risk commerce actions are represented as approval requests, not executed.

Next enforcement work:

- Load YAML policy at startup.
- Write explicit `policy.allowed` and `policy.denied` events.
- Persist policy decisions into `tool_calls.policy_decision`.

## Codex Worker Boundary

`codex.exec` uses a session-scoped workspace:

```text
{MANDOFORGE_WORKSPACE_ROOT}/{session_id}
```

Allowed non-approval sandbox modes:

- `read-only`
- `workspace-write`

The adapter captures:

- stdout
- stderr
- exit code
- final message file
- Codex task events in the session timeline

Next worker work:

- Parse JSONL event lines into individual `codex.task.event` events.
- Persist final files as artifacts.
- Add output-size limits.
- Move long tasks into a queue-backed worker process.

## Verification

Current verified path:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- memory backend smoke with `./scripts/smoke.sh`

Postgres runtime path is implemented but was not runtime-smoked in this environment because Docker daemon was not running and `pg_isready` is not installed locally. It should be verified with:

```bash
docker compose up -d postgres
DATABASE_URL=postgres://mandoforge:mandoforge@localhost:5432/mandoforge cargo run -p mandoforge-api
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```

