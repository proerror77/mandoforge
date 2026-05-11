# Stage 1 Plan: Commerce Agent OS Prototype

## Goal

Build a demonstrable Rust-native Managed Agents runtime that can answer:

> 昨天 GMV 为什么下降？请找出主要原因，并生成今天可执行的运营建议。

The demo must show a full loop: user message, manager plan, warehouse-backed attribution, Codex-generated artifact, operating recommendations, approval request, and replayable timeline.

## Product Boundary

Stage 1 is a single-tenant prototype. It proves the runtime shape, not the whole enterprise platform.

- Data access is demo warehouse only and read-only.
- Codex runs only in isolated session workspaces.
- High-risk actions stay draft-only and require approval.
- Specialist agents are represented as configs/tools, not independent sessions.
- Postgres event log is the durable source of truth.

## Architecture Slice

- `mandoforge-api`: Axum API server for agents, sessions, tools, artifacts, approvals, and SSE.
- Runtime store: Postgres via SQLx with in-memory fallback for local demos.
- Tool router: typed tool registry with policy decisions and audit events.
- Warehouse tools: schema introspection, read-only SQL, row/time limits.
- Codex worker: `codex exec --sandbox workspace-write --json --output-last-message`.
- UI: Agent list, session console, event timeline, tool/artifact detail, approval queue.

## Milestones

### Week 1: Foundation

- Rust workspace and Axum server.
- Docker Compose with Postgres.
- Core migrations for agents, agent_versions, sessions, session_events, workspaces, artifacts, tool_calls, approvals.
- Commerce demo schema and seed data.
- Mock harness demo path.
- GitHub repository and CI.

### Week 2: Durable API

- SQLx repository layer.
- Replace in-memory store with Postgres persistence.
- Session event append/read APIs.
- Agent CRUD and agent version snapshots.
- Basic API tests.

### Week 3: Streaming And Replay

- SSE stream for session events.
- Session replay view contract.
- Event type validation.
- Timeline filters for manager, tool, policy, approval, Codex, artifact.

### Week 4: Provider Loop

- OpenAI-compatible provider abstraction.
- Tool schema injection.
- Tool-call parsing.
- Harness turn loop with max-turn and max-tool-call limits.
- Context builder using recent event slices.

### Week 5: Tool Router And Warehouse

- `Tool` trait and registry.
- `warehouse.get_schema`.
- `warehouse.query` with read-only SQL enforcement.
- Query timeout and row limits.
- Tool call audit records with policy decision.

### Week 6: Codex Worker

- Workspace manager.
- `codex.exec` adapter.
- JSONL event capture.
- stdout/stderr/exit-code capture.
- Artifacts for final message, SQL, scripts, and logs.
- Timeout and output-size limits.

### Week 7: Commerce Manager

- Commerce manager prompt/config.
- Specialist-like tool configs for inventory and customer voice.
- GMV diagnostic prompt path.
- Seeded anomaly scenarios for SKU, stockout, ads, refunds, reviews.

### Week 8: Approval And Audit

- YAML policy loader.
- Approval-required decisions.
- Approval request, approve, reject APIs.
- Waiting-approval session state.
- Audit log projection.

### Week 9: UI Completion

- Agent list and builder minimum form.
- Session console.
- Event timeline.
- Tool detail and artifact detail.
- Approval queue.

### Week 10: Demo Polish

- Scripted demo.
- README and deployment guide.
- Smoke tests.
- SQL safety tests.
- Final acceptance pass.

## Acceptance Criteria

- The GMV demo returns GMV decline percentage.
- It lists top five abnormal SKUs.
- It includes at least three attribution dimensions: inventory, ads, refunds/customer voice.
- It produces at least four operating recommendations.
- It creates at least one approval request.
- It creates at least one artifact.
- The session timeline is replayable by event sequence.
- Tool calls show arguments, status, duration, result/error, and policy decision.
- Non-read SQL is denied and audited.

## Current Repository State

The current repository contains the Week 1 skeleton plus the first architecture hardening slice:

- Axum API with Postgres-backed runtime store when `DATABASE_URL` is set.
- In-memory fallback for quick local demos.
- Static UI for agents, timeline, report, and approvals.
- Postgres migrations and demo commerce schema.
- YAML Stage 1 policy.
- Docker Compose and CI.
- Smoke script for the mock GMV flow.

The next engineering step is splitting store/tool/harness code into modules and replacing the mock harness with the provider/tool-call loop.
