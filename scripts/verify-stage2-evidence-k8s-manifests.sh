#!/usr/bin/env bash
set -euo pipefail

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to verify Stage 2 evidence manifests" >&2
  exit 1
fi

manifests=(
  deploy/stage2-evidence/stage2-controller-env-secret.example.yaml
  deploy/stage2-evidence/stage2-evidence-gate-job.yaml
  deploy/stage2-evidence/stage2-production-evidence-pvc.example.yaml
  deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml
  deploy/stage2-evidence/observability-collector-evidence-job.example.yaml
)
archive_script="scripts/archive-stage2-production-evidence.sh"
observability_script="scripts/observability-collector-evidence-gate.sh"

for manifest in "${manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing Stage 2 evidence manifest: $manifest" >&2
    exit 1
  fi
done

if [[ ! -x "$archive_script" ]]; then
  echo "missing executable Stage 2 production evidence archive script: $archive_script" >&2
  exit 1
fi

if [[ ! -x "$observability_script" ]]; then
  echo "missing executable observability collector evidence script: $observability_script" >&2
  exit 1
fi

kubectl kustomize deploy/stage2-evidence >/tmp/mandoforge-stage2-evidence-kustomize.out
kubectl kustomize deploy/stage2-production-evidence --load-restrictor LoadRestrictionsNone \
  >/tmp/mandoforge-stage2-production-evidence-kustomize.out

if [[ ! -s /tmp/mandoforge-stage2-evidence-kustomize.out ]]; then
  echo "Stage 2 evidence kustomize render produced no output" >&2
  exit 1
fi

if [[ ! -s /tmp/mandoforge-stage2-production-evidence-kustomize.out ]]; then
  echo "Stage 2 production evidence kustomize render produced no output" >&2
  exit 1
fi

if ! grep -q "kind: Job" /tmp/mandoforge-stage2-evidence-kustomize.out; then
  echo "Stage 2 evidence kustomize render is missing a Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence-gate" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the strict production Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-observability-collector-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the observability collector evidence Job" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the persistent evidence PVC mount" >&2
  exit 1
fi

if ! grep -q "kind: Secret" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml; then
  echo "Stage 2 controller env example is not a Kubernetes Secret" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-controller-env" deploy/stage2-evidence/stage2-controller-env-secret.example.yaml; then
  echo "Stage 2 controller env Secret example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence-gate" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-production-evidence" deploy/stage2-evidence/stage2-production-evidence-pvc.example.yaml; then
  echo "Stage 2 production evidence PVC example has the wrong name" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-stage2-controller-env" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example does not consume the controller env Secret" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/stage2-production-evidence-gate-job.example.yaml; then
  echo "Stage 2 production evidence Job example does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "observability-collector-evidence-gate.sh" deploy/stage2-evidence/observability-collector-evidence-job.example.yaml; then
  echo "Observability collector evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/observability-collector-evidence-job.example.yaml; then
  echo "Observability collector evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/observability/collector/cluster/validate" "$observability_script"; then
  echo "Observability collector evidence script must validate cluster rollout" >&2
  exit 1
fi

if ! grep -q "MANDOFORGE_STAGE2_EVIDENCE_PVC:-mandoforge-stage2-production-evidence" "$archive_script"; then
  echo "Stage 2 production evidence archive script does not default to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "readOnly: true" "$archive_script"; then
  echo "Stage 2 production evidence archive script must mount the evidence PVC read-only" >&2
  exit 1
fi

if ! grep -q "archive_sha256" "$archive_script"; then
  echo "Stage 2 production evidence archive script must write checksum metadata" >&2
  exit 1
fi

if ! grep -q "manifest_file=" "$archive_script"; then
  echo "Stage 2 production evidence archive script must write a release manifest" >&2
  exit 1
fi

echo "stage2 evidence k8s manifests ok"
