# Runtime Truth Audit

This audit fixes the current Agent OS boundary after the architecture refocus.
It answers one question: what runtime behavior is implemented and evidenced
inside MandoForge today, and what core runtime work remains.

## Fixed Boundary

MandoForge is the Agent OS managed runtime. It owns sessions, events, tool
routing, policy, approvals, audit, artifacts, threads, resume cursors,
streaming, and worker leases.

Codex CLI, Claude Code CLI, Codex App Server, and future hosted runtimes are
runtime adapters called and supervised by MandoForge. They are not the Agent OS
itself.

Manager Agents are managed agents running on MandoForge. They coordinate
WorkItems, Assignments, Reviews, escalations, and child threads. They do not own
a second execution stack.

## Implemented Runtime Facts

| Area | Current status | Evidence |
| --- | --- | --- |
| Durable session input | `POST /api/sessions/:id/events` persists incoming session events and wakes the session loop. | Session-event tests cover `user.message`, `user.custom_tool_result`, and session goal events driving loop jobs. |
| Session-loop cursor | Session-loop jobs store `pending_event_seq_start`, `pending_event_seq_end`, and `processed_event_seq`. Workers process explicit event windows and advance the high-water mark on completion. | `db/migrations/0041_session_loop_event_cursor.sql` and `store_session_loop_jobs.rs`. |
| Runtime adapter event ingest | CLI JSONL or stream output is preserved as `runtime_adapter.event` session events with sensitive fields redacted. | `record_runtime_adapter_events` in `execution.rs`. |
| Normalized runtime turn metadata | Codex CLI and Claude Code CLI output is mapped into `runtime.turn.started`, `runtime.item`, `runtime.tool_call`, `runtime.usage`, `runtime.final`, and `runtime.turn.completed`. Final messages are stored as artifacts. | `record_runtime_adapter_turn_metadata`, tests for `codex_cli` / `claude_code`, and `scripts/verify-runtime-adapter-turn-metadata.sh`. |
| Runtime adapter audit summary | Completed `agent_cli.exec` adapter runs write `tool.completed` audit rows with profile, runtime type, runner, adapter-event count, normalized-turn count, and final-artifact count. | `execute_approved_agent_cli` / Remote Computer agent CLI audit details and `scripts/verify-runtime-adapter-turn-metadata.sh`. |
| Codex App Server taxonomy | App Server turn create, poll, and finalize paths emit the same normalized runtime event family with thread and turn lineage. | Codex App Server runtime recording paths in `execution.rs` and app-server tests. |
| Approval and tool-result loopback | Approval decisions and queued execution completion write durable events and enqueue session-loop continuation. | Approval, execution job, and session-loop tests. |
| Streaming replay and reconnect | `/api/sessions/:id/stream` supports `?after_seq=`, `Last-Event-ID`, SSE event ids, replay, and live push for newly appended events. | `stream_events`, `stream_after_seq`, and session stream tests. |
| Environment worker binding | Workers can bind session-loop and execution-job claim/run paths to an Environment id or worker pool/queue. | `WORKER_ENVIRONMENT_ID`, `WORKER_POOL`, `WORKER_QUEUE`, and worker binding route guards. |
| Restart/resume core drill | A local Postgres-backed gate enqueues a managed session event, restarts the API, drains the queued session-loop and execution jobs with a restarted worker, verifies processed cursor advancement, thread lineage, approval/execution loopback, stale-worker rejection, and runtime adapter final-message evidence. | `scripts/managed-session-restart-resume-core-gate.sh`. |

## Important Gaps

- Runtime turn records are event-based today. There is no dedicated
  `runtime_turns` projection table yet. That is acceptable for the Agent OS core
  because `session_events` is the source of truth, but a projection table may be
  useful later for analytics and UI queries.
- `MANDOFORGE_EXECUTION_WORKER` still defaults to inline execution for local
  development. Production-like drains should run with
  `MANDOFORGE_EXECUTION_WORKER=queue` and external workers.
- Codex App Server uses the normalized taxonomy, but richer native payload
  support can still be added as that adapter surface grows.
- Any specific deployment readiness claim still needs the same restart/resume
  evidence against that target, not only the local core drill.

## Next Core Work

1. Keep runtime correctness stable: session event log, cursor, queue worker,
   adapter taxonomy, approval/tool-result loopback, streaming, and audit.
2. Move productization upward into WorkItem, Project, Assignment, Review,
   Agent Teammate, Squad, Activity Feed, and Manager Agent records.
3. Add semantic objects and context packets on top of the runtime, not inside a
   separate orchestration stack.
