#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/production-deployment-safety-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
DEPLOYMENT_SAFETY_EVIDENCE_FILE="${PRODUCTION_DEPLOYMENT_SAFETY_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/production-deployment-safety/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "production deployment safety gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "production deployment safety gate failed: $*" >&2
  exit 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

ready_value() {
  case "$1" in
    ready|validated|completed|passed)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

static_contract_check() {
  require_executable scripts/production-launch-preflight.sh
  require_executable scripts/production-deployment-safety-gate.sh
  require_executable scripts/enterprise-product-completion-contract-gate.sh

  [[ -f deploy/k8s/kustomization.yaml ]] || fail "missing deploy/k8s/kustomization.yaml"
  [[ -f deploy/k8s/configmap.yaml ]] || fail "missing deploy/k8s/configmap.yaml"
  [[ -f deploy/k8s/api.yaml ]] || fail "missing deploy/k8s/api.yaml"
  [[ -f deploy/k8s/workspace-pvc.yaml ]] || fail "missing deploy/k8s/workspace-pvc.yaml"
  [[ -f deploy/k8s/secret.example.yaml ]] || fail "missing deploy/k8s/secret.example.yaml"
  [[ -f deploy/k8s/secret-delivery-contract.yaml ]] || fail "missing deploy/k8s/secret-delivery-contract.yaml"
  [[ -f deploy/kustomization.yaml ]] || fail "missing deploy/kustomization.yaml"
  [[ -f deploy/remote-computer-pilot/kustomization.yaml ]] || fail "missing deploy/remote-computer-pilot/kustomization.yaml"

  if grep -Eq '(^|[[:space:]-])secret\.example\.yaml([[:space:]]|$)' deploy/k8s/kustomization.yaml; then
    fail "deploy/k8s/kustomization.yaml must not apply secret.example.yaml"
  fi
  grep -Eq '^[[:space:]]*-[[:space:]]*k8s[[:space:]]*$' deploy/kustomization.yaml \
    || fail "deploy/kustomization.yaml must include the safe default k8s bundle"
  if grep -Eq 'remote-computer-pilot|remote-computer-state-juicefs-profile|remote-computer-warm-pool|remote-computer-keda' deploy/kustomization.yaml; then
    fail "deploy/kustomization.yaml must keep Remote Computer pilot resources opt-in"
  fi
  if grep -Eq 'POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@' deploy/k8s/secret.example.yaml; then
    fail "deploy/k8s/secret.example.yaml must not contain default Postgres credentials"
  fi
  grep -q 'MANDOFORGE_SECRET_DELIVERY_REQUIRED: "true"' deploy/k8s/secret-delivery-contract.yaml \
    || fail "secret delivery contract must require external secret delivery"
  grep -q 'claimName: mandoforge-workspaces' deploy/k8s/api.yaml \
    || fail "API workspace volume must use mandoforge-workspaces PVC"
  grep -q 'stage2-production-evidence-pvc.yaml' deploy/k8s/kustomization.yaml \
    || fail "deploy/k8s default kustomization must include the Stage 2 production evidence PVC"
  grep -q 'claimName: mandoforge-stage2-production-evidence' deploy/k8s/api.yaml \
    || fail "API deployment must mount the Stage 2 production evidence PVC"
  grep -q 'mountPath: /evidence' deploy/k8s/api.yaml \
    || fail "API deployment must expose the Stage 2 production evidence mount at /evidence"
  grep -q 'readOnly: true' deploy/k8s/api.yaml \
    || fail "API production evidence mount must be read-only"
  if grep -q 'emptyDir: {}' deploy/k8s/api.yaml; then
    fail "API workspace volume must not use emptyDir"
  fi
  grep -Eq 'MANDOFORGE_PROVIDER_RUNTIME_ENV:[[:space:]]*"production"' deploy/k8s/configmap.yaml \
    || fail "K8s config must force provider runtime production mode"
  grep -Eq 'MANDOFORGE_AGENT_RELEASE_ENVIRONMENT:[[:space:]]*"production"' deploy/k8s/configmap.yaml \
    || fail "K8s config must bind production sessions to production agent releases"
  grep -Eq 'MANDOFORGE_AGENT_RELEASE_ENFORCEMENT:[[:space:]]*"required"' deploy/k8s/configmap.yaml \
    || fail "K8s config must require the agent release execution gate"
  local controller_flag
  for controller_flag in \
    MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED \
    MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED; do
    grep -Eq "${controller_flag}:[[:space:]]*\"true\"" deploy/k8s/configmap.yaml \
      || fail "K8s config must require ${controller_flag}"
  done
  grep -Eq 'MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR:[[:space:]]*"/evidence"' deploy/k8s/configmap.yaml \
    || fail "K8s config must point enterprise readiness at the production evidence PVC mount"
  grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_RUNNER:[[:space:]]*"kubernetes"' deploy/k8s/configmap.yaml \
    || fail "K8s config must route Remote Computer runner to kubernetes"
  grep -Eq 'MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT:[[:space:]]*"kubernetes"' deploy/k8s/configmap.yaml \
    || fail "K8s config must route Remote Computer execution transport to kubernetes"

  if command -v kubectl >/dev/null 2>&1; then
    local deploy_render_file="$EVIDENCE_DIR/deploy-k8s-render.yaml"
    local deploy_root_render_file="$EVIDENCE_DIR/deploy-root-render.yaml"

    kubectl kustomize deploy/k8s >"$deploy_render_file"
    kubectl kustomize deploy >"$deploy_root_render_file"
    [[ -s "$deploy_render_file" ]] || fail "kubectl kustomize deploy/k8s produced no output"
    [[ -s "$deploy_root_render_file" ]] || fail "kubectl kustomize deploy produced no output"
    if grep -Eq 'kind:[[:space:]]*Secret|POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@' "$deploy_render_file"; then
      fail "rendered deploy/k8s output must not include example Secrets or default credentials"
    fi
    if grep -Eq 'kind:[[:space:]]*Secret|POSTGRES_PASSWORD:[[:space:]]*"mandoforge"|postgres://mandoforge:mandoforge@|replace-me|s3\.example\.com' "$deploy_root_render_file"; then
      fail "rendered deploy root output must not include example Secrets, placeholder storage credentials, or default credentials"
    fi
    grep -q 'name: mandoforge-secret-delivery-contract' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must include the secret delivery contract"
    grep -q 'MANDOFORGE_PROVIDER_RUNTIME_ENV: production' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must force provider runtime production mode"
    for controller_flag in \
      MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED \
      MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED \
      MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED \
      MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED; do
      grep -Eq "${controller_flag}:[[:space:]]*\"?true\"?" "$deploy_render_file" \
        || fail "rendered deploy/k8s output must require ${controller_flag}"
    done
    grep -q 'MANDOFORGE_ENTERPRISE_PRODUCT_EVIDENCE_DIR: /evidence' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must point enterprise readiness at /evidence"
    grep -q 'claimName: mandoforge-stage2-production-evidence' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must include the Stage 2 production evidence PVC"
    grep -q 'mountPath: /evidence' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must mount production evidence at /evidence"
    grep -q 'readOnly: true' "$deploy_render_file" \
      || fail "rendered deploy/k8s production evidence mount must be read-only"
    grep -q 'claimName: mandoforge-workspaces' "$deploy_render_file" \
      || fail "rendered deploy/k8s output must mount the workspace PVC"
  else
    echo "kubectl not found; skipped production deployment kustomize render validation" >&2
  fi

  grep -q "production-deployment-safety-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the production deployment safety gate"
  grep -q "production-deployment-safety-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the production deployment safety gate"
  grep -q "production-deployment-safety-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the production deployment safety gate"
  grep -q "enterprise-product-completion-contract-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the enterprise completion contract gate"
  grep -q "enterprise-product-completion-contract-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the enterprise completion contract gate"
  grep -q "production-deployment-safety-gate.sh" deploy/stage2-evidence/production-deployment-safety-job.example.yaml \
    || fail "production deployment safety Job must run the dedicated gate"
  grep -q "production-deployment-safety-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the production deployment safety Job"
}

safety_issue() {
  local artifact="$1"
  local status evidence_class target_id target_kind target_environment audit_id checked_at support_owner
  local archive_uri immutable archive_digest retention_policy

  [[ -s "$artifact" ]] || {
    printf 'missing production deployment safety evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  evidence_class="$(jq -r '.evidence_class // .required_evidence_class // ""' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  target_environment="$(jq -r '.target.environment // ""' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .deployment_owner // .oncall_owner // ""' "$artifact")"
  archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$artifact")"
  immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$artifact")"
  archive_digest="$(jq -r '.evidence_archive.digest // .archive.digest // ""' "$artifact")"
  retention_policy="$(jq -r '.evidence_archive.retention_policy // .archive.retention_policy // ""' "$artifact")"

  if ! ready_value "$status"; then
    printf 'production deployment safety status is not ready: %s' "$status"
    return 0
  fi
  if [[ "$evidence_class" != "customer_grade" ]]; then
    printf 'production deployment safety evidence class is not customer_grade: %s' "${evidence_class:-<empty>}"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'production deployment safety target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  if [[ "$target_environment" != "production" ]]; then
    printf 'production deployment safety target environment is not production: %s' "${target_environment:-<empty>}"
    return 0
  fi
  case "$target_kind" in
    production_deployment|customer_grade_deployment|kubernetes_cluster|managed_agent_cluster) ;;
    *)
      printf 'production deployment safety target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'production deployment safety evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$archive_digest" || -z "$retention_policy" ]]; then
    printf 'production deployment safety evidence lacks immutable archive URI, digest, or retention metadata'
    return 0
  fi

  jq -e '
    .checks.no_example_secret_applied == true
    and .checks.external_secret_delivery_proven == true
    and .checks.no_default_credentials == true
    and .checks.durable_workspace_storage == true
    and .checks.no_insecure_auth == true
    and .checks.provider_runtime_production == true
    and .checks.remote_computer_kubernetes == true
    and .checks.launch_preflight_passed == true
    and .checks.enterprise_completion_contract_inventory_passed == true
    and .checks.customer_data_boundary_documented == true
  ' "$artifact" >/dev/null || {
    printf 'production deployment safety summary is incomplete'
    return 0
  }

  return 1
}

write_summary() {
  local status="$1"
  local blocked_count="$2"
  local issue="$3"
  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg source "production-deployment-safety-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg evidence_file "$DEPLOYMENT_SAFETY_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      production_deployment_safety_evidence_file: $evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "production_deployment_safety_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "production_deployment_safety_evidence_file=$DEPLOYMENT_SAFETY_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "production deployment safety static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(safety_issue "$DEPLOYMENT_SAFETY_EVIDENCE_FILE")"; then
  echo "$issue_text" >&2
  issue="$issue_text"
  blocked_count=1
fi

if [[ "$blocked_count" == "0" ]]; then
  write_summary "ready" 0 ""
else
  write_summary "blocked" "$blocked_count" "$issue"
fi

cat "$EVIDENCE_DIR/summary.txt"

if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
  exit 1
fi

echo "production deployment safety gate ok"
