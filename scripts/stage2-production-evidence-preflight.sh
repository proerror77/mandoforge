#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/stage2-evidence/stage2-production-controllers.env.example}"

if [[ ! -f "$env_file" ]]; then
  echo "missing Stage 2 production evidence env file: $env_file" >&2
  exit 1
fi

pass_count=0
fail_count=0

trim() {
  printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

env_value() {
  local key="$1"
  awk -F= -v key="$key" '
    $0 ~ /^[[:space:]]*#/ { next }
    $0 !~ /=/ { next }
    {
      name=$1
      sub(/^[[:space:]]+/, "", name)
      sub(/[[:space:]]+$/, "", name)
      if (name == key) {
        value=$0
        sub(/^[^=]*=/, "", value)
        print value
      }
    }
  ' "$env_file" | tail -n 1
}

record_pass() {
  pass_count=$((pass_count + 1))
  printf 'ok %s\n' "$1"
}

record_fail() {
  fail_count=$((fail_count + 1))
  printf 'fail %s\n' "$1"
}

is_true() {
  case "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" in
    1|true|yes)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_false() {
  case "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" in
    0|false|no)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_distributed_state_backend() {
  case "$(normalize_kind "$1")" in
    juicefs|cephfs|longhorn_rwx)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

require_value() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if [[ -n "$value" ]]; then
    record_pass "$label: $key is set"
  else
    record_fail "$label: $key is empty or missing"
  fi
}

looks_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

looks_finance_system_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  looks_production_identity "$value" || return 1
  [[ ! "$value" =~ (^|[./:_-])(feishu|lark|drive|file|artifact)([./:_-]|$) ]] || return 1
}

require_production_identity() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if looks_production_identity "$value"; then
    record_pass "$label: $key names a non-pilot target identity"
  else
    record_fail "$label: $key must name a real non-pilot target identity"
  fi
}

require_distributed_state_backend() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if is_distributed_state_backend "$value"; then
    record_pass "$label: $key names a supported distributed state backend"
  else
    record_fail "$label: $key must be juicefs, cephfs, or longhorn-rwx"
  fi
}

require_finance_system_identity() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if looks_finance_system_identity "$value"; then
    record_pass "$label: $key names a real ERP/accounting system"
  else
    record_fail "$label: $key must name a true ERP/accounting system, not an artifact store"
  fi
}

require_true() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if is_true "$value"; then
    record_pass "$label: $key is enabled"
  else
    record_fail "$label: $key must be true/1"
  fi
}

require_false() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if is_false "$value"; then
    record_pass "$label: $key is disabled"
  else
    record_fail "$label: $key must be false/0"
  fi
}

require_eq() {
  local key="$1"
  local expected="$2"
  local label="$3"
  local value
  value="$(trim "$(env_value "$key")")"
  if [[ "$value" == "$expected" ]]; then
    record_pass "$label: $key=$expected"
  else
    record_fail "$label: $key must be $expected"
  fi
}

looks_production_url() {
  local value="$1"
  [[ "$value" =~ ^https?:// ]] || return 1
  [[ ! "$value" =~ example\.com ]] || return 1
  [[ ! "$value" =~ (^https?://)?(localhost|127\.0\.0\.1|\[::1\]) ]] || return 1
  [[ ! "$value" =~ (^|[./_-])(mock|pilot)([./_-]|$) ]] || return 1
}

require_production_url() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if looks_production_url "$value"; then
    record_pass "$label: $key points at a non-placeholder controller URL"
  else
    record_fail "$label: $key must be a real non-placeholder controller URL"
  fi
}

require_no_whiskey_url() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if [[ -n "$value" && ! "$value" =~ whiskey ]]; then
    record_pass "$label: $key is not a Whiskey pilot URL"
  else
    record_fail "$label: $key must not point at the Whiskey pilot target"
  fi
}

normalize_kind() {
  printf '%s' "$1" | tr '[:upper:]-' '[:lower:]_'
}

is_production_kms_provider() {
  case "$(normalize_kind "$1")" in
    external|external_kms|aws_kms|gcp_kms|azure_key_vault|hashicorp_vault_transit|vault_transit|hsm|cloudhsm|pkcs11_hsm)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

require_production_kms_provider() {
  local key="MANDOFORGE_KMS_PROVIDER"
  local value
  value="$(trim "$(env_value "$key")")"
  if is_production_kms_provider "$value"; then
    record_pass "vault-kms: $key is production-capable"
  else
    record_fail "vault-kms: $key must name external KMS/HSM, not reserved/mock/pilot"
  fi
}

check_global() {
  require_true RUN_STAGE2_PRODUCTION_VALIDATIONS global
  require_true VERIFY_STAGE2_VALIDATION_COVERAGE global
  require_true RUN_STAGE2_COMPLETION_AUDIT global
  require_false ALLOW_BLOCKED global
}

check_tenant() {
  local label="tenant-routing"
  require_production_identity MANDOFORGE_STAGE2_TENANT_DEPLOYMENT_ID "$label"
  require_true MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL "$label"
  require_value MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN "$label"
}

check_policy() {
  local label="policy-rollout"
  require_production_identity MANDOFORGE_STAGE2_POLICY_CONTROLLER_ID "$label"
  require_true MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL "$label"
  require_value MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_TOKEN "$label"
  require_true RUN_STAGE2_POLICY_DUE_RUN "$label"
}

check_vault_kms() {
  local label="vault-kms"
  require_production_kms_provider
  require_production_identity MANDOFORGE_STAGE2_KMS_BACKEND_ID "$label"
  require_production_identity MANDOFORGE_KMS_KEY_ID "$label"
  require_value MANDOFORGE_KMS_ROTATION_POLICY "$label"
  require_eq MANDOFORGE_KMS_VALIDATION_MODE external "$label"
  require_production_url MANDOFORGE_KMS_ENDPOINT "$label"
  require_no_whiskey_url MANDOFORGE_KMS_ENDPOINT "$label"
  require_value MANDOFORGE_KMS_TOKEN "$label"
  require_true MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL "$label"
  require_value MANDOFORGE_KMS_RECOVERY_CONTROLLER_TOKEN "$label"
  require_true RUN_STAGE2_SECRET_LIFECYCLE "$label"
}

check_worker_remote_computer() {
  local label="worker-remote-computer"
  require_production_identity MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID "$label"
  require_production_identity MANDOFORGE_STAGE2_WORKER_POOL "$label"
  require_production_identity MANDOFORGE_STAGE2_REMOTE_STATE_CLAIM "$label"
  require_distributed_state_backend MANDOFORGE_STAGE2_REMOTE_STATE_BACKEND "$label"
  require_true MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL "$label"
  require_value MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_TOKEN "$label"
  require_true MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL "$label"
  require_value MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_TOKEN "$label"
  require_true MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED "$label"
  require_true MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED "$label"
  require_production_url MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL "$label"
  require_no_whiskey_url MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL "$label"
  require_value MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_TOKEN "$label"
  require_true RUN_STAGE2_REMOTE_SIDECAR_RECOVERY "$label"
}

check_finance() {
  local label="finance-erp"
  require_finance_system_identity MANDOFORGE_STAGE2_FINANCE_SYSTEM_ID "$label"
  require_true MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL "$label"
  require_value MANDOFORGE_FINANCE_CLOSE_CONTROLLER_TOKEN "$label"
  require_true MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL "$label"
  require_value MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_TOKEN "$label"
  require_production_url FINANCE_EXPORT_DELIVERY_OBSERVER_URL "$label"
  require_no_whiskey_url FINANCE_EXPORT_DELIVERY_OBSERVER_URL "$label"
  require_value FINANCE_EXPORT_DELIVERY_OBSERVER_TOKEN "$label"
  require_true RUN_STAGE2_FINANCE_CONTROLLERS "$label"
  require_true RUN_STAGE2_FINANCE_EXPORT "$label"
}

check_managed_session_runtime() {
  local label="managed-session-runtime"
  require_production_identity MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID "$label"
  require_production_url MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL "$label"
  require_no_whiskey_url MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL "$label"
  require_value MANAGED_SESSION_RESTART_RESUME_CONTROLLER_TOKEN "$label"
  require_true RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME "$label"
}

echo "stage2_production_evidence_preflight_env=$env_file"
check_global
check_worker_remote_computer
check_tenant
check_policy
check_vault_kms
check_finance
check_managed_session_runtime

echo
echo "preflight_pass_count=$pass_count"
echo "preflight_fail_count=$fail_count"

if [[ "$fail_count" != "0" ]]; then
  echo "Stage 2 production evidence preflight failed; fix the env file before running strict production evidence." >&2
  exit 1
fi

echo "stage2 production evidence preflight ok"
