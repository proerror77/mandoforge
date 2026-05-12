# Stage 2 Gap Audit

This audit maps the PRD v2 Stage 2 target to the current repo state. It is intentionally strict: reserved boundaries count as groundwork, not completed product capability.

## Status Summary

| Stage 2 area | Current state | Gap |
| --- | --- | --- |
| Multi-tenancy org/team/project | Single default tenant remains the runtime scope, but `organizations`, `teams`, `projects`, and `memberships` tables plus Admin-only hierarchy APIs are implemented. Team- and project-scoped agents and their sessions now enforce membership access for non-admin principals, and agent/session list APIs hide scoped resources outside the caller's memberships. | Full org/team/project UI and production tenant lifecycle are not implemented. |
| RBAC | Role-based authorizer guards read, write, run, tool, approval, execution job, audit, and governance admin paths. `admin`, `operator`, `approver`, and `viewer` are recognized. If role headers are absent for a subject, roles are derived from persisted memberships. Scoped agent/session/tool/approval/job paths enforce team/project membership for non-admin principals. | Production policy administration is not implemented. |
| Provider governance | OpenAI-compatible provider transport exists; API keys can be direct env values or Vault references. Team-level `provider_access` rows and model allowlist enforcement are implemented for team-scoped agent creation. | Provider budgets, provider settings UI, and runtime provider selection from stored provider rows are not implemented. |
| Vault | Reserved provider and Vault KV v2 HTTP client boundary exist. | Production secret storage, secret CRUD, rotation, and scoped secret references are not implemented. |
| Worker queue | Postgres/in-memory execution queue, worker binary, lease claims, and reclaim boundary exist; Redis command/client boundary is reserved. | Live Redis/NATS backend and separate broker worker handoff are not enabled. |
| Approval v2 | Approve/reject exists; modify now updates pending tool args, records `approval.modified`, and preserves the approval for later approve/reject. | Comments are stored in `decision_payload`; expiry, delegated approvers, and parameter diff UI are not implemented. |
| MCP Gateway | `mcp.call` is enabled through the Tool Router, uses configured MCP Gateway server allowlists, and is persisted through tool call/event/audit paths. | MCP server registry, tool discovery import, per-team MCP config, and UI management are not implemented. |
| Codex App Server adapter | Codex CLI adapter exists. | App Server thread/turn/interrupt adapter is not implemented. |
| Evaluation | Eval datasets, cases, and version-bound run records are implemented with a first skeleton runner that records case count and agent version evidence. | Real scenario grading, regression gates, judge integrations, and eval dashboards are not implemented. |
| Observability | OTel exporter boundary exists and session event appends now export telemetry events when OTLP is enabled. | Rich traces/spans, metrics, dashboards, and retry/backpressure for exporter failures are not implemented. |
| Cost tracking | Token/tool duration data is partially present in events and tool calls. | Cost tables, provider price config, usage aggregation, and dashboards are not implemented. |
| UI v2 | Static Stage 1 console covers agents, sessions, timeline, approvals, and artifacts. | Admin/provider/vault/policy/worker/eval/usage pages are not implemented. |

## Next Stage 2 Slices

1. Add provider budgets and runtime provider selection from stored provider rows.
2. Add real eval graders for policy, tool selection, SQL safety, sandbox recovery, and final answer quality.
3. Add rich OTel spans/metrics for session run, provider call, tool execution, approval, and worker queue paths.
4. Add MCP server registry, tool discovery import, per-team MCP config, and UI management.
