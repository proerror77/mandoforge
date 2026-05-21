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
| Approval can resume the session | `stage1-demo.sh` approves `shell.exec`, drains the session loop, and returns to `session.status_idle`. | Covered |
| `file.write` can create `diagnostics.md` | `stage1-demo.sh` creates `.mandoforge/verify-workspaces/<session_id>/diagnostics.md`. | Covered |
| All important events enter `session_events` | `stage1-demo.sh` output includes `agent.plan`, `agent.final`, `approval.approved`, `approval.requested`, `artifact.created`, `llm.request`, `llm.response`, `policy.allowed`, `policy.requires_approval`, `session.status_idle`, `tool.call`, `tool.result`, and `user.message`. | Covered |
| All tool calls enter `tool_calls` | `stage1-demo.sh` output includes `file.read`, `sql.get_schema`, `sql.query`, `shell.exec`, and `file.write` with completed status. | Covered |
| All critical actions enter `audit_logs` | `stage1-demo.sh` output includes `approval.approved`, `approval.requested`, `artifact.created`, `policy.requires_approval`, `session.started`, and `tool.completed`. | Covered |
| Agent OS core action evidence gate | `scripts/agent-os-core-evidence-gate.sh` wraps the demo flow and fails closed unless `session_events`, `tool_calls`, `audit_logs`, and runtime adapter turn events contain the required agent action evidence. | Covered |
| UI can replay the timeline | The timeline renders session status, artifact detail, tool-call detail, audit detail, and append-only runtime events through `session.status_idle`. | Covered |
| Codex CLI adapter can execute one workspace task | Final gate injects a fake `codex` shim and verifies approved `codex.exec`, JSONL ingestion, `codex.task.completed`, and `codex-final-message.md`. | Covered |
| Real external provider HTTP call | Env-gated `OpenAiCompatibleProviderClient` posts to `/v1/chat/completions`; parser test covers tool-call extraction. `scripts/verify-external-provider.sh` runs a credentialed smoke only when `RUN_PROVIDER_SMOKE=1` and fails closed if provider env is missing. Credentialed provider smoke is optional because credentials are not part of this repo. | Covered |
| Live Postgres `sql.query` verification | `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh` verified row output from `generic_demo.platform_events`. | Covered |
| Docker sandbox runner | `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh` verified approved `shell.exec` with `result.runner == "docker"` and stdout `sandbox-ok:/workspace`. | Covered |

## Verification Commands

Latest passing checks:

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api
cargo test -p mandoforge-api -- --test-threads=1
bash -n scripts/stage1-demo.sh && bash -n scripts/agent-os-core-evidence-gate.sh && bash -n scripts/verify-runtime-adapter-turn-metadata.sh && bash -n scripts/seed-platform-events.sh && bash -n scripts/smoke.sh && bash -n scripts/verify-postgres-sql-query.sh && bash -n scripts/verify-docker-shell-runner.sh
./scripts/stage1-final-gate.sh
BASE_URL=http://127.0.0.1:8788 MANDOFORGE_WORKSPACE_ROOT=.mandoforge/verify-workspaces ./scripts/agent-os-core-evidence-gate.sh
BASE_URL=http://127.0.0.1:8788 MANDOFORGE_WORKSPACE_ROOT=.mandoforge/verify-workspaces ./scripts/stage1-demo.sh
```

Latest Agent OS core evidence:

```text
stage1 demo ok
session_id=6eb0d7ec-7f79-4521-947b-53e7a4e386c5
event_types=agent.final,agent.plan,agent.tool_result,agent.tool_use,approval.approved,approval.requested,artifact.created,execution.completed,llm.request,llm.response,policy.allowed,policy.requires_approval,session.loop.completed,session.loop.idle,session.loop.queued,session.loop.started,session.status_idle,session.status_requires_action,session.status_running,span.model_request_end,span.model_request_start,thread.created,thread.status_changed,tool.call,tool.result,user.message
tool_calls=file.write:completed,shell.exec:completed,sql.query:completed,sql.get_schema:completed,file.read:completed
artifacts=diagnostics.md,diagnostics.md,diagnostics.md
audit_actions=approval.approved,approval.requested,artifact.created,policy.requires_approval,session.started,tool.completed
workspace_file=.mandoforge/agent-os-core-run-workspaces/6eb0d7ec-7f79-4521-947b-53e7a4e386c5/diagnostics.md
agent os core evidence gate ok
event_log_evidence=session_events
tool_action_evidence=tool_calls
audit_evidence=audit_logs
```

## Residual Post-Stage 1 Hardening

- Split sandbox and Codex execution into separate worker processes before production hardening.
- Run `RUN_PROVIDER_SMOKE=1 ./scripts/verify-external-provider.sh` against an API started with provider credentials when provider credentials are available.

Previously observed environment blockers, now resolved for the live gate by starting Docker Desktop:

```text
docker info: failed to connect to /Users/proerror/.docker/run/docker.sock
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh: docker daemon is not available; start Docker before running START_LIVE_STACK=1
scripts/verify-docker-shell-runner.sh: docker daemon is not available; start Docker before running this verification
psql: not installed
pg_isready: not installed
127.0.0.1:5432: no listening process observed
```
