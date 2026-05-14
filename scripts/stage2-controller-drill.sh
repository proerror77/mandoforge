#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/stage2-controller-drill-evidence}"
STAGE2_MOCK_CONTROLLER_PORT="${STAGE2_MOCK_CONTROLLER_PORT:-18080}"
STAGE2_MOCK_CONTROLLER_HOST="${STAGE2_MOCK_CONTROLLER_HOST:-127.0.0.1}"
RUN_DRILL_ACTIONS="${RUN_STAGE2_CONTROLLER_DRILL_ACTIONS:-0}"
MOCK_BASE_URL="http://$STAGE2_MOCK_CONTROLLER_HOST:$STAGE2_MOCK_CONTROLLER_PORT"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Stage 2 controller drill requires $1" >&2
    exit 1
  fi
}

require_cmd node
require_cmd curl
require_cmd jq

auth_headers=(
  -H "x-mandoforge-subject: stage2-controller-drill"
  -H "x-mandoforge-roles: admin"
)

if ! curl -fsS "$BASE_URL/healthz" >/dev/null; then
  echo "Stage 2 controller drill requires a running MandoForge API at $BASE_URL" >&2
  exit 1
fi

node scripts/stage2-mock-controller.js &
mock_pid="$!"
cleanup() {
  kill "$mock_pid" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for _ in $(seq 1 50); do
  if curl -fsS "$MOCK_BASE_URL/healthz" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

curl -fsS "$MOCK_BASE_URL/healthz" >/dev/null

seed_stage2_controller_drill_mcp_scope() {
  local slug_suffix
  local organization_file
  local team_file
  local server_file
  local rollout_file
  local organization_id
  local team_id
  local server_id
  local activate_after

  slug_suffix="$(date -u +%Y%m%d%H%M%S)-$$"
  organization_file="$(mktemp)"
  team_file="$(mktemp)"
  server_file="$(mktemp)"
  rollout_file="$(mktemp)"
  activate_after="$(node -e 'console.log(new Date(Date.now() - 60000).toISOString())')"

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d "{\"name\":\"Stage 2 Controller Drill Org\",\"slug\":\"stage2-controller-drill-$slug_suffix\"}" \
    "$BASE_URL/api/organizations" >"$organization_file"
  organization_id="$(jq -r '.id' "$organization_file")"

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d "{\"name\":\"Stage 2 Controller Drill Team\",\"slug\":\"stage2-controller-drill-team-$slug_suffix\"}" \
    "$BASE_URL/api/organizations/$organization_id/teams" >"$team_file"
  team_id="$(jq -r '.id' "$team_file")"

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d '{
      "name": "stage2-controller-drill-docs",
      "transport": "http",
      "tool_allowlist": ["search"],
      "config": {
        "source": "stage2-controller-drill",
        "health_check": {"interval_seconds": 60}
      }
    }' \
    "$BASE_URL/api/teams/$team_id/mcp-servers" >"$server_file"
  server_id="$(jq -r '.id' "$server_file")"

  jq -n \
    --arg activate_after "$activate_after" \
    '{
      transport: "http+json",
      config: {
        source: "stage2-controller-drill-rolled-out",
        health_check: {interval_seconds: 60}
      },
      tool_allowlist: ["search"],
      status: "active",
      activate_after: $activate_after,
      reason: "stage2 controller drill MCP rollout evidence seed"
    }' | curl -fsS "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d @- \
      "$BASE_URL/api/teams/$team_id/mcp-servers/$server_id/rollouts" >"$rollout_file"

  mkdir -p "$EVIDENCE_DIR"
  jq -n \
    --arg status "seeded" \
    --arg organization_id "$organization_id" \
    --arg team_id "$team_id" \
    --arg server_id "$server_id" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile organization "$organization_file" \
    --slurpfile team "$team_file" \
    --slurpfile server "$server_file" \
    --slurpfile rollout "$rollout_file" \
    '{
      status: $status,
      organization_id: $organization_id,
      team_id: $team_id,
      server_id: $server_id,
      generated_at: $generated_at,
      organization: ($organization[0] // {}),
      team: ($team[0] // {}),
      server: ($server[0] // {}),
      rollout: ($rollout[0] // {})
    }' >"$EVIDENCE_DIR/stage2-controller-drill-mcp-seed.json"

  rm -f "$organization_file" "$team_file" "$server_file" "$rollout_file"
  export MANDOFORGE_STAGE2_TEAM_ID="$team_id"
}

seed_stage2_controller_drill_eval_release_scope() {
  local bootstrap_file
  local agents_file
  local run_file
  local release_file
  local dataset_id
  local agent_id
  local eval_run_id

  bootstrap_file="$(mktemp)"
  agents_file="$(mktemp)"
  run_file="$(mktemp)"
  release_file="$(mktemp)"

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d '{"name":"Stage 2 Controller Drill Regression"}' \
    "$BASE_URL/api/eval/suites/stage2-regression" >"$bootstrap_file"
  dataset_id="$(jq -r '.dataset.id' "$bootstrap_file")"

  curl -fsS "${auth_headers[@]}" "$BASE_URL/api/agents" >"$agents_file"
  agent_id="$(jq -r '.[0].id // empty' "$agents_file")"
  if [[ -z "$agent_id" ]]; then
    echo "Stage 2 controller drill could not seed eval/release evidence: no agent exists" >&2
    exit 1
  fi

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d "{\"agent_id\":\"$agent_id\"}" \
    "$BASE_URL/api/eval/datasets/$dataset_id/runs" >"$run_file"
  eval_run_id="$(jq -r '.id' "$run_file")"

  curl -fsS "${auth_headers[@]}" \
    -H "content-type: application/json" \
    -d "{\"eval_run_id\":\"$eval_run_id\",\"environment\":\"stage2-controller-drill\",\"min_score\":1.0}" \
    "$BASE_URL/api/agents/$agent_id/releases" >"$release_file"

  mkdir -p "$EVIDENCE_DIR"
  jq -n \
    --arg status "seeded" \
    --arg agent_id "$agent_id" \
    --arg dataset_id "$dataset_id" \
    --arg eval_run_id "$eval_run_id" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile bootstrap "$bootstrap_file" \
    --slurpfile agents "$agents_file" \
    --slurpfile run "$run_file" \
    --slurpfile release "$release_file" \
    '{
      status: $status,
      agent_id: $agent_id,
      dataset_id: $dataset_id,
      eval_run_id: $eval_run_id,
      generated_at: $generated_at,
      bootstrap: ($bootstrap[0] // {}),
      agents: ($agents[0] // []),
      run: ($run[0] // {}),
      release: ($release[0] // {})
    }' >"$EVIDENCE_DIR/stage2-controller-drill-eval-release-seed.json"

  rm -f "$bootstrap_file" "$agents_file" "$run_file" "$release_file"
}

export ALLOW_BLOCKED="${ALLOW_BLOCKED:-1}"
export RUN_STAGE2_PRODUCTION_VALIDATIONS=1
export VERIFY_STAGE2_VALIDATION_COVERAGE="${VERIFY_STAGE2_VALIDATION_COVERAGE:-1}"
export EVIDENCE_DIR

export MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED=true
export MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL="$MOCK_BASE_URL/tenant/routing/validate"
export MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/provider/deployment/validate"
export MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL="$MOCK_BASE_URL/provider/rollout/apply"
export MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL="$MOCK_BASE_URL/provider/rollout/rollback"
export MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_REQUIRED=true
export MANDOFORGE_POLICY_ROLLOUT_ORCHESTRATION_CONTROLLER_URL="$MOCK_BASE_URL/policy/rollout/orchestration/validate"
export MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED=true
export MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL="$MOCK_BASE_URL/vault/kms/recovery/validate"
export MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED=true
export MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL="$MOCK_BASE_URL/worker/load/validate"
export MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED=true
export MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL="$MOCK_BASE_URL/remote-computer/state-sync/validate"
export MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_REQUIRED=true
export MANDOFORGE_REMOTE_COMPUTER_SIDECAR_VALIDATION_URL="$MOCK_BASE_URL/remote-computer/sidecar/validate"
export MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/approval-notification/deployment/validate"
export MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED=true
export MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL="$MOCK_BASE_URL/approval-notification/ops/validate"
export MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/mcp/deployment/validate"
export MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED=true
export MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL="$MOCK_BASE_URL/mcp/rollout/apply"
export MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED=true
export MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL="$MOCK_BASE_URL/mcp/rollout/rollback"
export MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/codex-app-server/deployment/validate"
export MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED=true
export MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL="$MOCK_BASE_URL/codex-app-server/ops/validate"
export MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED=true
export MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL="$MOCK_BASE_URL/agents/releases/rollout/apply"
export MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/agents/releases/deployment/validate"
export MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED=true
export MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL="$MOCK_BASE_URL/agents/releases/orchestration/validate"
export MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED=true
export MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL="$MOCK_BASE_URL/agents/releases/rollout/rollback"
export MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED=true
export MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL="$MOCK_BASE_URL/observability/collector/deployment/validate"
export MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED=true
export MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL="$MOCK_BASE_URL/observability/collector/cluster/validate"
export MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED=true
export MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL="$MOCK_BASE_URL/observability/remediation/run"
export MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED=true
export MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL="$MOCK_BASE_URL/finance/close"
export MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED=true
export MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL="$MOCK_BASE_URL/finance/reconcile"

if [[ "$RUN_DRILL_ACTIONS" == "1" ]]; then
  seed_stage2_controller_drill_mcp_scope
  seed_stage2_controller_drill_eval_release_scope
  export RUN_STAGE2_SECRET_LIFECYCLE=1
  export RUN_STAGE2_PROVIDER_ROLLOUT=1
  export RUN_STAGE2_POLICY_DUE_RUN=1
  export RUN_STAGE2_MCP_DUE_RUN=1
  export RUN_STAGE2_MCP_ROLLBACK=1
  export RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1
  export RUN_STAGE2_APPROVAL_DELIVERY=1
  export RUN_STAGE2_CODEX_STALE_POLL=1
  export RUN_STAGE2_EVAL_RELEASE_AUTOMATION=1
  export RUN_STAGE2_EVAL_RELEASE_ROLLBACK=1
  export RUN_STAGE2_OBSERVABILITY_REMEDIATION=1
  export RUN_STAGE2_FINANCE_CONTROLLERS=1
  export RUN_STAGE2_FINANCE_EXPORT=1
  if command -v actionbook >/dev/null 2>&1; then
    export RUN_STAGE2_UI_ACTIONBOOK=1
    export RUN_STAGE2_UI_STATIC_ASSETS=1
  else
    echo "skipping Stage 2 UI smoke evidence in controller drill; actionbook is not installed" >&2
  fi
fi

./scripts/stage2-production-evidence-gate.sh
