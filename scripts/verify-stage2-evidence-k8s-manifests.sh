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
  deploy/stage2-evidence/remote-computer-evidence-job.example.yaml
  deploy/stage2-evidence/provider-governance-evidence-job.example.yaml
  deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml
  deploy/stage2-evidence/vault-evidence-job.example.yaml
)
archive_script="scripts/archive-stage2-production-evidence.sh"
observability_script="scripts/observability-collector-evidence-gate.sh"
remote_computer_script="scripts/remote-computer-evidence-gate.sh"
provider_script="scripts/provider-governance-evidence-gate.sh"
tenant_script="scripts/tenant-isolation-evidence-gate.sh"
vault_script="scripts/vault-evidence-gate.sh"

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

if [[ ! -x "$remote_computer_script" ]]; then
  echo "missing executable Remote Computer evidence script: $remote_computer_script" >&2
  exit 1
fi

if [[ ! -x "$provider_script" ]]; then
  echo "missing provider governance evidence script: $provider_script" >&2
  exit 1
fi

if [[ ! -x "$tenant_script" ]]; then
  echo "missing tenant isolation evidence script: $tenant_script" >&2
  exit 1
fi

if [[ ! -x "$vault_script" ]]; then
  echo "missing Vault evidence script: $vault_script" >&2
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

if ! grep -q "name: mandoforge-remote-computer-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the Remote Computer evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-provider-governance-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the provider governance evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-tenant-isolation-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the tenant isolation evidence Job" >&2
  exit 1
fi

if ! grep -q "name: mandoforge-vault-evidence" /tmp/mandoforge-stage2-production-evidence-kustomize.out; then
  echo "Stage 2 production evidence kustomize render is missing the Vault evidence Job" >&2
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

if ! grep -q "remote-computer-evidence-gate.sh" deploy/stage2-evidence/remote-computer-evidence-job.example.yaml; then
  echo "Remote Computer evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/remote-computer-evidence-job.example.yaml; then
  echo "Remote Computer evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/remote-computers/state-sync/validate" "$remote_computer_script"; then
  echo "Remote Computer evidence script must validate production state sync" >&2
  exit 1
fi

if ! grep -q "/api/remote-computers/sidecars/recovery/run" "$remote_computer_script"; then
  echo "Remote Computer evidence script must capture sidecar recovery evidence" >&2
  exit 1
fi

if ! grep -q "provider-governance-evidence-gate.sh" deploy/stage2-evidence/provider-governance-evidence-job.example.yaml; then
  echo "Provider governance evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/provider-governance-evidence-job.example.yaml; then
  echo "Provider governance evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/providers/deployment/validate" "$provider_script"; then
  echo "Provider governance evidence script must validate provider deployment" >&2
  exit 1
fi

if ! grep -q "/api/providers/production-rollout/run" "$provider_script"; then
  echo "Provider governance evidence script must capture provider rollout evidence" >&2
  exit 1
fi

if ! grep -q "tenant-isolation-evidence-gate.sh" deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml; then
  echo "Tenant isolation evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/tenant-isolation-evidence-job.example.yaml; then
  echo "Tenant isolation evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/tenant-isolation/routing/validate" "$tenant_script"; then
  echo "Tenant isolation evidence script must validate tenant production routing" >&2
  exit 1
fi

if ! grep -q "vault-evidence-gate.sh" deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job does not run the dedicated evidence gate" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-stage2-production-evidence" deploy/stage2-evidence/vault-evidence-job.example.yaml; then
  echo "Vault evidence Job does not persist evidence to the production evidence PVC" >&2
  exit 1
fi

if ! grep -q "/api/vault/kms/recovery/validate" "$vault_script"; then
  echo "Vault evidence script must validate KMS recovery readiness" >&2
  exit 1
fi

if ! grep -q "/api/vault/kms/rotation/run" "$vault_script"; then
  echo "Vault evidence script must capture KMS rotation evidence" >&2
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
