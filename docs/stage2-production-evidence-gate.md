# Stage 2 Production Evidence Gate

Stage 2 is intentionally fail-closed: green unit tests, static UI smoke checks, and local readiness panels do not prove the governed runtime has been exercised against real production targets.

Use `scripts/stage2-production-evidence-gate.sh` as the operator gate for that proof. It collects machine-readable readiness evidence into `.mandoforge/stage2-production-evidence/` and exits non-zero while `GET /api/stage2/readiness` reports open completion gaps.

For the end-to-end production adoption sequence, controller matrix, and residual backlog format, see [Stage 2 Production Adoption Runbook](stage2-production-adoption-runbook.md).

## Read-only Inventory

```bash
ALLOW_BLOCKED=1 ./scripts/stage2-production-evidence-gate.sh
```

This mode only reads health/readiness endpoints. It is suitable for local inventory and CI snapshots. It does not call external controllers.

## Controller-backed Validation

```bash
RUN_STAGE2_PRODUCTION_VALIDATIONS=1 \
MANDOFORGE_STAGE2_TEAM_ID=<team_uuid> \
./scripts/stage2-production-evidence-gate.sh
```

This mode calls the bounded validation endpoints for tenant routing, provider deployment, policy rollout orchestration, Vault recovery, worker load validation, Remote Computer state sync, approval notifications, Codex App Server, managed-session restart/resume, agent release deployment/orchestration, observability collector deployment/cluster rollout, and team-scoped MCP connector rollout.

Use `deploy/stage2-evidence/stage2-production-controllers.env.example` as the operator checklist for the external controller URLs, required flags, and opt-in validation switches. `deploy/stage2-evidence/stage2-controller-env-secret.example.yaml` shows the matching Kubernetes Secret shape, and `deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml` shows the strict production-validation Job that consumes that Secret through `envFrom`. These are templates only; real URLs and tokens belong in your secret manager, CI environment, or Kubernetes Secret generation pipeline.

For local controller-path rehearsal without real deployment targets, start the API and run:

```bash
BASE_URL=http://127.0.0.1:8787 \
./scripts/stage2-controller-drill.sh
```

The drill starts `scripts/stage2-mock-controller.js`, points every Stage 2 external controller URL at it, enables validation coverage checking, and writes evidence into `.mandoforge/stage2-controller-drill-evidence/`. It defaults to `ALLOW_BLOCKED=1` because mock-controller evidence proves the HTTP controller boundaries are wired, not that Stage 2 is production-complete. Set `RUN_STAGE2_CONTROLLER_DRILL_ACTIONS=1` only when you also want the optional rollout, recovery, stale-poll, regression, remediation, and finance controller actions included in the rehearsal.

To run the same rehearsal without manually starting the API:

```bash
./scripts/stage2-controller-drill-live-gate.sh
```

This starts a temporary local API on `127.0.0.1:8794`, injects mock controller URLs into that API process, runs `scripts/stage2-controller-drill.sh` against a mock controller on `127.0.0.1:18082` with optional controller actions enabled by default, and records evidence under `.mandoforge/stage2-controller-drill-live-evidence/`. It is a CI/local wiring proof only; it does not replace real production controller evidence.

To turn collected evidence into a requirement-by-requirement completion checklist, run:

```bash
ALLOW_BLOCKED=1 \
SOURCE_EVIDENCE_DIR=.mandoforge/stage2-production-evidence \
./scripts/stage2-completion-audit-gate.sh
```

This fetches `GET /api/stage2/readiness`, maps every `evidence_requirements[]` entry to the endpoint JSON artifacts in the source evidence directory, verifies declared evidence scripts, Job manifests, required controller flags, required evidence artifacts, and evidence freshness, then writes `.mandoforge/stage2-completion-audit/checklist.md` plus `.mandoforge/stage2-completion-audit/checklist.json`. For tenant routing, policy rollout, Vault/KMS, worker/Remote Computer, finance close, and managed-session restart/resume requirements, the audit also requires `production-evidence-run.json` and cross-checks its declared target identities against the captured controller artifacts. Worker/Remote Computer completion additionally requires `worker-remote-computer/summary.json` from the combined evidence gate so separate worker and Remote Computer artifacts cannot pass without a same-cluster summary. Evidence artifacts older than `STAGE2_EVIDENCE_MAX_AGE_HOURS` are treated as missing; the default is 24 hours, and `0` disables the age check for narrow debugging. It exits non-zero by default while Stage 2 readiness is blocked or required endpoint artifacts, scripts, manifests, flags, target identities, or fresh evidence artifacts are missing or inconsistent. `ALLOW_BLOCKED=1` is only for inventory and should not be used as completion proof.

The strict production evidence gate runs this completion audit automatically by default after evidence collection and writes it under `$EVIDENCE_DIR/completion-audit/`. Set `RUN_STAGE2_COMPLETION_AUDIT=0` only for a narrow debugging run where an archive-ready checklist is not expected.

The same freshness window is applied to the production evidence gate's endpoint coverage inventory. `validation-missing-endpoints.txt` includes stale validation snapshots, and `summary.txt` reports `validation_stale_endpoint_count` plus `max_evidence_age_hours` so an old evidence PVC cannot produce a green validation coverage summary.

The matching in-cluster template is `deploy/stage2-evidence/stage2-completion-audit-job.example.yaml`. It mounts the same Stage 2 production evidence PVC, reads endpoint artifacts from `/evidence`, checks the packaged `scripts/` and `deploy/` metadata in the runtime image, writes the checklist to `/evidence/completion-audit`, and remains fail-closed unless the production readiness gate and artifact coverage are complete.

After the strict evidence and completion-audit Jobs pass, archive the shared evidence PVC for the release record:

```bash
scripts/archive-stage2-production-evidence.sh .mandoforge/stage2-production-evidence-$(date -u +%Y%m%dT%H%M%SZ).tar.gz
```

The archive helper writes a `.sha256` checksum sidecar and `.manifest.txt` release manifest, then runs `scripts/verify-stage2-evidence-archive.sh` automatically by default. The verifier checks the tarball checksum and manifest, then extracts `completion-audit/checklist.json`. By default it fails if the checklist is still blocked or any endpoint, artifact, evidence script, evidence Job manifest, or required controller flag is missing. It also requires the archived `production-evidence-run.json` identity manifest and cross-checks its declared cluster id, tenant deployment id, policy controller id, KMS backend/key id, ERP/accounting system id, and managed-session runtime target id against the worker/Remote Computer, tenant routing, policy rollout, Vault/KMS, finance, and managed-session restart/resume artifacts so old pilot/mock/single-host/local-hostpath/non-ERP or mixed-target evidence cannot pass only because a checklist says ready. Worker/Remote Computer archives must include the combined `worker-remote-computer/summary.json` showing one shared production cluster, distributed state backend, unique checked state paths, and unique healthy sidecar replacement Pods. The declared identities and controller-reported identities must not be Whiskey, pilot, mock, example, sample, demo, local, localhost, or loopback values; finance system ids also must not be Feishu/Lark/Drive/file/artifact targets. `ALLOW_BLOCKED=1` is only for inventory archive inspection, and `VERIFY_STAGE2_EVIDENCE_ARCHIVE=0` should only be used when debugging the archive helper itself.

For a narrower collector rollout proof, run the dedicated observability collector gate:

```bash
RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 \
./scripts/observability-collector-evidence-gate.sh
```

That gate collects `GET /api/observability`, `GET /api/observability/collector-readiness`, `POST /api/observability/collector/deployment/validate`, `POST /api/observability/collector/cluster/validate`, and optional remediation evidence into `.mandoforge/observability-collector-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/observability-collector-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower Remote Computer substrate proof, run:

```bash
RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 \
./scripts/remote-computer-evidence-gate.sh
```

That gate collects `GET /api/remote-computers/readiness`, `GET /api/remote-computers/runner/readiness`, `POST /api/remote-computers/state-sync/validate`, and optional sidecar recovery evidence into `.mandoforge/remote-computer-evidence/`. The state-sync controller must validate the shared state claim, report a real multi-node cluster target, identify a supported distributed state backend such as JuiceFS, CephFS, or Longhorn RWX, and report a nonzero checked state-contract path count with unique per-path passed/validated/ready status detail. Each checked path detail must identify the same `state_claim`, `claim`, `pvc`, or `persistent_volume_claim` as the controller-level state claim so the evidence proves the paths belong to the reported shared state volume. When sidecar recovery is enabled, the sidecar validation controller must confirm real cluster-wide replacement evidence, healthy replacement Pods, and a nonzero unique checked Pod count. The matching in-cluster template is `deploy/stage2-evidence/remote-computer-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For the current Whiskey pilot blocker, run the combined worker/Remote Computer
gate:

```bash
RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 \
./scripts/worker-remote-computer-evidence-gate.sh
```

The strict all-up production evidence gate captures this combined summary after
worker load validation, state-sync validation, and sidecar recovery validation.
For a focused proof, this gate runs the worker and Remote Computer evidence gates into one
archive-ready evidence directory, then fails closed unless the same target has
fresh isolated worker-pool/load-validation evidence, Remote Computer state-sync
evidence, runner readiness, and sidecar replacement evidence. For the Whiskey
completion blocker, the controller evidence must also identify the same
multi-node real cluster across worker load, state sync, and sidecar replacement,
and state sync must report a supported distributed filesystem backend such as
`juicefs`, `cephfs`, or `longhorn-rwx`. Worker evidence must explicitly report
validated controller execution, real cluster target kind, production cluster id,
`node_count >= 2`, load validation, isolated worker-pool configuration, and
worker-pool load-check details in the combined summary with check name, worker
pool or queue, passed/validated/completed status, and audit id, trace id, run id,
or timestamp detail, with each load-check detail bound to the worker cluster id;
the standalone worker gate also rejects mismatches with
`MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID`. State-sync evidence must name the
state claim plus checked state-contract paths with matching per-path cluster id
and state-claim identity, passed statuses, and audit id,
trace id, run id, or timestamp detail, and sidecar validation must report
healthy replacement Pods plus checked Pod counts with the same cluster id and
audit detail; single-host, local-hostpath,
or shape-only controller evidence does not satisfy this proof.
The matching in-cluster template is
`deploy/stage2-evidence/worker-remote-computer-evidence-job.example.yaml`.

For a narrower provider governance proof, run:

```bash
RUN_STAGE2_PROVIDER_ROLLOUT=1 \
./scripts/provider-governance-evidence-gate.sh
```

That gate collects provider governance summary, provider policy gate report/history, deployment validation, and optional production rollout/rollback evidence into `.mandoforge/provider-governance-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/provider-governance-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower tenant-isolation proof, run:

```bash
./scripts/tenant-isolation-evidence-gate.sh
```

That gate collects tenant isolation readiness and audited production routing
validation evidence into `.mandoforge/tenant-isolation-evidence/`, including an
explicit `tenant-routing-validation-evidence.json` wrapper. It fails closed
unless the target reports `tenant_routed` runtime mode, cross-tenant routing
support, fresh controller evidence, validated routing evidence, and RLS enabled,
forced, and tenant-context configured. The routing controller response must also
identify a broader multi-tenant deployment (`multi_tenant_deployment`,
`enterprise_multi_tenant`, or `production_multi_tenant`), report at least two
tenants plus at least two unique audited tenant samples with tenant id plus audit
id, trace id, run id, or timestamp detail, and confirm RLS enforcement,
tenant context propagation, nonzero RLS table coverage, forced RLS for every
reported table with a unique schema/table identity, enabled/forced status, and audit id, trace id,
run id, or timestamp detail, and a nonzero cross-tenant negative-test count whose
details run between the audited sampled tenants and include source tenant,
target tenant, denied/blocked outcome, and audit id, trace id, run id, or
timestamp detail. Tenant sample, forced-RLS table, and negative-test detail rows
must also carry `deployment_id`, `tenant_deployment_id`, or
`routing_deployment_id` matching the controller deployment id; a
single-tenant or Whiskey-only pilot target is inventory evidence, not completion
proof. The matching in-cluster template is
`deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml`, which
persists its output under the Stage 2 production evidence PVC.

For a narrower Vault/KMS proof, run:

```bash
RUN_STAGE2_SECRET_LIFECYCLE=1 \
./scripts/vault-evidence-gate.sh
```

That gate collects Vault readiness, Vault health, KMS recovery validation, and
KMS rotation evidence into `.mandoforge/vault-evidence/`. It fails closed unless
Vault is healthy, the secret provider is ready, the KMS/HSM backend is external
and ready, rotation evidence is captured and validated against a reported
production backend kind/environment/backend id/key id with a production
rotation id, nonzero rotated record count, nonzero catalog update count, and
unique key/record-level rotation details that include key id, rotation id, catalog update
confirmation, rotated/validated/completed status, and audit id, trace id, run id,
or timestamp detail, and recovery evidence is captured with fresh validated
controller evidence for the same class of production backend identity. Recovery
evidence must also report a production recovery id, production recovery target
kind, and unique audited recovery steps with step names, matching backend/key/recovery
ids, passed/validated/completed status, and audit id, trace id, run id, or
timestamp detail. The matching in-cluster
template is `deploy/stage2-evidence/vault-evidence-job.example.yaml`, which
persists its output under the Stage 2 production evidence PVC.

For a narrower approval-notification proof, run:

```bash
RUN_STAGE2_APPROVAL_DELIVERY=1 \
./scripts/approval-notification-evidence-gate.sh
```

That gate collects approval notification routing, notification run history, deployment validation, ops validation, and optional delivery-run evidence into `.mandoforge/approval-notification-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/approval-notification-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower worker proof, run:

```bash
./scripts/worker-evidence-gate.sh
```

That gate collects worker readiness, runs the bounded worker load-validation endpoint, and writes queue/hardening/autoscaling/load-validation evidence into `.mandoforge/worker-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/worker-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower policy-rollout proof, run:

```bash
RUN_STAGE2_POLICY_DUE_RUN=1 \
./scripts/policy-rollout-evidence-gate.sh
```

That gate collects policy rollout orchestration readiness, validates the
external-controller boundary, and runs due-rollout supervision evidence into
`.mandoforge/policy-rollout-evidence/`. It fails closed unless the production
controller is required, configured, validated, fresh, identifies a production
policy-controller target, confirms a production policy store plus rollback
support, reports production policy store and deployment ids, reports a
production rollback plan/procedure/revision/run id with rollback audit or trace
evidence, emits audited orchestration steps bound to the same controller id,
policy store id, and deployment id with step names, passed/validated/completed
status, and audit id, trace id, run id, or timestamp detail, and is paired with captured
due-run evidence that scanned at least one unique policy revision bound to the
same controller id, policy store id, and deployment id, recorded `checked_at`,
and included audit id, trace id, run id, or timestamp detail on each scanned
revision. Duplicate orchestration step names or duplicate policy/revision scan
rows do not satisfy the reported counts. The matching in-cluster template
is `deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml`, which
persists its output under the Stage 2 production evidence PVC.

For a narrower Codex App Server proof, run:

```bash
RUN_STAGE2_CODEX_STALE_POLL=1 \
./scripts/codex-app-server-evidence-gate.sh
```

That gate collects Codex App Server health, control-plane summary, run/trace inventory, deployment validation, ops validation, and optional stale-poll evidence into `.mandoforge/codex-app-server-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower managed-session restart/resume proof, run:

```bash
RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME=1 \
./scripts/managed-session-runtime-evidence-gate.sh
```

That gate requires `MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL` or `MANAGED_SESSION_RESTART_RESUME_EVIDENCE_FILE` and writes `managed-session-restart-resume-evidence.json`. It fails closed unless the evidence proves a session event was enqueued, a worker drained the loop, API and worker processes restarted, the session resumed with its processed event cursor preserved, thread lineage survived, the runtime turn finalized with the final message intact, and stale-worker lease fencing rejected old finalization attempts. The proof must include concrete session-loop sequence windows, identical before/after processed cursor values that cover the pending event window, matching before/after thread ids, distinct active and stale lease ids with the rejection reason, and a runtime turn id plus final message or final-message artifact. The matching in-cluster template is `deploy/stage2-evidence/managed-session-runtime-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower MCP Gateway proof, run:

```bash
RUN_STAGE2_MCP_DUE_RUN=1 \
RUN_STAGE2_MCP_ROLLBACK=1 \
MANDOFORGE_STAGE2_TEAM_ID=<team_uuid> \
./scripts/mcp-gateway-evidence-gate.sh
```

That gate collects team-scoped MCP rollout summary, rollout run history, deployment validation, optional due-run supervision, and optional rollback evidence into `.mandoforge/mcp-gateway-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/mcp-gateway-evidence-job.example.yaml`, which explicitly enables due-run and rollback proof and persists its output under the Stage 2 production evidence PVC.

For a narrower eval/release proof, run:

```bash
RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 \
RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1 \
./scripts/eval-release-evidence-gate.sh
```

That gate collects agent release rollout summary, release automation history, deployment validation, orchestration validation, optional Stage 2 regression/due-run evidence, and optional rollback evidence into `.mandoforge/eval-release-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/eval-release-evidence-job.example.yaml`, which explicitly enables automation and rollback proof and persists its output under the Stage 2 production evidence PVC.

For a narrower finance proof, run:

```bash
RUN_STAGE2_FINANCE_CONTROLLERS=1 \
RUN_STAGE2_FINANCE_EXPORT=1 \
FINANCE_EXPORT_DELIVERY_OBSERVER_URL=https://controller.example.com/finance/healthz \
./scripts/finance-evidence-gate.sh
```

That gate collects finance dashboard summary, finance operations readiness,
finance close/accounting reconciliation controller evidence, CSV export capture,
export-delivery evidence, and delivery-observer evidence into
`.mandoforge/finance-evidence/`. It fails closed unless close completed through
a configured controller with a close id and unique audited steps that include audit id,
trace id, run id, or timestamp detail, reconciliation is reconciled and fresh
with a reconciliation id and unique audited checks with matching detail, the CSV is nonempty,
delivery succeeded to a configured target, and the observer reports an
accounting/ERP delivery mode such as `accounting_erp`, `erp`, `netsuite`,
`quickbooks`, `xero`, `sap`, or `oracle_erp`; `lark_drive` and `accept_only` do
not satisfy the ERP proof. The observer must also report a stable system id through `system_id`,
`erp_system_id`, `accounting_system_id`, or `target_id`, and ERP/accounting
delivery receipts must include unique receipt id, system id, posted/accepted status,
current export file name and byte count,
record count, and audit id, run id, or posting timestamp detail. The system id must
match `MANDOFORGE_STAGE2_FINANCE_SYSTEM_ID` in the all-up archive manifest. The
matching in-cluster template is
`deploy/stage2-evidence/finance-evidence-job.example.yaml`, which explicitly
enables controller and export proof and persists its output under the Stage 2
production evidence PVC.

The script deliberately skips higher-impact production actions unless explicitly enabled:

- `RUN_STAGE2_SECRET_LIFECYCLE=1` runs the KMS rotation endpoint.
- `RUN_STAGE2_PROVIDER_ROLLOUT=1` runs provider production rollout and rollback endpoints.
- `RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1` runs the Remote Computer sidecar recovery endpoint.
- `RUN_STAGE2_APPROVAL_DELIVERY=1` runs approval notification delivery.
- `RUN_STAGE2_CODEX_STALE_POLL=1` runs Codex App Server stale-run supervision.
- `RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME=1` runs the managed-session restart/resume proof.
- `RUN_STAGE2_MCP_DUE_RUN=1` runs MCP connector due-rollout supervision.
- `RUN_STAGE2_MCP_ROLLBACK=1` runs MCP connector rollback proof.
- `RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1` bootstraps the Stage 2 regression suite and runs due release automation.
- `RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1` runs eval/release rollback proof.
- `RUN_STAGE2_OBSERVABILITY_REMEDIATION=1` runs observability remediation supervision.
- `RUN_STAGE2_FINANCE_CONTROLLERS=1` runs finance close and accounting reconciliation endpoints.
- `RUN_STAGE2_FINANCE_EXPORT=1` runs usage export CSV capture and export delivery proof.
- `RUN_STAGE2_COMPLETION_AUDIT=1` writes `$EVIDENCE_DIR/completion-audit/checklist.json` from the collected evidence.
- `VERIFY_STAGE2_VALIDATION_COVERAGE=1` fails the gate when any declared validation endpoint from `/api/stage2/readiness` is missing from the collected evidence. Leave this off for read-only inventory or partial validation runs.

## Exit Rules

- Exit `0` only when Stage 2 readiness reports no open completion gaps.
- Exit non-zero when the API is unreachable, required tooling is missing, a validation call fails, or Stage 2 remains blocked.
- `ALLOW_BLOCKED=1` is only for evidence inventory; it must not be used to claim Stage 2 completion.

## Evidence

The evidence directory contains one JSON file per endpoint plus `summary.txt`.
It also records declared validation endpoints and any missing validation endpoint evidence.

Useful files:

- `api-stage2-readiness.json`
- `api-tenant-isolation-readiness.json`
- `api-execution-jobs-worker-readiness.json`
- `api-remote-computers-readiness.json`
- `api-observability-collector-readiness.json`
- `validation-declared-endpoints.txt`
- `validation-missing-endpoints.txt`
- `summary.txt`

These artifacts are intentionally local by default because they can include deployment metadata. Review before publishing them into issue trackers or release notes.
