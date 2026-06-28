#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/production-launch-preflight}"
STATIC_ONLY="${STATIC_ONLY:-0}"
CUSTOMER_DATA_WAIVED="${CUSTOMER_DATA_WAIVED:-0}"

mkdir -p "$EVIDENCE_DIR"

fail() {
  echo "production launch preflight failed: $*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || fail "missing required file: $1"
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable gate: $1"
}

require_file deploy/k8s/kustomization.yaml
require_file deploy/k8s/configmap.yaml
require_file deploy/k8s/api.yaml
require_file deploy/k8s/workspace-pvc.yaml
require_file deploy/k8s/secret.example.yaml
require_file deploy/k8s/secret-delivery-contract.yaml

if grep -Eq '(^|[[:space:]-])secret\.example\.yaml([[:space:]]|$)' deploy/k8s/kustomization.yaml; then
  fail "deploy/k8s/kustomization.yaml must not apply secret.example.yaml"
fi

if grep -Eq 'POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@' deploy/k8s/secret.example.yaml; then
  fail "deploy/k8s/secret.example.yaml must not contain default Postgres credentials"
fi

if ! grep -q 'MANDOFORGE_SECRET_DELIVERY_REQUIRED: "true"' deploy/k8s/secret-delivery-contract.yaml \
  || ! grep -q 'MANDOFORGE_SECRET_NAME: "mandoforge-secrets"' deploy/k8s/secret-delivery-contract.yaml \
  || ! grep -q 'MANDOFORGE_SECRET_MUST_NOT_BE_EXAMPLE: "true"' deploy/k8s/secret-delivery-contract.yaml; then
  fail "deploy/k8s/secret-delivery-contract.yaml must declare external mandoforge-secrets delivery"
fi

if grep -Eq 'MANDOFORGE_INSECURE_DEV_AUTH:[[:space:]]*"?(true|1)"?' deploy/k8s/configmap.yaml; then
  fail "K8s config must not enable insecure dev auth"
fi

if grep -Eq 'MANDOFORGE_TRUST_X_MANDOFORGE_SUBJECT:[[:space:]]*"?(true|1)"?' deploy/k8s/configmap.yaml; then
  fail "K8s config must not enable MANDOFORGE_TRUST_X_MANDOFORGE_SUBJECT (allows header-based identity spoofing)"
fi

if grep -Eq 'TRUSTED_TENANT_ID:[[:space:]]*"?(true|1|[^"[:space:]]+)"?' deploy/k8s/configmap.yaml; then
  fail "K8s config must not set TRUSTED_TENANT_ID (allows caller to spoof tenant identity)"
fi

if grep -Eq 'MANDOFORGE_ALLOW_(HOST|INLINE)_SHELL_EXEC:[[:space:]]*"?(true|1)"?' deploy/k8s/configmap.yaml; then
  fail "K8s config must not enable host or inline shell execution"
fi

if ! grep -Eq 'MANDOFORGE_PROVIDER_RUNTIME_ENV:[[:space:]]*"production"' deploy/k8s/configmap.yaml; then
  fail "K8s config must force provider runtime production mode"
fi

if ! grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_RUNNER:[[:space:]]*"kubernetes"' deploy/k8s/configmap.yaml; then
  fail "K8s config must route Remote Computer runner to kubernetes"
fi

if ! grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT:[[:space:]]*"kubernetes"' deploy/k8s/configmap.yaml; then
  fail "K8s config must route Remote Computer execution transport to kubernetes"
fi

if ! grep -q 'claimName: mandoforge-workspaces' deploy/k8s/api.yaml; then
  fail "API workspace volume must use the mandoforge-workspaces PVC"
fi

if grep -q 'emptyDir: {}' deploy/k8s/api.yaml; then
  fail "API workspace volume must not use emptyDir"
fi

if command -v kubectl >/dev/null 2>&1; then
  render_file="$EVIDENCE_DIR/deploy-k8s-render.yaml"
  kubectl kustomize deploy/k8s >"$render_file"
  [[ -s "$render_file" ]] || fail "kubectl kustomize deploy/k8s produced no output"
  if grep -Eq 'kind:[[:space:]]*Secret|POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@' "$render_file"; then
    fail "rendered deploy/k8s output must not include example Secrets or default credentials"
  fi
  if ! grep -q 'name: mandoforge-secret-delivery-contract' "$render_file"; then
    fail "rendered deploy/k8s output must include the secret delivery contract"
  fi
  if ! grep -q 'MANDOFORGE_PROVIDER_RUNTIME_ENV: production' "$render_file"; then
    fail "rendered deploy/k8s output must force provider runtime production mode"
  fi
  if ! grep -q 'claimName: mandoforge-workspaces' "$render_file"; then
    fail "rendered deploy/k8s output must mount the workspace PVC"
  fi
else
  echo "kubectl not found; skipped kustomize render validation" >&2
fi

gates=(
  scripts/runtime-production-readiness-gate.sh
  scripts/remote-computer-production-state-gate.sh
  scripts/stage2-production-evidence-gate.sh
  scripts/enterprise-product-readiness-gate.sh
  scripts/remote-computer-evidence-gate.sh
  scripts/native-connector-production-readiness-gate.sh
  scripts/live-connector-production-semantics-gate.sh
  scripts/ontology-release-workflow-trigger-gate.sh
  scripts/ontology-engine-readiness-gate.sh
  scripts/enterprise-security-admin-readiness-gate.sh
  scripts/enterprise-security-production-controls-gate.sh
  scripts/observability-collector-evidence-gate.sh
  scripts/observability-ops-production-gate.sh
  scripts/product-surfaces-production-gate.sh
)

for gate in "${gates[@]}"; do
  require_executable "$gate"
done

if [[ "$STATIC_ONLY" == "1" ]]; then
  for gate in \
    scripts/runtime-production-readiness-gate.sh \
    scripts/remote-computer-production-state-gate.sh \
    scripts/live-connector-production-semantics-gate.sh \
    scripts/ontology-release-workflow-trigger-gate.sh \
    scripts/enterprise-security-production-controls-gate.sh \
    scripts/observability-ops-production-gate.sh \
    scripts/product-surfaces-production-gate.sh; do
    gate_name="$(basename "$gate" .sh)"
    gate_dir="$EVIDENCE_DIR/$gate_name"
    echo "running static $gate"
    STATIC_ONLY=1 EVIDENCE_DIR="$gate_dir" "$gate"
  done
fi

{
  echo "static_deploy_config=passed"
  echo "secret_delivery_contract=static_present"
  echo "customer_data_waived=$CUSTOMER_DATA_WAIVED"
  echo "static_only=$STATIC_ONLY"
  echo "base_url=$BASE_URL"
} >"$EVIDENCE_DIR/summary.txt"

if [[ "$STATIC_ONLY" == "1" ]]; then
  cat "$EVIDENCE_DIR/summary.txt"
  echo "production launch static preflight ok"
  exit 0
fi

if [[ "$CUSTOMER_DATA_WAIVED" != "1" ]]; then
  fail "set CUSTOMER_DATA_WAIVED=1 only when real customer-data validation is explicitly outside launch scope"
fi

for gate in "${gates[@]}"; do
  gate_name="$(basename "$gate" .sh)"
  gate_dir="$EVIDENCE_DIR/$gate_name"
  echo "running $gate"
  BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" ALLOW_BLOCKED=0 "$gate"
done

cat "$EVIDENCE_DIR/summary.txt"
echo "production launch preflight ok"
