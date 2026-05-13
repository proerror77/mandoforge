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

The script deliberately skips higher-impact production actions unless explicitly enabled:

- `RUN_STAGE2_SECRET_LIFECYCLE=1` runs the KMS rotation endpoint.
- `RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1` runs the Remote Computer sidecar recovery endpoint.
- `RUN_STAGE2_FINANCE_CONTROLLERS=1` runs finance close and accounting reconciliation endpoints.

## Exit Rules

- Exit `0` only when Stage 2 readiness reports no open completion gaps.
- Exit non-zero when the API is unreachable, required tooling is missing, a validation call fails, or Stage 2 remains blocked.
- `ALLOW_BLOCKED=1` is only for evidence inventory; it must not be used to claim Stage 2 completion.

## Evidence

The evidence directory contains one JSON file per endpoint plus `summary.txt`.

Useful files:

- `api-stage2-readiness.json`
- `api-tenant-isolation-readiness.json`
- `api-execution-jobs-worker-readiness.json`
- `api-remote-computers-readiness.json`
- `api-observability-collector-readiness.json`
- `summary.txt`

These artifacts are intentionally local by default because they can include deployment metadata. Review before publishing them into issue trackers or release notes.
