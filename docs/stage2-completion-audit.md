# Agent OS Core Completion Audit

Objective: keep the repo-controlled completion decision focused on Agent OS
core behavior.

## Completion Boundary

Agent OS core is complete when the runtime can prove what agents did:

| Requirement | Evidence | Status |
| --- | --- | --- |
| Sessions are durable | Agents, agent versions, sessions, and session events persist in the store boundary. | Covered |
| The event log is authoritative | User messages, model spans, policy decisions, approvals, tool calls/results, artifacts, runtime turns, and terminal or idle session status enter `session_events`. | Covered |
| Tool actions are durable | `tool_calls` rows record tool name, input, status, result or error, and session linkage. | Covered |
| Operator decisions are auditable | `audit_logs` records session start, policy approval requirements, approval request/decision, tool completion, artifact creation, runtime adapter activity, and other operator-relevant side effects. | Covered |
| Long tasks can resume | Session-loop jobs keep pending event sequence ranges and processed high-water marks. | Covered |
| UI can replay state | Session timeline, blocking actions, artifacts, tool calls, audit rows, and thread lineage are derived from durable runtime state. | Covered |
| Runtime adapters normalize turns | Codex CLI, Claude Code CLI, and Codex App Server output maps into runtime turn started/item/tool/usage/final/completed events. | Covered |
| Core action evidence has a focused gate | `scripts/agent-os-core-evidence-gate.sh` fails unless the demo flow leaves required records in `session_events`, `tool_calls`, `audit_logs`, and the runtime adapter turn event family. | Covered |

## Focused Verification

```bash
cargo fmt --all -- --check
cargo check -p mandoforge-api --bins
cargo test -p mandoforge-api -- --test-threads=1
bash -n scripts/stage1-demo.sh scripts/agent-os-core-evidence-gate.sh scripts/verify-runtime-adapter-turn-metadata.sh scripts/stage1-final-gate.sh
shellcheck scripts/stage1-demo.sh scripts/agent-os-core-evidence-gate.sh scripts/verify-runtime-adapter-turn-metadata.sh scripts/stage1-final-gate.sh
git diff --check
```

Against a running API:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/agent-os-core-evidence-gate.sh
```

## Completion Decision

The repo-controlled Agent OS core decision is based on durable runtime evidence.
If an agent calls a tool, requests approval, writes an artifact, receives a tool
result, completes a runtime turn, or changes session state, that action must be
replayable through `session_events`, `tool_calls`, and `audit_logs`.
