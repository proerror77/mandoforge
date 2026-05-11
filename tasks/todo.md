# MandoForge Task Plan

## Now

- [x] Initialize Rust workspace and Axum API skeleton.
- [x] Add Stage 1 core database migrations.
- [x] Add commerce demo schema and seed data.
- [x] Add static prototype UI.
- [x] Add YAML governance policy.
- [x] Add smoke script and CI.
- [x] Write Stage 1 implementation plan.
- [x] Initialize git repository.
- [x] Create GitHub repository and push initial commit.

## Next Slice

- [ ] Add SQLx and Postgres connection pool.
- [ ] Implement repositories for agents, sessions, session_events, artifacts, approvals, and tool_calls.
- [ ] Replace in-memory store with Postgres-backed state.
- [ ] Add SQL safety tests for `warehouse.query`.
- [ ] Add API integration test for GMV demo replay.
- [ ] Add artifact detail panel in UI.
- [ ] Add tool-call detail panel with policy decision.

## Later Stage 1

- [ ] Implement OpenAI-compatible provider abstraction.
- [ ] Implement harness loop and context builder.
- [ ] Implement Codex JSONL event ingestion.
- [ ] Persist Codex workspace artifacts.
- [ ] Load and enforce `config/policy.stage1.yaml`.
- [ ] Add seeded anomaly generator for demo warehouse.
- [ ] Add deployment guide and demo script.
