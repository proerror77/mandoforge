#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-codex-app-server-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/codex-app-server-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_CODEX_STALE_POLL="${RUN_STAGE2_CODEX_STALE_POLL:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Codex App Server evidence gate requires $1" >&2
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
    echo "Codex App Server evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local health_file="$EVIDENCE_DIR/api-codex-app-server-health.json"
  local summary_file_json="$EVIDENCE_DIR/api-codex-app-server-control-plane-summary.json"
  local deployment_file="$EVIDENCE_DIR/api-codex-app-server-deployment-validate.json"
  local ops_file="$EVIDENCE_DIR/api-codex-app-server-ops-validate.json"
  local stale_poll_evidence_file="$EVIDENCE_DIR/codex-app-server-stale-poll-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local health_status
  local configured
  local run_count
  local active_turn_count
  local failed_turn_count
  local production_ops_status
  local deployment_status
  local deployment_validation_status
  local ops_validation_status
  local stale_poll_evidence_status
  local stale_poll_status
  local stale_poll_processed_count
  local stale_poll_failed_count
  local blocked_count

  health_status="$(jq -r '.status // "unknown"' "$health_file")"
  configured="$(jq -r '.configured // false' "$summary_file_json")"
  run_count="$(jq -r '.trace_summary.run_count // .run_count // 0' "$summary_file_json")"
  active_turn_count="$(jq -r '.trace_summary.active_turn_count // 0' "$summary_file_json")"
  failed_turn_count="$(jq -r '.trace_summary.failed_turn_count // 0' "$summary_file_json")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$summary_file_json")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$summary_file_json")"
  deployment_validation_status="$(jq -r '.status // "unknown"' "$deployment_file")"
  ops_validation_status="$(jq -r '.status // "unknown"' "$ops_file")"
  stale_poll_evidence_status="not_requested"
  stale_poll_status="not_run"
  stale_poll_processed_count="0"
  stale_poll_failed_count="0"
  if [[ -s "$stale_poll_evidence_file" ]]; then
    stale_poll_evidence_status="$(jq -r '.status // "unknown"' "$stale_poll_evidence_file")"
    stale_poll_status="$(jq -r '.response.status // "unknown"' "$stale_poll_evidence_file")"
    stale_poll_processed_count="$(jq -r '.response.processed_count // .response.polled_count // 0' "$stale_poll_evidence_file")"
    stale_poll_failed_count="$(jq -r '.response.failed_count // 0' "$stale_poll_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$summary_file_json")"

  {
    echo "codex_app_server_health_status=$health_status"
    echo "configured=$configured"
    echo "run_count=$run_count"
    echo "active_turn_count=$active_turn_count"
    echo "failed_turn_count=$failed_turn_count"
    echo "production_ops_status=$production_ops_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_validation_status=$deployment_validation_status"
    echo "ops_validation_status=$ops_validation_status"
    echo "stale_poll_evidence_status=$stale_poll_evidence_status"
    echo "stale_poll_status=$stale_poll_status"
    echo "stale_poll_processed_count=$stale_poll_processed_count"
    echo "stale_poll_failed_count=$stale_poll_failed_count"
    echo "production_blocked_count=$blocked_count"
    echo "stale_poll_run=$RUN_CODEX_STALE_POLL"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "health_issues:"
    jq -r '.issues[]? | "- \(.)"' "$health_file"
    echo
    echo "production_ops_message:"
    jq -r '.production_ops.message // "none"' "$summary_file_json"
    echo
    echo "deployment_message:"
    jq -r '.deployment_readiness.message // "none"' "$summary_file_json"
    echo
    echo "deployment_validation_issues:"
    jq -r '.issues[]? | "- \(.)"' "$deployment_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Codex App Server evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_stale_poll_evidence() {
  local stale_poll_response
  local stale_poll_evidence_file="$EVIDENCE_DIR/codex-app-server-stale-poll-evidence.json"

  stale_poll_response="$(fetch_json POST /api/codex-app-server/runs/poll-stale)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$stale_poll_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$stale_poll_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$stale_poll_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/codex-app-server/health >/dev/null
fetch_json GET /api/codex-app-server/control-plane/summary >/dev/null
fetch_json GET /api/codex-app-server/runs >/dev/null
fetch_json GET /api/codex-app-server/traces >/dev/null
fetch_json POST /api/codex-app-server/deployment/validate >/dev/null
fetch_json POST /api/codex-app-server/ops/validate >/dev/null

if [[ "$RUN_CODEX_STALE_POLL" == "1" ]]; then
  capture_stale_poll_evidence
else
  echo "skipping Codex App Server stale poll; set RUN_STAGE2_CODEX_STALE_POLL=1 to include stale-run supervision evidence" >&2
fi

fetch_json GET /api/codex-app-server/control-plane/summary >/dev/null
write_summary
