#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-vault-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/vault-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_SECRET_LIFECYCLE="${RUN_STAGE2_SECRET_LIFECYCLE:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "vault evidence gate requires $1" >&2
    exit 1
  fi
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
  local kms_status
  local rotation_status
  local recovery_status
  local recovery_evidence_status
  local recovery_validation_status
  local recovery_controller_fresh
  local recovery_controller_age_hours
  local rotation_evidence_status
  local rotation_run_status
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  provider_status="$(jq -r '.secret_provider.status // "unknown"' "$readiness_file")"
  health_status="$(jq -r '.status // "unknown"' "$health_file")"
  kms_status="$(jq -r '.kms.status // "unknown"' "$readiness_file")"
  rotation_status="$(jq -r '.production_rotation.status // "unknown"' "$readiness_file")"
  recovery_status="$(jq -r '.production_recovery.status // "unknown"' "$readiness_file")"
  recovery_controller_fresh="$(jq -r '.production_recovery.controller_evidence_fresh // false' "$readiness_file")"
  recovery_controller_age_hours="$(jq -r '.production_recovery.latest_controller_age_hours // "none"' "$readiness_file")"
  recovery_evidence_status="missing"
  recovery_validation_status="unknown"
  if [[ -s "$recovery_evidence_file" ]]; then
    recovery_evidence_status="$(jq -r '.status // "unknown"' "$recovery_evidence_file")"
    recovery_validation_status="$(jq -r '.response.status // "unknown"' "$recovery_evidence_file")"
  fi
  rotation_evidence_status="not_requested"
  rotation_run_status="not_run"
  if [[ -s "$rotation_evidence_file" ]]; then
    rotation_evidence_status="$(jq -r '.status // "unknown"' "$rotation_evidence_file")"
    rotation_run_status="$(jq -r '.response.status // "unknown"' "$rotation_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_rotation.production_blocked,
      .production_recovery.production_blocked
    ] | map(select(. == true)) | length' "$readiness_file")"

  {
    echo "vault_readiness_status=$status"
    echo "vault_health_status=$health_status"
    echo "secret_provider_status=$provider_status"
    echo "kms_status=$kms_status"
    echo "production_rotation_status=$rotation_status"
    echo "production_recovery_status=$recovery_status"
    echo "recovery_evidence_status=$recovery_evidence_status"
    echo "recovery_validation_status=$recovery_validation_status"
    echo "recovery_controller_evidence_fresh=$recovery_controller_fresh"
    echo "recovery_controller_age_hours=$recovery_controller_age_hours"
    echo "rotation_evidence_status=$rotation_evidence_status"
    echo "rotation_run_status=$rotation_run_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "secret_lifecycle_run=$RUN_SECRET_LIFECYCLE"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.resource_type)/\(.resource_name) - \(.message)"' "$readiness_file"
    echo
    echo "rotation_blocking_reasons:"
    jq -r '.production_rotation.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "recovery_blocking_reasons:"
    jq -r '.production_recovery.blocking_reasons[]? | "- \(.)"' "$readiness_file"
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
