# Stage 1 Completion Audit

Audit date: 2026-05-11

Objective: complete Stage 1 of the generic Agent OS Kernel MVP.

## Success Criteria And Evidence

| Requirement | Evidence | Status |
| --- | --- | --- |
| UI can create a generic agent | Chrome DevTools signoff on `http://127.0.0.1:8788/` created `UI Signoff Agent` through the Agent Builder. | Covered |
| UI can create and run a session | Static UI exposes session creation/run controls; API flow is covered by `generic_runtime_diagnostics_replay_api_flow`. | Covered |
| Harness can call a provider | `run_provider_harness` emits `llm.request`, calls `ProviderClient`, and emits `llm.response`. | Covered |
| Provider response can emit a tool call | `MockProviderClient` returns tool calls; `parse_openai_compatible_provider_response` parses OpenAI-compatible tool calls. | Covered |
| Tool Router can execute `file.read` | Provider-driven run records `file.read:completed`; unit/API tests pass. | Covered |
| Tool Router can execute read-only `sql.query` | In-memory/API path and SQL safety tests pass; `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh` verified live Postgres rows through `scripts/verify-postgres-sql-query.sh`. | Covered |
| `shell.exec` triggers approval | `stage1-demo.sh` shows `policy.requires_approval` and pending approval before approval. | Covered |
| Approval can resume the session | `stage1-demo.sh` approves `shell.exec` and reaches `session.completed`. | Covered |
| `file.write` can create `diagnostics.md` | `stage1-demo.sh` creates `.mandoforge/verify-workspaces/<session_id>/diagnostics.md`. | Covered |
| All important events enter `session_events` | `stage1-demo.sh` output includes `agent.plan`, `approval.approved`, `approval.requested`, `artifact.created`, `llm.request`, `llm.response`, `policy.allowed`, `policy.requires_approval`, `session.completed`, `tool.call`, `tool.result`, and `user.message`. | Covered |
| All tool calls enter `tool_calls` | `stage1-demo.sh` output includes `file.read`, `sql.get_schema`, `sql.query`, `shell.exec`, and `file.write` with completed status. | Covered |
| All critical actions enter `audit_logs` | `stage1-demo.sh` output includes `approval.approved`, `approval.requested`, `artifact.created`, `policy.requires_approval`, `session.started`, and `tool.completed`. | Covered |
| UI can replay the timeline | Chrome DevTools signoff rendered session status `completed`, artifact detail, tool-call detail, audit detail, and append-only timeline through `session.completed`. | Covered |
| Codex CLI adapter can execute one workspace task | Final gate injects a fake `codex` shim and verifies approved `codex.exec`, JSONL ingestion, `codex.task.completed`, and `codex-final-message.md`. | Covered |
| Real external provider HTTP call | Env-gated `OpenAiCompatibleProviderClient` posts to `/v1/chat/completions`; parser test covers tool-call extraction. Credentialed provider smoke is optional because credentials are not part of this repo. | Covered |
| Live Postgres `sql.query` verification | `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh` verified row output from `generic_demo.platform_events`. | Covered |
| Docker sandbox runner | `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh` verified approved `shell.exec` with `result.runner == "docker"` and stdout `sandbox-ok:/workspace`. | Covered |

## Verification Commands

Latest passing checks:

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api
cargo test -p mandoforge-api
bash -n scripts/stage1-demo.sh && bash -n scripts/seed-platform-events.sh && bash -n scripts/smoke.sh && bash -n scripts/verify-postgres-sql-query.sh && bash -n scripts/verify-docker-shell-runner.sh
./scripts/stage1-final-gate.sh
BASE_URL=http://127.0.0.1:8788 MANDOFORGE_WORKSPACE_ROOT=.mandoforge/verify-workspaces ./scripts/stage1-demo.sh
```

Latest static final gate evidence:

```text
stage1 demo ok
session_id=02160612-eda9-4ded-9f66-b284a6bc7c64
event_types=agent.plan,approval.approved,approval.requested,artifact.created,llm.request,llm.response,policy.allowed,policy.requires_approval,session.completed,tool.call,tool.result,user.message
tool_calls=file.write:completed,shell.exec:completed,sql.query:completed,sql.get_schema:completed,file.read:completed
artifacts=diagnostics.md,diagnostics.md
audit_actions=approval.approved,approval.requested,artifact.created,policy.requires_approval,session.started,tool.completed
workspace_file=.mandoforge/final-gate-workspaces/02160612-eda9-4ded-9f66-b284a6bc7c64/diagnostics.md
stage1 static+demo gate ok
set RUN_LIVE=1 with a running API, Postgres, and Docker to execute live gates
```

Latest live final gate evidence:

```text
stage1 demo ok
session_id=4c0832f2-1360-4b4b-9d67-fe30e37b0eb5
tool_calls=file.write:completed,shell.exec:completed,sql.query:completed,sql.get_schema:completed,file.read:completed
codex adapter verification ok
session_id=72c981e1-d061-4abe-aca9-e3aa0c666a68
artifacts=codex-final-message.md
postgres sql.query verification ok
session_id=1667f520-5854-4de9-89a9-b399f4057ee1
row_count=7
first_row={"count":62,"event_type":"approval.approved","status":"ok"}
docker shell runner verification ok
session_id=f6f3148f-be5f-4bcd-9dee-b9affd572561
runner=docker
stdout=sandbox-ok:/workspace
stage1 final gate ok
docker compose ps: no running project services after gate cleanup
```

Latest browser UI signoff:

```text
url=http://127.0.0.1:8788/
created_agent=UI Signoff Agent
session_id=8d7ea5e2-c85d-44e7-9231-f8e06a9df4a3
session_status=completed
visible_artifact=diagnostics.md
visible_tool_calls=shell.exec completed, sql.query completed, sql.get_schema completed, file.read completed
visible_audit_actions=approval.approved, tool.completed, artifact.created, approval.requested, policy.requires_approval, session.started
visible_timeline_last_event=session.completed
network=business API requests returned 200; after favicon.svg fix, page reload has no console errors
```

Latest local demo evidence:

```text
stage1 demo ok
session_id=eb7a766c-fee2-4ca6-aaf0-6e680ec81bb5
event_types=agent.plan,approval.approved,approval.requested,artifact.created,llm.request,llm.response,policy.allowed,policy.requires_approval,session.completed,tool.call,tool.result,user.message
tool_calls=file.write:completed,shell.exec:completed,sql.query:completed,sql.get_schema:completed,file.read:completed
artifacts=diagnostics.md,diagnostics.md
audit_actions=approval.approved,approval.requested,artifact.created,policy.requires_approval,session.started,tool.completed
workspace_file=.mandoforge/verify-workspaces/eb7a766c-fee2-4ca6-aaf0-6e680ec81bb5/diagnostics.md
```

## Residual Post-Stage 1 Hardening

- Split sandbox and Codex execution into separate worker processes before production hardening.
- Run a credentialed external provider smoke test when provider credentials are available.

Previously observed environment blockers, now resolved for the live gate by starting Docker Desktop:

```text
docker info: failed to connect to /Users/proerror/.docker/run/docker.sock
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh: docker daemon is not available; start Docker before running START_LIVE_STACK=1
scripts/verify-docker-shell-runner.sh: docker daemon is not available; start Docker before running this verification
psql: not installed
pg_isready: not installed
127.0.0.1:5432: no listening process observed
```
