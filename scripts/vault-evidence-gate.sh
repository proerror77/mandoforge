#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-vault-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/vault-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_SECRET_LIFECYCLE="${RUN_STAGE2_SECRET_LIFECYCLE:-1}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-}"

auth_headers=(
)
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "vault evidence gate requires $1" >&2
    exit 1
  fi
}

normalize_kms_kind() {
  printf '%s' "$1" | tr '[:upper:]-' '[:lower:]_'
}

is_production_kms_provider() {
  local value
  value="$(normalize_kms_kind "$1")"
  case "$value" in
    external|external_kms|aws_kms|gcp_kms|azure_key_vault|hashicorp_vault_transit|vault_transit|hsm|cloudhsm|pkcs11_hsm)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_kms_backend_kind() {
  local value
  value="$(normalize_kms_kind "$1")"
  case "$value" in
    external_kms|aws_kms|gcp_kms|azure_key_vault|hashicorp_vault_transit|vault_transit|hsm|cloudhsm|pkcs11_hsm)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_environment() {
  local value="$1"
  case "$value" in
    production|prod)
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
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

kms_recovery_step_detail_count() {
  jq -r '[
    .response.controller_execution.steps[]?
    | select(
        type == "object"
        and ((.name // .step // .kind // .action // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
  ] | length' "$1" 2>/dev/null || echo "0"
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d '{}' \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "vault evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-vault-readiness.json"
  local health_file="$EVIDENCE_DIR/api-vault-health.json"
  local recovery_evidence_file="$EVIDENCE_DIR/vault-kms-recovery-evidence.json"
  local rotation_evidence_file="$EVIDENCE_DIR/vault-kms-rotation-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local provider_status
  local health_status
  local kms_provider
  local kms_status
  local kms_configured
  local kms_endpoint_configured
  local rotation_latest_validated
  local rotation_status
  local recovery_status
  local recovery_evidence_status
  local recovery_validation_status
  local recovery_controller_fresh
  local recovery_controller_validated
  local recovery_controller_production_backend
  local recovery_rotation_validated
  local recovery_controller_age_hours
  local recovery_backend_kind
  local recovery_environment
  local recovery_backend_id
  local recovery_key_id
  local recovery_id
  local recovery_target_kind
  local recovery_step_count
  local recovery_step_detail_count
  local rotation_evidence_status
  local rotation_run_status
  local rotation_production_backend
  local rotation_backend_kind
  local rotation_environment
  local rotation_backend_id
  local rotation_key_id
  local rotation_id
  local rotation_rotated_count
  local rotation_catalog_updated_count
  local rotation_detail_count
  local rotation_action_count
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  provider_status="$(jq -r '.secret_provider.status // "unknown"' "$readiness_file")"
  health_status="$(jq -r '.status // "unknown"' "$health_file")"
  kms_provider="$(jq -r '.kms.provider // "reserved"' "$readiness_file")"
  kms_status="$(jq -r '.kms.status // "unknown"' "$readiness_file")"
  kms_configured="$(jq -r '.kms.configured // false' "$readiness_file")"
  kms_endpoint_configured="$(jq -r '.kms.endpoint_configured // false' "$readiness_file")"
  rotation_status="$(jq -r '.production_rotation.status // "unknown"' "$readiness_file")"
  rotation_latest_validated="$(jq -r '.production_rotation.latest_rotation_validated // false' "$readiness_file")"
  recovery_status="$(jq -r '.production_recovery.status // "unknown"' "$readiness_file")"
  recovery_controller_fresh="$(jq -r '.production_recovery.controller_evidence_fresh // false' "$readiness_file")"
  recovery_controller_validated="$(jq -r '.production_recovery.latest_controller_validated // false' "$readiness_file")"
  recovery_controller_production_backend="$(jq -r '.production_recovery.latest_controller_production_backend // false' "$readiness_file")"
  recovery_rotation_validated="$(jq -r '.production_recovery.latest_rotation_validated // false' "$readiness_file")"
  recovery_controller_age_hours="$(jq -r '.production_recovery.latest_controller_age_hours // "none"' "$readiness_file")"
  recovery_evidence_status="missing"
  recovery_validation_status="unknown"
  recovery_backend_kind="unknown"
  recovery_environment="unknown"
  recovery_backend_id=""
  recovery_key_id=""
  recovery_id=""
  recovery_target_kind="unknown"
  recovery_step_count="0"
  recovery_step_detail_count="0"
  if [[ -s "$recovery_evidence_file" ]]; then
    recovery_evidence_status="$(jq -r '.status // "unknown"' "$recovery_evidence_file")"
    recovery_validation_status="$(jq -r '.response.status // "unknown"' "$recovery_evidence_file")"
    recovery_backend_kind="$(jq -r '.response.controller_execution.backend_kind // "unknown"' "$recovery_evidence_file")"
    recovery_environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$recovery_evidence_file")"
    recovery_backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$recovery_evidence_file")"
    recovery_key_id="$(jq -r '.response.controller_execution.key_id // ""' "$recovery_evidence_file")"
    recovery_id="$(jq -r '.response.controller_execution.recovery_id // ""' "$recovery_evidence_file")"
    recovery_target_kind="$(jq -r '.response.controller_execution.recovery_target_kind // "unknown"' "$recovery_evidence_file")"
    recovery_step_count="$(jq -r 'if ((.response.controller_execution.steps // null) | type) == "array" then (.response.controller_execution.steps | length) else 0 end' "$recovery_evidence_file")"
    recovery_step_detail_count="$(kms_recovery_step_detail_count "$recovery_evidence_file")"
  fi
  rotation_evidence_status="not_requested"
  rotation_run_status="not_run"
  rotation_production_backend="false"
  rotation_backend_kind="unknown"
  rotation_environment="unknown"
  rotation_backend_id=""
  rotation_key_id=""
  rotation_id=""
  rotation_rotated_count="0"
  rotation_catalog_updated_count="0"
  rotation_detail_count="0"
  rotation_action_count="0"
  if [[ -s "$rotation_evidence_file" ]]; then
    rotation_evidence_status="$(jq -r '.status // "unknown"' "$rotation_evidence_file")"
    rotation_run_status="$(jq -r '.response.status // "unknown"' "$rotation_evidence_file")"
    rotation_production_backend="$(jq -r '.response.external_execution.production_backend // false' "$rotation_evidence_file")"
    rotation_backend_kind="$(jq -r '.response.external_execution.backend_kind // "unknown"' "$rotation_evidence_file")"
    rotation_environment="$(jq -r '.response.external_execution.environment // "unknown"' "$rotation_evidence_file")"
    rotation_backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$rotation_evidence_file")"
    rotation_key_id="$(jq -r '.response.external_execution.key_id // ""' "$rotation_evidence_file")"
    rotation_id="$(jq -r '.response.external_execution.rotation_id // ""' "$rotation_evidence_file")"
    rotation_rotated_count="$(jq -r '.response.rotated_count // .response.external_execution.rotated_count // 0' "$rotation_evidence_file")"
    rotation_catalog_updated_count="$(jq -r '.response.catalog_updated_count // 0' "$rotation_evidence_file")"
    rotation_detail_count="$(jq -r '[
      (
        .response.rotated_keys[]?,
        .response.rotation_details[]?,
        .response.key_rotations[]?,
        .response.external_execution.rotated_keys[]?,
        .response.external_execution.rotation_details[]?,
        .response.external_execution.key_rotations[]?
      )
      | select(
          type == "object"
          and ((.key_id // .key // .kms_key_id // "") | length > 0)
          and ((.rotation_id // .rotation // .operation_id // "") | length > 0)
          and ((.catalog_updated // .catalog_update_confirmed // false) == true)
          and ((.status // .result // "") | ascii_downcase | IN("rotated", "validated", "completed", "passed"))
          and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .rotated_at // .timestamp // "") | length > 0)
        )
    ] | length' "$rotation_evidence_file")"
    rotation_action_count="$(jq -r '[.response.actions[]? | select(. == "external_kms_rotation_confirmed")] | length' "$rotation_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_rotation.production_blocked,
      .production_recovery.production_blocked,
      (.secret_provider.status != "ready"),
      (.kms.provider == "reserved"),
      (.kms.status != "ready"),
      (.kms.configured != true),
      (.kms.endpoint_configured != true),
      (.production_rotation.latest_rotation_validated != true),
      (.production_recovery.latest_rotation_validated != true),
      (.production_recovery.latest_controller_validated != true),
      (.production_recovery.latest_controller_production_backend != true),
      (.production_recovery.controller_evidence_fresh != true)
    ] | map(select(. == true)) | length' "$readiness_file")"
  if ! is_production_kms_provider "$kms_provider"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$health_status" != "ready" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$RUN_SECRET_LIFECYCLE" != "1" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$recovery_evidence_status" != "captured" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$recovery_validation_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$rotation_evidence_status" != "captured" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$rotation_run_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$rotation_production_backend" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_kms_backend_kind "$rotation_backend_kind"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_environment "$rotation_environment"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$rotation_backend_id" || -z "$rotation_key_id" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$rotation_backend_id" || ! is_production_identity "$rotation_key_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$rotation_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$rotation_rotated_count" =~ ^[0-9]+$ || "$rotation_rotated_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$rotation_catalog_updated_count" =~ ^[0-9]+$ || "$rotation_catalog_updated_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$rotation_detail_count" =~ ^[0-9]+$ || ! "$rotation_rotated_count" =~ ^[0-9]+$ || ! "$rotation_catalog_updated_count" =~ ^[0-9]+$ || "$rotation_detail_count" -lt "$rotation_rotated_count" || "$rotation_detail_count" -lt "$rotation_catalog_updated_count" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$rotation_action_count" =~ ^[0-9]+$ || "$rotation_action_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$recovery_controller_production_backend" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_kms_backend_kind "$recovery_backend_kind"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_environment "$recovery_environment"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$recovery_backend_id" || -z "$recovery_key_id" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$recovery_backend_id" || ! is_production_identity "$recovery_key_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$recovery_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$recovery_target_kind" != "production_kms_backend" && "$recovery_target_kind" != "production_hsm_backend" && "$recovery_target_kind" != "enterprise_kms_backend" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$recovery_step_count" =~ ^[0-9]+$ || "$recovery_step_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$recovery_step_detail_count" =~ ^[0-9]+$ || ! "$recovery_step_count" =~ ^[0-9]+$ || "$recovery_step_detail_count" -lt "$recovery_step_count" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi

  {
    echo "vault_readiness_status=$status"
    echo "vault_health_status=$health_status"
    echo "secret_provider_status=$provider_status"
    echo "kms_provider=$kms_provider"
    echo "kms_status=$kms_status"
    echo "kms_configured=$kms_configured"
    echo "kms_endpoint_configured=$kms_endpoint_configured"
    echo "production_rotation_status=$rotation_status"
    echo "production_rotation_latest_validated=$rotation_latest_validated"
    echo "production_recovery_status=$recovery_status"
    echo "recovery_evidence_status=$recovery_evidence_status"
    echo "recovery_validation_status=$recovery_validation_status"
    echo "recovery_controller_evidence_fresh=$recovery_controller_fresh"
    echo "recovery_controller_validated=$recovery_controller_validated"
    echo "recovery_controller_production_backend=$recovery_controller_production_backend"
    echo "recovery_rotation_validated=$recovery_rotation_validated"
    echo "recovery_controller_age_hours=$recovery_controller_age_hours"
    echo "recovery_backend_kind=$recovery_backend_kind"
    echo "recovery_environment=$recovery_environment"
    echo "recovery_backend_id=$recovery_backend_id"
    echo "recovery_key_id=$recovery_key_id"
    echo "recovery_id=$recovery_id"
    echo "recovery_target_kind=$recovery_target_kind"
    echo "recovery_step_count=$recovery_step_count"
    echo "recovery_step_detail_count=$recovery_step_detail_count"
    echo "rotation_evidence_status=$rotation_evidence_status"
    echo "rotation_run_status=$rotation_run_status"
    echo "rotation_production_backend=$rotation_production_backend"
    echo "rotation_backend_kind=$rotation_backend_kind"
    echo "rotation_environment=$rotation_environment"
    echo "rotation_backend_id=$rotation_backend_id"
    echo "rotation_key_id=$rotation_key_id"
    echo "rotation_id=$rotation_id"
    echo "rotation_rotated_count=$rotation_rotated_count"
    echo "rotation_catalog_updated_count=$rotation_catalog_updated_count"
    echo "rotation_detail_count=$rotation_detail_count"
    echo "rotation_action_count=$rotation_action_count"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "secret_lifecycle_run=$RUN_SECRET_LIFECYCLE"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.resource_type)/\(.resource_name) - \(.message)"' "$readiness_file"
    echo
    echo "rotation_blocking_reasons:"
    jq -r '.production_rotation.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$provider_status" != "ready" ]]; then
      echo "- Vault secret provider is not ready: $provider_status"
    fi
    if [[ "$health_status" != "ready" ]]; then
      echo "- Vault health endpoint is not ready: $health_status"
    fi
    if [[ "$kms_provider" == "reserved" || "$kms_status" != "ready" || "$kms_configured" != "true" || "$kms_endpoint_configured" != "true" ]]; then
      echo "- external KMS/HSM backend is not fully configured: provider=$kms_provider status=$kms_status configured=$kms_configured endpoint=$kms_endpoint_configured"
    fi
    if ! is_production_kms_provider "$kms_provider"; then
      echo "- KMS/HSM provider is not a production backend: $kms_provider"
    fi
    if [[ "$RUN_SECRET_LIFECYCLE" != "1" ]]; then
      echo "- KMS rotation evidence capture is disabled"
    fi
    if [[ "$rotation_evidence_status" != "captured" ]]; then
      echo "- KMS rotation evidence was not captured"
    fi
    if [[ "$rotation_run_status" != "validated" ]]; then
      echo "- KMS rotation status is not validated: $rotation_run_status"
    fi
    if [[ "$rotation_production_backend" != "true" ]]; then
      echo "- KMS rotation did not confirm a production backend"
    fi
    if ! is_production_kms_backend_kind "$rotation_backend_kind"; then
      echo "- KMS rotation backend kind is not production: $rotation_backend_kind"
    fi
    if ! is_production_environment "$rotation_environment"; then
      echo "- KMS rotation environment is not production: $rotation_environment"
    fi
    if [[ -z "$rotation_backend_id" || -z "$rotation_key_id" ]]; then
      echo "- KMS rotation did not report backend_id and key_id"
    fi
    if ! is_production_identity "$rotation_backend_id" || ! is_production_identity "$rotation_key_id"; then
      echo "- KMS rotation backend_id or key_id is pilot/mock/local: backend_id=${rotation_backend_id:-<empty>} key_id=${rotation_key_id:-<empty>}"
    fi
    if ! is_production_identity "$rotation_id"; then
      echo "- KMS rotation did not report a production rotation_id: ${rotation_id:-<empty>}"
    fi
    if [[ ! "$rotation_rotated_count" =~ ^[0-9]+$ || "$rotation_rotated_count" == "0" ]]; then
      echo "- KMS rotation did not report any rotated records: rotated_count=$rotation_rotated_count"
    fi
    if [[ ! "$rotation_catalog_updated_count" =~ ^[0-9]+$ || "$rotation_catalog_updated_count" == "0" ]]; then
      echo "- KMS rotation did not update the secret catalog: catalog_updated_count=$rotation_catalog_updated_count"
    fi
    if [[ ! "$rotation_detail_count" =~ ^[0-9]+$ || ! "$rotation_rotated_count" =~ ^[0-9]+$ || ! "$rotation_catalog_updated_count" =~ ^[0-9]+$ || "$rotation_detail_count" -lt "$rotation_rotated_count" || "$rotation_detail_count" -lt "$rotation_catalog_updated_count" ]]; then
      echo "- KMS rotation did not include key-level rotation details for every counted key/catalog update: detail_count=$rotation_detail_count rotated_count=$rotation_rotated_count catalog_updated_count=$rotation_catalog_updated_count"
    fi
    if [[ ! "$rotation_action_count" =~ ^[0-9]+$ || "$rotation_action_count" == "0" ]]; then
      echo "- KMS rotation did not report the external_kms_rotation_confirmed action"
    fi
    echo
    echo "recovery_blocking_reasons:"
    jq -r '.production_recovery.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$recovery_evidence_status" != "captured" ]]; then
      echo "- KMS recovery evidence was not captured"
    fi
    if [[ "$recovery_validation_status" != "validated" ]]; then
      echo "- KMS recovery validation status is not validated: $recovery_validation_status"
    fi
    if [[ "$recovery_rotation_validated" != "true" ]]; then
      echo "- KMS recovery readiness does not reference validated rotation evidence"
    fi
    if [[ "$recovery_controller_validated" != "true" ]]; then
      echo "- KMS recovery controller evidence is not validated"
    fi
    if [[ "$recovery_controller_production_backend" != "true" ]]; then
      echo "- KMS recovery controller did not confirm a production backend"
    fi
    if ! is_production_kms_backend_kind "$recovery_backend_kind"; then
      echo "- KMS recovery backend kind is not production: $recovery_backend_kind"
    fi
    if ! is_production_environment "$recovery_environment"; then
      echo "- KMS recovery environment is not production: $recovery_environment"
    fi
    if [[ -z "$recovery_backend_id" || -z "$recovery_key_id" ]]; then
      echo "- KMS recovery did not report backend_id and key_id"
    fi
    if ! is_production_identity "$recovery_backend_id" || ! is_production_identity "$recovery_key_id"; then
      echo "- KMS recovery backend_id or key_id is pilot/mock/local: backend_id=${recovery_backend_id:-<empty>} key_id=${recovery_key_id:-<empty>}"
    fi
    if ! is_production_identity "$recovery_id"; then
      echo "- KMS recovery did not report a production recovery_id: ${recovery_id:-<empty>}"
    fi
    if [[ "$recovery_target_kind" != "production_kms_backend" && "$recovery_target_kind" != "production_hsm_backend" && "$recovery_target_kind" != "enterprise_kms_backend" ]]; then
      echo "- KMS recovery target kind is not production: $recovery_target_kind"
    fi
    if [[ ! "$recovery_step_count" =~ ^[0-9]+$ || "$recovery_step_count" == "0" ]]; then
      echo "- KMS recovery did not report any audited recovery steps"
    fi
    if [[ ! "$recovery_step_detail_count" =~ ^[0-9]+$ || ! "$recovery_step_count" =~ ^[0-9]+$ || "$recovery_step_detail_count" -lt "$recovery_step_count" ]]; then
      echo "- KMS recovery did not include audited detail for every recovery step: detail_count=$recovery_step_detail_count step_count=$recovery_step_count"
    fi
    if [[ "$recovery_controller_fresh" != "true" ]]; then
      echo "- KMS recovery controller evidence is not fresh"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Vault evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_recovery_evidence() {
  local recovery_response
  local recovery_evidence_file="$EVIDENCE_DIR/vault-kms-recovery-evidence.json"

  recovery_response="$(fetch_json POST /api/vault/kms/recovery/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$recovery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$recovery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$recovery_evidence_file"
}

capture_rotation_evidence() {
  local rotation_response
  local rotation_evidence_file="$EVIDENCE_DIR/vault-kms-rotation-evidence.json"

  rotation_response="$(fetch_json POST /api/vault/kms/rotation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rotation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rotation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rotation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/vault/readiness >/dev/null
fetch_json GET /api/vault/health >/dev/null
capture_recovery_evidence

if [[ "$RUN_SECRET_LIFECYCLE" == "1" ]]; then
  capture_rotation_evidence
else
  echo "skipping KMS rotation run; set RUN_STAGE2_SECRET_LIFECYCLE=1 to include secret lifecycle evidence" >&2
fi

fetch_json GET /api/vault/readiness >/dev/null
fetch_json GET /api/vault/health >/dev/null
write_summary
