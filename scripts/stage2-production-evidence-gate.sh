#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-stage2-production-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
RUN_VALIDATIONS="${RUN_STAGE2_PRODUCTION_VALIDATIONS:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
TEAM_ID="${MANDOFORGE_STAGE2_TEAM_ID:-}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "stage2 production evidence gate requires $1" >&2
    exit 1
  fi
}

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"

  if [[ "$method" == "GET" ]]; then
    curl -fsS "${auth_headers[@]}" "$BASE_URL$path" | tee "$target" >/dev/null
  else
    curl -fsS -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d '{}' \
      "$BASE_URL$path" | tee "$target" >/dev/null
  fi
  echo "$target"
}

collect_readiness() {
  fetch_json GET /api/stage2/readiness >/dev/null
  fetch_json GET /api/tenant-isolation/readiness >/dev/null
  fetch_json GET /api/providers/summary >/dev/null
  fetch_json GET /api/providers/policy-gate >/dev/null
  fetch_json GET /api/providers/policy-gate/runs >/dev/null
  fetch_json GET /api/vault/readiness >/dev/null
  fetch_json GET /api/execution-jobs/worker-readiness >/dev/null
  fetch_json GET /api/remote-computers/readiness >/dev/null
  fetch_json GET /api/remote-computers/runner/readiness >/dev/null
  fetch_json GET /api/approvals/notification-routing/summary >/dev/null
  fetch_json GET /api/approvals/notifications/runs >/dev/null
  fetch_json GET /api/codex-app-server/control-plane/summary >/dev/null
  fetch_json GET /api/usage/finance-operations/summary >/dev/null
  fetch_json GET /api/observability/collector-readiness >/dev/null
  fetch_json GET /api/scheduler/summary >/dev/null
  fetch_json GET /api/policy/rollout/orchestration/readiness >/dev/null
  fetch_json GET /api/agents/releases/automation-runs >/dev/null

  if [[ -n "$TEAM_ID" ]]; then
    fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/summary" >/dev/null
    fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/runs" >/dev/null
  fi
}

run_controller_validations() {
  fetch_json POST /api/tenant-isolation/routing/validate >/dev/null
  fetch_json POST /api/providers/policy-gate/run >/dev/null
  fetch_json POST /api/providers/deployment/validate >/dev/null
  fetch_json POST /api/policy/rollout/orchestration/validate >/dev/null
  fetch_json POST /api/vault/kms/recovery/validate >/dev/null
  fetch_json POST /api/execution-jobs/worker-load-validation/run >/dev/null
  fetch_json POST /api/remote-computers/state-sync/validate >/dev/null
  fetch_json POST /api/approvals/notifications/deployment/validate >/dev/null
  fetch_json POST /api/approvals/notifications/ops/validate >/dev/null
  fetch_json POST /api/codex-app-server/deployment/validate >/dev/null
  fetch_json POST /api/codex-app-server/ops/validate >/dev/null
  fetch_json POST /api/agents/releases/deployment/validate >/dev/null
  fetch_json POST /api/agents/releases/orchestration/validate >/dev/null
  fetch_json POST /api/observability/collector/deployment/validate >/dev/null
  fetch_json POST /api/observability/collector/cluster/validate >/dev/null

  if [[ -n "$TEAM_ID" ]]; then
    fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/deployment/validate" >/dev/null
    fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/rollouts/run-due" >/dev/null
  else
    echo "skipping MCP connector validation; set MANDOFORGE_STAGE2_TEAM_ID to include team-scoped MCP rollout evidence" >&2
  fi

  if [[ "${RUN_STAGE2_SECRET_LIFECYCLE:-0}" == "1" ]]; then
    fetch_json POST /api/vault/kms/rotation/run >/dev/null
  else
    echo "skipping KMS rotation run; set RUN_STAGE2_SECRET_LIFECYCLE=1 to include secret lifecycle evidence" >&2
  fi

  if [[ "${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-0}" == "1" ]]; then
    fetch_json POST /api/remote-computers/sidecars/recovery/run >/dev/null
  else
    echo "skipping Remote Computer sidecar recovery; set RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 to include replacement evidence" >&2
  fi

  if [[ "${RUN_STAGE2_FINANCE_CONTROLLERS:-0}" == "1" ]]; then
    fetch_json POST /api/usage/finance-operations/run >/dev/null
    fetch_json POST /api/usage/finance-operations/reconcile >/dev/null
  else
    echo "skipping finance close/reconciliation controllers; set RUN_STAGE2_FINANCE_CONTROLLERS=1 to include accounting evidence" >&2
  fi
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-stage2-readiness.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local open_gap_count
  local completion_blocked
  local evidence_requirement_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  open_gap_count="$(jq -r '.open_gap_count // 0' "$readiness_file")"
  completion_blocked="$(jq -r '.completion_blocked // true' "$readiness_file")"
  evidence_requirement_count="$(jq -r '.evidence_requirements | length' "$readiness_file")"

  {
    echo "stage2_status=$status"
    echo "completion_blocked=$completion_blocked"
    echo "open_gap_count=$open_gap_count"
    echo "evidence_requirement_count=$evidence_requirement_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "validations_run=$RUN_VALIDATIONS"
    echo "team_id=${TEAM_ID:-<none>}"
    echo
    echo "open_gaps:"
    jq -r '.open_gaps[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 production evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
collect_readiness

if [[ "$RUN_VALIDATIONS" == "1" ]]; then
  run_controller_validations
  collect_readiness
else
  echo "controller validations skipped; set RUN_STAGE2_PRODUCTION_VALIDATIONS=1 to execute validation endpoints" >&2
fi

write_summary
