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
RUN_COMPLETION_AUDIT="${RUN_STAGE2_COMPLETION_AUDIT:-1}"
MAX_EVIDENCE_AGE_HOURS="${STAGE2_EVIDENCE_MAX_AGE_HOURS:-24}"

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

file_mtime_epoch() {
  local path="$1"
  if stat -f %m "$path" >/dev/null 2>&1; then
    stat -f %m "$path"
    return 0
  fi
  stat -c %Y "$path"
}

artifact_is_fresh() {
  local path="$1"
  local now_epoch
  local mtime_epoch
  local max_age_seconds

  [[ -s "$path" ]] || return 1
  if [[ "$MAX_EVIDENCE_AGE_HOURS" == "0" ]]; then
    return 0
  fi

  now_epoch="$(date -u +%s)"
  mtime_epoch="$(file_mtime_epoch "$path")"
  max_age_seconds=$((MAX_EVIDENCE_AGE_HOURS * 3600))
  [[ $((now_epoch - mtime_epoch)) -le "$max_age_seconds" ]]
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

fetch_file() {
  local method="$1"
  local path="$2"
  local target="$3"
  local metadata="$4"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "stage2 evidence gate request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  cp "$response_body" "$target"
  local byte_count
  byte_count="$(wc -c <"$target" | tr -d ' ')"
  jq -n \
    --arg path "$path" \
    --arg target "$target" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson http_status "$http_status" \
    --argjson byte_count "$byte_count" \
    '{
      path: $path,
      target: $target,
      generated_at: $generated_at,
      http_status: $http_status,
      byte_count: $byte_count
    }' >"$metadata"
  rm -f "$response_body"
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
  echo "no active team discovered; MCP connector evidence will remain unresolved until a team exists or MANDOFORGE_STAGE2_TEAM_ID is set" >&2
}

run_local_script_validation() {
  local script_path="$1"
  local label
  label="local-script-$(slugify "$script_path")"
  local target="$EVIDENCE_DIR/$label.json"
  local stdout_file
  local stderr_file
  local status="passed"
  local exit_code=0
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"

  if [[ ! -x "$script_path" ]]; then
    status="failed"
    exit_code=127
    printf 'local validation script is missing or not executable: %s\n' "$script_path" >"$stderr_file"
  else
    set +e
    "$script_path" >"$stdout_file" 2>"$stderr_file"
    exit_code="$?"
    set -e
    if [[ "$exit_code" != "0" ]]; then
      status="failed"
    fi
  fi

  jq -n \
    --arg script "$script_path" \
    --arg status "$status" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg stdout "$(sed -n '1,80p' "$stdout_file")" \
    --arg stderr "$(sed -n '1,80p' "$stderr_file")" \
    --argjson exit_code "$exit_code" \
    '{
      script: $script,
      status: $status,
      generated_at: $generated_at,
      exit_code: $exit_code,
      stdout: $stdout,
      stderr: $stderr
    }' >"$target"

  rm -f "$stdout_file" "$stderr_file"

  if [[ "$status" != "passed" ]]; then
    echo "local validation script failed: $script_path" >&2
    cat "$target" >&2
    exit 1
  fi
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

run_local_validations() {
  if [[ "${RUN_STAGE2_UI_ACTIONBOOK:-0}" == "1" ]]; then
    run_local_script_validation ./scripts/verify-static-ui-actionbook.sh
  else
    echo "skipping static UI Actionbook validation; set RUN_STAGE2_UI_ACTIONBOOK=1 to include UI smoke evidence" >&2
  fi

  if [[ "${RUN_STAGE2_UI_STATIC_ASSETS:-0}" == "1" ]]; then
    run_local_script_validation ./scripts/verify-static-ui-assets.sh
  else
    echo "skipping static UI asset validation; set RUN_STAGE2_UI_STATIC_ASSETS=1 to include browserless UI asset evidence" >&2
  fi
}

capture_mcp_rollback_validation() {
  if [[ -z "$TEAM_ID" ]]; then
    echo "skipping MCP connector rollback; set MANDOFORGE_STAGE2_TEAM_ID or create an active team to include rollback evidence" >&2
    return 0
  fi

  local summary_file
  local selected_file="$EVIDENCE_DIR/mcp-rollback-candidate.json"
  local rollback_file="$EVIDENCE_DIR/mcp-rollback-evidence.json"
  local server_id
  local rollout_id

  summary_file="$(fetch_json GET "/api/teams/$TEAM_ID/mcp-servers/rollouts/summary")"
  jq '[
      .latest_rollouts[]?
      | select(.status == "applied")
      | select((.server_id // null) != null)
      | select((.rollout_id // null) != null)
    ][0] // {}' "$summary_file" >"$selected_file"

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
    echo "MCP connector rollback evidence requested, but no applied rollout is available to roll back" >&2
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

capture_mcp_deployment_validation() {
  if [[ -z "$TEAM_ID" ]]; then
    echo "skipping MCP connector deployment validation; set MANDOFORGE_STAGE2_TEAM_ID or create an active team to include deployment evidence" >&2
    return 0
  fi

  local deployment_response
  local deployment_file="$EVIDENCE_DIR/mcp-deployment-validation-evidence.json"

  deployment_response="$(fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/deployment/validate")"
  jq -n \
    --arg status "captured" \
    --arg team_id "$TEAM_ID" \
    --arg response_file "$deployment_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$deployment_response" \
    '{
      status: $status,
      team_id: $team_id,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$deployment_file"
}

capture_mcp_due_run_validation() {
  if [[ -z "$TEAM_ID" ]]; then
    echo "skipping MCP connector due-run; set MANDOFORGE_STAGE2_TEAM_ID or create an active team to include due-run evidence" >&2
    return 0
  fi

  local due_run_response
  local due_run_file="$EVIDENCE_DIR/mcp-rollout-due-run-evidence.json"

  due_run_response="$(fetch_json POST "/api/teams/$TEAM_ID/mcp-servers/rollouts/run-due")"
  jq -n \
    --arg status "captured" \
    --arg team_id "$TEAM_ID" \
    --arg response_file "$due_run_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$due_run_response" \
    '{
      status: $status,
      team_id: $team_id,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$due_run_file"
}

capture_eval_release_rollback_validation() {
  local summary_file
  local selected_file="$EVIDENCE_DIR/eval-release-rollback-candidate.json"
  local rollback_file="$EVIDENCE_DIR/eval-release-rollback-evidence.json"
  local agent_id
  local release_id

  summary_file="$(fetch_json GET /api/agents/releases/summary)"
  jq '[
      .latest_promoted_by_environment[]?
      | select((.agent_id // null) != null)
      | select((.release_id // null) != null)
    ][0] // {}' "$summary_file" >"$selected_file"

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

capture_eval_release_deployment_validation() {
  local deployment_response
  local deployment_file="$EVIDENCE_DIR/eval-release-deployment-validation-evidence.json"

  deployment_response="$(fetch_json POST /api/agents/releases/deployment/validate)"
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
    }' >"$deployment_file"
}

capture_eval_release_orchestration_validation() {
  local orchestration_response
  local orchestration_file="$EVIDENCE_DIR/eval-release-orchestration-validation-evidence.json"

  orchestration_response="$(fetch_json POST /api/agents/releases/orchestration/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$orchestration_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$orchestration_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$orchestration_file"
}

capture_eval_release_stage2_regression() {
  local regression_response
  local regression_file="$EVIDENCE_DIR/eval-release-stage2-regression-evidence.json"

  regression_response="$(fetch_json POST /api/eval/suites/stage2-regression)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$regression_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$regression_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$regression_file"
}

capture_eval_release_due_run() {
  local due_run_response
  local due_run_file="$EVIDENCE_DIR/eval-release-due-run-evidence.json"

  due_run_response="$(fetch_json POST /api/agents/releases/run-due)"
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
    }' >"$due_run_file"
}

capture_finance_close_validation() {
  local close_response
  local close_file="$EVIDENCE_DIR/finance-close-evidence.json"

  close_response="$(fetch_json POST /api/usage/finance-operations/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$close_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$close_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$close_file"
}

capture_finance_reconciliation_validation() {
  local reconciliation_response
  local reconciliation_file="$EVIDENCE_DIR/finance-reconciliation-evidence.json"

  reconciliation_response="$(fetch_json POST /api/usage/finance-operations/reconcile)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$reconciliation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$reconciliation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$reconciliation_file"
}

capture_finance_export_delivery_validation() {
  local delivery_response
  local delivery_file="$EVIDENCE_DIR/finance-export-delivery-evidence.json"

  delivery_response="$(fetch_json POST /api/usage/export/deliver)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$delivery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$delivery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$delivery_file"
}

capture_remote_computer_state_sync_validation() {
  local state_sync_response
  local state_sync_file="$EVIDENCE_DIR/remote-computer-state-sync-evidence.json"

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
    }' >"$state_sync_file"
}

capture_worker_load_validation() {
  local load_validation_response
  local load_validation_file="$EVIDENCE_DIR/worker-load-validation-evidence.json"

  load_validation_response="$(fetch_json POST /api/execution-jobs/worker-load-validation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$load_validation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$load_validation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$load_validation_file"
}

capture_remote_computer_sidecar_recovery_validation() {
  local sidecar_response
  local sidecar_file="$EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"

  sidecar_response="$(fetch_json POST /api/remote-computers/sidecars/recovery/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$sidecar_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$sidecar_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$sidecar_file"
}

capture_vault_kms_recovery_validation() {
  local recovery_response
  local recovery_file="$EVIDENCE_DIR/vault-kms-recovery-evidence.json"

  recovery_response="$(fetch_json POST /api/vault/kms/recovery/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$recovery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$recovery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$recovery_file"
}

capture_vault_kms_rotation_validation() {
  local rotation_response
  local rotation_file="$EVIDENCE_DIR/vault-kms-rotation-evidence.json"

  rotation_response="$(fetch_json POST /api/vault/kms/rotation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rotation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rotation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rotation_file"
}

capture_approval_notification_delivery_validation() {
  local delivery_response
  local delivery_file="$EVIDENCE_DIR/approval-notification-delivery-evidence.json"

  delivery_response="$(fetch_json POST /api/approvals/notifications/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$delivery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$delivery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$delivery_file"
}

capture_codex_app_server_stale_poll_validation() {
  local stale_poll_response
  local stale_poll_file="$EVIDENCE_DIR/codex-app-server-stale-poll-evidence.json"

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
    }' >"$stale_poll_file"
}

capture_observability_collector_deployment_validation() {
  local deployment_response
  local deployment_file="$EVIDENCE_DIR/observability-collector-deployment-evidence.json"

  deployment_response="$(fetch_json POST /api/observability/collector/deployment/validate)"
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
    }' >"$deployment_file"
}

capture_observability_collector_cluster_validation() {
  local cluster_response
  local cluster_file="$EVIDENCE_DIR/observability-collector-cluster-rollout-evidence.json"

  cluster_response="$(fetch_json POST /api/observability/collector/cluster/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$cluster_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$cluster_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$cluster_file"
}

capture_observability_remediation_validation() {
  local remediation_response
  local remediation_file="$EVIDENCE_DIR/observability-collector-remediation-evidence.json"

  remediation_response="$(fetch_json POST /api/observability/remediation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$remediation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$remediation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$remediation_file"
}

capture_provider_production_rollout() {
  local rollout_response
  local rollout_file="$EVIDENCE_DIR/provider-production-rollout-evidence.json"

  rollout_response="$(fetch_json POST /api/providers/production-rollout/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rollout_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollout_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rollout_file"
}

capture_provider_production_rollback() {
  local rollback_response
  local rollback_file="$EVIDENCE_DIR/provider-production-rollback-evidence.json"

  rollback_response="$(fetch_json POST /api/providers/production-rollout/rollback)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rollback_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollback_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rollback_file"
}

capture_policy_rollout_orchestration_validation() {
  local validation_response
  local validation_file="$EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"

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
    }' >"$validation_file"
}

capture_policy_rollout_due_run() {
  local due_run_response
  local due_run_file="$EVIDENCE_DIR/policy-rollout-due-run-evidence.json"

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
    }' >"$due_run_file"
}

run_controller_validations() {
  fetch_json POST /api/tenant-isolation/routing/validate >/dev/null
  fetch_json POST /api/providers/policy-gate/run >/dev/null
  fetch_json POST /api/providers/deployment/validate >/dev/null
  capture_policy_rollout_orchestration_validation
  capture_vault_kms_recovery_validation
  capture_worker_load_validation
  capture_remote_computer_state_sync_validation
  fetch_json POST /api/approvals/notifications/deployment/validate >/dev/null
  fetch_json POST /api/approvals/notifications/ops/validate >/dev/null
  fetch_json POST /api/codex-app-server/deployment/validate >/dev/null
  fetch_json POST /api/codex-app-server/ops/validate >/dev/null
  capture_eval_release_deployment_validation
  capture_eval_release_orchestration_validation
  capture_observability_collector_deployment_validation
  capture_observability_collector_cluster_validation
  fetch_json POST /api/scheduler/deployment/validate >/dev/null
  fetch_json POST /api/scheduler/run-due >/dev/null

  if [[ -n "$TEAM_ID" ]]; then
    capture_mcp_deployment_validation
    if [[ "${RUN_STAGE2_MCP_DUE_RUN:-0}" == "1" ]]; then
      capture_mcp_due_run_validation
    else
      echo "skipping MCP rollout due-run; set RUN_STAGE2_MCP_DUE_RUN=1 to include due-run evidence" >&2
    fi
  else
    echo "skipping MCP connector validation; set MANDOFORGE_STAGE2_TEAM_ID to include team-scoped MCP rollout evidence" >&2
  fi

  if [[ "${RUN_STAGE2_MCP_ROLLBACK:-0}" == "1" ]]; then
    capture_mcp_rollback_validation
  else
    echo "skipping MCP connector rollback; set RUN_STAGE2_MCP_ROLLBACK=1 to include rollback evidence" >&2
  fi

  if [[ "${RUN_STAGE2_SECRET_LIFECYCLE:-0}" == "1" ]]; then
    capture_vault_kms_rotation_validation
  else
    echo "skipping KMS rotation run; set RUN_STAGE2_SECRET_LIFECYCLE=1 to include secret lifecycle evidence" >&2
  fi

  if [[ "${RUN_STAGE2_PROVIDER_ROLLOUT:-0}" == "1" ]]; then
    capture_provider_production_rollout
    capture_provider_production_rollback
  else
    echo "skipping provider production rollout/rollback; set RUN_STAGE2_PROVIDER_ROLLOUT=1 to include provider rollout evidence" >&2
  fi

  if [[ "${RUN_STAGE2_POLICY_DUE_RUN:-0}" == "1" ]]; then
    capture_policy_rollout_due_run
  else
    echo "skipping policy rollout due-run; set RUN_STAGE2_POLICY_DUE_RUN=1 to include policy due-run evidence" >&2
  fi

  if [[ "${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-0}" == "1" ]]; then
    capture_remote_computer_sidecar_recovery_validation
  else
    echo "skipping Remote Computer sidecar recovery; set RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 to include replacement evidence" >&2
  fi

  if [[ "${RUN_STAGE2_APPROVAL_DELIVERY:-0}" == "1" ]]; then
    capture_approval_notification_delivery_validation
  else
    echo "skipping approval notification delivery; set RUN_STAGE2_APPROVAL_DELIVERY=1 to include delivery evidence" >&2
  fi

  if [[ "${RUN_STAGE2_CODEX_STALE_POLL:-0}" == "1" ]]; then
    capture_codex_app_server_stale_poll_validation
  else
    echo "skipping Codex App Server stale poll; set RUN_STAGE2_CODEX_STALE_POLL=1 to include stale-run supervision evidence" >&2
  fi

  if [[ "${RUN_STAGE2_EVAL_RELEASE_AUTOMATION:-0}" == "1" ]]; then
    capture_eval_release_stage2_regression
    capture_eval_release_due_run
  else
    echo "skipping eval/release automation; set RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1 to include regression and release due-run evidence" >&2
  fi

  if [[ "${RUN_STAGE2_EVAL_RELEASE_ROLLBACK:-0}" == "1" ]]; then
    capture_eval_release_rollback_validation
  else
    echo "skipping eval/release rollback; set RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1 to include rollback evidence" >&2
  fi

  if [[ "${RUN_STAGE2_OBSERVABILITY_REMEDIATION:-0}" == "1" ]]; then
    capture_observability_remediation_validation
  else
    echo "skipping observability remediation run; set RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 to include remediation evidence" >&2
  fi

  if [[ "${RUN_STAGE2_FINANCE_CONTROLLERS:-0}" == "1" ]]; then
    capture_finance_close_validation
    capture_finance_reconciliation_validation
  else
    echo "skipping finance close/reconciliation controllers; set RUN_STAGE2_FINANCE_CONTROLLERS=1 to include accounting evidence" >&2
  fi

  if [[ "${RUN_STAGE2_FINANCE_EXPORT:-0}" == "1" ]]; then
    fetch_file GET /api/usage/export.csv "$EVIDENCE_DIR/api-usage-export.csv" "$EVIDENCE_DIR/usage-export-csv-evidence.json"
    capture_finance_export_delivery_validation
  else
    echo "skipping finance export capture; set RUN_STAGE2_FINANCE_EXPORT=1 to include CSV and delivery evidence" >&2
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
  local stale=()
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
    if artifact_is_fresh "$expected_file"; then
      continue
    elif [[ -s "$expected_file" ]]; then
      stale+=("$resolved")
    else
      missing+=("$resolved")
    fi
  done < <(jq -r '.evidence_requirements[]?.readiness_endpoints[]?' "$readiness_file" | sort -u)

  if (( ${#missing[@]} > 0 )); then
    printf 'stage2 evidence gate did not collect declared readiness endpoint: %s\n' "${missing[@]}" >&2
    exit 1
  fi

  if (( ${#stale[@]} > 0 )); then
    printf 'stage2 evidence gate has stale declared readiness endpoint evidence: %s\n' "${stale[@]}" >&2
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
  local stale_file="$EVIDENCE_DIR/${label}-stale-endpoints.txt"
  local endpoint
  local resolved
  local expected_file
  local missing_count
  local stale_count

  : >"$declared_file"
  : >"$missing_file"
  : >"$stale_file"

  while IFS= read -r endpoint; do
    if [[ -z "$endpoint" ]]; then
      continue
    fi
    if ! resolved="$(resolve_requirement_endpoint "$endpoint")"; then
      continue
    fi
    echo "$resolved" >>"$declared_file"
    expected_file="$EVIDENCE_DIR/$(slugify "$resolved").json"
    if artifact_is_fresh "$expected_file"; then
      continue
    elif [[ -s "$expected_file" ]]; then
      echo "$resolved" >>"$stale_file"
      echo "$resolved" >>"$missing_file"
    else
      echo "$resolved" >>"$missing_file"
    fi
  done < <(jq -r ".evidence_requirements[]?.${field}[]?" "$readiness_file" | sort -u)

  missing_count="$(grep -c . "$missing_file" || true)"
  stale_count="$(grep -c . "$stale_file" || true)"
  if [[ "$fail_on_missing" == "1" && "$missing_count" != "0" ]]; then
    echo "stage2 evidence gate is missing declared $label endpoint evidence:" >&2
    sed 's/^/- /' "$missing_file" >&2
    if [[ "$stale_count" != "0" ]]; then
      echo "stale declared $label endpoint evidence:" >&2
      sed 's/^/- /' "$stale_file" >&2
    fi
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
  local validation_stale_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  open_gap_count="$(jq -r '.open_gap_count // 0' "$readiness_file")"
  completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$readiness_file")"
  evidence_requirement_count="$(jq -r '.evidence_requirements | length' "$readiness_file")"
  validation_declared_count="$(grep -c . "$EVIDENCE_DIR/validation-declared-endpoints.txt" 2>/dev/null || true)"
  validation_missing_count="$(grep -c . "$EVIDENCE_DIR/validation-missing-endpoints.txt" 2>/dev/null || true)"
  validation_stale_count="$(grep -c . "$EVIDENCE_DIR/validation-stale-endpoints.txt" 2>/dev/null || true)"

  {
    echo "stage2_status=$status"
    echo "completion_blocked=$completion_blocked"
    echo "open_gap_count=$open_gap_count"
    echo "evidence_requirement_count=$evidence_requirement_count"
    echo "validation_declared_endpoint_count=$validation_declared_count"
    echo "validation_missing_endpoint_count=$validation_missing_count"
    echo "validation_stale_endpoint_count=$validation_stale_count"
    echo "max_evidence_age_hours=$MAX_EVIDENCE_AGE_HOURS"
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
    echo
    echo "stale_validation_endpoints:"
    sed 's/^/- /' "$EVIDENCE_DIR/validation-stale-endpoints.txt" 2>/dev/null || true
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 production evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

run_completion_audit() {
  if [[ "$RUN_COMPLETION_AUDIT" != "1" ]]; then
    echo "completion audit skipped; set RUN_STAGE2_COMPLETION_AUDIT=1 to write archive-ready checklist evidence" >&2
    return 0
  fi

  local audit_script="scripts/stage2-completion-audit-gate.sh"
  if [[ ! -x "$audit_script" && -x "/app/$audit_script" ]]; then
    audit_script="/app/$audit_script"
  fi
  if [[ ! -x "$audit_script" ]]; then
    echo "Stage 2 production evidence gate could not find executable completion audit script" >&2
    exit 1
  fi

  ALLOW_BLOCKED=1 \
    BASE_URL="$BASE_URL" \
    MANDOFORGE_STAGE2_GATE_SUBJECT="$SUBJECT" \
    MANDOFORGE_STAGE2_GATE_ROLES="$ROLES" \
    MANDOFORGE_STAGE2_TEAM_ID="$TEAM_ID" \
    SOURCE_EVIDENCE_DIR="$EVIDENCE_DIR" \
    AUDIT_DIR="$EVIDENCE_DIR/completion-audit" \
    "$audit_script" >/dev/null

  if [[ ! -s "$EVIDENCE_DIR/completion-audit/checklist.json" ]]; then
    echo "Stage 2 completion audit did not write completion-audit/checklist.json" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
discover_team_id
collect_readiness
verify_readiness_inventory_coverage
write_endpoint_coverage "validation_endpoints" "validation" "0"

if [[ "$RUN_VALIDATIONS" == "1" ]]; then
  run_controller_validations
  run_local_validations
  collect_readiness
  verify_readiness_inventory_coverage
  write_endpoint_coverage "validation_endpoints" "validation" "$VERIFY_VALIDATION_COVERAGE"
else
  echo "controller validations skipped; set RUN_STAGE2_PRODUCTION_VALIDATIONS=1 to execute validation endpoints" >&2
  if [[ "$VERIFY_VALIDATION_COVERAGE" == "1" ]]; then
    write_endpoint_coverage "validation_endpoints" "validation" "1"
  fi
fi

run_completion_audit
write_summary
