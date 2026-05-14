#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-remote-computer-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/remote-computer-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_SIDECAR_RECOVERY="${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "remote computer evidence gate requires $1" >&2
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
    echo "remote computer evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-remote-computers-readiness.json"
  local runner_file="$EVIDENCE_DIR/api-remote-computers-runner-readiness.json"
  local state_sync_evidence_file="$EVIDENCE_DIR/remote-computer-state-sync-evidence.json"
  local sidecar_recovery_evidence_file="$EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local score
  local state_sync_status
  local sidecar_supervision_status
  local sidecar_recovery_status
  local runner_status
  local state_sync_evidence_status
  local state_sync_validation_status
  local sidecar_recovery_evidence_status
  local sidecar_recovery_run_status
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  score="$(jq -r '.readiness_score // 0' "$readiness_file")"
  state_sync_status="$(jq -r '.production_state_sync.status // "unknown"' "$readiness_file")"
  sidecar_supervision_status="$(jq -r '.sidecar_supervision.status // "unknown"' "$readiness_file")"
  sidecar_recovery_status="$(jq -r '.sidecar_recovery.status // "unknown"' "$readiness_file")"
  runner_status="$(jq -r '.status // "unknown"' "$runner_file")"
  state_sync_evidence_status="missing"
  state_sync_validation_status="unknown"
  if [[ -s "$state_sync_evidence_file" ]]; then
    state_sync_evidence_status="$(jq -r '.status // "unknown"' "$state_sync_evidence_file")"
    state_sync_validation_status="$(jq -r '.response.status // "unknown"' "$state_sync_evidence_file")"
  fi
  sidecar_recovery_evidence_status="not_requested"
  sidecar_recovery_run_status="not_run"
  if [[ -s "$sidecar_recovery_evidence_file" ]]; then
    sidecar_recovery_evidence_status="$(jq -r '.status // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_recovery_run_status="$(jq -r '.response.status // "unknown"' "$sidecar_recovery_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_state_sync.production_blocked,
      (.sidecar_recovery.status == "blocked")
    ] | map(select(. == true)) | length' "$readiness_file")"

  {
    echo "remote_computer_status=$status"
    echo "readiness_score=$score"
    echo "production_state_sync_status=$state_sync_status"
    echo "sidecar_supervision_status=$sidecar_supervision_status"
    echo "sidecar_recovery_status=$sidecar_recovery_status"
    echo "runner_status=$runner_status"
    echo "state_sync_evidence_status=$state_sync_evidence_status"
    echo "state_sync_validation_status=$state_sync_validation_status"
    echo "sidecar_recovery_evidence_status=$sidecar_recovery_evidence_status"
    echo "sidecar_recovery_run_status=$sidecar_recovery_run_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "sidecar_recovery_run=$RUN_SIDECAR_RECOVERY"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "state_sync_blocking_reasons:"
    jq -r '.production_state_sync.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "runbook_actions:"
    jq -r '.runbook_actions[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Remote Computer evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_state_sync_evidence() {
  local state_sync_response
  local state_sync_evidence_file="$EVIDENCE_DIR/remote-computer-state-sync-evidence.json"

  state_sync_response="$(fetch_json POST /api/remote-computers/state-sync/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$state_sync_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$state_sync_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$state_sync_evidence_file"
}

capture_sidecar_recovery_evidence() {
  local sidecar_recovery_response
  local sidecar_recovery_evidence_file="$EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"

  sidecar_recovery_response="$(fetch_json POST /api/remote-computers/sidecars/recovery/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$sidecar_recovery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$sidecar_recovery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$sidecar_recovery_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/remote-computers/readiness >/dev/null
fetch_json GET /api/remote-computers/runner/readiness >/dev/null
capture_state_sync_evidence

if [[ "$RUN_SIDECAR_RECOVERY" == "1" ]]; then
  capture_sidecar_recovery_evidence
else
  echo "skipping Remote Computer sidecar recovery; set RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 to include replacement evidence" >&2
fi

fetch_json GET /api/remote-computers/readiness >/dev/null
fetch_json GET /api/remote-computers/runner/readiness >/dev/null
write_summary
