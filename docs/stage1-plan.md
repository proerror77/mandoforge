# Stage 1 Plan: Generic Agent OS Kernel MVP

## Goal

Build a generic Rust-native Agent OS runtime kernel. Stage 1 validates the architecture loop, not an industry workflow.

The target loop is:

```text
Create Agent
-> Create Session
-> Run Harness
-> Call Provider
-> Parse Tool Call
-> Policy Check
-> Approval Pause
-> Human Approves
-> Resume
-> Execute Tool in Workspace/Sandbox
-> Create Artifact
-> Persist Events and Audit
-> Replay Timeline
```

## Product Boundary

Stage 1 is a single-tenant runtime kernel prototype.

- Agent is configuration: model, instructions, tools, policy, runtime limits.
- Session is the core execution object.
- Event log is append-only and replayable.
- Tool Router is the only external execution path.
- Sandbox and approval are separate control layers.
- Generic demo data is not industry-specific.

Stage 1 explicitly does not build commerce, finance, support, or other vertical agent templates.

## Architecture Slice

- `mandoforge-api`: Axum API server for agents, sessions, tools, artifacts, approvals, and SSE.
- Runtime store: Postgres via SQLx with in-memory fallback for local demos.
- Event store: append-only `session_events`.
- Tool router target: `file.read`, `file.write`, `sql.query`, `shell.exec`, `codex.exec`, `approval.request`, `artifact.create`.
- Policy engine target: YAML allow/block/approval-required rules.
- Sandbox target: session workspace and Docker wrapper.
- UI: Agent list, session console, event timeline, tool detail, approval queue, artifact viewer, audit log.
- Deployment: Docker Compose for local, K8s skeleton for cluster deployment.

## Milestones

### Week 1: Foundation

- Rust workspace and Axum server.
- Docker Compose with Postgres.
- K8s deployment skeleton.
- Core migrations for tenants, agents, agent_versions, providers, sessions, events, tools, approvals, artifacts, audit logs.
- Generic demo schema and seed data.
- Mock generic diagnostics harness.
- GitHub repository and CI.

### Week 2: Durable Runtime Store

- SQLx repository layer.
- Persist agents, sessions, session_events, artifacts, approvals.
- Persist tool_calls and audit_logs for generic diagnostics.
- Agent version snapshots.
- API tests for session replay and approval.

### Week 3: Agent / Session API

- Agent create/read/update/archive.
- Agent version APIs.
- Session create/message/run/pause/resume/interrupt.
- Session status events.

### Week 4: SSE And Replay

- SSE stream for session events.
- Replay view contract.
- Event type validation.
- Timeline filters for agent, LLM, tool, policy, approval, sandbox, Codex, artifact.

### Week 5: Provider Router

- OpenAI-compatible provider abstraction.
- LLM request/response events.
- Tool schema injection.
- Tool-call parsing.
- Max-turn, max-tool-call, and runtime limits.

### Week 6: Tool Router

- `Tool` trait.
- Tool registry.
- `file.read`.
- `file.write` approval path.
- `sql.query` read-only execution.
- Result normalization and artifact references.

### Week 7: Policy And Approval

- Load YAML policy.
- Emit `policy.allowed`, `policy.denied`, `policy.requires_approval`.
- Approval request, approve, reject.
- Resume after approval.

### Week 8: Sandbox And Shell

- Workspace manager.
- Workspace manifest.
- Docker command wrapper.
- `shell.exec` timeout, stdout, stderr, exit code.
- Sandbox event stream.

### Week 9: Codex CLI Adapter

- `codex.exec`.
- JSONL event parsing.
- Artifact capture.
- Timeout and output limits.
- Failure summary.

### Week 10: UI And Demo Polish

- Agent Builder.
- Session Console.
- Timeline.
- Tool Detail.
- Approval Queue.
- Artifact Viewer.
- Audit Log.
- README and deployment guide.

## Acceptance Criteria

- UI can create a generic agent.
- UI can create and run a session.
- Harness can call a provider.
- Provider response can emit a tool call.
- Tool Router can execute `file.read`.
- Tool Router can execute read-only `sql.query`.
- `shell.exec` triggers approval.
- Approval can resume the session.
- `file.write` can create `diagnostics.md`.
- All important events enter `session_events`.
- All tool calls enter `tool_calls`.
- All critical actions enter `audit_logs`.
- UI can replay the timeline.
- Codex CLI adapter can execute one workspace task.

## Current Repository State

Implemented:

- Axum API with generic orchestrator seed agent.
- Postgres-backed runtime store for agents, sessions, events, artifacts, approvals.
- In-memory fallback for local demos.
- Generic demo migrations and seed data.
- Mock Generic Runtime Diagnostics Demo.
- Tool call and audit log persistence for the demo path.
- Harness context builder plus mock OpenAI-compatible provider request/response loop.
- Env-gated OpenAI-compatible HTTP provider transport.
- Provider-emitted tool calls now execute through the shared Tool Router path.
- `ToolExecutor` trait and registry for `file.read`, `sql.get_schema`, and `sql.query`.
- `artifact.create` and `approval.request` executors.
- Postgres `sql.query` execution path with read-only validation and JSON row normalization.
- Stage 1 YAML policy loading and enforcement.
- Approval resume execution for `file.write` and `shell.exec`.
- Optional Docker sandbox wrapper for approved `shell.exec`.
- Approved `codex.exec` adapter path with JSONL event ingestion and final-message artifact capture.
- Static UI for Agent Builder, timeline/report/approval queue, artifact/tool-call details, and audit details.
- Stage 1 demo script and deployment guide.
- Stage 1 final gate script.
- Live Postgres `sql.query` verification script.
- Live Docker shell runner verification script.
- Final gate coverage for approved `codex.exec` JSONL ingestion and final-message artifact capture.
- Repeatable `generic_demo.platform_events` seed generator.
- Docker Compose.
- K8s skeleton under `deploy/k8s`.
- SQL safety and approval resume tests.

Post-Stage 1 hardening:

- Split worker processes for sandbox/Codex execution.
- Run a credentialed external provider smoke test when provider credentials are available.
