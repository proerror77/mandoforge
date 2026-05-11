# Generic Agent OS Runtime Architecture

## Fit Against PRD v2

The current architecture is partially aligned with Generic Agent OS PRD v2.

Aligned:

- Runtime-first direction.
- Agent is treated as configuration, not a microservice.
- Session and append-only event log are core objects.
- Postgres is the durable store when `DATABASE_URL` is set.
- Tool execution is routed through named tool endpoints.
- Approval and artifacts are first-class API/store concepts.
- Tool calls and audit logs are written for the current generic diagnostics path.
- Generic diagnostics demo has replaced the commerce GMV demo.
- Docker Compose and Kubernetes skeleton exist.

Not yet aligned:

- Harness still uses deterministic mock events instead of provider-driven tool-call turns.
- Tool Router is not yet a `Tool` trait registry.
- Policy YAML is not loaded/enforced centrally yet.
- Tool calls and audit logs need broader coverage for future provider-driven and worker paths.
- Approval does not yet resume the same harness turn.
- Sandbox is workspace/Codex-oriented; Docker shell sandbox runner is not implemented yet.
- MCP Gateway, OTel, RBAC, Vault, and queue workers are later-stage work.

## Runtime Layers

```text
Web UI
  Agent Builder / Session Console / Timeline / Approval / Audit

Rust Agent OS API
  Agents / Sessions / Events / Tools / Approvals / Artifacts / SSE

Managed Agent Runtime
  Session Store / Event Store / Context Builder
  Harness Loop / Provider Router / Tool Router
  Policy Engine / Approval Engine / Audit Logger
  Artifact Store / Workspace Manager / Telemetry

Execution Layer
  file.read / file.write / sql.query / shell.exec / codex.exec
  approval.request / artifact.create / mcp.call later

Sandbox / Worker Layer
  session workspace / Docker runner / Codex CLI adapter
  gVisor and distributed workers later

Context / Data Foundation
  files / artifacts / generic demo Postgres / MCP resources later
```

## Store Boundary

The code supports two backends:

- Postgres backend: enabled when `DATABASE_URL` is set. Startup connects through SQLx, executes Stage 1 migrations, seeds the demo tenant, and inserts the Generic Orchestrator Agent.
- Memory backend: enabled when `DATABASE_URL` is missing. This keeps local UI/API demos fast and avoids requiring Docker for every small change.

The public API shape is identical for both backends. Route handlers call `AppState` methods instead of touching storage directly.

Current store methods:

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
- `insert_tool_call`
- `update_tool_call_status`
- `list_tool_calls`
- `append_audit_log`
- `list_audit_logs`

Next store work:

- Keep the dedicated `store` module as the persistence boundary.
- Add agent version read APIs.
- Split store methods into smaller modules.

## Event Log Contract

`session_events` is the durable context object. It is not the model context window.

Rules:

- Events are append-only.
- `seq` is session-local and monotonic.
- Tool calls, tool results, policy decisions, approvals, artifacts, and final reports are linked by event sequence and payload references.
- UI replay must be derived from the event log, not from transient harness state.

Stage 1 event types:

- `user.message`
- `agent.plan`
- `agent.message`
- `agent.final`
- `llm.request`
- `llm.response`
- `llm.error`
- `tool.call`
- `tool.result`
- `tool.error`
- `policy.allowed`
- `policy.denied`
- `policy.requires_approval`
- `approval.requested`
- `approval.approved`
- `approval.rejected`
- `sandbox.started`
- `sandbox.output`
- `sandbox.completed`
- `sandbox.failed`
- `codex.task.started`
- `codex.task.event`
- `codex.task.completed`
- `codex.task.failed`
- `artifact.created`
- `session.started`
- `session.paused`
- `session.resumed`
- `session.waiting_approval`
- `session.completed`
- `session.failed`
- `session.interrupted`

## Tool Boundary

Target Stage 1 tools:

- `file.read`: low risk.
- `file.write`: medium risk, approval required.
- `sql.query`: medium risk, read-only enforced.
- `shell.exec`: high risk, approval required.
- `codex.exec`: high risk, approval required.
- `approval.request`: low risk.
- `artifact.create`: low risk.

The current code exposes descriptors, a `ToolExecutor` trait, a tool registry, `tool_calls`, policy events, audit logs, and normalized results for the generic diagnostics path.

## Policy Boundary

Stage 1 policy is defined in `config/policy.stage1.yaml`.

Current enforced checks:

- `sql.query` rejects non-read SQL.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without extra approval.

Next enforcement work:

- Check allowed tools per agent version rather than only the global policy file.
- Expand policy coverage for later worker, HTTP, and MCP tools.

## Sandbox Boundary

Sandbox and approval are separate:

- Sandbox controls files, process execution, network, timeout, and workspace path.
- Approval controls whether humans allow high-risk tool calls to proceed.

Stage 1 workspace target:

```text
workspaces/{session_id}/
  input/
  output/
  artifacts/
  tmp/
  logs/
  .agent-os/
    manifest.json
    policy.json
    events.jsonl
```

## Codex Worker Boundary

`codex.exec` uses a session-scoped workspace:

```text
{MANDOFORGE_WORKSPACE_ROOT}/{session_id}
```

Allowed non-extra-approval sandbox modes:

- `read-only`
- `workspace-write`

Next worker work:

- Keep `execution.rs` as the in-process execution boundary for approved `file.write`, `shell.exec`, and `codex.exec`.
- Keep output-size limits on approved shell and Codex execution results.
- Move long tasks into a queue-backed worker process.

## Deployment Boundary

Current deployment targets:

- Docker Compose for local API + Postgres.
- K8s skeleton for API + Postgres.

Stage 2/3 target components:

- `agent-os-api`
- `agent-os-web`
- `runtime-worker`
- `codex-worker`
- `sandbox-runner`
- `mcp-gateway`
- `policy-engine`
- `otel-collector`
- `postgres`
- `redis-or-nats`
- `object-storage`

## Verification

Current verified path:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- memory backend smoke with `./scripts/smoke.sh`

Postgres runtime path should be verified with:

```bash
docker compose up -d postgres
DATABASE_URL=postgres://mandoforge:mandoforge@localhost:5432/mandoforge cargo run -p mandoforge-api
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```
