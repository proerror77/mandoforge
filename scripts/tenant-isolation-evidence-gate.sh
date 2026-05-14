#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-tenant-isolation-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/tenant-isolation-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "tenant isolation evidence gate requires $1" >&2
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
    echo "tenant isolation evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-tenant-isolation-readiness.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local score
  local runtime_mode
  local routing_status
  local routing_controller_fresh
  local routing_controller_age_hours
  local rls_enabled
  local rls_forced
  local tenant_context
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  score="$(jq -r '.readiness_score // 0' "$readiness_file")"
  runtime_mode="$(jq -r '.runtime_tenant_mode // "unknown"' "$readiness_file")"
  routing_status="$(jq -r '.production_routing.status // "unknown"' "$readiness_file")"
  routing_controller_fresh="$(jq -r '.production_routing.controller_evidence_fresh // false' "$readiness_file")"
  routing_controller_age_hours="$(jq -r '.production_routing.latest_controller_age_hours // "none"' "$readiness_file")"
  rls_enabled="$(jq -r '.rls.enabled // false' "$readiness_file")"
  rls_forced="$(jq -r '.rls.forced // false' "$readiness_file")"
  tenant_context="$(jq -r '.rls.tenant_context_configured // false' "$readiness_file")"
  blocked_count="$(jq -r 'if .production_routing.production_blocked == true then 1 else 0 end' "$readiness_file")"

  {
    echo "tenant_isolation_status=$status"
    echo "readiness_score=$score"
    echo "runtime_tenant_mode=$runtime_mode"
    echo "production_routing_status=$routing_status"
    echo "production_routing_controller_evidence_fresh=$routing_controller_fresh"
    echo "production_routing_controller_age_hours=$routing_controller_age_hours"
    echo "rls_enabled=$rls_enabled"
    echo "rls_forced=$rls_forced"
    echo "tenant_context_configured=$tenant_context"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "routing_blocking_reasons:"
    jq -r '.production_routing.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "runbook_actions:"
    jq -r '.runbook_actions[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Tenant isolation evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/tenant-isolation/readiness >/dev/null
fetch_json POST /api/tenant-isolation/routing/validate >/dev/null
fetch_json GET /api/tenant-isolation/readiness >/dev/null
write_summary
