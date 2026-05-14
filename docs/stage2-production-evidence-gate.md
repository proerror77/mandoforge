# Stage 2 Production Evidence Gate

Stage 2 is intentionally fail-closed: green unit tests, static UI smoke checks, and local readiness panels do not prove the governed runtime has been exercised against real production targets.

Use `scripts/stage2-production-evidence-gate.sh` as the operator gate for that proof. It collects machine-readable readiness evidence into `.mandoforge/stage2-production-evidence/` and exits non-zero while `GET /api/stage2/readiness` reports open completion gaps.

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

This mode calls the bounded validation endpoints for tenant routing, provider deployment, policy rollout orchestration, Vault recovery, worker load validation, Remote Computer state sync, approval notifications, Codex App Server, agent release deployment/orchestration, observability collector deployment/cluster rollout, and team-scoped MCP connector rollout.

Use `deploy/stage2-evidence/stage2-production-controllers.env.example` as the operator checklist for the external controller URLs, required flags, and opt-in validation switches. `deploy/stage2-evidence/stage2-controller-env-secret.example.yaml` shows the matching Kubernetes Secret shape, and `deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml` shows the strict production-validation Job that consumes that Secret through `envFrom`. These are templates only; real URLs and tokens belong in your secret manager, CI environment, or Kubernetes Secret generation pipeline.

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

That gate collects `GET /api/remote-computers/readiness`, `GET /api/remote-computers/runner/readiness`, `POST /api/remote-computers/state-sync/validate`, and optional sidecar recovery evidence into `.mandoforge/remote-computer-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/remote-computer-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

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

That gate collects tenant isolation readiness and audited production routing validation evidence into `.mandoforge/tenant-isolation-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower Vault/KMS proof, run:

```bash
RUN_STAGE2_SECRET_LIFECYCLE=1 \
./scripts/vault-evidence-gate.sh
```

That gate collects Vault readiness, Vault health, KMS recovery validation, and optional KMS rotation evidence into `.mandoforge/vault-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/vault-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

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

That gate collects policy rollout orchestration readiness, validates the external-controller boundary, and optionally runs due-rollout supervision evidence into `.mandoforge/policy-rollout-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/policy-rollout-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

For a narrower Codex App Server proof, run:

```bash
RUN_STAGE2_CODEX_STALE_POLL=1 \
./scripts/codex-app-server-evidence-gate.sh
```

That gate collects Codex App Server health, control-plane summary, run/trace inventory, deployment validation, ops validation, and optional stale-poll evidence into `.mandoforge/codex-app-server-evidence/`. The matching in-cluster template is `deploy/stage2-evidence/codex-app-server-evidence-job.example.yaml`, which persists its output under the Stage 2 production evidence PVC.

The script deliberately skips higher-impact production actions unless explicitly enabled:

- `RUN_STAGE2_SECRET_LIFECYCLE=1` runs the KMS rotation endpoint.
- `RUN_STAGE2_PROVIDER_ROLLOUT=1` runs provider production rollout and rollback endpoints.
- `RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1` runs the Remote Computer sidecar recovery endpoint.
- `RUN_STAGE2_APPROVAL_DELIVERY=1` runs approval notification delivery.
- `RUN_STAGE2_CODEX_STALE_POLL=1` runs Codex App Server stale-run supervision.
- `RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1` bootstraps the Stage 2 regression suite and runs due release automation.
- `RUN_STAGE2_OBSERVABILITY_REMEDIATION=1` runs observability remediation supervision.
- `RUN_STAGE2_FINANCE_CONTROLLERS=1` runs finance close and accounting reconciliation endpoints.
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
