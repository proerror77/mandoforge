# Stage 2 Gap Audit

This audit maps the PRD v2 Stage 2 target to the current repo state. It is intentionally strict: reserved boundaries count as groundwork, not completed product capability.

## Status Summary

| Stage 2 area | Current state | Gap |
| --- | --- | --- |
| Multi-tenancy org/team/project | Single default tenant remains the runtime scope, but `organizations`, `teams`, `projects`, and `memberships` tables plus Admin-only hierarchy APIs are implemented. Team- and project-scoped agents and their sessions now enforce membership access for non-admin principals, and agent/session list APIs hide scoped resources outside the caller's memberships. | Full org/team/project UI and production tenant lifecycle are not implemented. |
| RBAC | Role-based authorizer guards read, write, run, tool, approval, execution job, audit, and governance admin paths. `admin`, `operator`, `approver`, and `viewer` are recognized. If role headers are absent for a subject, roles are derived from persisted memberships. Scoped agent/session/tool/approval/job paths enforce team/project membership for non-admin principals. | Production policy administration is not implemented. |
| Provider governance | OpenAI-compatible provider transport exists; API keys can be direct env values or Vault references. Admin provider registry APIs are implemented, team-level `provider_access` rows and model allowlist enforcement are implemented for team-scoped agent creation, and session runs now resolve active stored provider rows before falling back to env/mock. Stored provider configs can enforce a daily request budget. The static Provider Settings form can create/update stored providers with budget and pricing config. | Richer provider status management and token-level price/cost accounting are not implemented. |
| Vault | Reserved provider and Vault KV v2 HTTP client boundary exist. | Production secret storage, secret CRUD, rotation, and scoped secret references are not implemented. |
| Worker queue | Postgres/in-memory execution queue, worker binary, lease claims, and reclaim boundary exist; Redis command/client boundary is reserved. | Live Redis/NATS backend and separate broker worker handoff are not enabled. |
| Approval v2 | Approve/reject exists; modify now updates pending tool args, records `approval.modified`, and preserves the approval for later approve/reject. | Comments are stored in `decision_payload`; expiry, delegated approvers, and parameter diff UI are not implemented. |
| MCP Gateway | `mcp.call` is enabled through the Tool Router, uses configured MCP Gateway server allowlists, and is persisted through tool call/event/audit paths. Team-level MCP server registry APIs and persisted per-server tool allowlists are implemented; scoped sessions must use a registered active server/tool before the gateway call proceeds. Admins can import discovered tools from the configured MCP Gateway into a team server allowlist. | MCP UI management is not implemented. |
| Codex App Server adapter | Codex CLI adapter exists. | App Server thread/turn/interrupt adapter is not implemented. |
| Evaluation | Eval datasets, cases, and version-bound run records are implemented with deterministic Stage 2 graders for policy decisions, tool allowlist coverage, SQL safety, sandbox path checks, and final-answer required fragments. Runs persist per-case details, pass count, score, and agent version evidence. | Regression gates, judge integrations, drift detection, and eval dashboards are not implemented. |
| Observability | OTel exporter boundary exists and session event appends now export telemetry events when OTLP is enabled. Exported telemetry attributes classify session/provider/tool/approval/worker/sandbox/codex events as span-like signals and attach counters, status, duration, provider/client/tool IDs, approval IDs, worker IDs, and tool-call counts when present. | Dashboards, retry/backpressure, and full OTLP-native trace/metric encoding are not implemented. |
| Cost tracking | `GET /api/usage` aggregates session/event counts, provider requests/responses, configured provider per-request pricing, tool call status counts, tool runtime, and approval counts. | Token-level provider accounting, persisted cost rollups, budgets over cost windows, and dashboards are not implemented. |
| UI v2 | Static console covers agents, sessions, timeline, approvals, artifacts, tool calls, audit logs, and Admin Console panels for usage, stored providers, eval runs, and governance status. | Full CRUD UI for provider/vault/policy/worker/MCP/eval management and production dashboard polish are not implemented. |

## Next Stage 2 Slices

1. Add MCP UI management.
2. Add provider settings UI, token-level provider accounting, persisted cost rollups, and dashboards.
3. Add eval regression gates, judge integrations, drift detection, and dashboards.
4. Add dashboards, retry/backpressure, and full OTLP-native trace/metric encoding.
