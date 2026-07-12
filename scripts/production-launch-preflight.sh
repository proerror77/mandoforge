#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/production-launch-preflight}"
STATIC_ONLY="${STATIC_ONLY:-0}"
CUSTOMER_DATA_WAIVED="${CUSTOMER_DATA_WAIVED:-0}"
STAGE2_CONTROLLER_ENV_FILE="${STAGE2_CONTROLLER_ENV_FILE:-}"

mkdir -p "$EVIDENCE_DIR"

stage2_env_snapshot=""

cleanup() {
  if [[ -n "$stage2_env_snapshot" ]]; then
    rm -f "$stage2_env_snapshot"
  fi
}

trap cleanup EXIT

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

trim() {
  printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

load_stage2_controller_env_file() {
  local env_file="$1"
  local line
  local name
  local value
  local line_number=0

  while IFS= read -r line || [[ -n "$line" ]]; do
    line_number=$((line_number + 1))
    line="$(trim "$line")"
    [[ -z "$line" || "$line" == \#* ]] && continue
    if [[ "$line" == export[[:space:]]* ]]; then
      line="$(trim "${line#export}")"
    fi
    [[ "$line" == *=* ]] || fail "$env_file:$line_number must be KEY=value"
    name="$(trim "${line%%=*}")"
    value="${line#*=}"
    if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      fail "$env_file:$line_number has invalid env var name: ${name:-<empty>}"
    fi
    if [[ "$value" =~ ^\".*\"$ || "$value" =~ ^\'.*\'$ ]]; then
      value="${value:1:${#value}-2}"
    fi
    export "$name=$value"
  done <"$env_file"
}

require_file deploy/k8s/kustomization.yaml
require_file deploy/k8s/configmap.yaml
require_file deploy/k8s/api.yaml
require_file deploy/k8s/api-serviceaccount.yaml
require_file deploy/k8s/api-agent-sandbox-rbac.yaml
require_file deploy/k8s/agent-sandbox-controller-contract.yaml
require_file deploy/k8s/agent-sandbox-runtime.yaml
require_file deploy/k8s/agent-sandbox-egress-networkpolicy.yaml
require_file deploy/k8s/worker-isolated-pool-networkpolicy.yaml
require_file deploy/k8s/workspace-pvc.yaml
require_file deploy/k8s/stage2-production-evidence-pvc.yaml
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

if ! grep -Eq 'MANDOFORGE_AGENT_RELEASE_ENVIRONMENT:[[:space:]]*"production"' deploy/k8s/configmap.yaml; then
  fail "K8s config must bind production sessions to production agent releases"
fi

if ! grep -Eq 'MANDOFORGE_AGENT_RELEASE_ENFORCEMENT:[[:space:]]*"required"' deploy/k8s/configmap.yaml; then
  fail "K8s config must require the agent release execution gate"
fi

for controller_flag in \
  MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED \
  MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED \
  MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED \
  MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED; do
  if ! grep -Eq "${controller_flag}:[[:space:]]*\"true\"" deploy/k8s/configmap.yaml; then
    fail "K8s config must require ${controller_flag}"
  fi
done

if ! grep -Eq 'MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR:[[:space:]]*"/evidence"' deploy/k8s/configmap.yaml; then
  fail "K8s config must point enterprise readiness at the production evidence PVC mount"
fi

if ! grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_RUNNER:[[:space:]]*"agent-sandbox"' deploy/k8s/configmap.yaml; then
  fail "K8s config must route Remote Computer lifecycle to Agent Sandbox"
fi

if ! grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT:[[:space:]]*"kubernetes"' deploy/k8s/configmap.yaml; then
  fail "K8s config must route Remote Computer execution transport to kubernetes"
fi

if ! grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_TEMPLATE_PATH:[[:space:]]*"deploy/k8s/agent-sandbox-runtime.yaml"' deploy/k8s/configmap.yaml; then
  fail "K8s config must not retain the legacy direct-Pod template path"
fi

for disabled_flag in \
  MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED \
  MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED \
  MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED; do
  if ! grep -Eq "${disabled_flag}:[[:space:]]*\"false\"" deploy/k8s/configmap.yaml; then
    fail "checked-in production config must keep ${disabled_flag} fail-closed"
  fi
done

for resource in \
  agent-sandbox-controller-contract.yaml \
  api-serviceaccount.yaml \
  api-agent-sandbox-rbac.yaml \
  agent-sandbox-runtime.yaml \
  agent-sandbox-egress-networkpolicy.yaml; do
  grep -q "$resource" deploy/k8s/kustomization.yaml \
    || fail "deploy/k8s default kustomization must include $resource"
done

if grep -q 'agent-remote-computer.yaml' deploy/k8s/kustomization.yaml; then
  fail "deploy/k8s default kustomization must not include the legacy direct-Pod runtime"
fi

if ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CONTROLLER_VERSION: "v0.5.1"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_API: "extensions.agents.x-k8s.io/v1beta1"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_ASSET: "manifest.yaml"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CORE_INSTALL_SHA256: "8cfdf0a878f66b91d2e7103e77859d1412d850ce3f5fe5c3fa134c36bd55504a"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_ASSET: "extensions.yaml"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_EXTENSIONS_INSTALL_SHA256: "7c22b450e24ede3fddbcd5ae0ee7c78ea102d6c30635ff860cc486578a55932e"' deploy/k8s/agent-sandbox-controller-contract.yaml \
  || ! grep -q 'MANDOFORGE_AGENT_SANDBOX_CONTROLLER_REQUIRED: "true"' deploy/k8s/agent-sandbox-controller-contract.yaml; then
  fail "Agent Sandbox controller contract must pin v0.5.1, v1beta1, and both release asset digests"
fi

if ! grep -q 'serviceAccountName: mandoforge-api' deploy/k8s/api.yaml \
  || ! grep -q 'name: mandoforge-agent-sandbox-controller-contract' deploy/k8s/api.yaml \
  || ! grep -q 'mountPath: /var/run/secrets/kubernetes.io/serviceaccount' deploy/k8s/api.yaml \
  || ! grep -q 'name: mandoforge-api-agent-sandbox' deploy/k8s/api-agent-sandbox-rbac.yaml; then
  fail "API deployment must own Agent Sandbox access through scoped identity and RBAC"
fi

if grep -A1 'resources: \["pods"\]' deploy/k8s/api-agent-sandbox-rbac.yaml | grep -Eq '"create"|"delete"'; then
  fail "default API RBAC must not permit legacy direct Pod creation or deletion"
fi

if grep -q 'mountPath: /var/run/secrets/kubernetes.io/serviceaccount' deploy/k8s/worker.yaml \
  || grep -q 'mountPath: /var/run/secrets/kubernetes.io/serviceaccount' deploy/k8s/worker-isolated-pool.yaml; then
  fail "queue workers must not receive Kubernetes API credentials"
fi

if grep -q 'app: agent-remote-computer' deploy/k8s/worker-isolated-pool-networkpolicy.yaml \
  || grep -q 'port: 8080' deploy/k8s/worker-isolated-pool-networkpolicy.yaml; then
  fail "queue workers must not retain a direct runtime network path"
fi

if ! grep -q 'claimName: mandoforge-workspaces' deploy/k8s/api.yaml; then
  fail "API workspace volume must use the mandoforge-workspaces PVC"
fi

if ! grep -q 'stage2-production-evidence-pvc.yaml' deploy/k8s/kustomization.yaml \
  || ! grep -q 'mountPath: /evidence' deploy/k8s/api.yaml \
  || ! grep -q 'readOnly: true' deploy/k8s/api.yaml \
  || ! grep -q 'claimName: mandoforge-stage2-production-evidence' deploy/k8s/api.yaml; then
  fail "API deployment must mount the Stage 2 production evidence PVC read-only"
fi

if grep -q 'emptyDir: {}' deploy/k8s/api.yaml; then
  fail "API workspace volume must not use emptyDir"
fi

if command -v kubectl >/dev/null 2>&1; then
  render_file="$EVIDENCE_DIR/deploy-k8s-render.yaml"
  root_render_file="$EVIDENCE_DIR/deploy-root-render.yaml"
  kubectl kustomize deploy/k8s >"$render_file"
  kubectl kustomize deploy >"$root_render_file"
  [[ -s "$render_file" ]] || fail "kubectl kustomize deploy/k8s produced no output"
  [[ -s "$root_render_file" ]] || fail "kubectl kustomize deploy produced no output"
  if grep -Eq 'kind:[[:space:]]*Secret|POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@' "$render_file"; then
    fail "rendered deploy/k8s output must not include example Secrets or default credentials"
  fi
  if grep -Eq 'kind:[[:space:]]*Secret|POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@|replace-me|s3\.example\.com' "$root_render_file"; then
    fail "rendered deploy root output must not include example Secrets, placeholder storage credentials, or default credentials"
  fi
  if ! grep -q 'name: mandoforge-secret-delivery-contract' "$render_file"; then
    fail "rendered deploy/k8s output must include the secret delivery contract"
  fi
  if ! grep -q 'MANDOFORGE_PROVIDER_RUNTIME_ENV: production' "$render_file"; then
    fail "rendered deploy/k8s output must force provider runtime production mode"
  fi
  for controller_flag in \
    MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED; do
    if ! grep -Eq "${controller_flag}:[[:space:]]*\"?true\"?" "$render_file"; then
      fail "rendered deploy/k8s output must require ${controller_flag}"
    fi
  done
  if ! grep -q 'MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR: /evidence' "$render_file" \
    || ! grep -q 'mountPath: /evidence' "$render_file" \
    || ! grep -q 'readOnly: true' "$render_file" \
    || ! grep -q 'claimName: mandoforge-stage2-production-evidence' "$render_file"; then
    fail "rendered deploy/k8s output must expose Stage 2 production evidence to the API read-only"
  fi
  if ! grep -q 'claimName: mandoforge-workspaces' "$render_file"; then
    fail "rendered deploy/k8s output must mount the workspace PVC"
  fi
  if ! grep -q 'MANDOFORGE_REMOTE_COMPUTER_RUNNER: agent-sandbox' "$render_file" \
    || ! grep -q 'kind: SandboxTemplate' "$render_file" \
    || ! grep -q 'kind: SandboxWarmPool' "$render_file" \
    || ! grep -q 'name: mandoforge-agent-sandbox-egress' "$render_file"; then
    fail "rendered deploy/k8s output must select and provision the Agent Sandbox substrate"
  fi
  if ! grep -q 'serviceAccountName: mandoforge-api' "$render_file" \
    || ! grep -q 'name: mandoforge-api-agent-sandbox' "$render_file"; then
    fail "rendered deploy/k8s output must include scoped API Agent Sandbox access"
  fi
  if grep -q 'name: mandoforge-agent-remote-computer-template' "$render_file"; then
    fail "rendered deploy/k8s output must exclude the legacy direct-Pod runtime"
  fi
else
  echo "kubectl not found; skipped kustomize render validation" >&2
fi

gates=(
  scripts/stage2-production-evidence-gate.sh
  scripts/runtime-production-readiness-gate.sh
  scripts/production-deployment-safety-gate.sh
  scripts/remote-computer-production-state-gate.sh
  scripts/remote-computer-evidence-gate.sh
  scripts/native-connector-production-readiness-gate.sh
  scripts/live-connector-production-semantics-gate.sh
  scripts/ontology-engine-production-gate.sh
  scripts/ontology-release-workflow-trigger-gate.sh
  scripts/ontology-engine-readiness-gate.sh
  scripts/enterprise-security-admin-readiness-gate.sh
  scripts/enterprise-security-production-controls-gate.sh
  scripts/observability-collector-evidence-gate.sh
  scripts/observability-ops-production-gate.sh
  scripts/product-surfaces-production-gate.sh
  scripts/workflowpack-enterprise-lifecycle-gate.sh
  scripts/enterprise-product-completion-contract-gate.sh
  scripts/enterprise-product-readiness-gate.sh
)

for gate in "${gates[@]}"; do
  require_executable "$gate"
done
require_executable scripts/stage2-production-evidence-preflight.sh

if [[ "$STATIC_ONLY" == "1" ]]; then
  for gate in \
    scripts/runtime-production-readiness-gate.sh \
    scripts/production-deployment-safety-gate.sh \
    scripts/remote-computer-production-state-gate.sh \
    scripts/live-connector-production-semantics-gate.sh \
    scripts/ontology-engine-production-gate.sh \
    scripts/ontology-release-workflow-trigger-gate.sh \
    scripts/enterprise-security-production-controls-gate.sh \
    scripts/observability-ops-production-gate.sh \
    scripts/product-surfaces-production-gate.sh \
    scripts/workflowpack-enterprise-lifecycle-gate.sh; do
    gate_name="$(basename "$gate" .sh)"
    gate_dir="$EVIDENCE_DIR/$gate_name"
    echo "running static $gate"
    STATIC_ONLY=1 EVIDENCE_DIR="$gate_dir" "$gate"
  done
  echo "running static scripts/enterprise-product-completion-contract-gate.sh"
  ALLOW_BLOCKED=1 AUDIT_DIR="$EVIDENCE_DIR/enterprise-product-completion-contract-gate" \
    scripts/enterprise-product-completion-contract-gate.sh
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

default_stage2_capture_dir=".mandoforge/stage2-production-evidence"
if [[ "$EVIDENCE_DIR" == "/evidence" ]]; then
  default_stage2_capture_dir="$EVIDENCE_DIR"
fi
stage2_capture_dir="${ENTERPRISE_PRODUCT_EVIDENCE_DIR:-${MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR:-${STAGE2_EVIDENCE_DIR:-$default_stage2_capture_dir}}}"
mkdir -p "$stage2_capture_dir"
stage2_preflight_summary="$stage2_capture_dir/stage2-production-evidence-preflight.json"

if [[ -n "$STAGE2_CONTROLLER_ENV_FILE" ]]; then
  require_file "$STAGE2_CONTROLLER_ENV_FILE"
  STAGE2_PRODUCTION_PREFLIGHT_SUMMARY_FILE="$stage2_preflight_summary" \
    scripts/stage2-production-evidence-preflight.sh "$STAGE2_CONTROLLER_ENV_FILE"
  load_stage2_controller_env_file "$STAGE2_CONTROLLER_ENV_FILE"
else
  stage2_env_snapshot="$(mktemp -t mandoforge-stage2-production-env.XXXXXX)"
  env >"$stage2_env_snapshot"
  STAGE2_PRODUCTION_PREFLIGHT_SUMMARY_FILE="$stage2_preflight_summary" \
    scripts/stage2-production-evidence-preflight.sh "$stage2_env_snapshot"
fi

for gate in "${gates[@]}"; do
  gate_name="$(basename "$gate" .sh)"
  gate_dir="$EVIDENCE_DIR/$gate_name"
  echo "running $gate"
  case "$gate" in
    scripts/enterprise-product-completion-contract-gate.sh)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" SOURCE_EVIDENCE_DIR="$stage2_capture_dir" AUDIT_DIR="$stage2_capture_dir/enterprise-product-completion-contract-gate" ALLOW_BLOCKED=1 "$gate"
      ;;
    scripts/enterprise-product-readiness-gate.sh)
      BASE_URL="$BASE_URL" \
        EVIDENCE_DIR="$gate_dir" \
        MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR="$stage2_capture_dir" \
        ENTERPRISE_PRODUCT_COMPLETION_CHECKLIST="$stage2_capture_dir/enterprise-product-completion-contract-gate/checklist.json" \
        ALLOW_BLOCKED=0 \
        "$gate"
      ;;
    scripts/stage2-production-evidence-gate.sh|scripts/ontology-release-workflow-trigger-gate.sh|scripts/ontology-engine-readiness-gate.sh|scripts/enterprise-security-admin-readiness-gate.sh|scripts/observability-collector-evidence-gate.sh|scripts/remote-computer-evidence-gate.sh|scripts/native-connector-production-readiness-gate.sh)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" ALLOW_BLOCKED=0 "$gate"
      ;;
    scripts/live-connector-production-semantics-gate.sh)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" SOURCE_EVIDENCE_DIR="$stage2_capture_dir/live-connector-production-semantics" ALLOW_BLOCKED=0 "$gate"
      ;;
    scripts/enterprise-security-production-controls-gate.sh)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" SOURCE_EVIDENCE_DIR="$stage2_capture_dir/enterprise-security-production-controls" ALLOW_BLOCKED=0 "$gate"
      ;;
    scripts/observability-ops-production-gate.sh)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" SOURCE_EVIDENCE_DIR="$stage2_capture_dir/observability-ops-production" ALLOW_BLOCKED=0 "$gate"
      ;;
    *)
      BASE_URL="$BASE_URL" EVIDENCE_DIR="$gate_dir" SOURCE_EVIDENCE_DIR="$stage2_capture_dir" ALLOW_BLOCKED=0 "$gate"
      ;;
  esac
done

cat "$EVIDENCE_DIR/summary.txt"
echo "production launch preflight ok"
