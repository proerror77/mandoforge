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
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local provider_status
  local health_status
  local kms_status
  local rotation_status
  local recovery_status
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  provider_status="$(jq -r '.secret_provider.status // "unknown"' "$readiness_file")"
  health_status="$(jq -r '.status // "unknown"' "$health_file")"
  kms_status="$(jq -r '.kms.status // "unknown"' "$readiness_file")"
  rotation_status="$(jq -r '.production_rotation.status // "unknown"' "$readiness_file")"
  recovery_status="$(jq -r '.production_recovery.status // "unknown"' "$readiness_file")"
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

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/vault/readiness >/dev/null
fetch_json GET /api/vault/health >/dev/null
fetch_json POST /api/vault/kms/recovery/validate >/dev/null

if [[ "$RUN_SECRET_LIFECYCLE" == "1" ]]; then
  fetch_json POST /api/vault/kms/rotation/run >/dev/null
else
  echo "skipping KMS rotation run; set RUN_STAGE2_SECRET_LIFECYCLE=1 to include secret lifecycle evidence" >&2
fi

fetch_json GET /api/vault/readiness >/dev/null
fetch_json GET /api/vault/health >/dev/null
write_summary
