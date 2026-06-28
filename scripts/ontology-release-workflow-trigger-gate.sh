#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_ONTOLOGY_WORKFLOW_TRIGGER_GATE_SUBJECT:-ontology-release-workflow-trigger-gate}"
ROLES="${MANDOFORGE_ONTOLOGY_WORKFLOW_TRIGGER_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_ONTOLOGY_WORKFLOW_TRIGGER_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/ontology-release-workflow-trigger}"
STATIC_ONLY="${STATIC_ONLY:-0}"
DOMAIN_SCOPE="${MANDOFORGE_ONTOLOGY_WORKFLOW_TRIGGER_DOMAIN_SCOPE:-commerce}"
RELEASE_CLASS="${MANDOFORGE_ONTOLOGY_WORKFLOW_TRIGGER_RELEASE_CLASS:-customer_grade}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ontology release workflow trigger gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "ontology release workflow trigger gate failed: $*" >&2
  exit 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

static_contract_check() {
  require_executable scripts/ontology-engine-readiness-gate.sh
  require_executable scripts/ontology-release-loop-gate.sh
  require_executable scripts/ontology-release-workflow-trigger-gate.sh

  grep -q "ontology-release-workflow-trigger-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the ontology release workflow trigger gate"
  grep -q "ontology-release-workflow-trigger-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the ontology release workflow trigger gate"
  grep -q "ontology-release-workflow-trigger-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the ontology release workflow trigger gate"
  grep -q "ontology-release-workflow-trigger-gate.sh" deploy/stage2-evidence/ontology-release-workflow-trigger-job.example.yaml \
    || fail "ontology release workflow trigger Job must run the dedicated gate"
  grep -q "ontology-release-workflow-trigger-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the ontology release workflow trigger Job"
  grep -q "ontology-release-workflow-trigger-gate.sh" scripts/verify-stage2-evidence-k8s-manifests.sh \
    || fail "Stage 2 manifest verifier must check the ontology release workflow trigger gate"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  {
    echo "ontology_release_workflow_trigger_status=static_ready"
    echo "blocked_count=0"
  } >"$EVIDENCE_DIR/summary.txt"
  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    '{generated_at: $generated_at, status: "static_ready", blocked_count: 0}' \
    >"$EVIDENCE_DIR/summary.json"
  cat "$EVIDENCE_DIR/summary.txt"
  echo "ontology release workflow trigger static gate ok"
  exit 0
fi

headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi
json_headers=(-H "content-type: application/json")

fetch_json() {
  local method="$1"
  local path="$2"
  local body="$3"
  local outfile="$4"
  local status
  if [[ -n "$body" ]]; then
    status="$(curl -sS -X "$method" -o "$outfile" -w "%{http_code}" "${headers[@]}" "${json_headers[@]}" -d "$body" "$BASE_URL$path")"
  else
    status="$(curl -sS -X "$method" -o "$outfile" -w "%{http_code}" "${headers[@]}" "$BASE_URL$path")"
  fi
  if [[ "$status" != 2* ]]; then
    echo "ontology release workflow trigger gate request failed: $method $path HTTP $status" >&2
    head -c 800 "$outfile" >&2 || true
    echo >&2
    exit 1
  fi
}

ensure_agent_id() {
  local agents_file="$EVIDENCE_DIR/agents.json"
  local agent_file="$EVIDENCE_DIR/agent-created.json"
  local agent_id
  fetch_json GET /api/agents "" "$agents_file"
  agent_id="$(jq -r '[.[]? | select(.release_state == "released" or .release_state == "")][0].id // .[0].id // ""' "$agents_file")"
  if [[ -n "$agent_id" && "$agent_id" != "null" ]]; then
    printf '%s' "$agent_id"
    return
  fi
  fetch_json \
    POST \
    /api/agents \
    "$(jq -n --arg domain "$DOMAIN_SCOPE" '{
      name: "Ontology release workflow trigger gate agent",
      kind: "assistant",
      provider: "gate-fixture",
      model: "gate-fixture",
      agent_role: "ontology-release-workflow-trigger",
      system_prompt: "Gate fixture for validating ontology release workflow triggering.",
      tools: [],
      tool_policy: {},
      semantic_scopes: {domain_scope: $domain, workflow_scope: "ontology-release-downstream"},
      release_state: "released"
    }')" \
    "$agent_file"
  jq -r '.id' "$agent_file"
}

approve_release_proposals() {
  local run_file="$1"
  local proposal_ids
  proposal_ids="$(jq -r '.proposals[]? | select(.proposal_type == "object" or .proposal_type == "action") | .id' "$run_file")"
  [[ -n "$proposal_ids" ]] || fail "demo run has no object/action proposals to approve"
  while IFS= read -r proposal_id; do
    [[ -z "$proposal_id" ]] && continue
    fetch_json \
      POST \
      "/api/ontology/onboarding/proposals/$proposal_id/review" \
      '{"decision":"approve","reason":"ontology release workflow trigger gate fixture"}' \
      "$EVIDENCE_DIR/review-$proposal_id.json"
  done <<<"$proposal_ids"
}

stamp="$(date -u +%Y%m%d%H%M%S)"
agent_id="$(ensure_agent_id)"
definition_file="$EVIDENCE_DIR/workflow-definition.json"
fetch_json \
  POST \
  /api/workflow-definitions \
  "$(jq -n \
    --arg agent_id "$agent_id" \
    --arg domain "$DOMAIN_SCOPE" \
    --arg stamp "$stamp" \
    '{
      name: ("Ontology release downstream gate " + $stamp),
      entrypoint: "ontology-release-downstream-gate",
      trigger_type: "api",
      default_agent_id: $agent_id,
      step_graph: {},
      handoff_rules: {
        ontology_release_trigger: {
          enabled: true,
          event: "ontology_release.promoted",
          domain_scope: $domain
        },
        root_task_grant: {
          semantic_scopes: {
            domain_scope: $domain,
            workflow_scope: "ontology-release-downstream"
          }
        }
      },
      execution_strategy: "native_steps",
      runtime_capability_contract: {},
      event_ingestion_policy: "normalized",
      release_state: "released"
    }')" \
  "$definition_file"
definition_id="$(jq -r '.id' "$definition_file")"

run_file="$EVIDENCE_DIR/onboarding-run.json"
materialized_file="$EVIDENCE_DIR/materialized.json"
candidate_file="$EVIDENCE_DIR/candidate.json"
gated_file="$EVIDENCE_DIR/gated.json"
promoted_file="$EVIDENCE_DIR/promoted.json"

fetch_json POST /api/ontology/onboarding/demo-runs "" "$run_file"
run_id="$(jq -r '.id' "$run_file")"
approve_release_proposals "$run_file"
fetch_json POST "/api/ontology/onboarding/runs/$run_id/materialize" "" "$materialized_file"
fetch_json \
  POST \
  "/api/ontology/onboarding/runs/$run_id/release-candidate" \
  "$(jq -n --arg version "$DOMAIN_SCOPE-workflow-trigger-$stamp" --arg release_class "$RELEASE_CLASS" '{version: $version, release_class: $release_class}')" \
  "$candidate_file"
release_id="$(jq -r '.id' "$candidate_file")"
fetch_json POST "/api/ontology/releases/$release_id/gate" "" "$gated_file"
jq -e '.gate_result.status == "passed"' "$gated_file" >/dev/null \
  || fail "ontology release gate did not pass before workflow trigger validation"
fetch_json POST "/api/ontology/releases/$release_id/promote" "" "$promoted_file"

jq -e '
  .status == "active"
  and .gate_result.workflow_trigger.status == "triggered"
' "$promoted_file" >/dev/null || {
  jq '.gate_result.workflow_trigger // .gate_result' "$promoted_file" >&2
  fail "promoted release did not report triggered workflow state"
}

fetch_json GET /api/workflow-runs "" "$EVIDENCE_DIR/workflow-runs.json"
workflow_run_id="$(jq -r --arg definition_id "$definition_id" --arg release_id "$release_id" '
  [.[]? | select(.workflow_definition_id == $definition_id and .input_payload.ontology_release_id == $release_id)][0].id // ""
' "$EVIDENCE_DIR/workflow-runs.json")"
[[ -n "$workflow_run_id" && "$workflow_run_id" != "null" ]] \
  || fail "no workflow run was created for promoted ontology release $release_id"

jq -e --arg run_id "$workflow_run_id" --arg release_id "$release_id" --arg domain "$DOMAIN_SCOPE" '
  any(.[]?;
    .id == $run_id
    and .status == "queued"
    and .input_payload.trigger == "ontology_release.promoted"
    and .input_payload.ontology_release_id == $release_id
    and .input_payload.domain_scope == $domain
    and (.input_payload.action_catalog.tool_count // -1) >= 1
  )
' "$EVIDENCE_DIR/workflow-runs.json" >/dev/null \
  || fail "workflow run input payload does not prove ontology release trigger and action catalog"

fetch_json GET /api/audit-logs "" "$EVIDENCE_DIR/audit-logs.json"
jq -e --arg run_id "$workflow_run_id" --arg release_id "$release_id" '
  any(.[]?;
    .action == "ontology_release.workflow_run_triggered"
    and .resource_id == $release_id
    and .details.workflow_run_id == $run_id
  )
' "$EVIDENCE_DIR/audit-logs.json" >/dev/null \
  || fail "audit log does not contain ontology_release.workflow_run_triggered for workflow run"

fetch_json POST /api/scheduler/run-due "$(jq -n --arg key "ontology-trigger-gate-$stamp" '{idempotency_key: $key, owner: "ontology-release-workflow-trigger-gate"}')" "$EVIDENCE_DIR/scheduler-run-due.json"
jq -e '
  .ontology_release_workflow_triggers != null
  and (.ontology_release_workflow_triggers.failed_count // 0) == 0
' "$EVIDENCE_DIR/scheduler-run-due.json" >/dev/null \
  || fail "scheduler run-due did not expose a clean ontology release workflow trigger drain"

fetch_json GET /api/ontology/engine-readiness "" "$EVIDENCE_DIR/readiness.json"
jq -e '
  any(.checks[]?; .id == "domain-ontology-lifecycle" and .status == "ready")
  and any(.checks[]?; .id == "approved-release-materialization" and .status == "ready")
  and any(.checks[]?; .id == "migration-policy" and .status == "ready")
' "$EVIDENCE_DIR/readiness.json" >/dev/null \
  || fail "ontology readiness did not reflect promoted release lifecycle"

jq -n \
  --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg status "ready" \
  --arg domain_scope "$DOMAIN_SCOPE" \
  --arg workflow_definition_id "$definition_id" \
  --arg workflow_run_id "$workflow_run_id" \
  --arg ontology_release_id "$release_id" \
  '{
    generated_at: $generated_at,
    status: $status,
    domain_scope: $domain_scope,
    workflow_definition_id: $workflow_definition_id,
    workflow_run_id: $workflow_run_id,
    ontology_release_id: $ontology_release_id,
    checks: {
      release_promoted: true,
      workflow_trigger_reported: true,
      workflow_run_queued: true,
      audit_log_recorded: true,
      scheduler_drain_exposed: true,
      readiness_reflected: true
    }
  }' >"$EVIDENCE_DIR/summary.json"

{
  echo "ontology_release_workflow_trigger_status=ready"
  echo "domain_scope=$DOMAIN_SCOPE"
  echo "workflow_definition_id=$definition_id"
  echo "workflow_run_id=$workflow_run_id"
  echo "ontology_release_id=$release_id"
  echo "evidence_dir=$EVIDENCE_DIR"
} >"$EVIDENCE_DIR/summary.txt"

cat "$EVIDENCE_DIR/summary.txt"
echo "ontology release workflow trigger gate ok"
