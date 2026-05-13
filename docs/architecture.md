# Generic Agent OS Runtime Architecture

## Fit Against PRD v2

The current architecture is partially aligned with Generic Agent OS PRD v2.

Aligned:

- Runtime-first direction.
- Agent is treated as configuration, not a microservice.
- Sessions bind to an agent version.
- Session and append-only event log are core objects.
- Postgres is the durable store when `DATABASE_URL` is set.
- Tool execution is routed through a `ToolExecutor` registry and named tool endpoints.
- Approval and artifacts are first-class API/store concepts.
- Tool calls and audit logs are written for generic diagnostics, manual tool execution, approvals, and worker resume paths.
- Generic diagnostics demo has replaced the commerce GMV demo.
- Docker Compose and Kubernetes skeleton exist.
- Stage 1 YAML policy is loaded and enforced globally, then narrowed by session agent-version tool allowlists.
- Session run, tool catalog reads, manual tool execution, approval decisions, execution job drain, read/list API paths, and core write API paths now pass through the RBAC `Authorizer`; the demo default principal is an operator, and explicit invalid/no-role principals fail closed.
- Admin-only policy inspection and tool-decision simulation APIs expose the active YAML policy in structured form and let the static Policy Console test allow/deny/approval decisions without editing the runtime policy.
- Admin-only Vault health checks report whether the secret provider is reserved or Vault, expose only redacted configuration checks, and keep secret reads fail-closed unless Vault is explicitly configured.
- Approved shell execution can use the optional Docker runner.
- Approved jobs can be queued for external worker handoff through the execution job API and drained by `scripts/execution-worker-loop.sh`, the `mandoforge-worker` Rust binary, or the static Worker Dashboard; the queue is durable in Postgres mode and records `worker_id` plus a short lease for reclaim. Redis mode now uses `XADD` for enqueue, `XREADGROUP` for API-backed drain, and `XACK` for completion/failure acknowledgement. NATS mode publishes approved jobs to a Core NATS subject and drains through a queue subscription; it is broker-native handoff without JetStream durability. `nats_jetstream` uses request/reply publish ack, stream and durable consumer checks, pull-consumer drain, and explicit ack subject publication for completion/final failure.
- Worker deployment entries exist for Docker Compose and Kubernetes.
- Approval resume executes the approved tool, rebuilds harness context, resumes the provider for provider-run sessions, and then emits the final provider response before completing the session.
- Approval v2 groundwork includes an `approver` role, `POST /api/approvals/:id/modify`, which updates pending tool-call arguments, appends `approval.modified`, and leaves the approval pending for approve/reject, plus `expires_at` and `POST /api/approvals/:id/expire`, which append `approval.expired` and make later decisions fail closed. Manual approval requests can delegate a decision to an `approver_subject`; non-admin principals must match that subject while admins can override. Approval groups and escalation rules are persisted through Admin-only APIs, `POST /api/approvals/:id/escalate` assigns pending approvals to a configured group/rule, and non-admin approvers must belong to the delegated group before deciding. The static Approval Queue exposes original args, modified args, delegated approver/group, a JSON-path argument diff table, decision payload, and pending-approval modify/escalate controls before approve/reject.
- Stage 2 governance groundwork includes `organizations`, `teams`, `projects`, `memberships`, and `tenant_invitations` tables plus Admin-only CRUD/list routes for the hierarchy; principals can derive roles from persisted memberships when role headers are absent, scoped agent/session/tool/approval/job access checks team or project membership for non-admin principals, and agent/session list APIs hide scoped resources outside the caller's memberships. The runtime is still a single-tenant deployment, so any incoming `x-mandoforge-tenant-id` header must match the configured runtime tenant and fails closed otherwise; tenant isolation readiness now includes a production routing gate that blocks until runtime tenant routing, fail-closed headers, membership scope enforcement, and RLS are all ready. Organizations persist `owner_subject`, new organizations default ownership to the creating admin subject, ownership transfer is audited and rejected for archived organizations, archived empty organizations can be deleted only when no teams, memberships, or invitations remain, and tenant bootstrap provisioning creates an organization plus optional team/project and owner membership in one audited path. Admins can create/revoke scoped tenant invitations, invitees can accept pending token-bound invitations without a pre-existing role, acceptance creates the scoped membership, and invitation create/revoke/accept/expire decisions are audited. The static Admin Console can bootstrap tenant scopes, create/select organizations, create teams/projects, create memberships, transfer organization ownership, archive/delete organizations, create/revoke tenant invitations, and reuse selected team IDs in adjacent provider/MCP governance forms.
- Provider governance groundwork includes Admin-only provider registry routes, audited `PATCH /api/providers/:id/status` for active/disabled/archived emergency lifecycle control, `POST /api/providers/:id/status-approval` plus approve/reject routes for separation-of-duties provider status changes, `GET /api/providers/:id/health` for static provider configuration checks plus audited OpenAI-compatible `/v1/models` probes when `base_url` and `api_key_env` are configured, `provider_access` rows per team, model allowlist enforcement when creating a team-scoped agent, fail-closed creation when a stored provider is archived or disabled, runtime provider selection from stored provider rows with active-status enforcement, and daily request/cost budget gates before `llm.request` events are emitted. Direct provider status changes must declare `emergency=true` and a non-empty reason; audit records include the `provider_lifecycle_emergency` policy decision with previous/requested status evidence. Provider policy gates can be inspected through `GET /api/providers/policy-gate`, executed as audited manual runs through `POST /api/providers/policy-gate/run`, executed by the aggregate scheduler when no fresh run covers configured providers, and reviewed through `GET /api/providers/policy-gate/runs`; the run summary now includes production enforcement status so a failed, warning, stale, or missing gate explicitly blocks provider rollout. `POST /api/providers/production-rollout/run` records an audited production rollout only when the latest gate is fresh, passing, and covers the current provider set; otherwise it records a blocked rollout without enabling production use.
- Usage groundwork includes `GET /api/usage`, which aggregates provider request/response counts, prompt/completion/total tokens from provider responses, configured per-request and per-1K-token provider pricing, provider budget status over the last 24 hours, tool status counts, tool runtime, approval counts, sessions, and session events for the current tenant. Admins can persist current usage/cost snapshots through `POST /api/usage/rollups`, query `GET /api/usage/trends` for current-vs-rollup or rollup-vs-rollup cost/token/tool-call deltas, 7-day and 30-day run-rate forecasts, provider budget exhaustion projections, budget pressure, top-provider cost attribution, and operator recommendations, export an audited finance CSV through `GET /api/usage/export.csv`, and review provider cost/token breakdowns, provider budget forecast rows, trend windows, CSV export status, tool runtime breakdowns, and rollup snapshots in the static console. `GET /api/usage/finance-operations/summary` adds an operations view over that data by combining readiness score, rollup freshness, export status, alert delivery status, alert acknowledgement coverage, recent finance/export audit evidence, runbook actions, and a production close readiness gate that blocks until rollups are fresh, the export target is configured, recent delivered export evidence exists, current alerts are delivered, critical alerts are acknowledged, and no failed delivery evidence is present. `POST /api/usage/finance-operations/run` executes a controlled finance close run that creates a missing/stale rollup and then uses existing alert/export delivery boundaries only when their configuration is ready.
- Observability groundwork includes `GET /api/observability`, which summarizes telemetry configuration, session/tool/approval/job status counts, event categories, recent error events, and queue/approval backpressure signals for the static dashboard. `GET /api/observability/collector-readiness` exposes OTLP collector readiness, signal paths, health-check result, sampling state, collector-specific attention items, and a production-ops readiness gate that blocks until OTLP is enabled, the collector endpoint and logs/traces/metrics paths are configured, sampling is nonzero, and collector health is checked healthy. The HTTP telemetry exporter now emits OTLP-shaped logs, traces, and metrics payloads to `/v1/logs`, `/v1/traces`, and `/v1/metrics` from the same runtime session-event boundary. `POST /api/observability/remediation/run` turns safe backpressure signals into an audited remediation run by executing due approval expiration/escalation checks and Codex App Server stale-turn polling while returning explicit manual-action markers for worker drains and failure triage.
- External scheduler groundwork includes `POST /api/scheduler/run-due`, an Admin-only audited aggregate endpoint designed for cron or Kubernetes CronJob callers. It runs the same due-run boundaries used by the UI for provider policy gates, policy rollout activation, approval expiration/escalation, agent release automation, MCP scheduled health checks, MCP connector rollouts across active teams, Codex App Server stale-turn polling, current cost-alert delivery when active alert routing or fallback webhook configuration exists, and optional scheduled finance export delivery, then returns a single run summary for scheduler logs and operations review. The Kubernetes skeleton includes `mandoforge-scheduler`, a CronJob that calls this endpoint every five minutes for self-hosted pilot deployments.
- The static web console now includes Admin Console panels for usage, policy inspection/simulation, stored providers, eval runs, and governance status in addition to the Stage 1 session timeline, approvals, artifacts, tool calls, and audit views.
- Policy Center supports active YAML policy inspection, single-tool simulation, batch policy testing, validated persisted policy revisions, revision diffing against the active runtime policy, default or custom rollout gate suites, rollout percentage metadata, optional RFC3339 activation windows, activation metadata, and audit logging for policy simulation/test/revision actions. The static console renders policy diffs as added/changed/removed summaries and tables, and renders gate results as rollout, suite, pass/fail, activation window, and per-tool expected/actual decisions. Activating a 100% gated revision hot-swaps the in-memory runtime policy used by simulation, SQL safety, and Tool Router decisions; activating a partial rollout keeps the current baseline policy and applies the candidate to a deterministic percentage of sessions by session UUID bucket. Admins can inspect runtime rollout status, cancel a staged rollout, run due scheduled rollout activation for passed draft revisions inside their activation window, and roll back the active policy to the most recent archived active revision through audited API/UI paths.
- Approval v2 supports delegated approver and delegated group enforcement, escalation rules, expiry, argument modification, persisted notification channel policy with bounded retries/backoff, env-gated webhook notification delivery for pending approvals through `MANDOFORGE_APPROVAL_WEBHOOK_URL`, Slack delivery through `MANDOFORGE_APPROVAL_SLACK_WEBHOOK_URL`, email-relay delivery through `MANDOFORGE_APPROVAL_EMAIL_RELAY_URL`, and an Admin due-run API that expires overdue approvals, advances pending approvals through ordered escalation rules when their `after_seconds` threshold is due, writes timeline/audit records, and attempts notification delivery for each scheduled escalation. Delivery payloads include delegated approver or approval-group target subjects so downstream channels can fan out to the right people while MandoForge keeps the approval decision boundary centralized. `GET /api/approvals/notification-channel-policies` and `POST /api/approvals/notification-channel-policies` manage persisted channel policy without storing endpoint secrets. `POST /api/approvals/notifications/run` performs an audited bounded delivery pass for pending approvals, skips recently notified approvals, and `GET /api/approvals/notifications/runs` exposes delivery-run history plus production-ops readiness that blocks on missing channels, unroutable pending approvals, failed/reserved/stale runs, or missing delivery evidence.
- Provider Settings in the static console can create/update stored providers with base URLs, API key env/ref credentials, daily request budgets, per-request and per-1K-token pricing config, request separated status approvals, perform explicitly reasoned emergency activate/disable/archive actions, run/review provider policy gates, and inspect provider health check results.
- Provider key-reference rotation is exposed as an Admin API that updates `config.api_key_ref`, removes any env-key fallback, and writes an audit record containing only old/new secret references, never secret values.
- Usage/cost tracking includes warning/critical provider budget alerts, an Admin-only alert listing endpoint, audited acknowledgement route, audited alert route management for webhook/Slack/email channels, and a delivery route that uses configured webhook routes, Slack incoming webhook routes, HTTP email relay routes through `MANDOFORGE_COST_ALERT_EMAIL_RELAY_URL`, direct SMTP relay through `MANDOFORGE_COST_ALERT_SMTP_ADDR` and `MANDOFORGE_COST_ALERT_SMTP_FROM`, or the fallback `MANDOFORGE_COST_ALERT_WEBHOOK_URL`. Alert delivery attempts now write audit metadata, the aggregate scheduler due-plan/run surfaces process alert delivery when current alerts and a configured route/target exist, and the static Usage panel can create routes, trigger delivery or acknowledgement, run Finance Ops, show the latest status, and review Finance Operations readiness/runbook actions.
- Finance usage exports can be downloaded through `GET /api/usage/export.csv`, delivered manually through `POST /api/usage/export/deliver`, or run from the aggregate scheduler when `MANDOFORGE_USAGE_EXPORT_SCHEDULE` is enabled. Delivery is fail-closed/reserved until `MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL` is configured, and audit records store metadata only, not the CSV body.
- Evaluation groundwork includes eval datasets, cases, and version-bound run records with deterministic Stage 2 graders for policy decisions, tool allowlist coverage, SQL safety, sandbox path checks, and final-answer required fragments. `grading_policy.kind = "judge"` uses an env-gated `EvalJudgeClient` boundary when `MANDOFORGE_EVAL_JUDGE_URL` is configured, or a persisted `eval_judge` provider profile when `grading_policy.judge_profile` names an active profile. Judge profiles store endpoint/model/timeout and optional `vault:path#key` API key references in the provider registry, augment judge requests with profile/model evidence, and never resolve or expose secret values in UI/audit payloads. Unconfigured judge cases fail closed with explicit run details. `POST /api/eval/suites/stage2-regression` bootstraps a default regression dataset covering high-risk policy, blocked tools, required tool coverage, read/write SQL safety, sandbox path boundaries, final-answer evidence, and optional external judge scoring. `POST /api/eval/runs/:id/gate` evaluates a run against score/status requirements and returns pass/fail reasons. `POST /api/agents/:id/releases` enforces that the referenced eval run targets the same agent version, is completed, and meets `min_score` before directly promoting a release; `POST /api/agents/:id/release-requests` creates a `pending_approval` production promotion request with requested-by, delegated approver, request reason, eval evidence, and optional automation policy. Approve/reject endpoints enforce separation of duties before a pending request can become `promoted` or `rejected`. `POST /api/agents/releases/run-due` scans pending release requests, auto-promotes only system-delegated requests whose automation window is open and whose eval score still meets the gate, and auto-rejects expired requests fail-closed with audit records. `GET /api/agents/releases/automation-runs` summarizes audited release automation history and reports a production rollout readiness gate that blocks on missing/stale supervision, expired/stale pending releases, skipped auto-pending automation, or any remaining pending release request. `GET /api/agents/releases/summary` aggregates release status, environment, pending/manual/automated queues, expired/stale attention items, and latest promoted releases by environment for rollout dashboards. `POST /api/agents/:id/releases/:release_id/rollback` marks promoted releases as rolled back. `GET /api/eval/runs/:id/drift` compares a run to the previous run for the same dataset and agent. The static console can create eval judge profiles, bootstrap the Stage 2 suite, create eval datasets, add JSON eval cases, run an agent against a dataset, inspect cases/runs, gate a run at 100%, check drift, request manual or automated prod approval, run due release automation, review release automation run history and production readiness, review rollout summary metrics/attention items/latest promotions, promote passing runs to staging/prod, approve/reject pending releases, list releases, and roll back promoted releases.
- OTel groundwork is now wired into the session event append path, so session, provider, tool, approval, sandbox, codex, and worker events can be exported through the configured telemetry exporter with native OTLP HTTP log/trace/metric payloads, status, counters, duration, provider/client/tool IDs, approval IDs, worker IDs, and tool-call counts when those fields are present.
- OpenAI-compatible provider credentials can be direct env values or `vault:path#key` secret references; vault references use the `SecretProvider` boundary and fail closed on the default reserved provider. `MANDOFORGE_SECRET_PROVIDER=vault` explicitly selects the Vault KV v2 provider boundary, while the default remains `reserved`.
- The static Vault panel can run `GET /api/vault/health`, register scoped secret references, list reference metadata, rotate references without exposing secret values, and execute `POST /api/vault/kms/rotation/run` as an audited KMS rotation gate. Vault readiness includes a production rotation gate that blocks until Vault is healthy, external KMS/HSM readiness is configured with `MANDOFORGE_KMS_ENDPOINT`, all consumer refs resolve to catalog entries, no active refs are stale, and a recent validated rotation gate run exists. The rotation gate reports `blocked` unless Vault and external KMS/HSM settings are present, including `MANDOFORGE_KMS_ENDPOINT` for provider validation, and it does not claim secret value rotation without concrete provider support.
- Codex App Server groundwork includes an env-gated `CodexAppServerClient` boundary and Admin-only adapter routes for health, thread creation, turn creation, turn polling, stale-turn polling, command execution, interrupt, persisted run listing, trace summary/detail, and artifact sync into MandoForge session artifacts. If `MANDOFORGE_CODEX_APP_SERVER_URL` is unset, the adapter fails closed. Thread/turn/command/interrupt/poll responses are persisted in `codex_app_server_runs` for replay/debugging, and bounded polling updates run status/error with audit history. `POST /api/codex-app-server/runs/poll-stale` finds stale non-terminal turn runs, polls each bounded candidate, writes per-run poll audit records plus a stale-poll due-run audit record, and `POST /api/scheduler/run-due` invokes the same path for external cron/Kubernetes supervision. `GET /api/codex-app-server/control-plane/summary` now includes a production-ops readiness gate that blocks when the adapter is unconfigured, failed/interrupted traces exist, stale non-terminal turns exist, stale-poll supervision is missing/stale, or the latest stale-poll due-run failed. `GET /api/codex-app-server/traces/:trace_key` returns the run list, status timeline, errors, and latest response for a single trace. Approved `codex.exec` can run with `auto`, `cli`, or `app-server` execution strategy; `auto` tries App Server when configured and records `codex.task.fallback` before returning to the CLI path if the App Server attempt fails. Queue-backed approved `codex.exec` work is drained by execution workers instead of running inline: the worker creates App Server thread/turn records, polls bounded turn status, writes `codex.task.event` timeline records, records retryable failures back to the execution queue, and completes the tool call only after a terminal completed turn. The static Admin Console exposes steering actions, stale-turn polling, trace detail drill-downs, imports returned artifacts into the timeline/audit path, lists persisted steering runs, summarizes active/terminal/failed long-running turns, and renders reserved/fail-closed production ops responses clearly.

Not yet aligned:

- External worker mode is API-drained for memory/Postgres/Redis/NATS queues. Redis Streams enqueue and drain are available through `MANDOFORGE_EXECUTION_QUEUE_BACKEND=redis`; Core NATS publish and queue-subscription drain are available through `MANDOFORGE_EXECUTION_QUEUE_BACKEND=nats`. NATS JetStream publish, durable pull-consumer drain, and explicit ack are available through `MANDOFORGE_EXECUTION_QUEUE_BACKEND=nats_jetstream` or `jetstream`.
- Credentialed external provider verification exists for both env-key and Vault-reference OpenAI-compatible provider configs; secret values are resolved only through the `SecretProvider` boundary and are not persisted in health/audit payloads.
- MCP Gateway execution is now available through `mcp.call` when configured; global server allowlists are enforced by the gateway config, and team-scoped sessions must also pass the persisted MCP server registry/tool allowlist before the HTTP call. Admins can call the team MCP server discovery endpoint to import gateway-discovered tools into the persisted allowlist. The team MCP server lifecycle APIs can patch transport/config/allowlists, activate/disable/archive connectors, run audited single-server health checks, run audited team-level batch health checks, and run due-only scheduled health checks based on connector `config.health_check.interval_seconds`. MCP connector configs can declare normalized `vault:path#key` `secret_refs`; the API validates those references and health checks report reference counts/paths without resolving or exposing secret values. At runtime, scoped `mcp.call` resolves configured secret refs through the `SecretProvider` boundary before calling the MCP Gateway and fails closed before gateway I/O if refs cannot be read; secret values are not written into tool args, timeline, or audit payloads. Scheduled due runs persist last health metadata back to connector config for cron/worker-driven supervision. Connector rollout APIs can stage config/transport/tool/status changes with candidate health preflight, activation windows, audited due-run application, and rollback to the previous snapshot. `GET /api/teams/:team_id/mcp-servers/rollouts/summary` aggregates connector status, transport, pending/manual/scheduled/due/expired rollout queues, preflight failures, attention items, and latest rollout history for rollout dashboards; `GET /api/teams/:team_id/mcp-servers/rollouts/runs` exposes audited rollout due-run history plus production rollout readiness and production orchestration readiness that block on failed preflight, expired/due/pending rollouts, manual apply steps, failed/stale/missing due-runs, or missing fresh scheduler supervision evidence. Disabled/archived connectors fail closed before the MCP Gateway call. The static Admin Console can manage config JSON, secret refs, allowlists, health checks, due health, discovery import, status changes, rollout requests, due rollout runs, apply, rollback, and review connector rollout summary metrics/attention/history.
- Adding broader remediation automation, remaining production RBAC policy expansion, and production Vault providers remain later-stage work.

## Runtime Layers

```text
Web UI
  Agent Builder / Session Console / Timeline / Approval / Audit

Rust Agent OS API
  Agents / Sessions / Events / Tools / Approvals / Artifacts / SSE

Managed Agent Runtime
  Session Store / Event Store / Context Builder
  Harness Loop / Provider Router / Tool Router
  Policy Engine / Approval Engine / Audit Logger
  Artifact Store / Workspace Manager / Telemetry

Execution Layer
  file.read / file.write / sql.query / shell.exec / codex.exec
  approval.request / artifact.create / mcp.call

Sandbox / Worker Layer
  session workspace / Docker runner / Codex CLI adapter / Codex App Server adapter
  gVisor and distributed workers later

Context / Data Foundation
  files / artifacts / generic demo Postgres / MCP resources later
```

## Store Boundary

The code supports two backends:

- Postgres backend: enabled when `DATABASE_URL` is set. Startup connects through SQLx, executes Stage 1 migrations, seeds the demo tenant, and inserts the Generic Orchestrator Agent.
- Memory backend: enabled when `DATABASE_URL` is missing. This keeps local UI/API demos fast and avoids requiring Docker for every small change.

The public API shape is identical for both backends. Route handlers call `AppState` methods instead of touching storage directly.

Current store method groups:

- `store_entities`: agents, agent versions, and sessions.
- `store_events`: append-only session events.
- `store_tool_calls`: tool-call persistence and status updates.
- `store_artifacts`: artifact persistence.
- `store_approvals`: approval persistence and decisions.
- `store_audit`: audit-log persistence.
- `store_seed`: demo agent seed wiring.

Store boundary rules:

- Keep the `store_*` modules as the persistence boundary.
- Keep agent version read APIs available for agent/version inspection.
- Keep Postgres row mapping in `store_rows` so storage queries and row decoding can evolve separately.
- Keep backend type definitions in `store_backend` so startup wiring and store methods share the same backend contract.
- Keep agent, agent-version, and session storage methods in `store_entities`.
- Keep session event log storage methods in `store_events`.
- Keep tool-call storage methods in `store_tool_calls`.
- Keep artifact storage methods in `store_artifacts`.
- Keep approval storage methods in `store_approvals`.
- Keep audit-log storage methods in `store_audit`.
- Keep demo seed storage wiring in `store_seed`.

## Event Log Contract

`session_events` is the durable context object. It is not the model context window.

Rules:

- Events are append-only.
- `seq` is session-local and monotonic.
- Tool calls, tool results, policy decisions, approvals, artifacts, and final reports are linked by event sequence and payload references.
- UI replay must be derived from the event log, not from transient harness state.

Stage 1 event types:

- `user.message`
- `agent.plan`
- `agent.message`
- `agent.final`
- `llm.request`
- `llm.response`
- `llm.error`
- `tool.call`
- `tool.result`
- `tool.error`
- `policy.allowed`
- `policy.denied`
- `policy.requires_approval`
- `approval.requested`
- `approval.approved`
- `approval.rejected`
- `sandbox.started`
- `sandbox.output`
- `sandbox.completed`
- `sandbox.failed`
- `codex.task.started`
- `codex.task.event`
- `codex.task.completed`
- `codex.task.failed`
- `execution.queued`
- `artifact.created`
- `session.started`
- `session.paused`
- `session.resumed`
- `session.waiting_approval`
- `session.completed`
- `session.failed`
- `session.interrupted`

## Tool Boundary

Target Stage 1 tools:

- `file.read`: low risk.
- `file.write`: medium risk, approval required.
- `sql.query`: medium risk, read-only enforced.
- `shell.exec`: high risk, approval required.
- `codex.exec`: high risk, approval required.
- `approval.request`: low risk.
- `artifact.create`: low risk.

The current code exposes descriptors, a `ToolExecutor` trait, a tool registry, `tool_calls`, policy events, audit logs, and normalized results for the generic diagnostics path.

## Policy Boundary

Stage 1 policy is defined in `config/policy.stage1.yaml`.

Current enforced checks:

- Sessions bind to an agent version at creation time.
- Tool execution is constrained by the session agent version's enabled tools before global YAML policy is applied.
- Agent version `approval_policy` can narrow allowed tools, block tools, or require approval for a tool.
- `sql.query` rejects non-read SQL.
- `codex.exec` only allows `read-only` and `workspace-write` sandbox modes without extra approval.

Next enforcement work:

- Expand policy coverage for later worker, HTTP, and MCP tools.

## Sandbox Boundary

Sandbox and approval are separate:

- Sandbox controls files, process execution, network, timeout, and workspace path.
- Approval controls whether humans allow high-risk tool calls to proceed.

Stage 1 workspace target:

```text
workspaces/{session_id}/
  input/
  output/
  artifacts/
  tmp/
  logs/
  .agent-os/
    manifest.json
    policy.json
    events.jsonl
```

## Codex Worker Boundary

`codex.exec` uses a session-scoped workspace:

```text
{MANDOFORGE_WORKSPACE_ROOT}/{session_id}
```

Allowed non-extra-approval sandbox modes:

- `read-only`
- `workspace-write`

Current worker boundary:

- Keep `execution.rs` as the in-process execution boundary for approved `file.write`, `shell.exec`, and `codex.exec`.
- Keep output-size limits on approved shell and Codex execution results.
- Keep `execution_queue.rs` as the queue facade for approved tool jobs.
- Keep `ExecutionQueueBackend` as the backend seam for memory, Postgres, and later broker-backed queues.
- Keep `execution_queue_broker.rs` as the broker backend boundary. Redis Stream enqueue/group/read/ack operations, Core NATS publish/queue-subscription drain operations, and raw NATS JetStream publish/pull/ack operations are implemented behind the `ExecutionQueueBackend` facade and locally verified with mock TCP tests. Core NATS mode is not a JetStream durability claim; JetStream mode requires `MANDOFORGE_NATS_URL` plus optional `MANDOFORGE_NATS_STREAM` and consumer group overrides.
- Keep `BrokerQueueConfig` and `BrokerQueueHealthCheck` as the broker configuration and readiness boundary before selecting a concrete Redis or NATS client.
- Keep `codex_app_server.rs` as the env-gated HTTP adapter boundary for experimental Codex App Server thread, turn, polling, stale polling, command, and interrupt APIs. The reserved client fails closed unless `MANDOFORGE_CODEX_APP_SERVER_URL` is configured, and the static Codex App Server panel should surface that reserved state without bypassing the approval-governed `codex.exec` path. Keep App Server artifact sync inside the existing artifact/event/audit path so replay does not depend on App Server state. Persisted Codex App Server runs are grouped by `GET /api/codex-app-server/traces` into per-turn trace summaries with run/command/poll/error counts, latest status, terminal state, and operation sets for dashboard review; `GET /api/codex-app-server/traces/:trace_key` is the trace drill-down boundary for per-run status timelines and latest responses. Scheduler due-runs call the stale-turn polling path so cron/Kubernetes supervision can advance old non-terminal turns without relying on manual UI polling. Keep `codex.exec` strategy selection explicit: `app-server` fails closed on App Server errors, `cli` stays on Codex CLI, and `auto` records fallback before returning to CLI. When `MANDOFORGE_EXECUTION_WORKER=queue`, approved Codex App Server work must remain worker-drained through execution jobs so long-running turn supervision is observable, auditable, and recoverable through the same worker dashboard.
- Execution jobs persist `attempt_count`, `max_attempts`, and `last_error`. Worker failures should call `retry_or_fail`: retryable jobs return to `queued` for another lease, while exhausted jobs move to `failed`.
- Keep `MANDOFORGE_EXECUTION_QUEUE_BACKEND` fail-closed for unsupported values: `auto`, `memory`, `postgres`, `redis`, and `nats` are selectable now; generic `broker` remains reserved.
- Keep `ExecutionWorker` as the swappable worker interface and `InlineExecutionWorker` as the current local implementation.
- Keep queue-backed worker mode, the API-drained `mandoforge-worker` binary, and the shell worker loop as the current external-worker handoff.
- Keep `GET /api/execution-jobs/worker-readiness` as the operator gate for this boundary. It reports active queue semantics, worker mode, queued/running/retryable/failed pressure, stale leases, K8s worker manifest coverage, worker Pod hardening status, ServiceAccount/token automount state, RuntimeDefault seccomp, privilege/capability/read-only-root checks, resource bounds, NetworkPolicy presence, HPA/KEDA autoscaling manifest presence, parsed scale targets/min/max replicas, KEDA trigger types, queue-depth scaling evidence, worker load-validation evidence, a production-ops gate, attention items, and runbook actions. `POST /api/execution-jobs/worker-load-validation/run` records an audited validation run without faking success; it reports `attention` unless a durable queue, hardened worker Pod, queue-depth autoscaling, isolated worker pool, and explicit production-like load-validation gate are all present. The production-ops gate remains blocked until durable queue mode, queue-backed workers, hardened worker Pod manifests, queue-depth autoscaling, isolated worker pool evidence, validated load evidence, no failed jobs, and no stale leases are all true. JetStream mode now reports durable broker handoff, worker KEDA exposes queue-depth scaling configuration, and the worker manifest is restricted by default; the same readiness path still exposes remaining gaps such as production load validation and hard-isolated worker pools.
- Replace the API-drained queue with a broker-backed queue in a later production stage.

## Tenant Isolation Boundary

- Keep the runtime bound to one configured tenant ID until cross-tenant request routing is implemented. Incoming `x-mandoforge-tenant-id` headers must match that runtime tenant and fail closed otherwise.
- Keep org/team/project membership enforcement as the application-layer scope boundary for Stage 2 team pilots.
- Keep `GET /api/tenant-isolation/readiness` as the operator gate for this boundary. It reports runtime tenant mode, header fail-closed status, scoped resource counts, tenant-scoped table coverage, Postgres Row Level Security migration/runtime state, and a production routing gate that remains blocked while the runtime is single-tenant.
- `db/migrations/0024_tenant_rls_policies.sql` enables and forces RLS for tracked tenant-scoped tables, including an indirect `agent_versions` policy through `agents`. Postgres connections set `mandoforge.tenant_id` to the configured runtime tenant before use.
- Keep runtime tenant switching and cross-tenant RLS tests as production blockers before claiming full production multi-tenant serving.

## Vault Boundary

- Keep `secret_records` as a reference catalog only: it stores path/key/scope/version metadata, never secret values.
- When `POST /api/vault/secrets` or `POST /api/vault/secrets/:id/rotate` includes a `value`, write that value through the configured `SecretProvider` before mutating the catalog. Reserved or incomplete providers fail closed and do not create or rotate the catalog record.
- Vault KV v2 is the first concrete provider for read/write. External KMS/HSM envelope encryption and production rotation workflows remain explicit Stage 2 gaps; `GET /api/vault/readiness` exposes a fail-closed production rotation gate, and `POST /api/vault/kms/rotation/run` records audited blocker/attention evidence until those production providers are configured with provider, key, rotation policy, validation endpoint, resolved refs, and recent validated rotation evidence.

## Deployment Boundary

Current deployment targets:

- Docker Compose for local API + Postgres + worker.
- K8s skeleton for API + Postgres + worker.

Stage 2/3 target components:

- `agent-os-api`
- `agent-os-web`
- `runtime-worker`
- `codex-worker`
- `sandbox-runner`
- `mcp-gateway` with `McpGatewayConfig`, `McpGatewayClient`, and a local-verified `HttpMcpGatewayClient` HTTP boundary behind `mcp.call`.
- `policy-engine` with `Principal`, `Permission`, and `Authorizer` as the RBAC boundary; session run, manual tool execution, approval decisions, execution job drain, read/list APIs, and core write APIs are enforced request paths.
- `otel-collector` with `ObservabilityConfig`, `TelemetryExporter`, and a local-verified `HttpTelemetryExporter` OTLP HTTP boundary before wiring runtime export paths.
- `vault` or compatible secret store with `SecretProviderKind`, `SecretProviderConfig`, `SecretRef`, `SecretProvider`, and explicit `reserved` / `vault` provider selection before enabling runtime secret reads by default.
- `postgres`
- `redis-or-nats`
- `object-storage`

## Agent Remote Computer Target

MandoForge should evolve from generic worker drains toward a Kubernetes Pod-based Agent Remote Computer substrate. In that model, a governed session leases an isolated Pod that behaves like a remote machine for the agent: OS tools, workspace, approved skills, runtime config, code execution, artifact output, and state mounts all live behind the same Tool Router, Policy Engine, Approval Engine, event log, and audit path.

The current repo does not yet implement this substrate as a complete Pod execution path. Today it has K8s API/worker/scheduler skeletons, queue-backed execution jobs, worker leases, Docker shell sandbox support, a worker readiness gate, and a Remote Computer control-plane skeleton with a Pod template, restricted service account, RWX PVC placeholder, mounted state-contract ConfigMap for Memory/Notes/Skills/artifacts/locks, JuiceFS CSI profile manifest, production state-sync readiness gate, warm-pool Deployment example, KEDA ScaledObject example, an artifact discovery sidecar ConfigMap and fail-closed sidecar container, an opt-in `deploy/kustomization.yaml` bundle for those Remote Computer examples, deny-by-default NetworkPolicy, `remote_computers` / `remote_computer_leases` storage, lease heartbeat/release/fail APIs, `remote_computer_session_attachments`, stale attachment listing, Admin-only stale reclaim, manual execution-job-to-lease assignment, automatic worker assignment to an active matching Remote Computer lease during normal execution-job drains, automatic warm-pool lease claims from available Remote Computers, assignment lifecycle status updates for completed/retry-released/failed/canceled jobs, persisted `remote_computer_state_locks` for Memory/Notes/Skills/artifacts write coordination, persisted sidecar heartbeat records, readiness detection and scheduler audit runs for missing/stale artifact-discovery sidecars, an audited sidecar recovery run that fail-closed plans Pod replacement and only attempts delete/create when both Kubernetes runner gates and `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED` are enabled, a reserved `RemoteComputerRunner` boundary, a Kubernetes runner with dry-run/probe evidence and live-create payload injection for session/remote-computer/assignment sidecar env, Pod exec intent dry-run, a gated Kubernetes exec WebSocket client boundary with timeout and bounded stdout/stderr/status capture, env-gated job paths that can bind approved `file.write`, `shell.exec`, and `codex.exec` jobs to an assigned Remote Computer Pod, execution-job cancellation that can delete the assigned Pod when the Kubernetes execution transport gate is enabled, Codex final-message artifact capture from Pod exec output, push-based Remote Computer artifact sync into the Artifact Store, bounded shared-workspace artifact discovery via `POST /api/remote-computers/artifacts/discover`, execution transport readiness that reports assignment counts plus remaining sidecar discovery hardening steps, and doubly gated Pod create/delete HTTP calls, Admin-only runner readiness/dry-run/mutate routes, and session event/audit records. It does not yet mount a real distributed Memory/Notes/Skills filesystem into leased agent Pods.

Target control flow:

```text
Agent Session
  -> Harness / Tool Router / Policy Engine
  -> Execution Job Queue
  -> Remote Computer Manager
  -> Kubernetes Pod lease
  -> Mounted state filesystem
  -> Sandbox / Codex / shell / tool execution
  -> Artifact + Event + Audit sync
```

Stage 2 now includes a readiness and lease-state Remote Computer control plane: Pod template, PVC/RWX state mount placeholder plus a matching JuiceFS profile, mounted state-contract ConfigMap, JuiceFS profile manifest, production state-sync readiness gate, warm-pool example manifest, KEDA ScaledObject example, fail-closed artifact discovery sidecar manifest, readiness API, UI readiness panel, lease store, session attachment state, manual and automatic execution-job-to-lease handoff assignment, automatic warm-pool lease claiming, state lock acquire/release APIs for shared state write coordination, sidecar heartbeat API, missing/stale sidecar readiness alerts, scheduler-backed sidecar supervision audit runs, audited sidecar recovery planning with env-gated Pod replacement, worker acknowledgement, assignment lifecycle status updates, reserved Pod exec transport planning, Kubernetes Pod exec intent dry-run, gated runner live-exec WebSocket capture with timeout/output bounds, env-gated assigned-Pod `file.write`, `shell.exec`, and `codex.exec`, execution-job cancellation through assigned Pod deletion, live-create sidecar env injection, push-based Remote Computer artifact sync, bounded artifact discovery from shared workspaces, scheduler-backed stale reclaim, event names, reserved runner readiness/dry-run/mutation evidence, and honest blockers for distributed filesystem, production sidecar replacement validation, and full job-bound Pod execution transport. Stage 3 should make this the primary sandbox substrate by creating Kubernetes Pods per session, syncing artifacts/events back to MandoForge, assigning warm pools, and integrating KEDA/HPA plus a real distributed state filesystem option such as JuiceFS CSI.

See [Agent Remote Computer Plan](agent-remote-computer-plan.md).

## Verification

Current verified path:

- `cargo fmt --all -- --check`
- `cargo check -p mandoforge-api --bins`
- `cargo test -p mandoforge-api` including local mock Vault KV v2 HTTP coverage for token, namespace, path, and secret parsing.
- `node --check web/app.js`
- `./scripts/verify-static-ui-actionbook.sh`, the preferred local browser smoke when Playwright or Chrome DevTools MCP is timing out.
- `kubectl kustomize deploy/k8s >/tmp/mandoforge-kustomize.out`
- `./scripts/stage1-final-gate.sh`
- `RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh`

After the actionbook smoke, verify that the self-started API and CDP listeners were cleaned up:

```bash
lsof -nP -iTCP:8791 -sTCP:LISTEN || true
lsof -nP -iTCP:9324 -sTCP:LISTEN || true
```

Focused manual Postgres smoke can still be run with:

```bash
docker compose up -d postgres
DATABASE_URL=postgres://mandoforge:mandoforge@localhost:5432/mandoforge cargo run -p mandoforge-api
BASE_URL=http://127.0.0.1:8787 ./scripts/smoke.sh
```
