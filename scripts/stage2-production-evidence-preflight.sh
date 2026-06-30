#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/stage2-evidence/stage2-production-controllers.env.example}"
summary_file="${STAGE2_PRODUCTION_PREFLIGHT_SUMMARY_FILE:-}"

if [[ ! -f "$env_file" ]]; then
  echo "missing Stage 2 production evidence env file: $env_file" >&2
  exit 1
fi

pass_count=0
fail_count=0
checks_jsonl=""

if [[ -n "$summary_file" ]]; then
  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required to write Stage 2 production evidence preflight summary JSON" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$summary_file")"
  checks_jsonl="$(mktemp)"
  trap 'rm -f "$checks_jsonl"' EXIT
fi

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
  record_check "passed" "$1"
}

record_fail() {
  fail_count=$((fail_count + 1))
  printf 'fail %s\n' "$1"
  record_check "failed" "$1"
}

record_check() {
  local status="$1"
  local message="$2"
  local scope="${message%%:*}"
  local detail="$message"

  [[ -n "$checks_jsonl" ]] || return 0
  jq -n \
    --arg status "$status" \
    --arg scope "$scope" \
    --arg detail "$detail" \
    '{
      status: $status,
      scope: $scope,
      detail: $detail
    }' >>"$checks_jsonl"
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

require_rls_table_list() {
  local key="$1"
  local label="$2"
  local value
  local table
  local -a tables
  value="$(trim "$(env_value "$key")")"
  if [[ -z "$value" ]]; then
    record_fail "$label: $key must list expected schema.table RLS coverage"
    return
  fi

  IFS=',' read -r -a tables <<<"$value"
  for table in "${tables[@]}"; do
    table="$(trim "$table")"
    if [[ ! "$table" =~ ^[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      record_fail "$label: $key entry must be schema.table: ${table:-<empty>}"
      return
    fi
  done
  record_pass "$label: $key lists expected schema.table RLS coverage"
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

looks_production_archive_uri() {
  local value="$1"
  [[ "$value" =~ ^(s3|gs|az|https):// ]] || return 1
  [[ ! "$value" =~ example\.com ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

looks_evidence_digest() {
  local value="$1"
  [[ "$value" =~ ^(sha256:)?[A-Fa-f0-9]{64}$ ]] || return 1
}

require_production_archive_uri() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if looks_production_archive_uri "$value"; then
    record_pass "$label: $key points at immutable production evidence storage"
  else
    record_fail "$label: $key must point at real immutable evidence storage, not pilot/mock/local/example storage"
  fi
}

require_evidence_digest() {
  local key="$1"
  local label="$2"
  local value
  value="$(trim "$(env_value "$key")")"
  if looks_evidence_digest "$value"; then
    record_pass "$label: $key is a sha256 evidence archive digest"
  else
    record_fail "$label: $key must be a sha256 evidence archive digest"
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

check_evidence_archive_metadata() {
  local label="evidence-archive"
  require_production_identity MANDOFORGE_STAGE2_SUPPORT_OWNER "$label"
  require_production_archive_uri MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_URI "$label"
  require_evidence_digest MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_DIGEST "$label"
  require_value MANDOFORGE_STAGE2_EVIDENCE_RETENTION_POLICY "$label"
}

check_tenant() {
  local label="tenant-routing"
  require_production_identity MANDOFORGE_STAGE2_TENANT_DEPLOYMENT_ID "$label"
  require_rls_table_list MANDOFORGE_STAGE2_TENANT_RLS_TABLES "$label"
  require_true MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED "$label"
  require_production_url MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL "$label"
  require_no_whiskey_url MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL "$label"
  require_value MANDOFORGE_TENANT_ROUTING_CONTROLLER_TOKEN "$label"
}

check_policy() {
  local label="policy-rollout"
  require_production_identity MANDOFORGE_STAGE2_POLICY_CONTROLLER_ID "$label"
  require_production_identity MANDOFORGE_STAGE2_POLICY_STORE_ID "$label"
  require_production_identity MANDOFORGE_STAGE2_POLICY_DEPLOYMENT_ID "$label"
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
check_evidence_archive_metadata
check_worker_remote_computer
check_tenant
check_policy
check_vault_kms
check_finance
check_managed_session_runtime

echo
echo "preflight_pass_count=$pass_count"
echo "preflight_fail_count=$fail_count"

if [[ -n "$summary_file" ]]; then
  jq -n \
    --arg source "stage2-production-evidence-preflight" \
    --arg env_file "$env_file" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson pass_count "$pass_count" \
    --argjson fail_count "$fail_count" \
    --slurpfile checks "$checks_jsonl" \
    '{
      source: $source,
      status: (if $fail_count == 0 then "passed" else "failed" end),
      generated_at: $generated_at,
      env_file: $env_file,
      pass_count: $pass_count,
      fail_count: $fail_count,
      checks: $checks
    }' >"$summary_file"
fi

if [[ "$fail_count" != "0" ]]; then
  echo "Stage 2 production evidence preflight failed; fix the env file before running strict production evidence." >&2
  exit 1
fi

echo "stage2 production evidence preflight ok"
