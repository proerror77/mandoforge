# Stage 2 Completion Audit

Audit date: 2026-05-13

Objective: complete Stage 2, "Governed Runtime Pilot", for the Rust-native Generic Agent OS runtime.

This audit is intentionally strict. A reserved boundary, UI placeholder, passing unit test, or summary dashboard counts only when it maps to a concrete Stage 2 requirement. Items marked Partial or Gap must be closed before Stage 2 can be declared complete.

## Success Criteria And Evidence

| Requirement | Evidence | Status |
| --- | --- | --- |
| Multi-tenancy with org/team/project scopes | `organizations`, `teams`, `projects`, `memberships`, and `tenant_invitations` exist; scoped agent/session access is tested by `admin_can_manage_stage2_governance_scope`, `tenant_provisioning_bootstrap_creates_owner_scope_and_audit`, and `tenant_invitations_create_accept_revoke_and_audit_membership`. | Partial: runtime still uses one default tenant boundary, so production cross-tenant isolation is not complete. |
| RBAC for admin/operator/approver/viewer | `authorization.rs` defines roles and permissions; read/write/run/tool/approval/job/admin routes enforce them; tests include `read_routes_enforce_rbac_role_header`, `write_routes_enforce_rbac_role_header`, `session_run_enforces_rbac_role`, and `approval_decision_enforces_rbac_role`. | Covered |
| Provider governance and model allowlists | Provider registry, team provider access, model allowlists, provider health, lifecycle approvals, emergency status changes, credential refs, budget enforcement, governance summary, provider policy gate report, and audited provider gate run/history surfaces are implemented; tests include `provider_health_resolves_vault_api_key_ref_for_external_probe`, `provider_status_approval_requires_separate_approver_and_audits_decision`, and `provider_summary_aggregates_governance_signals`. | Partial: current UI has lifecycle/summary/gate run surfaces, but a full production provider policy gate workflow is not complete. |
| Vault and secret references | Secret ref catalog, Vault KV v2 client boundary, rotation records, provider secret resolution, MCP secret refs, eval judge secret refs, and `GET /api/vault/readiness` production-readiness gate exist. The readiness gate checks Vault provider health, reserved KMS configuration, registered refs, provider/MCP/eval judge consumers, unresolved refs, and stale rotations; the static Vault panel renders the gate. Tests include `vault_secret_provider_reads_kv_v2_secret_over_http`, `secret_refs_reject_absolute_or_parent_paths`, `eval_judge_profile_api_normalizes_secret_ref_and_audits`, and `vault_readiness_reports_secret_consumers_and_kms_gate`. | Partial: production secret value storage and real external KMS/HSM rotation execution remain out of scope, but blockers are now API/UI-visible through the readiness gate. |
| Worker queue, leases, retries, and resume | In-memory/Postgres execution queue, Rust `mandoforge-worker` binary, shell worker loop, K8s worker Deployment, worker HPA skeleton, Redis Streams backend, Core NATS backend, lease/retry behavior, stale running reclaim, queue-backed Codex App Server execution, and `GET /api/execution-jobs/worker-readiness` are implemented. The readiness gate reports queue semantics, worker mode, queued/running/retryable/failed pressure, stale leases, K8s worker manifest coverage, HPA/KEDA autoscaling manifest presence, parsed scale targets/min/max replicas, skeleton validation status, and runbook actions; the static Worker Dashboard renders it. Tests include `execution_queue_tracks_job_lifecycle`, `queue_backed_worker_defers_approved_tool_until_job_run`, Redis broker tests, and NATS broker publish/message parsing tests. `cargo check -p mandoforge-api --bin mandoforge-worker` passes. | Partial: NATS is Core NATS broker handoff without JetStream durability, and production worker hardening/validated autoscaling remains incomplete but is now API/UI-visible. |
| Agent Remote Computer substrate | K8s readiness skeleton exists for a Pod-based remote computer direction: `deploy/k8s/agent-remote-computer.yaml`, service account, RWX state PVC placeholder, deny-by-default NetworkPolicy, `GET /api/remote-computers/readiness`, static UI readiness panel, and planned `remote_computer.*` event names. | Partial: there is no `remote_computers` store, no session-to-Pod lease lifecycle, no warm pool, no real distributed Memory/Notes/Skills filesystem, and no tool execution inside leased Pods yet. |
| Approval v2: modify, delegate, expire, groups, escalation, delivery | Approval modification, expiry, delegated approver, approval groups, escalation rules, due escalation, webhook/Slack/email-relay notification delivery, `GET /api/approvals/notification-routing/summary`, bounded `POST /api/approvals/notifications/run`, and `GET /api/approvals/notifications/runs` are implemented. The routing summary reports configured approval notification channels, pending/delegated/group-routed approval counts, routable versus unroutable pending approvals, group/rule coverage, and attention items; the delivery run skips recently notified approvals, writes audit history, and the static Approval Governance panel renders routing plus run history. Tests include `approval_modify_updates_waiting_tool_args_before_approve`, `delegated_approval_requires_matching_subject_or_admin`, `approval_groups_and_escalation_rules_delegate_decisions`, `due_approval_escalation_run_advances_pending_approvals_by_rule_order`, and `approval_notification_routing_summary_reports_routable_pending_work`. | Partial: production multi-channel delivery now has webhook/Slack/email-relay boundaries and audited run history, but persisted channel policy and provider-grade notification operations are not complete. |
| MCP Gateway with registry, allowlists, policy, secret refs, health, rollout | `mcp.call`, team MCP server registry, discovery import, secret refs, health checks, due health, rollout request/apply/due/rollback, rollout run history, and summary UI exist; tests include `mcp_call_executes_through_tool_router_and_gateway_policy` and `mcp_server_config_normalizes_secret_refs_without_secret_values`. | Partial: connector rollout now has audited due-run history, but production connector rollout orchestration is still limited beyond summary, due-run, manual apply, and rollback. |
| Codex App Server adapter | Env-gated health/thread/turn/command/interrupt/poll/stale-poll/artifact-sync/trace/control-summary routes exist; approved `codex.exec` supports CLI/App Server strategies and queue-backed polling; tests include `codex_app_server_routes_require_admin_and_call_adapter`, `approved_codex_exec_can_use_app_server_strategy`, and queue-backed polling tests. | Partial: production App Server operations beyond steering, polling, traces, artifact sync, and summary are not complete. |
| Evaluation, regression gates, release approvals, rollback, drift | Eval datasets/cases/runs, deterministic Stage 2 graders, optional judge client/profile, Stage 2 regression suite, gates, release requests, approval automation, due run, release automation run history, rollback, and drift checks exist; tests include `stage2_eval_suite_bootstrap_creates_passing_regression_cases_and_audits` and eval judge/gate tests. | Partial: release automation now has audited run history, but production multi-step release rollout orchestration is not complete. |
| Observability with OTel traces/metrics/logs | OTLP-shaped logs/traces/metrics export from event append boundary, observability summary, collector readiness gate, remediation plan/run, scheduler due-plan, and scheduler orchestration summary exist; tests include `appended_session_events_export_telemetry_when_enabled`, `http_telemetry_exporter_posts_logs_traces_and_metrics_to_otlp_boundary`, and `observability_summary_reports_dashboard_backpressure`. | Partial: collector readiness is now API/UI-visible, but broader remediation automation and production collector hardening are not complete. |
| Cost tracking, budgets, alerts, trends, finance exports | Usage summary/trends, provider cost budgets, alert routes/delivery/ack, finance summary, CSV export, scheduled export delivery, alert-delivery audit metadata, Finance Operations readiness/runbook UI panels, and a controlled finance close run endpoint exist; tests include `builds_usage_trend_from_rollups_and_budget_pressure`, `builds_usage_finance_dashboard_attention_items`, `builds_usage_finance_operations_summary_from_audit_history`, and `usage_finance_summary_requires_admin_and_reports_dashboard`. | Partial: production finance operations now include readiness, audit-evidence summaries, and a bounded close run, but full production finance close workflows are not complete. |
| UI v2 admin console | Static UI covers tenant governance, provider settings, Vault, approval governance, policy, eval/release, MCP, worker, usage, Codex App Server, observability, remediation, and scheduler orchestration panels; `./scripts/verify-static-ui-actionbook.sh` validates a static UI smoke path without relying on browser MCP. | Partial: full production CRUD flows, dashboard polish, and richer rollout operations remain incomplete. |
| Kubernetes scheduler/deployment groundwork | `deploy/k8s` kustomize renders successfully, including scheduler CronJob for aggregate due-run supervision. Latest check: `kubectl kustomize deploy/k8s >/tmp/mandoforge-kustomize.out && wc -l /tmp/mandoforge-kustomize.out` returned 249 lines. | Covered for Stage 2 skeleton; not a complete enterprise deployment. |

## Latest Verification Commands

Latest passing checks:

```bash
cargo fmt --all -- --check
node --check web/app.js
git diff --check
cargo test -p mandoforge-api
./scripts/verify-static-ui-actionbook.sh
kubectl kustomize deploy/k8s >/tmp/mandoforge-kustomize.out && wc -l /tmp/mandoforge-kustomize.out
lsof -nP -iTCP:8791 -sTCP:LISTEN || true
lsof -nP -iTCP:9324 -sTCP:LISTEN || true
```

Latest evidence:

```text
cargo test -p mandoforge-api: 121 passed
static UI actionbook smoke ok
kustomize rendered 249 lines
ports 8791 and 9324 had no residual listeners after Actionbook verification
```

## Open Completion Gaps

These gaps prevent Stage 2 from being marked complete:

1. Full production cross-tenant runtime isolation beyond the default tenant boundary.
2. Production multi-step rollout orchestration beyond preview/history/due-run/rollback surfaces.
3. Production provider policy gate workflow beyond current lifecycle approval, summary, audited gate run/history, and report surfaces.
4. Production secret value storage and real external KMS/HSM rotation execution beyond the new Vault readiness gate.
5. Production JetStream-style queue durability and worker hardening/validated autoscaling beyond the existing Rust worker binary, K8s worker skeleton, and HPA skeleton.
6. Production approval notification operations beyond current webhook/Slack/email-relay delivery, audited delivery runs, and routing readiness summary.
7. Production MCP connector rollout orchestration beyond current summary, audited due-run history, manual apply, and rollback.
8. Production Codex App Server control-plane operations beyond current steering, polling, traces, artifact sync, and summary.
9. Production multi-step eval/release rollout orchestration beyond current approval, due-run, summary, and rollback.
10. Broader remediation automation and production collector-specific observability hardening beyond the current readiness gate.
11. Production finance operations beyond current readiness, bounded close run, alerts, forecast, audit-evidence, and export surfaces.
12. Full production UI CRUD flows and dashboard polish.

## Completion Decision

Stage 2 is not complete as of this audit. The repo has substantial governed-runtime capability, but the remaining Partial and Gap items above must be either implemented or explicitly moved out of the Stage 2 success criteria before the goal can be marked complete.
