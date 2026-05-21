# Stage 2 Production Adoption Runbook

This runbook is for validating a specific production-like or production deployment of the completed Stage 2 Governed Runtime Pilot.

It does not reopen the repo-controlled Stage 2 completion decision. It proves whether one named environment has supplied real controller targets, generated fresh evidence, and passed the fail-closed production gates.

## Adoption Definition

A Stage 2 production adoption is complete for an environment only when all of the following are true:

- The target environment is named in the evidence record.
- Controller URLs and credentials point at real deployment targets, not mock controllers.
- Required evidence gates run with `ALLOW_BLOCKED=0`.
- `scripts/stage2-production-evidence-gate.sh` exits `0`.
- `scripts/stage2-completion-audit-gate.sh` writes an unblocked checklist.
- `scripts/archive-stage2-production-evidence.sh` writes an archive, checksum, and manifest.
- `scripts/verify-stage2-evidence-archive.sh <archive>` passes without `ALLOW_BLOCKED=1`.
- `docs/stage2-completion-audit.md` is updated with the validated environment and any residual adoption backlog.

If any item is missing, the environment is still in adoption inventory mode.

## Inputs

Before running the production gate, the integration owner must prepare:

- Environment name, cluster, namespace, API base URL, and release version.
- Controller target inventory using the matrix below.
- Secret-rendering source for `deploy/stage2-evidence/stage2-production-controllers.env.example`.
- Evidence PVC for the strict in-cluster production evidence Job.
- Human approval for optional higher-impact actions such as KMS rotation, provider rollout, rollback, notification delivery, sidecar recovery, remediation, and finance close.

Do not commit real tokens, real production URLs, generated Secrets, or evidence bundles that contain sensitive deployment metadata.

## Controller Matrix

The controller matrix is the source of truth for Stage 2 production adoption wiring. Every row must either pass against a real target or remain listed in the residual adoption backlog.

All declared target identities used by the strict all-up gate must name real production targets. The preflight, Secret render, completion audit, and archive verifier reject Whiskey, pilot, mock, example, sample, demo, local, localhost, and loopback identities, including KMS backend/key identities. Finance identities additionally reject Feishu/Lark/Drive/file/artifact targets because those prove artifact delivery, not accounting-system adoption.

| Adoption area | Controller env and opt-in flags | Focused gate | Required proof |
| --- | --- | --- | --- |
| Tenant routing and RLS | `MANDOFORGE_STAGE2_TENANT_DEPLOYMENT_ID`, `MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED=true`, `MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL`, `MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN` | `scripts/tenant-isolation-evidence-gate.sh` | Fresh captured and validated routing evidence against a broader multi-tenant deployment with tenant context, at least two tenants, at least two unique audited tenant samples with tenant id plus audit id, trace id, run id, or timestamp detail, nonzero RLS table coverage, forced RLS on every reported table with a unique schema/table identity, enabled/forced status, and audit id, trace id, run id, or timestamp detail, and a nonzero cross-tenant negative-test count between the audited sampled tenants with source tenant, target tenant, denied/blocked outcome, and audit id, trace id, run id, or timestamp detail confirmed by the controller. The controller-reported deployment id must match the all-up run manifest, and every tenant sample, forced-RLS table, and negative-test detail row must bind back to that same deployment id. |
| Policy rollout orchestration | `MANDOFORGE_STAGE2_POLICY_CONTROLLER_ID`, `MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_REQUIRED=true`, `MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL`, `MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_TOKEN`, `RUN_STAGE2_POLICY_DUE_RUN=1` | `scripts/policy-rollout-evidence-gate.sh` | Fresh captured orchestration validation and due-run proof against a real policy rollout controller target; controller evidence must report a production controller kind/environment, controller id, production rollout scope, production policy store id, deployment id, rollback support, a production rollback plan/procedure/revision/run id with rollback audit or trace evidence, and audited orchestration steps bound to the same controller id, policy store id, and deployment id, with step names, passed/validated/completed status, and audit id, trace id, run id, or timestamp detail. Due-run evidence must be captured and must scan at least one policy revision, and each scanned revision must include audit id, trace id, run id, or timestamp detail. The controller id must match the all-up run manifest. |
| Provider governance | `MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL`, `MANDOFORGE_PROVIDER_ROLLOUT_TOKEN`, `MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL`, `MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_TOKEN`, `RUN_STAGE2_PROVIDER_ROLLOUT=1` | `scripts/provider-governance-evidence-gate.sh` | Fresh provider gate, deployment validation, production rollout, and rollback proof against real provider deployment targets. |
| Vault and KMS/HSM lifecycle | `MANDOFORGE_STAGE2_KMS_BACKEND_ID`, `MANDOFORGE_KMS_PROVIDER=external`, `MANDOFORGE_KMS_KEY_ID`, `MANDOFORGE_KMS_ROTATION_POLICY`, `MANDOFORGE_KMS_VALIDATION_MODE=external`, `MANDOFORGE_KMS_ENDPOINT`, `MANDOFORGE_KMS_TOKEN`, `MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED=true`, `MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL`, `MANDOFORGE_KMS_RECOVERY_CONTROLLER_TOKEN`, `RUN_STAGE2_SECRET_LIFECYCLE=1` | `scripts/vault-evidence-gate.sh` | Fresh captured KMS rotation and recovery validation proof against a real Vault plus external KMS/HSM backend; rotation and recovery evidence must report production backend kind, environment, backend id, key id, production rotation/recovery ids, nonzero rotated record and catalog update counts, key-level rotation details with key id, rotation id, catalog update confirmation, rotated/validated/completed status, and audit id, trace id, run id, or timestamp detail, plus audited recovery steps with step names, matching backend/key/recovery ids, passed/validated/completed status, and audit id, trace id, run id, or timestamp detail. Backend and key ids must match the all-up run manifest. |
| Worker queue and autoscaling | `MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID`, `MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED=true`, `MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL`, `MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_TOKEN` | `scripts/worker-evidence-gate.sh` | Fresh queue pressure, load-validation, autoscaling, and isolated worker-pool evidence against a real multi-node cluster profile. The controller must report `status=validated`, real cluster target kind, production cluster id, `node_count >= 2`, `load_validated=true`, and `isolated_worker_pool_configured=true`, plus worker-pool load-check details with check name, worker pool or queue, passed/validated/completed status, and audit id, trace id, run id, or timestamp detail. The cluster id must match `MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID` and the all-up run manifest. |
| Remote Computer state and sidecars | `MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID`, `MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED=true`, `MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL`, `MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_TOKEN`, `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED=true`, `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL`, `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_TOKEN`, `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED`, `RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1` | `scripts/remote-computer-evidence-gate.sh` | Fresh distributed state-sync and sidecar-recovery evidence against real shared state storage and cluster resources. The state-sync controller must report a real multi-node cluster, supported distributed state backend, shared state claim, checked state-contract path count, and per-path detail that names the same state claim/PVC plus a passed/validated/ready status and audit id, trace id, run id, or timestamp detail, and the sidecar validation controller must report cluster-wide replacement, healthy replacement Pods, and checked Pod count with the same audit detail. The state-sync and sidecar cluster ids must match the all-up run manifest. |
| Worker plus Remote Computer Whiskey blocker | Same worker queue and Remote Computer controller env as the two rows above, with `RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1` | `scripts/worker-remote-computer-evidence-gate.sh` | Combined archive-ready proof that the captured isolated worker pool, load-validation controller, Remote Computer state-sync, runner readiness, and sidecar replacement evidence all pass for the same multi-node real cluster and distributed state backend, including worker-pool load-check detail rows in the combined summary with worker pool or queue plus audit id, trace id, run id, or timestamp detail, audited state claim/path checks whose path details bind to the same state claim/PVC and have passed path statuses, and audited healthy replacement Pod checks; single-host or local-hostpath pilot evidence is insufficient. |
| Approval notifications | `MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED=true`, `MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL`, `MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_TOKEN`, `RUN_STAGE2_APPROVAL_DELIVERY=1` | `scripts/approval-notification-evidence-gate.sh` | Fresh deployment, production ops, and optional delivery proof against real webhook, Slack, or email targets. |
| MCP connector rollout | `MANDOFORGE_STAGE2_TEAM_ID` or team auto-discovery, `MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL`, `MANDOFORGE_MCP_ROLLOUT_CONTROLLER_TOKEN`, `MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED=true`, `MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL`, `MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_TOKEN`, `RUN_STAGE2_MCP_DUE_RUN=1`, `RUN_STAGE2_MCP_ROLLBACK=1` | `scripts/mcp-gateway-evidence-gate.sh` | Fresh team-scoped deployment, rollout, due-run, and rollback proof against a real connector target. |
| Codex App Server | `MANDOFORGE_CODEX_APP_SERVER_URL`, `MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED=true`, `MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL`, `MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_TOKEN`, `RUN_STAGE2_CODEX_STALE_POLL=1` | `scripts/codex-app-server-evidence-gate.sh` | Fresh deployment, ops, control-plane, trace inventory, and optional stale-poll supervision evidence against a real App Server target. |
| Managed-session restart/resume | `MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID`, `MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL`, `MANAGED_SESSION_RESTART_RESUME_CONTROLLER_TOKEN`, `RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME=1` | `scripts/managed-session-runtime-evidence-gate.sh` | Fresh restart/resume drill evidence proving session event enqueue, worker drain, API and worker restart, resumed session cursor state, thread lineage preservation, runtime turn finalization, and stale-worker lease fencing. Evidence must include concrete session-loop sequence windows, before/after processed cursor values, matching before/after thread ids, distinct active and stale lease ids with rejection reason, and a runtime turn id plus final message or artifact. The target id must match the all-up run manifest. |
| Eval and release rollout | `MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED=true`, `MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL`, `MANDOFORGE_AGENT_RELEASE_CONTROLLER_TOKEN`, `MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED=true`, `MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL`, `MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_TOKEN`, `MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED=true`, `MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL`, `MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_TOKEN`, `RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1`, `RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1` | `scripts/eval-release-evidence-gate.sh` | Fresh release deployment, orchestration, Stage 2 regression, due-run, and rollback proof against a real release target. |
| Observability collector | `MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_TOKEN`, `MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED=true`, `MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL`, `MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_TOKEN`, `MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED=true`, `MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL`, `MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_TOKEN`, `RUN_STAGE2_OBSERVABILITY_REMEDIATION=1` | `scripts/observability-collector-evidence-gate.sh` | Fresh collector deployment, cluster rollout, and remediation proof against a real collector deployment. |
| Scheduler deployment | `MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_REQUIRED=true`, `MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_URL`, `MANDOFORGE_SCHEDULER_DEPLOYMENT_CONTROLLER_TOKEN` | `scripts/scheduler-evidence-gate.sh` | Fresh scheduler deployment validation and authenticated due-plan/run-due evidence. |
| Finance close and reconciliation | `MANDOFORGE_STAGE2_FINANCE_SYSTEM_ID`, `MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED=true`, `MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL`, `MANDOFORGE_FINANCE_CLOSE_CONTROLLER_TOKEN`, `MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED=true`, `MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL`, `MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_TOKEN`, `MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL`, `FINANCE_EXPORT_DELIVERY_OBSERVER_URL`, `FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN`, `RUN_STAGE2_FINANCE_CONTROLLERS=1`, `RUN_STAGE2_FINANCE_EXPORT=1` | `scripts/finance-evidence-gate.sh` | Fresh close, reconciliation, export CSV, and authenticated export delivery proof against a true accounting-system or ERP target; close evidence must include a configured controller, close id, close action, and audited steps with audit id, trace id, run id, or timestamp detail; reconciliation evidence must include a reconciliation id and audited checks with matching detail; ERP/accounting delivery receipts must include receipt id, system id, posted/accepted status, record count, current export file name and byte count, and audit id, run id, or posting timestamp detail. Feishu Drive artifact delivery alone is not sufficient. The observer-reported ERP/accounting system id must match the all-up run manifest. |

## Procedure

1. Confirm the target release and environment.

   ```bash
   git rev-parse HEAD
   kubectl config current-context
   kubectl get ns
   ```

2. Verify the template and manifest contract before adding real values.

   ```bash
   scripts/verify-stage2-controller-env-template.sh
   scripts/verify-stage2-evidence-k8s-manifests.sh
   scripts/verify-stage2-evidence-archive.sh --self-test
   ```

3. Create a reviewed production env file from `deploy/stage2-evidence/stage2-production-controllers.env.example`.

   ```bash
   scripts/stage2-production-evidence-preflight.sh /secure/path/stage2-production-controllers.env
   ```

   This preflight is not completion evidence. It fails closed when the open production-adoption targets still point at placeholders, local mock controllers, Whiskey pilot URLs, disabled sidecar replacement, or missing token/key values.

   Keep `ALLOW_BLOCKED=0`, `RUN_STAGE2_PRODUCTION_VALIDATIONS=1`, `VERIFY_STAGE2_VALIDATION_COVERAGE=1`, and `RUN_STAGE2_COMPLETION_AUDIT=1` for a strict adoption attempt.

4. Render the Kubernetes Secret from the reviewed env file.

   ```bash
   scripts/render-stage2-controller-secret.sh <reviewed-env-file> > /tmp/mandoforge-stage2-controller-env.yaml
   kubectl apply -f /tmp/mandoforge-stage2-controller-env.yaml
   ```

5. For local operator runs, export the reviewed env file and point gates at the target API.

   ```bash
   set -a
   . <reviewed-env-file>
   set +a
   export BASE_URL=https://mandoforge-api.<environment.example>
   ```

   In-cluster Jobs set `BASE_URL=http://mandoforge-api:8787` and read controller values from `mandoforge-stage2-controller-env`.

6. Run focused gates first when onboarding a new environment.

   ```bash
   scripts/tenant-isolation-evidence-gate.sh
   scripts/policy-rollout-evidence-gate.sh
   scripts/provider-governance-evidence-gate.sh
   scripts/vault-evidence-gate.sh
   scripts/worker-evidence-gate.sh
   scripts/remote-computer-evidence-gate.sh
   scripts/approval-notification-evidence-gate.sh
   scripts/mcp-gateway-evidence-gate.sh
   scripts/codex-app-server-evidence-gate.sh
   scripts/eval-release-evidence-gate.sh
   scripts/observability-collector-evidence-gate.sh
   scripts/scheduler-evidence-gate.sh
   scripts/finance-evidence-gate.sh
   ```

   Focused gates are useful for diagnosis, but they are not the final adoption proof.

7. Run the strict all-up production gate.

   ```bash
   scripts/stage2-production-evidence-gate.sh
   ```

   In cluster, render and run the production evidence bundle from `deploy/stage2-production-evidence`.

8. Run or inspect the completion audit checklist.

   ```bash
   SOURCE_EVIDENCE_DIR=.mandoforge/stage2-production-evidence \
   scripts/stage2-completion-audit-gate.sh
   ```

   The strict production evidence gate writes this automatically under `$EVIDENCE_DIR/completion-audit/` when `RUN_STAGE2_COMPLETION_AUDIT=1`. The audit expects the same `production-evidence-run.json` target manifest as the archive verifier and fails if the tenant routing, policy rollout, Vault/KMS, worker/Remote Computer, finance, or managed-session restart/resume artifacts report a different production target.

9. Archive the evidence PVC.

   ```bash
   scripts/archive-stage2-production-evidence.sh .mandoforge/stage2-production-evidence-$(date -u +%Y%m%dT%H%M%SZ).tar.gz
   scripts/verify-stage2-evidence-archive.sh .mandoforge/stage2-production-evidence-<timestamp>.tar.gz
   ```

10. Update `docs/stage2-completion-audit.md`.

   Record the validated environment, release commit, archive checksum, controller matrix result, and residual adoption backlog. Do not mark an adoption row complete without fresh evidence from a real target.

## Evidence Expectations

The strict evidence directory should include:

- `summary.txt`
- `api-stage2-readiness.json`
- `validation-declared-endpoints.txt`
- `validation-missing-endpoints.txt`
- One JSON artifact per readiness endpoint.
- One JSON artifact per required validation endpoint.
- Explicit required evidence artifacts such as rollout, rollback, recovery, close, reconciliation, export, and sidecar evidence where the matrix enables them.
- `managed-session-restart-resume-evidence.json`
- `production-evidence-run.json`
- `completion-audit/checklist.json`
- `completion-audit/checklist.md`

The archive helper adds:

- `<archive>.sha256`
- `<archive>.manifest.txt`

## Pass/Fail Rules

Pass:

- The all-up production gate exits `0`.
- The completion checklist reports unblocked status and no missing or stale required evidence.
- The archive verifier passes without `ALLOW_BLOCKED=1`.
- Every controller matrix row is either passed or explicitly listed as residual adoption backlog for that environment.

Fail closed:

- Any required controller URL, token, or required flag is missing.
- Any evidence artifact is missing or stale under `STAGE2_EVIDENCE_MAX_AGE_HOURS`.
- Any controller returns an unhealthy status.
- Any validation endpoint is declared but not covered.
- Any optional higher-impact action is needed for adoption but was not explicitly enabled.
- Any artifact was produced by `scripts/stage2-controller-drill.sh`, `scripts/stage2-controller-drill-live-gate.sh`, or another mock-only path.

Inventory only:

- `ALLOW_BLOCKED=1` runs.
- Local mock-controller drill evidence.
- Read-only evidence snapshots.
- Partial focused-gate runs.

## Residual Backlog Format

Use this format when an environment is partially adopted:

```text
Environment: <name>
Release: <commit>
Evidence archive: <path or release artifact id>
Archive checksum: <sha256>
Passed adoption rows:
- <area>: <evidence artifact names>

Blocked adoption rows:
- <area>: <blocking reason, missing controller, missing target, or stale evidence>

Next retry owner:
- <owner or team>
```
