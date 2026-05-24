#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-scheduler-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
SCHEDULER_TOKEN="${MANDOFORGE_SCHEDULER_TOKEN:-}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/scheduler-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

if [[ -n "$SCHEDULER_TOKEN" ]]; then
  auth_headers+=(-H "x-mandoforge-scheduler-token: $SCHEDULER_TOKEN")
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "scheduler evidence gate requires $1" >&2
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
    echo "scheduler evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local summary_file="$EVIDENCE_DIR/api-scheduler-summary.json"
  local plan_file="$EVIDENCE_DIR/api-scheduler-due-plan.json"
  local run_file="$EVIDENCE_DIR/api-scheduler-run-due.json"
  local report_file="$EVIDENCE_DIR/summary.txt"
  local status
  local deployment_status
  local production_blocked
  local deployment_controller_required
  local deployment_controller_configured
  local latest_deployment_controller_status
  local deployment_controller_fresh
  local deployment_controller_age_hours
  local actionable_count
  local blocked_count
  local run_status
  local run_action_count
  local recent_run_count
  local workflow_scheduled_status
  local workflow_scheduled_due_count
  local workflow_scheduled_activated_count

  status="$(jq -r '.status // "unknown"' "$summary_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$summary_file")"
  production_blocked="$(jq -r 'if ((.deployment_readiness // {}) | has("production_blocked")) then .deployment_readiness.production_blocked else true end' "$summary_file")"
  deployment_controller_required="$(jq -r '.deployment_readiness.controller_required // false' "$summary_file")"
  deployment_controller_configured="$(jq -r '.deployment_readiness.controller_configured // false' "$summary_file")"
  latest_deployment_controller_status="$(jq -r '.deployment_readiness.latest_controller_status // "none"' "$summary_file")"
  deployment_controller_fresh="$(jq -r '.deployment_readiness.controller_evidence_fresh // false' "$summary_file")"
  deployment_controller_age_hours="$(jq -r '.deployment_readiness.latest_controller_age_hours // "none"' "$summary_file")"
  actionable_count="$(jq -r '.actionable_count // .plan.actionable_count // 0' "$plan_file")"
  blocked_count="$(jq -r '.blocked_count // .plan.blocked_count // 0' "$plan_file")"
  run_status="$(jq -r '.status // "unknown"' "$run_file")"
  run_action_count="$(jq -r '.actions | length // 0' "$run_file")"
  recent_run_count="$(jq -r '.recent_run_count // 0' "$summary_file")"
  workflow_scheduled_status="$(jq -r '.workflow_scheduled_steps.status // "not_reported"' "$run_file")"
  workflow_scheduled_due_count="$(jq -r '.workflow_scheduled_steps.due_step_count // 0' "$run_file")"
  workflow_scheduled_activated_count="$(jq -r '.workflow_scheduled_steps.activated_count // 0' "$run_file")"

  {
    echo "scheduler_summary_status=$status"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_production_blocked=$production_blocked"
    echo "deployment_controller_required=$deployment_controller_required"
    echo "deployment_controller_configured=$deployment_controller_configured"
    echo "latest_deployment_controller_status=$latest_deployment_controller_status"
    echo "deployment_controller_fresh=$deployment_controller_fresh"
    echo "deployment_controller_age_hours=$deployment_controller_age_hours"
    echo "due_plan_actionable_count=$actionable_count"
    echo "due_plan_blocked_count=$blocked_count"
    echo "run_due_status=$run_status"
    echo "run_due_action_count=$run_action_count"
    echo "workflow_scheduled_step_status=$workflow_scheduled_status"
    echo "workflow_scheduled_step_due_count=$workflow_scheduled_due_count"
    echo "workflow_scheduled_step_activated_count=$workflow_scheduled_activated_count"
    echo "recent_run_count_before_run=$recent_run_count"
    echo "scheduler_token_supplied=$([[ -n "$SCHEDULER_TOKEN" ]] && echo true || echo false)"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$summary_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$summary_file"
    echo
    echo "due_plan_items:"
    jq -r '.actions[]? | "- \(.area)/\(.action): \(.status) - \(.reason)"' "$plan_file"
    echo
    echo "run_due_actions:"
    jq -r '.actions[]? | "- \(.)"' "$run_file"
  } >"$report_file"

  cat "$report_file"

  if [[ "$production_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Scheduler evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/scheduler/summary >/dev/null
fetch_json GET /api/scheduler/due-plan >/dev/null
fetch_json POST /api/scheduler/deployment/validate >/dev/null
fetch_json GET /api/scheduler/summary >/dev/null
fetch_json POST /api/scheduler/run-due >/dev/null
write_summary
