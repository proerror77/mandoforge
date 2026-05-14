#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-policy-rollout-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/policy-rollout-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_POLICY_DUE_RUN="${RUN_STAGE2_POLICY_DUE_RUN:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "policy rollout evidence gate requires $1" >&2
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
    echo "policy rollout evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-policy-rollout-orchestration-readiness.json"
  local validation_evidence_file="$EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"
  local due_run_evidence_file="$EVIDENCE_DIR/policy-rollout-due-run-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local readiness_status
  local validation_status
  local validation_evidence_status
  local due_run_evidence_status
  local due_run_status
  local production_blocked
  local rollout_active
  local active_revision_id
  local staged_revision_id
  local due_run_fresh
  local latest_due_run_status
  local controller_required
  local controller_configured
  local latest_controller_status
  local controller_fresh
  local controller_age_hours
  local blocked_count

  readiness_status="$(jq -r '.status // "unknown"' "$readiness_file")"
  validation_status="unknown"
  validation_evidence_status="missing"
  if [[ -s "$validation_evidence_file" ]]; then
    validation_evidence_status="$(jq -r '.status // "unknown"' "$validation_evidence_file")"
    validation_status="$(jq -r '.response.status // "unknown"' "$validation_evidence_file")"
  fi
  due_run_evidence_status="not_requested"
  due_run_status="not_run"
  if [[ -s "$due_run_evidence_file" ]]; then
    due_run_evidence_status="$(jq -r '.status // "unknown"' "$due_run_evidence_file")"
    due_run_status="$(jq -r '.response.status // "unknown"' "$due_run_evidence_file")"
  fi
  production_blocked="$(jq -r 'if has("production_blocked") then .production_blocked else true end' "$readiness_file")"
  rollout_active="$(jq -r '.rollout_active // false' "$readiness_file")"
  active_revision_id="$(jq -r '.active_revision_id // "none"' "$readiness_file")"
  staged_revision_id="$(jq -r '.staged_revision_id // "none"' "$readiness_file")"
  due_run_fresh="$(jq -r '.due_run_fresh // false' "$readiness_file")"
  latest_due_run_status="$(jq -r '.latest_due_run_status // "none"' "$readiness_file")"
  controller_required="$(jq -r '.controller_required // false' "$readiness_file")"
  controller_configured="$(jq -r '.controller_configured // false' "$readiness_file")"
  latest_controller_status="$(jq -r '.latest_controller_status // "none"' "$readiness_file")"
  controller_fresh="$(jq -r '.controller_evidence_fresh // false' "$readiness_file")"
  controller_age_hours="$(jq -r '.latest_controller_age_hours // "none"' "$readiness_file")"
  blocked_count="$(jq -r 'if .production_blocked == true then 1 else 0 end' "$readiness_file")"

  {
    echo "policy_rollout_readiness_status=$readiness_status"
    echo "policy_rollout_validation_status=$validation_status"
    echo "policy_rollout_validation_evidence_status=$validation_evidence_status"
    echo "policy_rollout_due_run_evidence_status=$due_run_evidence_status"
    echo "policy_rollout_due_run_status=$due_run_status"
    echo "production_blocked=$production_blocked"
    echo "production_blocked_count=$blocked_count"
    echo "rollout_active=$rollout_active"
    echo "active_revision_id=$active_revision_id"
    echo "staged_revision_id=$staged_revision_id"
    echo "due_run_fresh=$due_run_fresh"
    echo "latest_due_run_status=$latest_due_run_status"
    echo "controller_required=$controller_required"
    echo "controller_configured=$controller_configured"
    echo "latest_controller_status=$latest_controller_status"
    echo "controller_evidence_fresh=$controller_fresh"
    echo "controller_age_hours=$controller_age_hours"
    echo "policy_due_run=$RUN_POLICY_DUE_RUN"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "blocking_reasons:"
    jq -r '.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "validation_issues:"
    if [[ -s "$validation_evidence_file" ]]; then
      jq -r '.response.issues[]? | "- \(.)"' "$validation_evidence_file"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Policy rollout evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_policy_due_run_evidence() {
  local due_run_response
  local due_run_evidence_file="$EVIDENCE_DIR/policy-rollout-due-run-evidence.json"

  due_run_response="$(fetch_json POST /api/policy/rollout/run-due)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$due_run_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$due_run_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$due_run_evidence_file"
}

capture_policy_orchestration_validation_evidence() {
  local validation_response
  local validation_evidence_file="$EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"

  validation_response="$(fetch_json POST /api/policy/rollout/orchestration/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$validation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$validation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$validation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/policy/rollout/orchestration/readiness >/dev/null

if [[ "$RUN_POLICY_DUE_RUN" == "1" ]]; then
  capture_policy_due_run_evidence
else
  echo "skipping policy rollout due-run; set RUN_STAGE2_POLICY_DUE_RUN=1 to include due-run evidence" >&2
fi

capture_policy_orchestration_validation_evidence
fetch_json GET /api/policy/rollout/orchestration/readiness >/dev/null
write_summary
