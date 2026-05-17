# Whiskey Adoption Completion Audit

Snapshot date: 2026-05-17.

This audit maps the requested Whiskey adoption plan to concrete repository, CI, deploy, and evidence artifacts. It is intentionally stricter than the status ledger: a green CI run or a passing evidence gate only counts when it directly covers the requested item.

## Objective Restatement

Move from writing Stage 3 direction to exercising Stage 2 and Stage 3 capabilities on `wishky-2-1` as a repeatable production-like pilot:

- preserve the loopback API contract at `127.0.0.1:18787`;
- publish pullable GHCR images instead of compiling on Whiskey;
- keep deploy and evidence collection as repo scripts/docs;
- archive every adoption run under `/opt/mandoforge-adoption/archives/`;
- sync local archive copies under `.mandoforge/remote-adoption/whiskey/`;
- move Codex App Server, Worker, and the remaining adoption lanes out of chat-only backlog and into real Whiskey evidence;
- avoid claiming full production completion where Whiskey remains a single-host pilot.

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Current status |
| --- | --- | --- |
| Push local commits including `579e927 Package runtime assets in deploy image`. | `git log --all --oneline` contains `579e927 Package runtime assets in deploy image`; latest pushed WorkflowPack/Whiskey adoption commits are `b8d74cc Add WorkflowPack connector quality assessment`, `847bcc5 Record WorkflowPack connector-quality Whiskey evidence`, `4393a78 Bind WorkflowPack connector quality to MCP server state`, `5acc49a Format MCP-bound connector quality changes`, and `650d9d5 Trigger MCP health before connector quality proof`. | Passed. |
| Package runtime assets in the deploy image. | Commit `579e927`; Deploy workflow images since then run Whiskey evidence scripts from the container and remote synced repo files. | Passed. |
| Capture Whiskey deploy as repo docs/scripts, not remote-only compose. | [Whiskey Adoption Runbook](whiskey-adoption-runbook.md), [Whiskey Adoption Status](whiskey-adoption-status.md), `scripts/whiskey-adoption-deploy.sh`, `scripts/whiskey-adoption-evidence.sh`, `deploy/whiskey/docker-compose.adoption.yml`, and controller files under `deploy/whiskey/`. | Passed. |
| Publish a pullable GHCR image. | Deploy workflow `25994179964` succeeded for `ghcr.io/proerror77/mandoforge/mandoforge-api:5acc49a`; Whiskey `docker compose ps` shows API and worker running that image. | Passed. |
| Keep Whiskey API internal on `127.0.0.1:18787`. | Remote `docker compose ps` reports `127.0.0.1:18787->8787/tcp`; [Whiskey Adoption Status](whiskey-adoption-status.md) records the same endpoint. | Passed. |
| Add repeatable deploy/runbook. | `MANDOFORGE_IMAGE_TAG=<tag> scripts/whiskey-adoption-deploy.sh` is documented in [Whiskey Adoption Runbook](whiskey-adoption-runbook.md); deploy script copies compose/controllers, renders `whiskey.env`, starts controllers, pulls the image, and starts API/Postgres/worker. | Passed. |
| Archive every adoption evidence run remotely. | Latest full remote archive is `/opt/mandoforge-adoption/archives/mandoforge-whiskey-pilot-20260517T152304Z.tar.gz`; latest strict archive is `/opt/mandoforge-adoption/archives/stage2-production-whiskey-20260517T152304Z.tar.gz`. | Passed. |
| Sync evidence locally. | Latest local copies are `.mandoforge/remote-adoption/whiskey/mandoforge-whiskey-pilot-20260517T152304Z.tar.gz` and `.mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260517T152304Z.tar.gz`. | Passed. |
| Verify Stage 2 strict archive. | `ALLOW_BLOCKED=1 ./scripts/verify-stage2-evidence-archive.sh .mandoforge/remote-adoption/whiskey/stage2-production-whiskey-20260517T152304Z.tar.gz` passes. Archive summary reports `stage2_status=ready`, `completion_blocked=false`, `open_gap_count=0`, `validation_missing_endpoint_count=0`, and `validation_stale_endpoint_count=0`. | Passed for Whiskey pilot evidence coverage. |
| Configure real Codex App Server URL and deployment/ops controller target for Whiskey. | [Whiskey Adoption Runbook](whiskey-adoption-runbook.md) records `MANDOFORGE_CODEX_APP_SERVER_URL=ws://host.docker.internal:18788`, deployment controller on `:18789/deployment/validate`, and ops controller on `:18789/ops/validate`; [Whiskey Adoption Status](whiskey-adoption-status.md) records the lane as passed for Whiskey pilot. | Passed for Whiskey pilot; native WebSocket steering beyond health/deployment/ops remains product follow-up. |
| Run `scripts/codex-app-server-evidence-gate.sh`. | `scripts/whiskey-adoption-evidence.sh` runs the focused Codex App Server gate; status ledger records healthy deployment/ops/stale-poll evidence with `production_blocked_count=0`. | Passed for Whiskey pilot. |
| Run Worker load evidence on Whiskey. | Worker lane in [Whiskey Adoption Status](whiskey-adoption-status.md) records durable Postgres queue, queue worker mode, controller evidence fresh, load validation status validated, and no failed jobs/stale leases. | Passed for Whiskey single-host pilot. |
| Decide whether full Remote Computer cluster evidence needs k3s. | [Whiskey Adoption Status](whiskey-adoption-status.md) documents that k3s must not be installed automatically and requires explicit approval. | Decision preserved; not approved. |
| Record Remote Computer evidence without overclaiming. | Latest strict evidence records Remote Computer readiness and sidecar recovery audit, but `production_state_sync_status=blocked`; status ledger states this is not pod replacement proof. | Inventory passed; full cluster/state adoption not complete. |
| Tenant routing/RLS adoption. | Latest full archive `tenant-isolation/summary.txt` reports `tenant_isolation_status=ready`, `readiness_score=100`, `runtime_tenant_mode=tenant_routed`, `production_routing_status=ready`, `rls_enabled=true`, `rls_forced=true`, `tenant_context_configured=true`, and `production_blocked_count=0`. | Passed for Whiskey tenant-routed pilot. |
| MCP connector deployment/rollout. | Whiskey status records pilot `whiskey-docs` connector deployment, due rollout, and rollback evidence as passed. | Passed for Whiskey pilot connector. |
| Eval/release rollout. | Whiskey status records rollout, orchestration, deployment, due-run, and rollback evidence as passed for `whiskey-eval-release`. | Passed for Whiskey pilot release target. |
| OTel collector. | Whiskey status records collector deployment, cluster rollout, and remediation evidence as passed. | Passed for Whiskey pilot collector target. |
| Provider rollout/rollback. | Whiskey status records provider policy gate, deployment controller, rollout, and rollback evidence as passed. | Passed for Whiskey pilot provider target. |
| Approval notifications. | Whiskey status records webhook delivery, deployment controller, and ops controller evidence as passed. | Passed for Whiskey pilot webhook target. |
| Vault/KMS/HSM. | Whiskey status records Vault health, KMS rotation, and KMS recovery controller evidence as passed. | Passed for Whiskey pilot KMS lifecycle target. |
| Finance/accounting reconciliation. | Whiskey status records finance close, reconciliation, CSV export, and delivery evidence as passed. | Passed for Whiskey pilot accounting target. |
| Stage 3 WorkflowPack adoption on Whiskey. | Latest full archive `workflow-packs/summary.txt` reports `workflow_pack_status=version_created_after_rollback_and_archive`, `install_status=installed`, `stage_status=staged`, `release_status=released`, `rollback_status=rolled_back`, `update_status=installed`, `update_version=0.1.1`, `update_manifest_path=packs/ai-governance/package-v0.1.1.yaml`, `blocked_onboarding_status=blocked`, `onboarding_status=ready`, `onboarding_workflow=profile-onboarding`, `onboarding_eval=profile-onboarding-regression`, `required_profile_count=6`, `profile_schema_count=6`, `inline_profile_count=0`, `persisted_profile_count=6`, `provided_profile_count=6`, `placeholder_profile_count=0`, `connector_requirement_count=1`, `ready_connector_count=1`, `onboarding_blocker_count=0`, `blocked_connector_quality_status=blocked`, `connector_quality_status=ready`, `connector_quality_requirement_count=1`, `connector_quality_ready_connector_count=1`, `connector_quality_sample_count=1`, `connector_quality_passing_sample_count=1`, `connector_quality_blocker_count=0`, `connector_quality_bound_team_id=9ebe8ad5-960f-4d1b-82fb-afaf0468381c`, `connector_quality_bound_server_id=c85beded-8cf9-4197-93a7-254b4f899dea`, `connector_quality_bound_server_name=whiskey-docs`, `connector_quality_bound_server_health_status=healthy`, `installed_default_profile_asset_count=6`, `updated_default_profile_asset_count=6`, `persisted_profile_asset_count=6`, `persisted_profile_list_count=6`, `persisted_profile_saved_min_version=2`, `persisted_profile_saved_max_version=2`, `archive_status=archived`, `eval_gate_status=passed`, `release_gate_status=passed`, `rolled_back_get_status=rolled_back`, `rolled_back_list_count=1`, `archived_get_status=404`, `active_after_archive_count=0`, and `updated_active_after_archive_count=1`. | Passed for Whiskey pilot lifecycle with install-defaults, immutable version-update, persisted profile-asset, onboarding assessment, connector-quality evidence, and real MCP server binding. |

## Residual Blockers

These are not documentation gaps; they are real environment or product gaps that should remain visible.

| Blocker | Why it is still blocked | Required next step |
| --- | --- | --- |
| Remote Computer cluster/state adoption | Whiskey has no approved k3s/cluster and no real distributed state filesystem or lock-aware state sync. Sidecar recovery is audited inventory/no-op evidence, not pod replacement proof. | Choose a real cluster path, or explicitly approve a constrained k3s pilot, then configure distributed state and rerun Remote Computer evidence. |
| External enterprise targets | Whiskey controllers validate live MandoForge boundaries, but most lanes still use local pilot targets rather than real Slack/email, enterprise Vault/HSM, ERP, production provider fleet, external SaaS MCP, or production OTel stack. | Replace pilot controllers with real environment targets lane by lane and archive fresh evidence. |

## Completion Decision

The Whiskey production-like pilot is repeatable, archived, locally synced, GHCR-backed, and broadly evidenced. Tenant routing/RLS plus WorkflowPack install defaults, immutable version updates, persisted onboarding profile assets, onboarding assessment, connector-quality checks, and real MCP server binding now pass for the Whiskey tenant-routed pilot. The objective should not be marked fully complete because the requested adoption plan includes Worker / Remote Computer cluster-state completion, and Remote Computer still has explicit production blockers.

The next concrete work should be one of:

- approve/configure a real cluster path for Remote Computer state/sidecar evidence; or
- continue Stage 3 real non-pilot external connector-target adoption while keeping the Remote Computer and external enterprise target blockers visible.
