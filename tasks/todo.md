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
- [ ] Expand `tool_calls` / `audit_logs` coverage to provider-driven and worker paths.
- [ ] Split `AppState` store methods into a dedicated `store` module.
- [x] Add SQL safety tests for `sql.query`.
- [ ] Add API integration test for Generic Runtime Diagnostics replay.
- [ ] Add artifact detail panel in UI.
- [ ] Add tool-call detail panel with policy decision.

## Later Stage 1

- [ ] Implement OpenAI-compatible provider abstraction.
- [ ] Implement harness loop and context builder.
- [ ] Implement Codex JSONL event ingestion.
- [ ] Persist Codex workspace artifacts.
- [ ] Load and enforce `config/policy.stage1.yaml`.
- [ ] Add seeded platform_events generator for generic runtime demo.
- [ ] Add deployment guide and demo script.
