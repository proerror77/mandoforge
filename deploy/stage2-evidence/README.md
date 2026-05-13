# Stage 2 Evidence Gates

This directory contains Kubernetes entry points for proving the Stage 2 governed-runtime gates. They are evidence collectors, not a substitute for the strict completion audit in `docs/stage2-completion-audit.md`.

Stage 2 is complete only when `/api/stage2/readiness` reports no open gaps and the strict production gate collects controller-backed evidence from real targets.

## Files

- `stage2-evidence-gate-job.yaml` runs the default blocked inventory gate. It keeps `ALLOW_BLOCKED=1` and leaves production validations disabled, so it is useful for collecting current readiness evidence without claiming completion.
- `stage2-production-controllers.env.example` lists the controller and KMS environment contract required for strict production validation. It intentionally contains placeholder URLs and empty token/team/key values.
- `stage2-controller-env-secret.example.yaml` is the Kubernetes Secret shape for those controller settings. Replace placeholders through your secret manager or deployment pipeline; do not commit real values.
- `stage2-production-evidence-pvc.example.yaml` is the persistent evidence volume shape for strict production runs.
- `stage2-production-evidence-gate-job.example.yaml` runs the strict production gate and reads validation flags from `mandoforge-stage2-controller-env`.

## Local Manifest Verification

Run this before changing the evidence manifests:

```bash
scripts/verify-stage2-controller-env-template.sh
scripts/verify-stage2-evidence-k8s-manifests.sh
```

The second script performs client-side dry-run validation for the Secret, default evidence Job, strict production Job, and `kubectl kustomize deploy/stage2-evidence`.

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
kubectl apply -n agent-os -f deploy/stage2-evidence/stage2-controller-env-secret.example.yaml
kubectl apply -n agent-os -f deploy/stage2-evidence/stage2-production-evidence-pvc.example.yaml
kubectl create -n agent-os -f deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml
kubectl wait --for=condition=complete job/mandoforge-stage2-production-evidence-gate -n agent-os --timeout=30m
kubectl logs job/mandoforge-stage2-production-evidence-gate -n agent-os
```

For a real run, create `mandoforge-stage2-controller-env` from the same keys but with production controller URLs, tokens, team id, and KMS settings supplied outside git. The strict Job sets `ALLOW_BLOCKED=0` through that Secret and fails closed if required controller evidence is missing, stale, or unhealthy. Keep the `mandoforge-stage2-production-evidence` PVC until the evidence directory has been archived with the release record.

## Completion Rule

Do not mark Stage 2 complete from manifest render success, dry-run success, or the blocked inventory Job. Completion requires strict production evidence for every open gap listed in `docs/stage2-completion-audit.md`.
