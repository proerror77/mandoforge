#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-provider-governance-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/provider-governance-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_PROVIDER_ROLLOUT="${RUN_STAGE2_PROVIDER_ROLLOUT:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "provider governance evidence gate requires $1" >&2
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
    echo "provider governance evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local summary_json="$EVIDENCE_DIR/api-providers-summary.json"
  local gate_json="$EVIDENCE_DIR/api-providers-policy-gate.json"
  local runs_json="$EVIDENCE_DIR/api-providers-policy-gate-runs.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local provider_count
  local active_count
  local deployment_status
  local gate_status
  local gate_enforcement_status
  local blocked_count

  provider_count="$(jq -r '.provider_count // 0' "$summary_json")"
  active_count="$(jq -r '.active_provider_count // 0' "$summary_json")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$summary_json")"
  gate_status="$(jq -r '.status // "unknown"' "$gate_json")"
  gate_enforcement_status="$(jq -r '.production_enforcement.status // "unknown"' "$runs_json")"
  blocked_count="$(jq -r '[
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$summary_json")"
  blocked_count="$((blocked_count + $(jq -r 'if .production_enforcement.production_blocked == true then 1 else 0 end' "$runs_json")))"

  {
    echo "provider_count=$provider_count"
    echo "active_provider_count=$active_count"
    echo "provider_policy_gate_status=$gate_status"
    echo "provider_policy_gate_enforcement_status=$gate_enforcement_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "provider_rollout_run=$RUN_PROVIDER_ROLLOUT"
    echo
    echo "governance_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$summary_json"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$summary_json"
    echo
    echo "policy_gate_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$runs_json"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Provider governance evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/providers/summary >/dev/null
fetch_json GET /api/providers/policy-gate >/dev/null
fetch_json GET /api/providers/policy-gate/runs >/dev/null
fetch_json POST /api/providers/policy-gate/run >/dev/null
fetch_json POST /api/providers/deployment/validate >/dev/null

if [[ "$RUN_PROVIDER_ROLLOUT" == "1" ]]; then
  fetch_json POST /api/providers/production-rollout/run >/dev/null
  fetch_json POST /api/providers/production-rollout/rollback >/dev/null
else
  echo "skipping provider production rollout/rollback; set RUN_STAGE2_PROVIDER_ROLLOUT=1 to include provider rollout evidence" >&2
fi

fetch_json GET /api/providers/summary >/dev/null
fetch_json GET /api/providers/policy-gate >/dev/null
fetch_json GET /api/providers/policy-gate/runs >/dev/null
write_summary
