# Whiskey Adoption Status

Snapshot date: 2026-05-25 after semantic governance console deployment and workflow-step worker smoke.

This file tracks the current production-like adoption state for `wishky-2-1`. It is a release/status ledger, not a replacement for [Whiskey Adoption Runbook](whiskey-adoption-runbook.md).

## Current Deployment

- Host: `wishky-2-1`.
- Image: `ghcr.io/proerror77/mandoforge/mandoforge-api:whiskey-20260525-6e08fab`.
- API: `127.0.0.1:18787`.
- Postgres: `127.0.0.1:15432`.
- Compose project: `mandoforge-adoption`.
- Tenant routing mode: `tenant_routed`.
- Latest remote archive: `/opt/mandoforge-adoption/archives/mandoforge-whiskey-pilot-20260518T050542Z.tar.gz`.
- Latest local archive copy: `.mandoforge/remote-adoption/whiskey/mandoforge-whiskey-pilot-20260518T050542Z.tar.gz`.
- Latest Stage 2 strict archive copy: `.mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260518T050542Z.tar.gz`.
- Latest strict archive summary: `stage2_status=ready`, `completion_blocked=false`, `open_gap_count=0`, `validation_missing_endpoint_count=0`, `validation_stale_endpoint_count=0`.
- Latest semantic console / worker smoke: API and worker containers run image
  `whiskey-20260525-6e08fab`; `/healthz` returns `{"status":"ok"}`;
  static UI serves `index-BCX-AyF_.js` and `index-DeJQG8Pc.css`; authenticated
  semantic API readback reports `scope_rank` as the effective retrieval backend,
  memory governance `status=ready`, 3 semantic objects, and 0 semantic links.
  The fixed workflow-step worker smoke passed with workflow run
  `2cf0120b-89b6-4d11-bcc6-ad8aa428b34c`, completed step
  `c81855c4-acd9-4b27-8c89-0d0bf4c66f69`, worker
  `whiskey-pilot-worker-1`, context packet
  `cecee29d-3ea6-421b-9c51-939bf897f933`, 4 tool calls, and 2 artifacts.

## Current Remaining Scope

- Current Whiskey production-like pilot blocker: none for the single-node local-hostpath Remote Computer pilot; multi-node distributed state remains a post-pilot promotion.
- Post-pilot enterprise promotions kept visible but not counted as the current blocker: broader multi-tenant routing targets, a real production policy controller, a real secret backend, broader docs/wiki knowledge targets, and downstream ERP/accounting integration.
- Fastest operator handoff from the synced artifacts: `scripts/whiskey-adoption-next-actions.sh`.

## Lane Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| Codex App Server deployment/ops | Passed for Whiskey pilot | Strict evidence archive `stage2-production-whiskey-20260517T084848Z.tar.gz` reports `codex_app_server_health_status=healthy`, `deployment_readiness_status=ready`, `ops_validation_status=validated`, `production_blocked_count=0`, and stale-poll evidence under `api-codex-app-server-runs-poll-stale.json` with `failed_count=0`. | Extend native WebSocket steering beyond health/deployment/ops when product work needs thread/turn/command execution through the native protocol. |
| Worker load validation | Passed for Whiskey single-host pilot | Strict evidence archive `stage2-production-whiskey-20260517T070211Z.tar.gz` reports `load_validation.status=validated`, `production_ops.status=ready`, `controller_evidence_fresh=true`, worker container running, durable Postgres queue, queue worker mode, no failed jobs, and no stale leases. The controller records `scope=whiskey-single-host`; this is not a k3s multi-replica autoscaling proof. | Keep as single-host evidence unless k3s/cluster adoption is approved; full scale-out evidence still needs a real cluster profile. |
| Remote Computer cluster/state | Passed for Whiskey single-node local-hostpath pilot | Latest strict archive `stage2-production-whiskey-20260518T050542Z.tar.gz` reports `stage2_status=ready`, `completion_blocked=false`, and `validation_missing_endpoint_count=0`. The paired full pilot evidence reports `remote_computer_status=ready`, `readiness_score=100`, `production_state_sync_status=ready`, `runner_status=dry_run_ready`, `production_blocked_count=0`, and no state-sync blocking reasons. Live verification also shows the warm-pool Pod `2/2 Running`, the `mandoforge-remote-computer-state` PVC/PV `Bound`, provider `local-hostpath`, and a successful write test under `/agent-state`. | Keep this as single-node Whiskey pilot proof only. Move to JuiceFS/CephFS/Longhorn RWX before claiming multi-node distributed Memory/Notes/Skills state. |
| Tenant routing/RLS | Passed for Whiskey tenant-routed pilot | Full pilot archive `mandoforge-whiskey-pilot-20260517T111701Z.tar.gz` reports `tenant_isolation_status=ready`, `readiness_score=100`, `runtime_tenant_mode=tenant_routed`, `production_routing_status=ready`, `rls_enabled=true`, `rls_forced=true`, `tenant_context_configured=true`, `production_blocked_count=0`, and fresh controller evidence. The deployed API is configured with `MANDOFORGE_TENANT_ROUTING_MODE=tenant_routed` and Postgres RLS context refresh on connection acquire. | Keep cross-tenant negative tests and external enterprise multi-tenant targets as follow-up; this is Whiskey pilot readiness, not a multi-cluster enterprise tenant rollout. |
| MCP connector rollout | Passed for Whiskey authenticated Lark docs/wiki search connector target | Strict evidence archive `stage2-production-whiskey-20260518T012511Z.tar.gz` reports the `whiskey-docs` connector deployment, due rollout, and rollback path as healthy, with `validation_missing_endpoint_count=0` for the full Stage 2 coverage. The paired full pilot archive `mandoforge-whiskey-pilot-20260518T012511Z.tar.gz` shows the same bound `whiskey-docs` connector serving `lark-docs-search-authenticated` results, with `connector_quality_live_title=07 — Six Systems Client Brief · Operation Forge` and `connector_quality_live_url=https://mandonothing.feishu.cn/docx/PNBkdzs2KoGDb5xxTKCcrF5pnYF#doxcnUuec41N6zBW1azgtxXCawe`. The strict archive still carries both `workflow-packs/whiskey-mcp-lark-docs-scope-latest.txt` and `workflow-packs/whiskey-mcp-lark-docs-login-prompt-latest.txt`, but they are now supporting evidence for the live adopted path rather than a blocked next step. | Keep this as the real authenticated Lark docs/wiki target; broader knowledge-system expansion remains optional follow-up, not a blocker for Whiskey docs adoption. |
| Eval/release rollout | Passed for Whiskey pilot release target | Strict evidence archive `stage2-production-whiskey-20260517T080520Z.tar.gz` reports due-run `promoted_count=1`, `controller_execution_count=1`, `controller_failed_count=0`, orchestration validation `status=validated`, deployment validation `status=healthy`, deployment and orchestration controller execution `status=validated`, and rollback response `status=rolled_back` for the `whiskey-eval-release` target. | Keep this as a Whiskey pilot release proof; external production release targets still need their own rollout/orchestration/deployment/rollback controllers and policy. |
| OTel collector | Passed for Whiskey real single-node collector target | Strict evidence archive `stage2-production-whiskey-20260517T235226Z.tar.gz` reports `api-observability-collector-readiness.json` with `status=ready`, `production_ops.status=ready`, `deployment_readiness.status=ready`, `cluster_rollout.status=ready`, and `remediation_supervision.status=ready`. The same archive includes `observability-collector/otel-collector-evidence.json`, which records a running `otel/opentelemetry-collector-contrib:0.123.0` service with `otlp_endpoint=http://otel-collector:4318` and `health_endpoint=http://otel-collector:13133/healthz`, plus `observability-collector/otel-collector-live-signals.log`, which shows the collector receiving logs, traces, and metrics batches from the Whiskey API. The API validation artifacts still report deployment `status=healthy`, cluster rollout `status=validated`, and remediation `status=completed`. | Move from a real single-node Whiskey collector to a retained, alerted, multi-node collector target if observability adoption needs external storage, alert routing, or cross-node telemetry. |
| Provider rollout/rollback | Passed for Whiskey real DeepSeek provider target | Full pilot archive `mandoforge-whiskey-pilot-20260517T192125Z.tar.gz` reports `provider-governance/summary.txt` with `provider_count=1`, `active_provider_count=1`, `provider_policy_gate_status=passed`, `provider_policy_gate_enforcement_status=ready`, `provider_name=whiskey-deepseek-provider`, `provider_type=openai_compatible`, `provider_health_status=active`, `provider_health_external_probe=healthy`, `provider_health_api_key_ref_resolved=true`, `deployment_readiness_status=ready`, `provider_rollout_run_status=applied`, `provider_rollback_run_status=rolled_back`, and `production_blocked_count=0`. The paired `provider-governance/whiskey-provider-rollout-seed.json` records `method=PATCH`, `base_url=https://api.deepseek.com`, `default_model=deepseek-v4-flash`, and `config.api_key_ref=vault:providers/whiskey-deepseek#api_key`, with the referenced secret reused from Whiskey's Vault boundary. | Move from a single real model endpoint to a broader production provider fleet, credential rotation policy, and traffic-switch target if provider rollout needs to cover more than one approved model surface. |
| Approval notifications | Passed for Whiskey real Lark IM delivery target | Full pilot archive `mandoforge-whiskey-pilot-20260517T235226Z.tar.gz` reports `approval-notifications/summary.txt` with `production_ops_status=ready`, `deployment_readiness_status=ready`, `configured_channel_count=1`, `active_policy_count=1`, `unroutable_pending_approval_count=0`, `delivery_evidence_status=captured`, `delivery_status=delivered`, `delivery_target_count=2`, `delivery_delivered=true`, `delivery_observer_status=ok`, `delivery_mode=lark_im`, `delivery_forwarding_status=delivered`, `delivery_forwarding_channel=lark_im`, `delivery_forwarded_message_id=om_x100b6f91086e6ca0b14d1f658a664f8`, `delivery_forwarded_chat_id=oc_d950f4d66f237b6c43de439bfd77cf27`, and `production_blocked_count=0`. The Whiskey approval webhook now forwards to a real Feishu/Lark private chat via `lark-cli`, and the observer health endpoint records the latest forwarded message identifiers. | Move from self-directed Lark IM proof to a dedicated approver chat or group target if the production notification path should fan out beyond a single operator inbox. |
| Vault/KMS/HSM | Passed for Whiskey pilot KMS lifecycle target | Strict evidence archive `stage2-production-whiskey-20260517T092054Z.tar.gz` reports `api-vault-readiness.json` with `status=passed`, `secret_provider.status=healthy`, `kms.status=ready`, `production_rotation.status=ready`, and `production_recovery.status=ready`. Rotation evidence reports `status=validated`, `rotated_count=1`, `catalog_updated_count=1`, and external execution `status=validated`; recovery evidence reports `status=validated`, `latest_rotation_validated=true`, controller execution `status=validated`, and no issues. | Keep this as a Whiskey pilot lifecycle proof; real KMS/HSM or enterprise Vault adoption still needs external target credentials, envelope-encryption policy, approval-backed secret value rotation, and recovery/rollback procedures. |
| Finance/accounting reconciliation | Passed for Whiskey real Lark Drive export target | Full pilot archive `mandoforge-whiskey-pilot-20260517T235226Z.tar.gz` reports `finance/summary.txt` with `production_close_status=ready`, `production_blocked=false`, `finance_close_run_status=completed`, `finance_reconciliation_run_status=reconciled`, `finance_export_delivery_status=delivered`, `finance_export_delivery_target_configured=true`, `finance_export_delivery_observer_status=ok`, `finance_export_delivery_mode=lark_drive`, `finance_export_delivery_file_token=E6SubdtnQo0sgPxTm1RcU544nRc`, `finance_export_delivery_file_url=https://www.feishu.cn/file/E6SubdtnQo0sgPxTm1RcU544nRc`, and `finance_export_delivery_file_name=mandoforge-usage-export.csv`. The Whiskey finance export webhook now uploads the generated CSV to a real Feishu Drive file before close and reconciliation gates run. | Move from a real export artifact target to a true downstream accounting ledger or ERP if finance close needs reconciliation against an external system of record. |

## Stage 3 Pilot Matrix

| Lane | Whiskey status | Evidence | Next action |
| --- | --- | --- | --- |
| WorkflowPack / AI Governance Pack | Passed for Whiskey pilot lifecycle with install defaults, immutable version-update, persisted profile assets, onboarding assessment, connector quality, real MCP binding, and authenticated Lark docs/wiki proof | Full pilot archive `mandoforge-whiskey-pilot-20260518T012511Z.tar.gz` includes `workflow-packs/summary.txt` with `workflow_pack_status=version_created_after_rollback_and_archive`, `pack_id=ai-governance`, `validated_file_count=42`, `install_status=installed`, `stage_status=staged`, `release_status=released`, `rollback_status=rolled_back`, `update_status=installed`, `update_version=0.1.1`, `update_manifest_path=packs/ai-governance/package-v0.1.1.yaml`, `blocked_onboarding_status=blocked`, `onboarding_status=ready`, `onboarding_workflow=profile-onboarding`, `onboarding_eval=profile-onboarding-regression`, `required_profile_count=6`, `profile_schema_count=6`, `inline_profile_count=0`, `persisted_profile_count=6`, `provided_profile_count=6`, `placeholder_profile_count=0`, `connector_requirement_count=1`, `ready_connector_count=1`, `onboarding_blocker_count=0`, `blocked_connector_quality_status=blocked`, `connector_quality_status=ready`, `connector_quality_requirement_count=1`, `connector_quality_ready_connector_count=1`, `connector_quality_sample_count=1`, `connector_quality_passing_sample_count=1`, `connector_quality_blocker_count=0`, `connector_quality_bound_team_id=9ebe8ad5-960f-4d1b-82fb-afaf0468381c`, `connector_quality_bound_server_id=c85beded-8cf9-4197-93a7-254b4f899dea`, `connector_quality_bound_server_name=whiskey-docs`, `connector_quality_bound_server_health_status=healthy`, `connector_quality_live_source=lark-docs-search-authenticated`, `connector_quality_live_auth_mode=authenticated`, `connector_quality_live_title=07 — Six Systems Client Brief · Operation Forge`, `connector_quality_live_url=https://mandonothing.feishu.cn/docx/PNBkdzs2KoGDb5xxTKCcrF5pnYF#doxcnUuec41N6zBW1azgtxXCawe`, `installed_default_profile_asset_count=6`, `updated_default_profile_asset_count=6`, `persisted_profile_asset_count=6`, `persisted_profile_list_count=6`, `persisted_profile_saved_min_version=2`, `persisted_profile_saved_max_version=2`, `archive_status=archived`, `eval_gate_status=passed`, `release_gate_status=passed`, `rolled_back_get_status=rolled_back`, `rolled_back_list_count=1`, `archived_get_status=404`, and `updated_active_after_archive_count=1`. The gate now proves install/update defaults, persisted customer assets, fail-closed onboarding and connector-quality checks, binds to the real Whiskey `whiskey-docs` MCP server, and reaches `ready` from authenticated Lark docs/wiki search retrieval. The strict archive still carries the scope and login-prompt artifacts as reproducibility evidence for the same adopted path. | Keep this Lark-backed WorkflowPack proof current while keeping Remote Computer blockers explicit. |

## k3s Decision

Do not install k3s automatically on Whiskey. Before any cluster experiment, run:

```bash
scripts/whiskey-remote-computer-k3s-preflight.sh
```

If that preflight is accepted, the next repo-native step is:

```bash
scripts/whiskey-remote-computer-k3s-prepare.sh
scripts/whiskey-remote-computer-k3s-constrained-pilot.sh
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

The latest consolidated host inventory on 2026-05-18T01:48:51Z returned `preflight_status=constrained_pilot_only` and `verify_status=ready` with:

- `cpu_count=2`
- `mem_available_mib=1644`
- `swap_used_mib=1952`
- `root_avail_gib=83.0`
- `cgroup_fs=cgroup2fs`
- `reserved_ports=none`
- `br_netfilter_loaded=true`
- `bridge_nf_call_iptables=1`

That means the constrained single-node `k3s` pilot is now live on Whiskey, but the host is still resource-tight and the remaining production blocker has moved up-stack from cluster bootstrapping to shared-state and live-runner hardening.

Whiskey is now a single-node `k3s` production-like pilot: useful for API, Codex App Server deployment/ops, scheduler, manifest application, and cluster-backed inventory evidence, but still not enough for complete Remote Computer cluster/state adoption until shared state and live runner gates are real.

The wrapper `scripts/whiskey-remote-computer-k3s-constrained-pilot.sh` now stitches together host inventory, prerequisite preparation, install, verify, and manifest-render review into one repo-native command. It defaults to `dry_run`, keeps the helper artifacts under the same output directory, writes a timestamped plan under `.mandoforge/remote-adoption/whiskey/`, and prints the next repo-native `scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence` step while still retaining the lower-level `kubectl apply -k deploy` detail for debugging.

After `scripts/whiskey-remote-computer-k3s-verify.sh` reports `ready`, the repo-native follow-up command is `scripts/whiskey-remote-computer-k3s-cluster-stage.sh`. It syncs the `deploy/` bundle to Whiskey, auto-installs the official KEDA core manifest if `ScaledObject` CRDs are missing, verifies local and remote `kubectl kustomize` render counts, writes timestamped plus `-latest` cluster-stage artifacts into the same sync directory, and only applies manifests or reruns evidence when `--apply-manifests` or `--run-evidence` are passed explicitly. When `--run-evidence` is used, the adoption evidence script now syncs those cluster-stage artifacts into the normal Remote Computer evidence archives as part of the approved path.
