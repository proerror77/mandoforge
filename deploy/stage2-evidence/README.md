# Stage 2 Evidence Gates

This directory contains Kubernetes entry points for proving the Stage 2 governed-runtime gates. They are evidence collectors, not a substitute for the strict completion audit in `docs/stage2-completion-audit.md`.

Stage 2 is complete only when `/api/stage2/readiness` reports no open gaps and the strict production gate collects controller-backed evidence from real targets.

## Files

- `stage2-evidence-gate-job.yaml` runs the default blocked inventory gate. It keeps `ALLOW_BLOCKED=1` and leaves production validations disabled, so it is useful for collecting current readiness evidence without claiming completion.
- `stage2-production-controllers.env.example` lists the controller, KMS, and explicit evidence opt-in flags required for strict production validation. It intentionally contains placeholder URLs and empty token/key values; `MANDOFORGE_STAGE2_TEAM_ID` may stay empty so evidence gates can auto-discover the first active team.
- `stage2-controller-env-secret.example.yaml` is the Kubernetes Secret shape for those controller settings. Replace placeholders through your secret manager or deployment pipeline; do not commit real values.
- `stage2-production-evidence-pvc.example.yaml` is the persistent evidence volume shape for strict production runs.
- `stage2-production-evidence-gate-job.example.yaml` runs the strict production gate and reads validation flags from `mandoforge-stage2-controller-env`.
- `../../scripts/render-stage2-controller-secret.sh` renders the real controller Secret from an env file and runs the strict production evidence preflight first unless `ALLOW_STAGE2_CONTROLLER_PLACEHOLDERS=1` is set.
- `../../scripts/stage2-production-evidence-preflight.sh` checks a real env file before rendering the Secret. It validates that the still-open external production backlog points at non-placeholder, non-pilot controller targets and production KMS backend/key identities without printing token values.

## Local Manifest Verification

Run this before changing the evidence manifests:

```bash
scripts/verify-stage2-controller-env-template.sh
scripts/verify-stage2-evidence-k8s-manifests.sh
```

The second script performs offline validation for the Secret shape, default evidence Job, strict production Job, default evidence kustomize render, and strict production evidence kustomize render.

## Blocked Inventory Run

Use the default kustomize target to collect an inventory in a non-production or incomplete environment:

```bash
kubectl create namespace agent-os --dry-run=client -o yaml | kubectl apply -f -
kubectl apply -k deploy/stage2-evidence
kubectl wait --for=condition=complete job/mandoforge-stage2-evidence-gate -n agent-os --timeout=10m
kubectl logs job/mandoforge-stage2-evidence-gate -n agent-os
```

This run can succeed while Stage 2 remains blocked because it is designed to document open gaps.

## Strict Production Run

Use this only after real validation controllers and credentials are configured:

```bash
kubectl create namespace agent-os --dry-run=client -o yaml | kubectl apply -f -
scripts/stage2-production-evidence-preflight.sh /secure/path/stage2-production-controllers.env
scripts/render-stage2-controller-secret.sh /secure/path/stage2-production-controllers.env | kubectl apply -f -
kubectl kustomize deploy/stage2-production-evidence --load-restrictor LoadRestrictionsNone | kubectl apply -f -
kubectl wait --for=condition=complete job/mandoforge-stage2-production-evidence-gate -n agent-os --timeout=30m
kubectl logs job/mandoforge-stage2-production-evidence-gate -n agent-os
scripts/archive-stage2-production-evidence.sh .mandoforge/stage2-production-evidence-$(date -u +%Y%m%dT%H%M%SZ).tar.gz
scripts/verify-stage2-evidence-archive.sh .mandoforge/stage2-production-evidence-YYYYMMDDTHHMMSSZ.tar.gz
```

For a real run, create `/secure/path/stage2-production-controllers.env` from the same keys but with production controller URLs, tokens, KMS settings, target identity values, and explicit evidence opt-in flags supplied outside git. Set `MANDOFORGE_STAGE2_TEAM_ID` only when you want to pin MCP evidence to a specific team; otherwise the evidence gates discover the first active team through the governance APIs and write `team-discovery.json`. The render script refuses `example.com` placeholders, empty token/key values, and pilot/mock/local target identities by default. The strict Job writes `production-evidence-run.json`, sets `ALLOW_BLOCKED=0` through that Secret, and fails closed if required controller evidence is missing, stale, unhealthy, or does not match the declared cluster, worker pool, Remote Computer state claim, Remote Computer state backend, tenant deployment, expected tenant RLS table set, policy controller, KMS backend/key, ERP system, and managed-session runtime target identities. Finance system identities must name a real ERP/accounting target rather than Feishu/Lark/Drive/file/artifact delivery. Keep the `mandoforge-stage2-production-evidence` PVC until the evidence directory has been archived with the release record, including the generated `.sha256` checksum and `.manifest.txt` release manifest.

The production kustomize bundle references the reviewed PVC and Job examples under `deploy/stage2-evidence`, so `kubectl kustomize` needs `--load-restrictor LoadRestrictionsNone`. It does not include the example Secret; render and apply the real Secret first.

## Completion Rule

Do not mark Stage 2 complete from manifest render success, dry-run success, or the blocked inventory Job. Completion requires strict production evidence for every open gap listed in `docs/stage2-completion-audit.md`.
