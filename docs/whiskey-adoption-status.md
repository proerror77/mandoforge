# Whiskey Adoption Status

Snapshot date: 2026-05-17.

This file tracks the current production-like adoption state for `wishky-2-1`. It is a release/status ledger, not a replacement for [Whiskey Adoption Runbook](whiskey-adoption-runbook.md).

## Current Deployment

- Host: `wishky-2-1`.
- Image: `ghcr.io/proerror77/mandoforge/mandoforge-api:0f8361b`.
- API: `127.0.0.1:18787`.
- Postgres: `127.0.0.1:15432`.
- Compose project: `mandoforge-adoption`.
- Latest remote archive: `/opt/mandoforge-adoption/archives/mandoforge-whiskey-pilot-20260517T081736Z.tar.gz`.
- Latest local archive copy: `.mandoforge/remote-adoption/whiskey/mandoforge-whiskey-pilot-20260517T081736Z.tar.gz`.
- Latest Stage 2 strict archive copy: `.mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260517T081736Z.tar.gz`.

## Lane Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| Codex App Server deployment/ops | Passed for Whiskey pilot | `codex_app_server_health_status=healthy`, `deployment_readiness_status=ready`, `ops_validation_status=validated`, `production_blocked_count=0` | Extend native WebSocket steering beyond health/deployment/ops when product work needs thread/turn/command execution through the native protocol. |
| Worker load validation | Passed for Whiskey single-host pilot | Strict evidence archive `stage2-production-whiskey-20260517T070211Z.tar.gz` reports `load_validation.status=validated`, `production_ops.status=ready`, `controller_evidence_fresh=true`, worker container running, durable Postgres queue, queue worker mode, no failed jobs, and no stale leases. The controller records `scope=whiskey-single-host`; this is not a k3s multi-replica autoscaling proof. | Keep as single-host evidence unless k3s/cluster adoption is approved; full scale-out evidence still needs a real cluster profile. |
| Remote Computer cluster/state | Inventory collected; production state sync blocked | `remote_computer_status=critical`, `production_state_sync_status=blocked`, `production_blocked_count=1` | Requires distributed state filesystem plus lock-aware state sync. Full pod/state/sidecar evidence requires k3s or another real cluster. |
| Tenant routing/RLS | Controller evidence collected; production routing still blocked | Strict evidence archive `stage2-production-whiskey-20260517T065120Z.tar.gz` includes `api-tenant-isolation-routing-validate.json` with controller `status=validated`; readiness reports `controller_evidence_fresh=true`, `rls_ready=true`, and the remaining blocker `runtime still serves one configured tenant instead of routing per tenant`. | Implement real tenant-routed runtime before marking this lane production-ready; strict evidence now proceeds past the tenant controller URL blocker and exposes the later optional rollout/rollback/finance/sidecar gaps. |
| MCP connector rollout | Passed for Whiskey pilot connector | Strict evidence archive `stage2-production-whiskey-20260517T074456Z.tar.gz` reports `server_count=1`, `healthy_count=1`, deployment controller `status=validated`, due rollout `applied_count=1`, `failed_count=0`, `controller_execution_count=1`, and rollback `status=rolled_back` for the team-scoped `whiskey-docs` connector. | Keep this as a Whiskey pilot connector proof; external SaaS MCP targets still need their own controller credentials and rollout evidence. |
| Eval/release rollout | Passed for Whiskey pilot release target | Strict evidence archive `stage2-production-whiskey-20260517T080520Z.tar.gz` reports due-run `promoted_count=1`, `controller_execution_count=1`, `controller_failed_count=0`, orchestration validation `status=validated`, deployment validation `status=healthy`, deployment and orchestration controller execution `status=validated`, and rollback response `status=rolled_back` for the `whiskey-eval-release` target. | Keep this as a Whiskey pilot release proof; external production release targets still need their own rollout/orchestration/deployment/rollback controllers and policy. |
| OTel collector | Passed for Whiskey pilot collector target | Strict evidence archive `stage2-production-whiskey-20260517T081736Z.tar.gz` reports `api-observability-collector-readiness.json` with `status=ready`, `production_ops.status=ready`, `deployment_readiness.status=ready`, `cluster_rollout.status=ready`, and `remediation_supervision.status=ready`. The API validation artifacts report deployment `status=healthy` with controller execution `validated`, cluster rollout `status=validated` with controller execution `validated`, and remediation `status=completed` with controller execution `remediated`. | Keep this as a Whiskey pilot collector proof; external production collector targets still need their own collector endpoint, storage/retention, alerting, and controller credentials. |
| Provider rollout/rollback | Not validated on Whiskey | Stage 2 inventory still lists provider deployment, rollout, rollback, and policy-gate validation endpoints as missing strict validations. | Requires real provider deployment targets. |
| Approval notifications | Not validated on Whiskey | Stage 2 inventory still lists notification deployment/ops/run endpoints as missing strict validations. | Requires real webhook, Slack, or email routes. |
| Vault/KMS/HSM | Not validated on Whiskey | Stage 2 inventory still lists KMS recovery and rotation endpoints as missing strict validations. | Requires real KMS/HSM or Vault-backed lifecycle targets. |
| Finance/accounting reconciliation | Not validated on Whiskey | Stage 2 inventory still lists finance run and reconcile endpoints as missing strict validations. | Requires a real accounting/export/reconciliation target. |

## Stage 3 Pilot Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| WorkflowPack / AI Governance Pack | Passed for Whiskey pilot lifecycle | Full pilot archive `mandoforge-whiskey-pilot-20260517T081736Z.tar.gz` includes `workflow-packs/summary.txt` with `workflow_pack_status=released`, `pack_id=ai-governance`, `validated_file_count=42`, `install_status=installed`, `stage_status=staged`, `release_status=released`, `eval_gate_status=passed`, and `release_gate_status=passed`. The release evidence records explicit gate evidence from `workflow-pack-evidence-gate`. | Keep extending from lifecycle proof to customer-specific onboarding quality, connector data quality, and WorkflowPack rollback/archive semantics. |

## k3s Decision

Do not install k3s automatically on Whiskey. The host has 2 vCPU and 3.4 GiB RAM, with roughly 1.9 GiB available during the current pilot and existing swap use. A single-node k3s pilot is feasible only as an explicit constrained experiment with capped Remote Computer warm-pool replicas and no public ingress.

Until that decision is made, Whiskey remains a single-host production-like pilot: useful for API, Codex App Server deployment/ops, scheduler, and inventory evidence, but not enough for complete Remote Computer cluster/state adoption.
