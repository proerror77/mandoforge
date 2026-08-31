#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-mcp-gateway-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
TEAM_ID="${MANDOFORGE_STAGE2_TEAM_ID:-}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/mcp-gateway-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_MCP_DUE_RUN="${RUN_STAGE2_MCP_DUE_RUN:-0}"
RUN_MCP_ROLLBACK="${RUN_STAGE2_MCP_ROLLBACK:-0}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-}}"

auth_headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "MCP Gateway evidence gate requires $1" >&2
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
    echo "MCP Gateway evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_team_discovery() {
  local status="$1"
  local source="$2"
  local team_id="$3"
  local organization_id="${4:-}"
  local target="$EVIDENCE_DIR/team-discovery.json"

  jq -n \
    --arg status "$status" \
    --arg source "$source" \
    --arg team_id "$team_id" \
    --arg organization_id "$organization_id" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    '{
      status: $status,
      source: $source,
      team_id: (if $team_id == "" then null else $team_id end),
      organization_id: (if $organization_id == "" then null else $organization_id end),
      generated_at: $generated_at
    }' >"$target"
}

discover_team_id() {
  if [[ -n "$TEAM_ID" ]]; then
    write_team_discovery "configured" "MANDOFORGE_STAGE2_TEAM_ID" "$TEAM_ID"
    return 0
  fi

  local organizations_file
  organizations_file="$(fetch_json GET /api/organizations)"
  local organization_id
  while IFS= read -r organization_id; do
    [[ -z "$organization_id" ]] && continue
    local teams_file
    teams_file="$(fetch_json GET "/api/organizations/$organization_id/teams")"
    TEAM_ID="$(jq -r 'map(select((.archived_at // null) == null)) | .[0].id // empty' "$teams_file")"
    if [[ -n "$TEAM_ID" ]]; then
      write_team_discovery "discovered" "api" "$TEAM_ID" "$organization_id"
      return 0
    fi
  done < <(jq -r 'map(select((.archived_at // null) == null)) | .[].id' "$organizations_file")

  write_team_discovery "unavailable" "api" ""
  echo "MCP Gateway evidence gate could not discover an active team; set MANDOFORGE_STAGE2_TEAM_ID or create an organization team first" >&2
  exit 1
}

write_summary() {
  local rollout_summary_file="$EVIDENCE_DIR/api-teams-$TEAM_ID-mcp-servers-rollouts-summary.json"
  local rollout_runs_file="$EVIDENCE_DIR/api-teams-$TEAM_ID-mcp-servers-rollouts-runs.json"
  local deployment_evidence_file="$EVIDENCE_DIR/mcp-deployment-validation-evidence.json"
  local due_run_evidence_file="$EVIDENCE_DIR/mcp-rollout-due-run-evidence.json"
  local rollback_file="$EVIDENCE_DIR/mcp-rollback-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local server_count
  local pending_rollout_count
  local due_pending_count
  local failed_preflight_count
  local rollout_run_count
  local latest_run_status
  local production_ops_status
  local production_orchestration_status
  local deployment_status
  local deployment_validation_status
  local deployment_evidence_status
  local due_run_evidence_status
  local due_run_status
  local controller_required
  local controller_configured
  local latest_controller_status
  local deployment_controller_fresh
  local deployment_controller_age_hours
  local blocked_count

  server_count="$(jq -r '.server_count // 0' "$rollout_summary_file")"
  pending_rollout_count="$(jq -r '.pending_rollout_count // 0' "$rollout_summary_file")"
  due_pending_count="$(jq -r '.due_pending_count // 0' "$rollout_summary_file")"
  failed_preflight_count="$(jq -r '.failed_preflight_count // 0' "$rollout_summary_file")"
  rollout_run_count="$(jq -r '.run_count // 0' "$rollout_runs_file")"
  latest_run_status="$(jq -r '.latest_run.status // "none"' "$rollout_runs_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$rollout_runs_file")"
  production_orchestration_status="$(jq -r '.production_orchestration.status // "unknown"' "$rollout_runs_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$rollout_runs_file")"
  deployment_evidence_status="missing"
  deployment_validation_status="unknown"
  if [[ -s "$deployment_evidence_file" ]]; then
    deployment_evidence_status="$(jq -r '.status // "unknown"' "$deployment_evidence_file")"
    deployment_validation_status="$(jq -r '.response.status // "unknown"' "$deployment_evidence_file")"
  fi
  due_run_evidence_status="not_requested"
  due_run_status="not_run"
  if [[ -s "$due_run_evidence_file" ]]; then
    due_run_evidence_status="$(jq -r '.status // "unknown"' "$due_run_evidence_file")"
    due_run_status="$(jq -r '.response.status // "unknown"' "$due_run_evidence_file")"
  fi
  controller_required="$(jq -r '.deployment_readiness.controller_required // false' "$rollout_runs_file")"
  controller_configured="$(jq -r '.deployment_readiness.controller_configured // false' "$rollout_runs_file")"
  latest_controller_status="$(jq -r '.deployment_readiness.latest_controller_status // "none"' "$rollout_runs_file")"
  deployment_controller_fresh="$(jq -r '.deployment_readiness.controller_evidence_fresh // false' "$rollout_runs_file")"
  deployment_controller_age_hours="$(jq -r '.deployment_readiness.latest_controller_age_hours // "none"' "$rollout_runs_file")"
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .production_orchestration.production_blocked,
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$rollout_runs_file")"

  {
    echo "mcp_team_id=$TEAM_ID"
    echo "server_count=$server_count"
    echo "pending_rollout_count=$pending_rollout_count"
    echo "due_pending_count=$due_pending_count"
    echo "failed_preflight_count=$failed_preflight_count"
    echo "rollout_run_count=$rollout_run_count"
    echo "latest_run_status=$latest_run_status"
    echo "production_ops_status=$production_ops_status"
    echo "production_orchestration_status=$production_orchestration_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_evidence_status=$deployment_evidence_status"
    echo "deployment_validation_status=$deployment_validation_status"
    echo "due_run_evidence_status=$due_run_evidence_status"
    echo "due_run_status=$due_run_status"
    echo "deployment_controller_required=$controller_required"
    echo "deployment_controller_configured=$controller_configured"
    echo "latest_deployment_controller_status=$latest_controller_status"
    echo "deployment_controller_evidence_fresh=$deployment_controller_fresh"
    echo "deployment_controller_age_hours=$deployment_controller_age_hours"
    echo "production_blocked_count=$blocked_count"
    echo "mcp_due_run=$RUN_MCP_DUE_RUN"
    echo "mcp_rollback_run=$RUN_MCP_ROLLBACK"
    if [[ -s "$rollback_file" ]]; then
      echo "mcp_rollback_status=$(jq -r '.status // "unknown"' "$rollback_file")"
      echo "mcp_rollback_server_id=$(jq -r '.server_id // "none"' "$rollback_file")"
      echo "mcp_rollback_rollout_id=$(jq -r '.rollout_id // "none"' "$rollback_file")"
    fi
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "rollout_attention_items:"
    jq -r '.attention_items[]? | "- \(.reason // .message // .kind // "attention")"' "$rollout_summary_file"
    echo
    echo "run_attention_items:"
    jq -r '.attention_items[]? | "- \(.message // .kind // "attention")"' "$rollout_runs_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$rollout_runs_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "MCP Gateway evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_deployment_validation_evidence() {
  local deployment_response
  local deployment_evidence_file="$EVIDENCE_DIR/mcp-deployment-validation-evidence.json"

  deployment_response="$(fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/deployment/validate")"
  jq -n \
    --arg status "captured" \
    --arg response_file "$deployment_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$deployment_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$deployment_evidence_file"
}

capture_due_run_evidence() {
  local due_run_response
  local due_run_evidence_file="$EVIDENCE_DIR/mcp-rollout-due-run-evidence.json"

  due_run_response="$(fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/rollouts/run-due")"
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

capture_rollback_evidence() {
  local rollout_summary_file="$EVIDENCE_DIR/api-teams-$TEAM_ID-mcp-servers-rollouts-summary.json"
  local selected_file="$EVIDENCE_DIR/mcp-rollback-candidate.json"
  local rollback_file="$EVIDENCE_DIR/mcp-rollback-evidence.json"
  local server_id
  local rollout_id

  jq '[
      .latest_rollouts[]?
      | select(.status == "applied")
      | select((.server_id // null) != null)
      | select((.rollout_id // null) != null)
    ][0] // {}' "$rollout_summary_file" >"$selected_file"

  server_id="$(jq -r '.server_id // empty' "$selected_file")"
  rollout_id="$(jq -r '.rollout_id // empty' "$selected_file")"

  if [[ -z "$server_id" || -z "$rollout_id" ]]; then
    jq -n \
      --arg status "blocked" \
      --arg reason "no_applied_mcp_rollout_available_for_rollback" \
      --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
      '{
        status: $status,
        reason: $reason,
        generated_at: $generated_at
      }' >"$rollback_file"
    echo "MCP Gateway rollback evidence requested, but no applied rollout is available to roll back" >&2
    exit 1
  fi

  local rollback_response
  rollback_response="$(fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/$server_id/rollouts/$rollout_id/rollback")"
  jq -n \
    --arg status "captured" \
    --arg server_id "$server_id" \
    --arg rollout_id "$rollout_id" \
    --arg response_file "$rollback_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollback_response" \
    '{
      status: $status,
      server_id: $server_id,
      rollout_id: $rollout_id,
      response_file: $response_file,
      generated_at: $generated_at,
      response: ($response[0] // {})
    }' >"$rollback_file"
}

require_cmd curl
require_cmd jq

mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
discover_team_id
fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/summary" >/dev/null
fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/runs" >/dev/null
capture_deployment_validation_evidence

if [[ "$RUN_MCP_DUE_RUN" == "1" ]]; then
  capture_due_run_evidence
else
  echo "skipping MCP rollout due-run; set RUN_STAGE2_MCP_DUE_RUN=1 to include due-run evidence" >&2
fi

fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/runs" >/dev/null
fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/summary" >/dev/null
if [[ "$RUN_MCP_ROLLBACK" == "1" ]]; then
  capture_rollback_evidence
  fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/runs" >/dev/null
  fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/summary" >/dev/null
else
  echo "skipping MCP rollout rollback; set RUN_STAGE2_MCP_ROLLBACK=1 to include rollback evidence" >&2
fi
write_summary
