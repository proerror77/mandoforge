# Whiskey Adoption Status

Snapshot date: 2026-05-17.

This file tracks the current production-like adoption state for `wishky-2-1`. It is a release/status ledger, not a replacement for [Whiskey Adoption Runbook](whiskey-adoption-runbook.md).

## Current Deployment

- Host: `wishky-2-1`.
- Image: `ghcr.io/proerror77/mandoforge/mandoforge-api:5acc49a`.
- API: `127.0.0.1:18787`.
- Postgres: `127.0.0.1:15432`.
- Compose project: `mandoforge-adoption`.
- Tenant routing mode: `tenant_routed`.
- Latest remote archive: `/opt/mandoforge-adoption/archives/mandoforge-whiskey-pilot-20260517T225823Z.tar.gz`.
- Latest local archive copy: `.mandoforge/remote-adoption/whiskey/mandoforge-whiskey-pilot-20260517T225823Z.tar.gz`.
- Latest Stage 2 strict archive copy: `.mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260517T225823Z.tar.gz`.
- Latest strict archive summary: `stage2_status=ready`, `completion_blocked=false`, `open_gap_count=0`, `validation_missing_endpoint_count=0`, `validation_stale_endpoint_count=0`.

## Lane Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| Codex App Server deployment/ops | Passed for Whiskey pilot | Strict evidence archive `stage2-production-whiskey-20260517T084848Z.tar.gz` reports `codex_app_server_health_status=healthy`, `deployment_readiness_status=ready`, `ops_validation_status=validated`, `production_blocked_count=0`, and stale-poll evidence under `api-codex-app-server-runs-poll-stale.json` with `failed_count=0`. | Extend native WebSocket steering beyond health/deployment/ops when product work needs thread/turn/command execution through the native protocol. |
| Worker load validation | Passed for Whiskey single-host pilot | Strict evidence archive `stage2-production-whiskey-20260517T070211Z.tar.gz` reports `load_validation.status=validated`, `production_ops.status=ready`, `controller_evidence_fresh=true`, worker container running, durable Postgres queue, queue worker mode, no failed jobs, and no stale leases. The controller records `scope=whiskey-single-host`; this is not a k3s multi-replica autoscaling proof. | Keep as single-host evidence unless k3s/cluster adoption is approved; full scale-out evidence still needs a real cluster profile. |
| Remote Computer cluster/state | Inventory and sidecar recovery audit collected; production state sync blocked | Latest strict archive `stage2-production-whiskey-20260517T225823Z.tar.gz` reports `remote_computer_status=critical`, `production_state_sync_status=blocked`, `production_blocked_count=1`, `sidecar_recovery_evidence_status=captured`, `sidecar_recovery_run_status=noop`, and strict validation coverage with `validation_missing_endpoint_count=0`. The same archive now includes `remote-computer-k3s/remote-computer-k3s-host-inventory-latest.txt`, which records `status=preinstall_inventory`, `preflight_status=constrained_pilot_only`, `verify_status=not_installed`, `mem_available_mib=1650`, `swap_used_mib=1413`, `br_netfilter_loaded=false`, and `bridge_nf_call_iptables=missing`. This is still not pod replacement proof. | Requires distributed state filesystem plus lock-aware state sync. Full pod/state/sidecar evidence requires k3s or another real cluster. |
| Tenant routing/RLS | Passed for Whiskey tenant-routed pilot | Full pilot archive `mandoforge-whiskey-pilot-20260517T111701Z.tar.gz` reports `tenant_isolation_status=ready`, `readiness_score=100`, `runtime_tenant_mode=tenant_routed`, `production_routing_status=ready`, `rls_enabled=true`, `rls_forced=true`, `tenant_context_configured=true`, `production_blocked_count=0`, and fresh controller evidence. The deployed API is configured with `MANDOFORGE_TENANT_ROUTING_MODE=tenant_routed` and Postgres RLS context refresh on connection acquire. | Keep cross-tenant negative tests and external enterprise multi-tenant targets as follow-up; this is Whiskey pilot readiness, not a multi-cluster enterprise tenant rollout. |
| MCP connector rollout | Passed for Whiskey authenticated GitHub repo-contents connector target | Strict evidence archive `stage2-production-whiskey-20260517T225823Z.tar.gz` still reports the `whiskey-docs` connector deployment, due rollout, and rollback path as healthy, with `validation_missing_endpoint_count=0` for the full Stage 2 coverage. The paired full pilot archive `mandoforge-whiskey-pilot-20260517T225823Z.tar.gz` shows the same bound `whiskey-docs` connector serving `github-repo-contents-authenticated` results from the private `proerror77/Goodchance@main` repository. | Keep this as a real internal repository knowledge target; the next step is a broader Lark docs/wiki search scope or another enterprise knowledge system, not a return to private-chat evidence. |
| Eval/release rollout | Passed for Whiskey pilot release target | Strict evidence archive `stage2-production-whiskey-20260517T080520Z.tar.gz` reports due-run `promoted_count=1`, `controller_execution_count=1`, `controller_failed_count=0`, orchestration validation `status=validated`, deployment validation `status=healthy`, deployment and orchestration controller execution `status=validated`, and rollback response `status=rolled_back` for the `whiskey-eval-release` target. | Keep this as a Whiskey pilot release proof; external production release targets still need their own rollout/orchestration/deployment/rollback controllers and policy. |
| OTel collector | Passed for Whiskey real single-node collector target | Strict evidence archive `stage2-production-whiskey-20260517T225823Z.tar.gz` reports `api-observability-collector-readiness.json` with `status=ready`, `production_ops.status=ready`, `deployment_readiness.status=ready`, `cluster_rollout.status=ready`, and `remediation_supervision.status=ready`. The same archive now includes `observability-collector/otel-collector-evidence.json`, which records a running `otel/opentelemetry-collector-contrib:0.123.0` service with `otlp_endpoint=http://otel-collector:4318` and `health_endpoint=http://otel-collector:13133/healthz`, plus `observability-collector/otel-collector-live-signals.log`, which shows the collector receiving logs, traces, and metrics batches from the Whiskey API. The API validation artifacts still report deployment `status=healthy`, cluster rollout `status=validated`, and remediation `status=completed`. | Move from a real single-node Whiskey collector to a retained, alerted, multi-node collector target if observability adoption needs external storage, alert routing, or cross-node telemetry. |
| Provider rollout/rollback | Passed for Whiskey real DeepSeek provider target | Full pilot archive `mandoforge-whiskey-pilot-20260517T192125Z.tar.gz` reports `provider-governance/summary.txt` with `provider_count=1`, `active_provider_count=1`, `provider_policy_gate_status=passed`, `provider_policy_gate_enforcement_status=ready`, `provider_name=whiskey-deepseek-provider`, `provider_type=openai_compatible`, `provider_health_status=active`, `provider_health_external_probe=healthy`, `provider_health_api_key_ref_resolved=true`, `deployment_readiness_status=ready`, `provider_rollout_run_status=applied`, `provider_rollback_run_status=rolled_back`, and `production_blocked_count=0`. The paired `provider-governance/whiskey-provider-rollout-seed.json` records `method=PATCH`, `base_url=https://api.deepseek.com`, `default_model=deepseek-v4-flash`, and `config.api_key_ref=vault:providers/whiskey-deepseek#api_key`, with the referenced secret reused from Whiskey's Vault boundary. | Move from a single real model endpoint to a broader production provider fleet, credential rotation policy, and traffic-switch target if provider rollout needs to cover more than one approved model surface. |
| Approval notifications | Passed for Whiskey real Lark IM delivery target | Full pilot archive `mandoforge-whiskey-pilot-20260517T225823Z.tar.gz` reports `approval-notifications/summary.txt` with `production_ops_status=ready`, `deployment_readiness_status=ready`, `configured_channel_count=1`, `active_policy_count=1`, `unroutable_pending_approval_count=0`, `delivery_evidence_status=captured`, `delivery_status=delivered`, `delivery_target_count=1`, `delivery_delivered=true`, `delivery_observer_status=ok`, `delivery_mode=lark_im`, `delivery_forwarding_status=delivered`, `delivery_forwarding_channel=lark_im`, `delivery_forwarded_message_id=om_x100b6f90320048a8b4b1776a4606bb1`, `delivery_forwarded_chat_id=oc_d950f4d66f237b6c43de439bfd77cf27`, and `production_blocked_count=0`. The Whiskey approval webhook now forwards to a real Feishu/Lark private chat via `lark-cli`, and the observer health endpoint records the latest forwarded message identifiers. | Move from self-directed Lark IM proof to a dedicated approver chat or group target if the production notification path should fan out beyond a single operator inbox. |
| Vault/KMS/HSM | Passed for Whiskey pilot KMS lifecycle target | Strict evidence archive `stage2-production-whiskey-20260517T092054Z.tar.gz` reports `api-vault-readiness.json` with `status=passed`, `secret_provider.status=healthy`, `kms.status=ready`, `production_rotation.status=ready`, and `production_recovery.status=ready`. Rotation evidence reports `status=validated`, `rotated_count=1`, `catalog_updated_count=1`, and external execution `status=validated`; recovery evidence reports `status=validated`, `latest_rotation_validated=true`, controller execution `status=validated`, and no issues. | Keep this as a Whiskey pilot lifecycle proof; real KMS/HSM or enterprise Vault adoption still needs external target credentials, envelope-encryption policy, approval-backed secret value rotation, and recovery/rollback procedures. |
| Finance/accounting reconciliation | Passed for Whiskey real Lark Drive export target | Full pilot archive `mandoforge-whiskey-pilot-20260517T225823Z.tar.gz` reports `finance/summary.txt` with `production_close_status=ready`, `production_blocked=false`, `finance_close_run_status=completed`, `finance_reconciliation_run_status=reconciled`, `finance_export_delivery_status=delivered`, `finance_export_delivery_target_configured=true`, `finance_export_delivery_observer_status=ok`, `finance_export_delivery_mode=lark_drive`, `finance_export_delivery_file_token=QA48blOtpojU36xrQbYcO8w1nwf`, `finance_export_delivery_file_url=https://www.feishu.cn/file/QA48blOtpojU36xrQbYcO8w1nwf`, and `finance_export_delivery_file_name=mandoforge-usage-export.csv`. The Whiskey finance export webhook now uploads the generated CSV to a real Feishu Drive file before close and reconciliation gates run. | Move from a real export artifact target to a true downstream accounting ledger or ERP if finance close needs reconciliation against an external system of record. |

## Stage 3 Pilot Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| WorkflowPack / AI Governance Pack | Passed for Whiskey pilot lifecycle with install defaults, immutable version-update, persisted profile assets, onboarding assessment, connector quality, real MCP binding, and authenticated GitHub repo-contents proof | Full pilot archive `mandoforge-whiskey-pilot-20260517T225823Z.tar.gz` includes `workflow-packs/summary.txt` with `workflow_pack_status=version_created_after_rollback_and_archive`, `pack_id=ai-governance`, `validated_file_count=42`, `install_status=installed`, `stage_status=staged`, `release_status=released`, `rollback_status=rolled_back`, `update_status=installed`, `update_version=0.1.1`, `update_manifest_path=packs/ai-governance/package-v0.1.1.yaml`, `blocked_onboarding_status=blocked`, `onboarding_status=ready`, `onboarding_workflow=profile-onboarding`, `onboarding_eval=profile-onboarding-regression`, `required_profile_count=6`, `profile_schema_count=6`, `inline_profile_count=0`, `persisted_profile_count=6`, `provided_profile_count=6`, `placeholder_profile_count=0`, `connector_requirement_count=1`, `ready_connector_count=1`, `onboarding_blocker_count=0`, `blocked_connector_quality_status=blocked`, `connector_quality_status=ready`, `connector_quality_requirement_count=1`, `connector_quality_ready_connector_count=1`, `connector_quality_sample_count=1`, `connector_quality_passing_sample_count=1`, `connector_quality_blocker_count=0`, `connector_quality_bound_team_id=9ebe8ad5-960f-4d1b-82fb-afaf0468381c`, `connector_quality_bound_server_id=c85beded-8cf9-4197-93a7-254b4f899dea`, `connector_quality_bound_server_name=whiskey-docs`, `connector_quality_bound_server_health_status=healthy`, `connector_quality_live_source=github-repo-contents-authenticated`, `connector_quality_live_auth_mode=authenticated`, `connector_quality_live_title=clients/ios-app/README.md`, `connector_quality_live_url=https://github.com/proerror77/Goodchance/blob/main/clients/ios-app/README.md`, `installed_default_profile_asset_count=6`, `updated_default_profile_asset_count=6`, `persisted_profile_asset_count=6`, `persisted_profile_list_count=6`, `persisted_profile_saved_min_version=2`, `persisted_profile_saved_max_version=2`, `archive_status=archived`, `eval_gate_status=passed`, `release_gate_status=passed`, `rolled_back_get_status=rolled_back`, `rolled_back_list_count=1`, `archived_get_status=404`, `active_after_archive_count=0`, and `updated_active_after_archive_count=1`. The gate now proves install/update defaults, persisted customer assets, fail-closed onboarding and connector-quality checks, binds to the real Whiskey `whiskey-docs` MCP server, and reaches `ready` from authenticated private repository content retrieval. | Move next to a broader Lark docs/wiki search scope or another enterprise knowledge system while keeping Remote Computer blockers explicit. |

## k3s Decision

Do not install k3s automatically on Whiskey. Before any cluster experiment, run:

```bash
scripts/whiskey-remote-computer-k3s-preflight.sh
```

If that preflight is accepted, the next repo-native step is:

```bash
scripts/whiskey-remote-computer-k3s-prepare.sh
```

That script defaults to `dry_run` and only reports the host changes needed for a constrained pilot. It does not mutate Whiskey unless `--apply` is passed explicitly.

After the host prerequisites are in place, the next repo-native step is:

```bash
scripts/whiskey-remote-computer-k3s-install.sh
```

That script also defaults to `dry_run`. It prints the exact k3s installer command and systemd actions it would run on Whiskey, but does not install anything unless `--apply` is passed explicitly.

After installation, the verification step is:

```bash
scripts/whiskey-remote-computer-k3s-verify.sh
scripts/whiskey-remote-computer-k3s-host-inventory.sh
```

On the current Whiskey host, the verification reports `status=not_installed`, which is the expected pre-install baseline, and the consolidated host inventory reports `status=preinstall_inventory`. `scripts/whiskey-adoption-evidence.sh` syncs that inventory into the latest full archive under `remote-computer/remote-computer-k3s-host-inventory-latest.txt` and into the latest strict archive under `remote-computer-k3s/remote-computer-k3s-host-inventory-latest.txt`.

The latest consolidated host inventory on 2026-05-17T22:56:43Z returned `preflight_status=constrained_pilot_only` and `verify_status=not_installed` with:

- `cpu_count=2`
- `mem_available_mib=1650`
- `swap_used_mib=1413`
- `root_avail_gib=83.0`
- `cgroup_fs=cgroup2fs`
- `reserved_ports=none`
- `br_netfilter_loaded=false`
- `bridge_nf_call_iptables=missing`

That means a single-node k3s pilot is feasible only as an explicit constrained experiment with capped Remote Computer warm-pool replicas, no public ingress, and preflight remediation for `br_netfilter` plus bridge iptables before installation.

Until that decision is made, Whiskey remains a single-host production-like pilot: useful for API, Codex App Server deployment/ops, scheduler, and inventory evidence, but not enough for complete Remote Computer cluster/state adoption.
