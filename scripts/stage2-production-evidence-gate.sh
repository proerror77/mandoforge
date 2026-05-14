#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-stage2-production-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
SCHEDULER_TOKEN="${MANDOFORGE_SCHEDULER_TOKEN:-}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
RUN_VALIDATIONS="${RUN_STAGE2_PRODUCTION_VALIDATIONS:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
TEAM_ID="${MANDOFORGE_STAGE2_TEAM_ID:-}"
VERIFY_VALIDATION_COVERAGE="${VERIFY_STAGE2_VALIDATION_COVERAGE:-0}"

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

if [[ -n "$SCHEDULER_TOKEN" ]]; then
  auth_headers+=(-H "x-mandoforge-scheduler-token: $SCHEDULER_TOKEN")
fi

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
    echo "stage2 evidence gate request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

collect_readiness() {
  fetch_json GET /api/stage2/readiness >/dev/null
  fetch_json GET /api/tenant-isolation/readiness >/dev/null
  fetch_json GET /api/providers/summary >/dev/null
  fetch_json GET /api/providers/policy-gate >/dev/null
  fetch_json GET /api/providers/policy-gate/runs >/dev/null
  fetch_json GET /api/vault/readiness >/dev/null
  fetch_json GET /api/vault/health >/dev/null
  fetch_json GET /api/execution-jobs/worker-readiness >/dev/null
  fetch_json GET /api/remote-computers/readiness >/dev/null
  fetch_json GET /api/remote-computers/runner/readiness >/dev/null
  fetch_json GET /api/approvals/notification-routing/summary >/dev/null
  fetch_json GET /api/approvals/notifications/runs >/dev/null
  fetch_json GET /api/codex-app-server/control-plane/summary >/dev/null
  fetch_json GET /api/usage/finance-operations/summary >/dev/null
  fetch_json GET /api/observability >/dev/null
  fetch_json GET /api/observability/collector-readiness >/dev/null
  fetch_json GET /api/scheduler/summary >/dev/null
  fetch_json GET /api/scheduler/due-plan >/dev/null
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
  fetch_json POST /api/scheduler/run-due >/dev/null

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

  if [[ "${RUN_STAGE2_PROVIDER_ROLLOUT:-0}" == "1" ]]; then
    fetch_json POST /api/providers/production-rollout/run >/dev/null
    fetch_json POST /api/providers/production-rollout/rollback >/dev/null
  else
    echo "skipping provider production rollout/rollback; set RUN_STAGE2_PROVIDER_ROLLOUT=1 to include provider rollout evidence" >&2
  fi

  if [[ "${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-0}" == "1" ]]; then
    fetch_json POST /api/remote-computers/sidecars/recovery/run >/dev/null
  else
    echo "skipping Remote Computer sidecar recovery; set RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 to include replacement evidence" >&2
  fi

  if [[ "${RUN_STAGE2_APPROVAL_DELIVERY:-0}" == "1" ]]; then
    fetch_json POST /api/approvals/notifications/run >/dev/null
  else
    echo "skipping approval notification delivery; set RUN_STAGE2_APPROVAL_DELIVERY=1 to include delivery evidence" >&2
  fi

  if [[ "${RUN_STAGE2_CODEX_STALE_POLL:-0}" == "1" ]]; then
    fetch_json POST /api/codex-app-server/runs/poll-stale >/dev/null
  else
    echo "skipping Codex App Server stale poll; set RUN_STAGE2_CODEX_STALE_POLL=1 to include stale-run supervision evidence" >&2
  fi

  if [[ "${RUN_STAGE2_EVAL_RELEASE_AUTOMATION:-0}" == "1" ]]; then
    fetch_json POST /api/eval/suites/stage2-regression >/dev/null
    fetch_json POST /api/agents/releases/run-due >/dev/null
  else
    echo "skipping eval/release automation; set RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 to include regression and release due-run evidence" >&2
  fi

  if [[ "${RUN_STAGE2_OBSERVABILITY_REMEDIATION:-0}" == "1" ]]; then
    fetch_json POST /api/observability/remediation/run >/dev/null
  else
    echo "skipping observability remediation run; set RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 to include remediation evidence" >&2
  fi

  if [[ "${RUN_STAGE2_FINANCE_CONTROLLERS:-0}" == "1" ]]; then
    fetch_json POST /api/usage/finance-operations/run >/dev/null
    fetch_json POST /api/usage/finance-operations/reconcile >/dev/null
  else
    echo "skipping finance close/reconciliation controllers; set RUN_STAGE2_FINANCE_CONTROLLERS=1 to include accounting evidence" >&2
  fi
}

resolve_requirement_endpoint() {
  local endpoint="$1"
  if [[ "$endpoint" == ./* ]]; then
    return 1
  fi
  if [[ "$endpoint" == *"{team_id}"* ]]; then
    if [[ -z "$TEAM_ID" ]]; then
      return 1
    fi
    endpoint="${endpoint//\{team_id\}/$TEAM_ID}"
  fi
  printf '%s\n' "$endpoint"
}

verify_readiness_inventory_coverage() {
  local readiness_file="$EVIDENCE_DIR/api-stage2-readiness.json"
  local missing=()
  local endpoint
  local resolved
  local expected_file

  while IFS= read -r endpoint; do
    if [[ -z "$endpoint" ]]; then
      continue
    fi
    if ! resolved="$(resolve_requirement_endpoint "$endpoint")"; then
      continue
    fi
    expected_file="$EVIDENCE_DIR/$(slugify "$resolved").json"
    if [[ ! -s "$expected_file" ]]; then
      missing+=("$resolved")
    fi
  done < <(jq -r '.evidence_requirements[]?.readiness_endpoints[]?' "$readiness_file" | sort -u)

  if (( ${#missing[@]} > 0 )); then
    printf 'stage2 evidence gate did not collect declared readiness endpoint: %s\n' "${missing[@]}" >&2
    exit 1
  fi
}

write_endpoint_coverage() {
  local field="$1"
  local label="$2"
  local fail_on_missing="$3"
  local readiness_file="$EVIDENCE_DIR/api-stage2-readiness.json"
  local declared_file="$EVIDENCE_DIR/${label}-declared-endpoints.txt"
  local missing_file="$EVIDENCE_DIR/${label}-missing-endpoints.txt"
  local endpoint
  local resolved
  local expected_file
  local missing_count

  : >"$declared_file"
  : >"$missing_file"

  while IFS= read -r endpoint; do
    if [[ -z "$endpoint" ]]; then
      continue
    fi
    if ! resolved="$(resolve_requirement_endpoint "$endpoint")"; then
      continue
    fi
    echo "$resolved" >>"$declared_file"
    expected_file="$EVIDENCE_DIR/$(slugify "$resolved").json"
    if [[ ! -s "$expected_file" ]]; then
      echo "$resolved" >>"$missing_file"
    fi
  done < <(jq -r ".evidence_requirements[]?.${field}[]?" "$readiness_file" | sort -u)

  missing_count="$(grep -c . "$missing_file" || true)"
  if [[ "$fail_on_missing" == "1" && "$missing_count" != "0" ]]; then
    echo "stage2 evidence gate is missing declared $label endpoint evidence:" >&2
    sed 's/^/- /' "$missing_file" >&2
    exit 1
  fi
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-stage2-readiness.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local open_gap_count
  local completion_blocked
  local evidence_requirement_count
  local validation_declared_count
  local validation_missing_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  open_gap_count="$(jq -r '.open_gap_count // 0' "$readiness_file")"
  completion_blocked="$(jq -r '.completion_blocked // true' "$readiness_file")"
  evidence_requirement_count="$(jq -r '.evidence_requirements | length' "$readiness_file")"
  validation_declared_count="$(grep -c . "$EVIDENCE_DIR/validation-declared-endpoints.txt" 2>/dev/null || true)"
  validation_missing_count="$(grep -c . "$EVIDENCE_DIR/validation-missing-endpoints.txt" 2>/dev/null || true)"

  {
    echo "stage2_status=$status"
    echo "completion_blocked=$completion_blocked"
    echo "open_gap_count=$open_gap_count"
    echo "evidence_requirement_count=$evidence_requirement_count"
    echo "validation_declared_endpoint_count=$validation_declared_count"
    echo "validation_missing_endpoint_count=$validation_missing_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "validations_run=$RUN_VALIDATIONS"
    echo "validation_coverage_required=$VERIFY_VALIDATION_COVERAGE"
    echo "team_id=${TEAM_ID:-<none>}"
    echo
    echo "open_gaps:"
    jq -r '.open_gaps[]? | "- \(.)"' "$readiness_file"
    echo
    echo "missing_validation_endpoints:"
    sed 's/^/- /' "$EVIDENCE_DIR/validation-missing-endpoints.txt" 2>/dev/null || true
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
verify_readiness_inventory_coverage
write_endpoint_coverage "validation_endpoints" "validation" "0"

if [[ "$RUN_VALIDATIONS" == "1" ]]; then
  run_controller_validations
  collect_readiness
  verify_readiness_inventory_coverage
  write_endpoint_coverage "validation_endpoints" "validation" "$VERIFY_VALIDATION_COVERAGE"
else
  echo "controller validations skipped; set RUN_STAGE2_PRODUCTION_VALIDATIONS=1 to execute validation endpoints" >&2
  if [[ "$VERIFY_VALIDATION_COVERAGE" == "1" ]]; then
    write_endpoint_coverage "validation_endpoints" "validation" "1"
  fi
fi

write_summary
