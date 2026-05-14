#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-eval-release-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/eval-release-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_EVAL_RELEASE_AUTOMATION="${RUN_STAGE2_EVAL_RELEASE_AUTOMATION:-0}"
RUN_EVAL_RELEASE_ROLLBACK="${RUN_STAGE2_EVAL_RELEASE_ROLLBACK:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "eval/release evidence gate requires $1" >&2
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
    echo "eval/release evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local rollout_summary_file="$EVIDENCE_DIR/api-agents-releases-summary.json"
  local automation_file="$EVIDENCE_DIR/api-agents-releases-automation-runs.json"
  local deployment_file="$EVIDENCE_DIR/api-agents-releases-deployment-validate.json"
  local orchestration_file="$EVIDENCE_DIR/api-agents-releases-orchestration-validate.json"
  local rollback_file="$EVIDENCE_DIR/eval-release-rollback-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local release_count
  local pending_count
  local promoted_count
  local rejected_count
  local rolled_back_count
  local automation_run_count
  local latest_run_status
  local production_ops_status
  local production_orchestration_status
  local deployment_status
  local deployment_validation_status
  local orchestration_validation_status
  local deployment_controller_required
  local orchestration_controller_required
  local deployment_controller_configured
  local orchestration_controller_configured
  local blocked_count

  release_count="$(jq -r '.release_count // 0' "$rollout_summary_file")"
  pending_count="$(jq -r '.pending_count // 0' "$rollout_summary_file")"
  promoted_count="$(jq -r '.promoted_count // 0' "$rollout_summary_file")"
  rejected_count="$(jq -r '.rejected_count // 0' "$rollout_summary_file")"
  rolled_back_count="$(jq -r '.rolled_back_count // 0' "$rollout_summary_file")"
  automation_run_count="$(jq -r '.run_count // 0' "$automation_file")"
  latest_run_status="$(jq -r '.latest_run.status // "none"' "$automation_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$automation_file")"
  production_orchestration_status="$(jq -r '.production_orchestration.status // "unknown"' "$automation_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$automation_file")"
  deployment_validation_status="$(jq -r '.status // "unknown"' "$deployment_file")"
  orchestration_validation_status="$(jq -r '.status // "unknown"' "$orchestration_file")"
  deployment_controller_required="$(jq -r '.deployment_readiness.controller_required // false' "$automation_file")"
  orchestration_controller_required="$(jq -r '.production_orchestration.controller_required // false' "$automation_file")"
  deployment_controller_configured="$(jq -r '.deployment_readiness.controller_configured // false' "$automation_file")"
  orchestration_controller_configured="$(jq -r '.production_orchestration.controller_configured // false' "$automation_file")"
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .production_orchestration.production_blocked,
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$automation_file")"

  {
    echo "release_count=$release_count"
    echo "pending_count=$pending_count"
    echo "promoted_count=$promoted_count"
    echo "rejected_count=$rejected_count"
    echo "rolled_back_count=$rolled_back_count"
    echo "automation_run_count=$automation_run_count"
    echo "latest_run_status=$latest_run_status"
    echo "production_ops_status=$production_ops_status"
    echo "production_orchestration_status=$production_orchestration_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_validation_status=$deployment_validation_status"
    echo "orchestration_validation_status=$orchestration_validation_status"
    echo "deployment_controller_required=$deployment_controller_required"
    echo "deployment_controller_configured=$deployment_controller_configured"
    echo "orchestration_controller_required=$orchestration_controller_required"
    echo "orchestration_controller_configured=$orchestration_controller_configured"
    echo "production_blocked_count=$blocked_count"
    echo "eval_release_automation=$RUN_EVAL_RELEASE_AUTOMATION"
    echo "eval_release_rollback=$RUN_EVAL_RELEASE_ROLLBACK"
    if [[ -s "$rollback_file" ]]; then
      echo "eval_release_rollback_status=$(jq -r '.status // "unknown"' "$rollback_file")"
      echo "eval_release_rollback_agent_id=$(jq -r '.agent_id // "none"' "$rollback_file")"
      echo "eval_release_rollback_release_id=$(jq -r '.release_id // "none"' "$rollback_file")"
    fi
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "release_attention_items:"
    jq -r '.attention_items[]? | "- \(.reason // .message // .kind // \"attention\")"' "$rollout_summary_file"
    echo
    echo "automation_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // \"attention\")"' "$automation_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$automation_file"
    echo
    echo "orchestration_blocking_reasons:"
    jq -r '.production_orchestration.blocking_reasons[]? | "- \(.)"' "$automation_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "eval/release evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_rollback_evidence() {
  local rollout_summary_file="$EVIDENCE_DIR/api-agents-releases-summary.json"
  local selected_file="$EVIDENCE_DIR/eval-release-rollback-candidate.json"
  local rollback_file="$EVIDENCE_DIR/eval-release-rollback-evidence.json"
  local agent_id
  local release_id

  jq '[
      .latest_promoted_by_environment[]?
      | select((.agent_id // null) != null)
      | select((.release_id // null) != null)
    ][0] // {}' "$rollout_summary_file" >"$selected_file"

  agent_id="$(jq -r '.agent_id // empty' "$selected_file")"
  release_id="$(jq -r '.release_id // empty' "$selected_file")"

  if [[ -z "$agent_id" || -z "$release_id" ]]; then
    jq -n \
      --arg status "blocked" \
      --arg reason "no_promoted_agent_release_available_for_rollback" \
      --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
      '{
        status: $status,
        reason: $reason,
        generated_at: $generated_at
      }' >"$rollback_file"
    echo "eval/release rollback evidence requested, but no promoted release is available to roll back" >&2
    exit 1
  fi

  local rollback_response
  rollback_response="$(fetch_json POST "/api/agents/$agent_id/releases/$release_id/rollback")"
  jq -n \
    --arg status "captured" \
    --arg agent_id "$agent_id" \
    --arg release_id "$release_id" \
    --arg response_file "$rollback_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollback_response" \
    '{
      status: $status,
      agent_id: $agent_id,
      release_id: $release_id,
      response_file: $response_file,
      generated_at: $generated_at,
      response: ($response[0] // {})
    }' >"$rollback_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/agents/releases/summary >/dev/null
fetch_json GET /api/agents/releases/automation-runs >/dev/null
fetch_json POST /api/agents/releases/deployment/validate >/dev/null
fetch_json POST /api/agents/releases/orchestration/validate >/dev/null

if [[ "$RUN_EVAL_RELEASE_AUTOMATION" == "1" ]]; then
  fetch_json POST /api/eval/suites/stage2-regression >/dev/null
  fetch_json POST /api/agents/releases/run-due >/dev/null
else
  echo "skipping eval/release automation; set RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 to include regression and release due-run evidence" >&2
fi

fetch_json GET /api/agents/releases/automation-runs >/dev/null
fetch_json GET /api/agents/releases/summary >/dev/null
if [[ "$RUN_EVAL_RELEASE_ROLLBACK" == "1" ]]; then
  capture_rollback_evidence
  fetch_json GET /api/agents/releases/automation-runs >/dev/null
  fetch_json GET /api/agents/releases/summary >/dev/null
else
  echo "skipping eval/release rollback; set RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1 to include rollback evidence" >&2
fi
write_summary
