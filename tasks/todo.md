# MandoForge Generic Agent OS Task Plan

## Now

- [x] Initialize Rust workspace and Axum API skeleton.
- [x] Add Stage 1 core database migrations.
- [x] Add generic demo schema and seed data.
- [x] Add static prototype UI.
- [x] Add YAML governance policy.
- [x] Add smoke script and CI.
- [x] Write Stage 1 implementation plan.
- [x] Initialize git repository.
- [x] Create GitHub repository and push initial commit.

## Next Slice

- [x] Add SQLx and Postgres connection pool.
- [x] Implement repositories for agents, sessions, session_events, artifacts, and approvals.
- [x] Replace in-memory-only state with Postgres-backed state plus local fallback.
- [x] Persist `tool_calls` rows for generic diagnostics and manual tool execution paths.
- [x] Persist `audit_logs` rows for generic diagnostics and approval decisions.
- [x] Expand `tool_calls` / `audit_logs` coverage to provider-driven and worker paths.
- [x] Split policy, provider, and shell runner support modules out of `main.rs` without behavior changes.
- [x] Split `AppState` store methods into a dedicated `store` module after Stage 1 signoff.
- [x] Split approved file, shell, and Codex execution into a dedicated `execution` module before worker-process extraction.
- [x] Add output-size limits for approved shell and Codex execution results.
- [x] Add an in-process execution queue boundary for approved tool jobs before external worker extraction.
- [x] Add an execution queue backend trait so future broker-backed queues can plug in behind the same facade.
- [x] Add fail-closed execution queue backend selection for future broker-backed queues.
- [x] Add reserved broker execution queue backend skeleton and contract test.
- [x] Add broker queue configuration and health-check boundaries before Redis/NATS implementation.
- [x] Add `ExecutionWorker` trait and `InlineExecutionWorker` implementation for swappable worker execution.
- [x] Add agent version read APIs.
- [x] Add queue-backed execution worker mode and job-drain API for external worker handoff.
- [x] Add external execution worker loop script and verification.
- [x] Persist execution jobs in Postgres when a durable store is configured.
- [x] Add execution job worker claims and lease-based stale running job reclaim.
- [x] Add Rust `mandoforge-worker` binary for API-drained execution jobs.
- [x] Add Docker Compose and Kubernetes worker deployment entries.
- [x] Split Postgres row mappers out of the store module.
- [x] Split store backend type definitions out of the store method module.
- [x] Split agent, agent-version, and session store methods out of the remaining store module.
- [x] Split session event store methods out of the remaining store module.
- [x] Split tool-call store methods out of the remaining store module.
- [x] Split artifact store methods out of the remaining store module.
- [x] Split approval store methods out of the remaining store module.
- [x] Split audit-log store methods out of the remaining store module.
- [x] Split demo seed store method and remove the catch-all store module.
- [x] Resume the provider harness after approved tool execution for provider-run sessions.
- [x] Bind sessions to agent versions and enforce agent-version tool allowlists.
- [x] Add SQL safety tests for `sql.query`.
- [x] Add API integration test for Generic Runtime Diagnostics replay.
- [x] Add artifact detail panel in UI.
- [x] Add tool-call detail panel with policy decision.
- [x] Add Agent Builder form in UI.
- [x] Add audit detail panel in UI.
- [x] Execute approved `file.write` and `shell.exec` tool calls from the approval resume path.
- [x] Add optional Docker sandbox runner for approved `shell.exec`.
- [x] Add `ToolExecutor` trait and registry for Stage 1 allowed read/query tools.
- [x] Add Postgres execution path for `sql.query` when a durable store is configured.
- [x] Add live Postgres integration verification script for `sql.query`.
- [x] Execute live Postgres integration verification for `sql.query`.
- [x] Add live Docker shell runner verification script.
- [x] Execute live Docker shell runner verification.
- [x] Add `artifact.create` and `approval.request` executors.

## Later Stage 1

- [x] Add provider abstraction with mock OpenAI-compatible harness response.
- [x] Implement harness context builder and provider request/response events.
- [x] Implement Codex JSONL event ingestion.
- [x] Persist Codex final-message artifact from approved `codex.exec`.
- [x] Load and enforce `config/policy.stage1.yaml`.
- [x] Add env-gated OpenAI-compatible HTTP provider transport.
- [x] Add credential-gated external provider smoke verification script.
- [x] Add seeded platform_events generator for generic runtime demo.
- [x] Add deployment guide and demo script.
- [x] Add Stage 1 final gate script with self-started in-memory demo and optional live stack mode.
- [x] Add Stage 1 completion audit with evidence gaps.

## Stage 2 / 3 Boundary Work

- [x] Add reserved MCP Gateway config and client boundary before enabling `mcp.call`.
- [x] Add reserved OTel observability config and exporter boundary before enabling telemetry export.
- [x] Add RBAC principal, permission, and authorizer boundary before request-path enforcement.
- [x] Add reserved Vault secret provider boundary before runtime secret reads.
- [x] Enforce RBAC on manual tool execution with a default demo operator principal.
- [x] Enforce RBAC on approval decisions with a default demo operator principal.
- [x] Enforce RBAC on execution job drain with a default demo operator principal.
- [x] Route provider API key vault references through the reserved secret provider boundary.
- [x] Enforce RBAC on session run with a default demo operator principal.
- [x] Add Vault KV v2 secret provider client skeleton without enabling runtime secret reads by default.
- [x] Add explicit reserved/vault secret provider selection while keeping reserved as the default.
- [x] Add local mock Vault KV v2 HTTP verification for token, namespace, path, and secret parsing.
- [x] Enforce RBAC on read/list API paths with default demo operator compatibility.
- [x] Enforce RBAC on core write API paths with default demo operator compatibility.
- [x] Enforce RBAC on the tool catalog API while keeping health/static public.
- [x] Add local-verified HTTP MCP Gateway client boundary before enabling `mcp.call`.
- [x] Add local-verified HTTP OTel exporter boundary before wiring runtime telemetry export.
- [x] Add local-verified Redis Stream command boundary before enabling a live broker queue backend.
- [x] Add local-verified Redis RESP client boundary before enabling Redis queue backend selection.
- [x] Add local-verified Redis worker read command boundary before enabling broker worker handoff.
- [x] Add Stage 2 gap audit mapping PRD requirements to current code evidence and remaining gaps.
- [x] Add Stage 2 approver role and pending approval modify endpoint that updates waiting tool args before approval.
- [x] Add persistent org/team/project/membership schema and Admin-only hierarchy APIs.
- [x] Derive principal roles from persisted memberships when role headers are absent.
- [x] Enforce team membership for scoped agent/session/tool/approval/job resource paths.
- [x] Filter agent/session list APIs by principal membership scope.
- [x] Add project-level permissions instead of only team-level inheritance.
- [x] Add provider access/model allowlist enforcement for team-scoped agent creation.
- [x] Add provider budgets and runtime provider selection from stored provider rows.
- [x] Add eval dataset/case/run schema and a first version-bound eval runner skeleton.
- [x] Add real eval graders for policy, tool selection, SQL safety, sandbox recovery, and final answer quality.
- [x] Wire OTel export into append-only session event paths.
- [x] Add rich OTel spans/metrics for session/provider/tool/approval/worker paths.
- [x] Enable `mcp.call` through Tool Router, MCP Gateway allowlist, events, tool calls, and audit.
- [x] Add MCP server registry and per-team tool allowlist enforcement before `mcp.call`.
- [ ] Add MCP tool discovery import and UI management.
- [x] Add usage/cost aggregation API for provider requests, tool runtime, approvals, and configured provider pricing.
- [x] Add static Admin Console panels for usage, providers, eval runs, and governance status.
