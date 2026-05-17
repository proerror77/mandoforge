# Whiskey Adoption Status

Snapshot date: 2026-05-17.

This file tracks the current production-like adoption state for `wishky-2-1`. It is a release/status ledger, not a replacement for [Whiskey Adoption Runbook](whiskey-adoption-runbook.md).

## Current Deployment

- Host: `wishky-2-1`.
- Image: `ghcr.io/proerror77/mandoforge/mandoforge-api:828087c`.
- API: `127.0.0.1:18787`.
- Postgres: `127.0.0.1:15432`.
- Compose project: `mandoforge-adoption`.
- Latest remote archive: `/opt/mandoforge-adoption/archives/mandoforge-whiskey-pilot-20260517T065120Z.tar.gz`.
- Latest local archive copy: `.mandoforge/remote-adoption/whiskey/mandoforge-whiskey-pilot-20260517T065120Z.tar.gz`.
- Latest Stage 2 strict archive copy: `.mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260517T065120Z.tar.gz`.

## Lane Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| Codex App Server deployment/ops | Passed for Whiskey pilot | `codex_app_server_health_status=healthy`, `deployment_readiness_status=ready`, `ops_validation_status=validated`, `production_blocked_count=0` | Extend native WebSocket steering beyond health/deployment/ops when product work needs thread/turn/command execution through the native protocol. |
| Worker load validation | Inventory collected; production ops blocked | `worker_readiness_status=critical`, `load_validation_run_status=attention`, `production_blocked_count=1` | Decide between a constrained single-host load controller and a k3s/cluster-backed queue-pressure validation. Do not mark production worker ops passed from manifests alone. |
| Remote Computer cluster/state | Inventory collected; production state sync blocked | `remote_computer_status=critical`, `production_state_sync_status=blocked`, `production_blocked_count=1` | Requires distributed state filesystem plus lock-aware state sync. Full pod/state/sidecar evidence requires k3s or another real cluster. |
| Tenant routing/RLS | Controller evidence collected; production routing still blocked | Strict evidence archive `stage2-production-whiskey-20260517T065120Z.tar.gz` includes `api-tenant-isolation-routing-validate.json` with controller `status=validated`; readiness reports `controller_evidence_fresh=true`, `rls_ready=true`, and the remaining blocker `runtime still serves one configured tenant instead of routing per tenant`. | Implement real tenant-routed runtime before marking this lane production-ready; strict evidence now proceeds past the tenant controller URL blocker and exposes the later optional rollout/rollback/finance/sidecar gaps. |
| MCP connector rollout | Not validated on Whiskey | Stage 2 inventory still lists MCP deployment/rollout validation endpoints as missing strict validations. | Requires a real team connector target and rollout controller. |
| Eval/release rollout | Not validated on Whiskey | Stage 2 inventory still lists release deployment/orchestration/due-run endpoints as missing strict validations. | Requires a real release controller and rollback target. |
| OTel collector | Not validated on Whiskey | Stage 2 inventory still lists collector deployment/cluster validation and remediation endpoints as missing strict validations. | Requires a real collector deployment target. |
| Provider rollout/rollback | Not validated on Whiskey | Stage 2 inventory still lists provider deployment, rollout, rollback, and policy-gate validation endpoints as missing strict validations. | Requires real provider deployment targets. |
| Approval notifications | Not validated on Whiskey | Stage 2 inventory still lists notification deployment/ops/run endpoints as missing strict validations. | Requires real webhook, Slack, or email routes. |
| Vault/KMS/HSM | Not validated on Whiskey | Stage 2 inventory still lists KMS recovery and rotation endpoints as missing strict validations. | Requires real KMS/HSM or Vault-backed lifecycle targets. |
| Finance/accounting reconciliation | Not validated on Whiskey | Stage 2 inventory still lists finance run and reconcile endpoints as missing strict validations. | Requires a real accounting/export/reconciliation target. |

## k3s Decision

Do not install k3s automatically on Whiskey. The host has 2 vCPU and 3.4 GiB RAM, with roughly 1.9 GiB available during the current pilot and existing swap use. A single-node k3s pilot is feasible only as an explicit constrained experiment with capped Remote Computer warm-pool replicas and no public ingress.

Until that decision is made, Whiskey remains a single-host production-like pilot: useful for API, Codex App Server deployment/ops, scheduler, and inventory evidence, but not enough for complete Remote Computer cluster/state adoption.
