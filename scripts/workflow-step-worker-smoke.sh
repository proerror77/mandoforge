#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_WORKFLOW_STEP_SMOKE_SUBJECT:-workflow-step-worker-smoke}"
ROLES="${MANDOFORGE_WORKFLOW_STEP_SMOKE_ROLES:-admin,operator}"
AUTH_TOKEN="${MANDOFORGE_WORKFLOW_STEP_SMOKE_AUTH_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"
WORKER_POOL="${MANDOFORGE_WORKFLOW_STEP_SMOKE_WORKER_POOL:-managed-agent}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/workflow-step-worker-smoke}"
RUN_ID="${MANDOFORGE_WORKFLOW_STEP_SMOKE_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$-${RANDOM:-0}}"
WAIT_ATTEMPTS="${MANDOFORGE_WORKFLOW_STEP_SMOKE_WAIT_ATTEMPTS:-60}"
WAIT_SECONDS="${MANDOFORGE_WORKFLOW_STEP_SMOKE_WAIT_SECONDS:-1}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "workflow step worker smoke requires $1" >&2
    exit 1
  fi
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local payload
  local label="${4:-$(slugify "$path")}"
  local expected_prefix="${5:-2}"
  local target="$EVIDENCE_DIR/$label.json"
  local request_target="$EVIDENCE_DIR/$label.request.json"
  local response_body
  local response_json
  local http_status
  response_body="$(mktemp)"
  response_json="$(mktemp)"
  if [[ $# -ge 3 ]]; then
    payload="$3"
  else
    payload="{}"
  fi
  printf '%s' "$payload" >"$request_target"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d "$payload" \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != "$expected_prefix"* ]]; then
    echo "workflow step worker smoke request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,120p' "$response_body" >&2
    rm -f "$response_body" "$response_json"
    exit 1
  fi

  if ! jq . "$response_body" >"$response_json" 2>/dev/null; then
    jq -n --rawfile raw "$response_body" '{raw: $raw}' >"$response_json"
  fi

  jq -n \
    --arg method "$method" \
    --arg path "$path" \
    --arg request_file "$request_target" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson http_status "$http_status" \
    --slurpfile response "$response_json" \
    '{
      method: $method,
      path: $path,
      request_file: $request_file,
      generated_at: $generated_at,
      http_status: $http_status,
      response: ($response[0] // {})
    }' >"$target"
  rm -f "$response_body" "$response_json"
  printf '%s\n' "$target"
}

response_path() {
  jq -r '.response' "$1"
}

wait_for_step_status() {
  local workflow_run_id="$1"
  local wanted="$2"
  local label="$3"
  local steps_file
  local status
  for _ in $(seq 1 "$WAIT_ATTEMPTS"); do
    steps_file="$(fetch_json GET "/api/workflow-runs/$workflow_run_id/steps" '{}' "$label")"
    status="$(jq -r '.[0].status // empty' < <(response_path "$steps_file"))"
    if [[ "$status" == "$wanted" ]]; then
      printf '%s\n' "$steps_file"
      return 0
    fi
    if [[ "$status" == "failed" ]]; then
      echo "workflow step failed while waiting for $wanted" >&2
      jq '.response[0] | {id, step_key, status, output_payload}' "$steps_file" >&2
      return 1
    fi
    sleep "$WAIT_SECONDS"
  done
  echo "workflow step did not reach $wanted after $WAIT_ATTEMPTS attempts" >&2
  jq '.response[0] | {id, step_key, status, output_payload}' "$steps_file" >&2
  return 1
}

wait_for_workflow_status() {
  local workflow_run_id="$1"
  local wanted="$2"
  local label="$3"
  local run_file
  local status
  for _ in $(seq 1 "$WAIT_ATTEMPTS"); do
    run_file="$(fetch_json GET "/api/workflow-runs/$workflow_run_id" '{}' "$label")"
    status="$(jq -r '.status // empty' < <(response_path "$run_file"))"
    if [[ "$status" == "$wanted" ]]; then
      printf '%s\n' "$run_file"
      return 0
    fi
    if [[ "$status" == "failed" ]]; then
      echo "workflow run failed while waiting for $wanted" >&2
      jq '.response | {id, status, started_at, completed_at}' "$run_file" >&2
      return 1
    fi
    sleep "$WAIT_SECONDS"
  done
  echo "workflow run did not reach $wanted after $WAIT_ATTEMPTS attempts" >&2
  jq '.response | {id, status, started_at, completed_at}' "$run_file" >&2
  return 1
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null

semantic_scopes="$(
  jq -nc '{
    project_scope: "agent-os",
    repo_scope: "mandoforge",
    service_scope: "mandoforge-api",
    workflow_scope: "workflow-step-worker-smoke",
    policy_scope: "managed-runtime",
    memory_scope: "task-board-smoke"
  }'
)"

agent_file="$(fetch_json POST /api/agents "$(
  jq -nc --arg run_id "$RUN_ID" --argjson scopes "$semantic_scopes" '{
    name: ("Workflow Step Smoke Agent " + $run_id),
    kind: "specialist",
    agent_role: "specialist",
    provider: "openai-compatible",
    model: "gpt-5.4-mini",
    tools: ["file.read", "sql.get_schema", "sql.query", "shell.exec", "artifact.create"],
    semantic_scopes: $scopes,
    release_state: "active"
  }'
)" api-workflow-step-smoke-agent)"
agent_id="$(jq -r '.response.id // empty' "$agent_file")"

environment_file="$(fetch_json POST /api/environments "$(
  jq -nc --arg run_id "$RUN_ID" --arg worker_pool "$WORKER_POOL" '{
    name: ("Workflow Step Smoke Environment " + $run_id),
    environment_type: "local",
    worker_queue_binding: {queue: $worker_pool},
    release_state: "active",
    status: "enabled"
  }'
)" api-workflow-step-smoke-environment)"
environment_id="$(jq -r '.response.id // empty' "$environment_file")"

work_item_file="$(fetch_json POST /api/work-items "$(
  jq -nc --arg run_id "$RUN_ID" --argjson scopes "$semantic_scopes" '{
    title: ("Workflow step worker smoke " + $run_id),
    source: "manual",
    priority: "high",
    metadata: {
      gate: "workflow-step-worker-smoke",
      semantic_scopes: $scopes,
      runtime_evidence_required: true,
      demo_run_id: $run_id
    }
  }'
)" api-workflow-step-smoke-work-item)"
work_item_id="$(jq -r '.response.id // empty' "$work_item_file")"

definition_file="$(fetch_json POST /api/workflow-definitions "$(
  jq -nc \
    --arg run_id "$RUN_ID" \
    --arg agent_id "$agent_id" \
    --arg environment_id "$environment_id" \
    --argjson scopes "$semantic_scopes" '{
      name: ("Workflow Step Worker Smoke " + $run_id),
      entrypoint: "workflow-step-worker-smoke",
      trigger_type: "manual",
      default_agent_id: $agent_id,
      default_environment_id: $environment_id,
      step_graph: {
        steps: [
          {
            key: "inspect_runtime",
            type: "agent",
            start: true,
            input: {
              objective: "Read files, inspect/query schema, then pause for shell approval."
            }
          }
        ]
      },
      handoff_rules: {
        root_task_grant: {
          semantic_scopes: $scopes,
          memory_scope: {
            mode: "snapshot_only",
            allowed_object_types: ["work_item"],
            allowed_object_ids: [],
            minimum_trust_level: "source_attested",
            max_objects: 5,
            writeback_allowed: false
          },
          tool_scope: {
            read: ["file.read", "sql.get_schema", "sql.query"],
            write: ["artifact.create", "shell.exec"],
            external_write: []
          },
          connector_scope: {
            mode: "read_only",
            allowed_connector_ids: [],
            allowed_tool_names: [],
            tenant_scope: {},
            side_effect_classes: []
          }
        }
      },
      release_state: "released"
    }'
)" api-workflow-step-smoke-definition)"
definition_id="$(jq -r '.response.id // empty' "$definition_file")"

run_file="$(fetch_json POST /api/workflow-runs "$(
  jq -nc --arg run_id "$RUN_ID" --arg definition_id "$definition_id" --arg work_item_id "$work_item_id" '{
    workflow_definition_id: $definition_id,
    source_work_item_id: $work_item_id,
    title: ("Workflow step worker smoke run " + $run_id),
    input_payload: {
      objective: "prove queue worker can execute, pause, resume, and complete a workflow step",
      smoke_run_id: $run_id
    }
  }'
)" api-workflow-step-smoke-run)"
workflow_run_id="$(jq -r '.response.id // empty' "$run_file")"
session_id="$(jq -r '.response.primary_session_id // empty' "$run_file")"

requires_action_steps_file="$(wait_for_step_status "$workflow_run_id" requires_action api-workflow-step-smoke-steps-requires-action)"
approval_id="$(jq -r '.response[0].approval_ids[0] // empty' "$requires_action_steps_file")"
if [[ -z "$approval_id" ]]; then
  echo "workflow step reached requires_action without an approval id" >&2
  jq '.response[0] | {id, status, approval_ids, output_payload}' "$requires_action_steps_file" >&2
  exit 1
fi

fetch_json POST "/api/approvals/$approval_id/approve" '{}' api-workflow-step-smoke-approval-approved >/dev/null

completed_run_file="$(wait_for_workflow_status "$workflow_run_id" completed api-workflow-step-smoke-run-completed)"
completed_steps_file="$(wait_for_step_status "$workflow_run_id" completed api-workflow-step-smoke-steps-completed)"
events_file="$(fetch_json GET "/api/sessions/$session_id/events" '{}' api-workflow-step-smoke-events)"
approvals_file="$(fetch_json GET /api/approvals '{}' api-workflow-step-smoke-approvals)"

jq -e --arg approval_id "$approval_id" '
  any(.response[]; .id == $approval_id and .status == "approved")
' "$approvals_file" >/dev/null
jq -e '
  any(.response[]; .event_type == "execution.completed")
  and any(.response[]; .event_type == "agent.final")
  and any(.response[]; .event_type == "workflow.step.completed")
  and any(.response[]; .event_type == "workflow.run.completed")
' "$events_file" >/dev/null
jq -e '
  .response[0].status == "completed"
  and (.response[0].tool_call_ids | length) >= 4
  and (.response[0].approval_ids | length) >= 1
  and (.response[0].artifact_ids | length) >= 1
  and .response[0].output_payload.worker_execution.session_loop_resume == true
' "$completed_steps_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "workflow_step_worker_smoke=passed"
  echo "run_id=$RUN_ID"
  echo "base_url=$BASE_URL"
  echo "worker_pool=$WORKER_POOL"
  echo "agent_id=$agent_id"
  echo "environment_id=$environment_id"
  echo "work_item_id=$work_item_id"
  echo "workflow_definition_id=$definition_id"
  echo "workflow_run_id=$workflow_run_id"
  echo "workflow_run_status=$(jq -r '.response.status' "$completed_run_file")"
  echo "session_id=$session_id"
  echo "workflow_step_run_id=$(jq -r '.response[0].id' "$completed_steps_file")"
  echo "workflow_step_status=$(jq -r '.response[0].status' "$completed_steps_file")"
  echo "claimed_by_worker=$(jq -r '.response[0].claimed_by_worker' "$completed_steps_file")"
  echo "context_packet_id=$(jq -r '.response[0].context_packet_id' "$completed_steps_file")"
  echo "approval_id=$approval_id"
  echo "tool_call_count=$(jq -r '.response[0].tool_call_ids | length' "$completed_steps_file")"
  echo "artifact_count=$(jq -r '.response[0].artifact_ids | length' "$completed_steps_file")"
} >"$summary_file"

cat "$summary_file"
