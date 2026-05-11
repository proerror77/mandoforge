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
