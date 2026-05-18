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
- [x] Add an execution queue backend trait so future broker-backed queues can plug in behind the same facade.
- [x] Add fail-closed execution queue backend selection for future broker-backed queues.
- [x] Add reserved broker execution queue backend skeleton and contract test.
- [x] Add broker queue configuration and health-check boundaries before Redis/NATS implementation.
- [x] Add `ExecutionWorker` trait and `InlineExecutionWorker` implementation for swappable worker execution.
- [x] Add agent version read APIs.
- [x] Add queue-backed execution worker mode and job-drain API for external worker handoff.
- [x] Add external execution worker loop script and verification.
- [x] Persist execution jobs in Postgres when a durable store is configured.
- [x] Add execution job worker claims and lease-based stale running job reclaim.
- [x] Add execution job retry-or-fail attempts with last-error tracking.
- [x] Add Rust `mandoforge-worker` binary for API-drained execution jobs.
- [x] Add Docker Compose and Kubernetes worker deployment entries.
- [x] Add static Worker Dashboard for execution job status and API drain.
- [x] Add Redis Streams execution queue enqueue backend with mock Redis coverage.
- [x] Split Postgres row mappers out of the store module.
- [x] Split store backend type definitions out of the store method module.
- [x] Split agent, agent-version, and session store methods out of the remaining store module.
- [x] Split session event store methods out of the remaining store module.
- [x] Split tool-call store methods out of the remaining store module.
- [x] Split artifact store methods out of the remaining store module.
- [x] Split approval store methods out of the remaining store module.
- [x] Split audit-log store methods out of the remaining store module.
- [x] Split demo seed store method and remove the catch-all store module.
- [x] Resume the provider harness after approved tool execution for provider-run sessions.
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
- [x] Add env-gated Codex App Server thread/turn/interrupt/command adapter boundary.
- [x] Load and enforce `config/policy.stage1.yaml`.
- [x] Add env-gated OpenAI-compatible HTTP provider transport.
- [x] Add credential-gated external provider smoke verification script.
- [x] Add seeded platform_events generator for generic runtime demo.
- [x] Add deployment guide and demo script.
- [x] Add Stage 1 final gate script with self-started in-memory demo and optional live stack mode.
- [x] Add Stage 1 completion audit with evidence gaps.

## Stage 2 / 3 Boundary Work

- [x] Close Stage 2 as a repo-controlled Governed Runtime Pilot with completion audit, controller-drill evidence, CI, and an explicit external production adoption backlog.
- [x] Add Agent Team operating model for parallel lane ownership, integration gates, and merge discipline.
- [x] Add Stage 2 production adoption and Stage 3 product roadmap.
- [x] Add Stage 2 production adoption runbook and controller matrix.
- [x] Add reserved MCP Gateway config and client boundary before enabling `mcp.call`.
- [x] Add reserved OTel observability config and exporter boundary before enabling telemetry export.
- [x] Add RBAC principal, permission, and authorizer boundary before request-path enforcement.
- [x] Add reserved Vault secret provider boundary before runtime secret reads.
- [x] Enforce RBAC on manual tool execution with a default demo operator principal.
- [x] Enforce RBAC on approval decisions with a default demo operator principal.
- [x] Enforce RBAC on execution job drain with a default demo operator principal.
- [x] Route provider API key vault references through the reserved secret provider boundary.
- [x] Enforce RBAC on session run with a default demo operator principal.
- [x] Add Vault KV v2 secret provider client skeleton without enabling runtime secret reads by default.
- [x] Add explicit reserved/vault secret provider selection while keeping reserved as the default.
- [x] Add local mock Vault KV v2 HTTP verification for token, namespace, path, and secret parsing.
- [x] Add Admin-only Vault health check API and static Vault health action.
- [x] Add audited scoped secret reference catalog and rotation API/UI.
- [x] Enforce RBAC on read/list API paths with default demo operator compatibility.
- [x] Enforce RBAC on core write API paths with default demo operator compatibility.
- [x] Enforce RBAC on the tool catalog API while keeping health/static public.
- [x] Add Admin-only policy inspection and tool-decision simulation APIs.
- [x] Add audited batch policy test API.
- [x] Add local-verified HTTP MCP Gateway client boundary before enabling `mcp.call`.
- [x] Add local-verified HTTP OTel exporter boundary before wiring runtime telemetry export.
- [x] Add local-verified Redis Stream command boundary before enabling a live broker queue backend.
- [x] Add local-verified Redis RESP client boundary before enabling Redis queue backend selection.
- [x] Add local-verified Redis worker read command boundary before enabling broker worker handoff.
- [x] Enable Redis Stream readgroup/ack drain through the API-backed execution worker path.
- [x] Add Stage 2 gap audit mapping PRD requirements to current code evidence and remaining gaps.
- [x] Add Stage 2 approver role and pending approval modify endpoint that updates waiting tool args before approval.
- [x] Add approval expiry persistence, expire API, timeline events, and decision fail-closed behavior.
- [x] Add static approval argument review and modify UI.
- [x] Add static approval JSON-path diff visualization for modified args.
- [x] Add delegated approval subject enforcement and UI visibility.
- [x] Add env-gated approval webhook delivery route and static delivery action.
- [x] Add approval groups, escalation rules, group decision enforcement, and static governance controls.
- [x] Add persistent org/team/project/membership schema and Admin-only hierarchy APIs.
- [x] Make Postgres migration runner execute all SQL files under `db/migrations`.
- [x] Derive principal roles from persisted memberships when role headers are absent.
- [x] Enforce team membership for scoped agent/session/tool/approval/job resource paths.
- [x] Filter agent/session list APIs by principal membership scope.
- [x] Add project-level permissions instead of only team-level inheritance.
- [x] Add static org/team/project/membership management UI.
- [x] Add provider access/model allowlist enforcement for team-scoped agent creation.
- [x] Add provider budgets and runtime provider selection from stored provider rows.
- [x] Add provider daily cost budget enforcement over a 24-hour window.
- [x] Add provider active/disabled status management and runtime enforcement.
- [x] Add provider configuration health checks and static UI action.
- [x] Add eval dataset/case/run schema and a first version-bound eval runner skeleton.
- [x] Add real eval graders for policy, tool selection, SQL safety, sandbox recovery, and final answer quality.
- [x] Add eval regression gate API and static run gate action.
- [x] Add eval drift detection API and static run drift action.
- [x] Add eval-gated agent release promotion API.
- [x] Add agent release rollback API.
- [x] Add env-gated eval judge client boundary with fail-closed unconfigured behavior.
- [x] Add static release promotion and rollback controls.
- [x] Add static Eval Dashboard controls for datasets, cases, and runs.
- [x] Wire OTel export into append-only session event paths.
- [x] Add rich OTel spans/metrics for session/provider/tool/approval/worker paths.
- [x] Enable `mcp.call` through Tool Router, MCP Gateway allowlist, events, tool calls, and audit.
- [x] Add MCP server registry and per-team tool allowlist enforcement before `mcp.call`.
- [x] Add MCP tool discovery import from the configured MCP Gateway into team server allowlists.
- [x] Add static MCP UI management for team server allowlists and discovery import.
- [x] Add MCP connector lifecycle APIs/UI for config patching and activate/disable/archive status.
- [x] Add audited MCP connector health check API and static UI action.
- [x] Add audited team-level MCP connector health run API and static UI action.
- [x] Add due-only scheduled MCP connector health runs with persisted last health metadata.
- [x] Add usage/cost aggregation API for provider requests, tool runtime, approvals, and configured provider pricing.
- [x] Add token-level provider usage and cost accounting for OpenAI-compatible responses.
- [x] Add persisted usage/cost rollup snapshots and static rollup UI.
- [x] Add static cost dashboard breakdowns for provider cost and tool runtime.
- [x] Add provider budget forecast and alert rows to usage summary.
- [x] Add provider cost alert listing and webhook delivery route.
- [x] Add audited cost alert acknowledgement route and static UI action.
- [x] Add audited cost alert routing rules with webhook delivery and reserved Slack/email channels.
- [x] Add live Slack incoming-webhook delivery for cost alert routes.
- [x] Add env-gated email relay delivery for cost alert routes.
- [x] Add Actionbook CLI static UI smoke verification as the browser fallback.
- [x] Add static Admin Console panels for usage, providers, eval runs, and governance status.
- [x] Add static Policy Console for policy inspection and tool-decision simulation.
- [x] Add static Policy Console batch test controls.
- [x] Add audited policy revision create/list/activate APIs and static rollout metadata UI.
- [x] Add policy revision diff and safety gate before activation.
- [x] Add table-based policy diff and gate summaries to the static Policy Console.
- [x] Add configurable policy revision gate suites and rollout percentage metadata.
- [x] Add optional policy activation windows enforced before rollout activation.
- [x] Hot-swap the active runtime policy when a gated revision is activated.
- [x] Enforce partial policy rollout percentages by deterministic session bucket.
- [x] Add runtime rollout status and audited staged rollout cancellation controls.
- [x] Add static Provider Settings form for creating/updating stored providers with budget and pricing config.
- [x] Add static Provider Settings fields for base URL and API key env/ref config.
- [x] Add audited external `/v1/models` provider health probes for env-key OpenAI-compatible providers.
- [x] Add Vault secret-ref-backed `/v1/models` provider health probes without exposing secret values.
- [x] Add audited provider API key reference rotation that removes env-key fallback.
- [x] Add static Codex App Server steering panel for health, thread, turn, command, interrupt, and fail-closed responses.
- [x] Add Codex App Server artifact synchronization into artifacts, timeline, and audit logs.
- [x] Add CLI/App Server fallback orchestration for approved Codex worker execution.
- [x] Persist Codex App Server steering responses for replay/debugging.
- [x] Add bounded Codex App Server turn polling and retry with persisted run status/audit.
- [x] Add production Codex App Server worker-backed steering orchestration.
- [x] Add Codex App Server retry orchestration across worker leases.
- [x] Add long-running Codex App Server steering summary metrics.
- [x] Stage 3: add external scheduler integration with idempotent due-plan/run ownership, retry policy, and audit evidence.
- [x] Stage 3: add richer per-turn Codex App Server trace dashboards for command, poll, interrupt, worker lease, retry, fallback, and artifact sync paths.
- [x] Stage 3: productize assigned Remote Computer Pod execution without bypassing Tool Router, Policy Engine, Approval Engine, event log, or audit paths.
- [x] Stage 3: add typed agent handoff events with allowlisted target agents, enum intents, JSON schema validation, risk level, approval requirement, and audit trace.
- [x] Stage 3: add first-class WorkflowPack / DomainPack install, validation, staging, eval, and release gates.
- [x] Stage 3: add Stage 3 lane-local verification scripts and integration-owner gates for scheduler, Codex traces, Remote Computer execution, handoffs, and workflow packs.

## External Production Adoption Backlog

Current Whiskey production-like pilot blocker:

- [ ] Run isolated worker-pool validation and Remote Computer state-sync / sidecar replacement evidence against a real cluster and distributed state filesystem.

Post-pilot enterprise promotions that should stay visible but are not the current Whiskey pilot blocker:

- [x] Run Codex App Server deployment and ops evidence against a real App Server target on Whiskey.
- [x] Run worker load validation evidence against the Whiskey single-host queue worker target.
- [x] Run MCP connector deployment, rollout, and rollback evidence against a real `whiskey-docs` team connector target.
- [x] Run eval/release rollout, orchestration, deployment, and rollback evidence against the Whiskey production-like release target.
- [x] Run OTel collector deployment, cluster rollout, and remediation evidence against the Whiskey single-node collector target.
- [x] Run provider gate, rollout, and rollback evidence against the real DeepSeek provider deployment target on Whiskey.
- [x] Run approval notification delivery evidence against a real Feishu/Lark IM target.
- [x] Run finance close, export delivery, and reconciliation evidence against a real Feishu Drive export target.
- [ ] Run tenant routing evidence against a broader real multi-tenant deployment with RLS enabled, forced, and tenant context configured beyond the current Whiskey tenant-routed pilot.
- [ ] Run policy rollout orchestration against a real production policy controller target instead of the current Whiskey pilot controller.
- [ ] Run Vault/KMS/HSM rotation and recovery evidence against a real secret backend instead of the current Whiskey pilot KMS/Vault boundary.
- [x] Promote the `whiskey-docs` connector from authenticated private GitHub repo contents to a broader Lark docs/wiki or other enterprise knowledge target.
- [ ] Promote finance export from Feishu Drive artifact delivery to a true accounting-system or ERP target.

## Workflow Pack / Domain Pack Adaptation

- [x] Record the Workflow Pack adaptation plan from vertical workflow plugin references.
- [x] Define the `WorkflowPack` / `DomainPack` manifest contract and package validation rules.
- [x] Add profile onboarding / cold-start workflow contract for company, department, approval matrix, connector map, risk policy, and output style.
- [x] Add typed `agent_handoff_events` with allowlisted target agents, enum intents, JSON schema validation, risk level, approval requirement, and audit trace.
- [x] Add Reader / Analyzer / Writer worker role declarations and tool-scope enforcement for untrusted input workflows.
- [x] Add connector manifest policy fields for provenance, tenant scope, write gating, and prompt-injection boundary.
- [x] Build the first AI Governance Pack slice: AI use-case triage, AI impact assessment, vendor AI review, and policy monitor.
- [x] Add pack-level eval fixtures and release gates so workflow pack behavior cannot regress silently.

## Roadmap V2 / Original Agent OS Alignment

- [x] Record the Roadmap v2 framing: runtime-first implementation of the original Enterprise Agent OS plan.
- [x] Clarify that Workflow Packs / Domain Packs run on top of Agent OS and are not the OS itself.
- [x] Reframe Stage 4 as Managed Agent Control Plane + Manager Agent + Minimal Semantic Kernel.
- [x] Reframe Stage 5 as Full Semantic Layer / Context OS.

## Stage 4 / Managed Agent Control Plane + Manager Agent

- [x] Stage 4.1: Add first-class Agent Runtime Profile storage and APIs so `agent_cli.exec` can resolve governed runtime profiles instead of environment-only profile configuration.
- [x] Stage 4.1: Add runtime profile lifecycle audit events and fail-closed profile allowlist semantics for managed `agent_cli` profiles.
- [ ] Stage 4.1: Extend runtime profile release gates and fail-closed allowlist semantics to Codex App Server, Claude Code, Gemini, OpenCode, Aider, and future hosted runtimes.
- [x] Stage 4.2: Add Managed Agent Registry fields for manager/specialist kind, runtime profile binding, tool policy, MCP servers, skills, Workflow Pack memberships, Remote Computer profile, semantic scopes, and release state.
- [ ] Stage 4.2: Add Agent Builder / Console surfaces for runtime profile, tools, skills, MCP, Remote Computer profile, and semantic scope selection.
- [x] Stage 4.3: Add Minimal Semantic Kernel scope fields for project, repo, service, workflow, policy, and memory scope.
- [x] Stage 4.3: Add a minimal context packet builder that can assemble task, agent, scopes, policy reminders, relevant repo doc references, and known freshness warnings without requiring the full Stage 5 semantic layer.
- [x] Stage 4.4: Add Manager Agent planner records for task intake, decomposition, specialist selection, risk classification, and result review.
- [x] Stage 4.5: Extend typed agent handoff / assignment records with semantic scopes, runtime profile, Remote Computer requirement, review status, and human escalation status.
- [x] Stage 4.5: Add Manager Agent -> Specialist Agent handoff execution path that preserves policy, approval, audit, timeline, and Remote Computer assignment.
- [x] Stage 4.6: Add `backend-coder` as the first Managed Agent demo proving runtime profile, minimal semantic scopes, Manager Agent assignment, Remote Computer execution, artifacts, audit, and replay.
- [x] Stage 4.6: Document that `backend-coder` is a validation demo, not the MandoForge product endpoint.

## Stage 5 / Semantic Layer + Context OS

- [x] Stage 5.1: Add `semantic_sources` for repo docs, session history, artifacts, Workflow Packs, MCP sources, Feishu/Lark, GitHub, and uploads.
- [x] Stage 5.1: Add `semantic_objects` for decisions, runbooks, code modules, workflows, policies, memories, and artifacts with provenance, trust, freshness, and source URI metadata.
- [x] Stage 5.1: Add `semantic_links` so agents, projects, repos, services, workflows, policies, packs, and memories can be related explicitly.
- [x] Stage 5.2: Add versioned `context_packets` generated from task intent, Managed Agent config, semantic scopes, retrieved objects, policy reminders, and freshness warnings.
- [x] Stage 5.2: Add context packet replay in the Session Timeline so it is clear what context an agent saw before acting.
- [x] Stage 5.3: Add memory writeback candidates from completed sessions, artifacts, handoff reviews, and human approvals.
- [x] Stage 5.3: Add human approval / rejection flow before writeback candidates become durable organizational memory.
- [ ] Stage 5.4: Add freshness and trust gates for high-risk tasks so stale or untrusted context cannot silently drive execution.
- [ ] Stage 5.5: Add optional retrieval backends after the object/link/context packet model is stable; do not make vector search the first semantic layer dependency.
