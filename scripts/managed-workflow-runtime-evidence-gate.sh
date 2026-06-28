#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_WORKFLOW_RUNTIME_GATE_SUBJECT:-managed-workflow-runtime-evidence-gate}"
ROLES="${MANDOFORGE_WORKFLOW_RUNTIME_GATE_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_WORKFLOW_RUNTIME_GATE_TOKEN:-${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}}"
SCHEDULER_TOKEN="${MANDOFORGE_SCHEDULER_TOKEN:-}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/managed-workflow-runtime-evidence}"
RUN_ID="${MANDOFORGE_WORKFLOW_RUNTIME_GATE_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$-${RANDOM:-0}}"

# Stage 2 metadata contract required artifacts:
# local-script-scripts-managed-workflow-runtime-evidence-gate.sh.json
# api-workflow-runtime-proof-graph.json
# api-workflow-runtime-proof-transitions.json
# api-workflow-runtime-proof-memory-governance-partition.json

auth_headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

if [[ -n "$SCHEDULER_TOKEN" ]]; then
  auth_headers+=(-H "x-mandoforge-scheduler-token: $SCHEDULER_TOKEN")
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "managed workflow runtime evidence gate requires $1" >&2
    exit 1
  fi
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

urlencode() {
  jq -sRr @uri
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
    echo "managed workflow runtime evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,80p' "$response_body" >&2
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

workflow_step_id_by_key() {
  local steps_file="$1"
  local step_key="$2"
  local label="${3:-$step_key}"
  local step_id
  step_id="$(jq -r --arg step_key "$step_key" '
    .response[]
    | select(.step_key == $step_key and (.status == "queued" or .status == "running" or .status == "requires_action"))
    | .id
  ' "$steps_file" | head -n 1)"
  if [[ -z "$step_id" ]]; then
    echo "managed workflow runtime evidence gate could not resolve workflow step '$label' in queued/running/requires_action state" >&2
    jq -r '.response[] | {step_key, status, id, scheduled_at, output_payload}' "$steps_file" >&2
    exit 1
  fi
  printf '%s\n' "$step_id"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null

agents_file="$(fetch_json GET /api/agents '{}' api-agents)"
agent_id="$(jq -r '.response | map(select(.release_state == "active")) | .[0].id // .response[0].id // empty' "$agents_file")"
if [[ -z "$agent_id" ]]; then
  created_agent_file="$(fetch_json POST /api/agents \
    '{"name":"Runtime Proof Agent","kind":"managed","provider":"mock","model":"mock-runtime-proof","agent_role":"worker","tools":[],"release_state":"active"}' \
    api-agents-created)"
  agent_id="$(jq -r '.response.id // empty' "$created_agent_file")"
fi
if [[ -z "$agent_id" ]]; then
  echo "managed workflow runtime evidence gate could not resolve or create an agent" >&2
  exit 1
fi

definition_payload="$(jq -nc --arg agent_id "$agent_id" '{
  name: "Managed Workflow Runtime Evidence",
  entrypoint: "managed-workflow-runtime-evidence",
  trigger_type: "manual",
  default_agent_id: $agent_id,
  step_graph: {
    fan_out: {max_parallel: 2},
    steps: [
      {key: "intake", type: "agent", start: true},
      {key: "collect_a", type: "agent", depends_on: ["intake"], retry: {max_attempts: 2, delay_seconds: 1}},
      {key: "collect_b", type: "agent", depends_on: ["intake"]},
      {key: "merge", type: "agent", depends_on: ["collect_a", "collect_b"], fan_in: {mode: "quorum", min_success: 2}},
      {key: "draft_result", type: "agent", depends_on: ["merge"], condition: {source_step: "merge", path: "ready", equals: true}}
    ]
  },
  handoff_rules: {
    root_task_grant: {
      memory_scope: {
        mode: "snapshot_only",
        allowed_scope_keys: [],
        allowed_object_types: [],
        allowed_source_types: [],
        allowed_object_ids: [],
        minimum_trust_level: "verified",
        max_objects: 0,
        approval_memory_allowed: false,
        handoff_memory_allowed: false,
        writeback_allowed: true
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
      },
      external_effects: {
        publish: false,
        payment: false,
        ad_spend: false,
        email_send: false,
        external_commit: false
      }
    }
  },
  release_state: "released"
}')"
definition_file="$(fetch_json POST /api/workflow-definitions "$definition_payload" api-workflow-definitions-runtime-proof)"
definition_id="$(jq -r '.response.id // empty' "$definition_file")"

run_payload="$(jq -nc --arg definition_id "$definition_id" '{
  workflow_definition_id: $definition_id,
  title: "Managed workflow runtime proof",
  input_payload: {
    objective: "prove workflow runtime scheduler, graph, artifact, and memory governance surfaces",
    evidence_gate: "managed-workflow-runtime-evidence-gate"
  }
}')"
run_file="$(fetch_json POST /api/workflow-runs "$run_payload" api-workflow-runs-runtime-proof)"
run_id="$(jq -r '.response.id // empty' "$run_file")"
session_id="$(jq -r '.response.primary_session_id // empty' "$run_file")"

steps_file="$(fetch_json GET "/api/workflow-runs/$run_id/steps" '{}' api-workflow-runtime-proof-steps-initial)"
intake_step_id="$(workflow_step_id_by_key "$steps_file" intake)"
fetch_json PATCH "/api/workflow-step-runs/$intake_step_id" '{"status":"completed","output_payload":{"accepted":true}}' api-workflow-runtime-proof-step-intake >/dev/null

steps_file="$(fetch_json GET "/api/workflow-runs/$run_id/steps" '{}' api-workflow-runtime-proof-steps-after-intake)"
collect_a_step_id="$(workflow_step_id_by_key "$steps_file" collect_a)"
collect_b_step_id="$(workflow_step_id_by_key "$steps_file" collect_b)"

collect_b_initial_claim_file="$(fetch_json POST "/api/workflow-step-runs/$collect_b_step_id/claim" "$(jq -nc --arg agent_id "$agent_id" '{
  agent_id: $agent_id,
  worker_id: "managed-workflow-lease-drill-a",
  lease_seconds: 1
}')" api-workflow-runtime-proof-step-collect-b-initial-claim)"
sleep 2
collect_b_reclaim_file="$(fetch_json POST "/api/workflow-step-runs/$collect_b_step_id/claim" "$(jq -nc --arg agent_id "$agent_id" '{
  agent_id: $agent_id,
  worker_id: "managed-workflow-lease-drill-b",
  lease_seconds: 300
}')" api-workflow-runtime-proof-step-collect-b-lease-reclaim)"

fetch_json PATCH "/api/workflow-step-runs/$collect_a_step_id" \
  '{"status":"failed","output_payload":{"error":"transient source failure for retry proof"}}' \
  api-workflow-runtime-proof-step-collect-a-failed >/dev/null

scheduler_run_file=""
activated_retry_id=""
for attempt in 1 2 3 4 5 6; do
  if ((attempt > 1)); then
    sleep 1
  fi

  scheduler_run_file="$(fetch_json POST /api/scheduler/run-due "$(jq -nc --arg key "managed-workflow-runtime-proof-${RUN_ID}-${attempt}" '{
    idempotency_key: $key,
    owner: "managed-workflow-runtime-evidence-gate",
    retry_policy: {max_attempts: 2, backoff_seconds: 1}
  }')" api-scheduler-run-due-workflow-runtime-proof)"

  activated_retry_id="$(jq -r '.response.workflow_scheduled_steps.activated_step_ids[0] // empty' "$scheduler_run_file")"
  if [[ -n "$activated_retry_id" ]]; then
    break
  fi
done
if [[ -z "$activated_retry_id" ]]; then
  echo "scheduler due-run did not activate a workflow scheduled retry step" >&2
  if [[ -n "$scheduler_run_file" && -s "$scheduler_run_file" ]]; then
    sed -n '1,120p' "$scheduler_run_file" >&2
  fi
  exit 1
fi

fetch_json PATCH "/api/workflow-step-runs/$activated_retry_id" \
  '{"status":"completed","output_payload":{"records":3,"source":"collect_a_retry"}}' \
  api-workflow-runtime-proof-step-collect-a-retry-completed >/dev/null
fetch_json PATCH "/api/workflow-step-runs/$collect_b_step_id" \
  '{"status":"completed","output_payload":{"records":2,"source":"collect_b"}}' \
  api-workflow-runtime-proof-step-collect-b-completed >/dev/null

steps_file="$(fetch_json GET "/api/workflow-runs/$run_id/steps" '{}' api-workflow-runtime-proof-steps-after-collectors)"
merge_step_id="$(workflow_step_id_by_key "$steps_file" merge)"
fetch_json PATCH "/api/workflow-step-runs/$merge_step_id" \
  '{"status":"completed","output_payload":{"ready":true,"merged_records":5}}' \
  api-workflow-runtime-proof-step-merge-completed >/dev/null

artifact_file="$(fetch_json POST /api/codex-app-server/artifacts/sync "$(jq -nc --arg session_id "$session_id" '{
  session_id: $session_id,
  turn_id: "managed-workflow-runtime-proof",
  command_id: "draft-result",
  artifacts: [
    {
      name: "managed-workflow-runtime-proof.json",
      artifact_type: "workflow_result",
      path: "evidence/managed-workflow-runtime-proof.json",
      content: {
        status: "completed",
        evidence_gate: "managed-workflow-runtime-evidence-gate",
        result: "workflow graph, scheduler, transition, artifact, and memory governance proof"
      },
      metadata: {source: "managed-workflow-runtime-evidence-gate"}
    }
  ]
}')" api-workflow-runtime-proof-artifact-sync)"
artifact_id="$(jq -r '.response.artifacts[0].id // empty' "$artifact_file")"

steps_file="$(fetch_json GET "/api/workflow-runs/$run_id/steps" '{}' api-workflow-runtime-proof-steps-before-draft)"
draft_step_id="$(workflow_step_id_by_key "$steps_file" draft_result)"
fetch_json PATCH "/api/workflow-step-runs/$draft_step_id" "$(jq -nc --arg artifact_id "$artifact_id" '{
  status: "completed",
  output_payload: {result: "draft artifact generated", artifact_id: $artifact_id},
  artifact_ids: [$artifact_id]
}')" api-workflow-runtime-proof-step-draft-completed >/dev/null

final_run_file="$(fetch_json GET "/api/workflow-runs/$run_id" '{}' api-workflow-runtime-proof-run-final)"
final_steps_file="$(fetch_json GET "/api/workflow-runs/$run_id/steps" '{}' api-workflow-runtime-proof-steps-final)"
transitions_file="$(fetch_json GET "/api/workflow-runs/$run_id/transitions" '{}' api-workflow-runtime-proof-transitions)"
graph_file="$(fetch_json GET "/api/workflow-runs/$run_id/graph" '{}' api-workflow-runtime-proof-graph)"
artifacts_file="$(fetch_json GET "/api/sessions/$session_id/artifacts" '{}' api-workflow-runtime-proof-artifacts)"

memory_source_file="$(fetch_json POST /api/semantic-sources "$(jq -nc --arg run_id "$run_id" '{
  source_type: "memory",
  source_uri: ("memory://managed-workflow-runtime-proof/" + $run_id),
  display_name: "Managed workflow runtime proof memory",
  metadata: {source: "managed-workflow-runtime-evidence-gate"}
}')" api-workflow-runtime-proof-memory-source)"
memory_source_id="$(jq -r '.response.id // empty' "$memory_source_file")"
fetch_json POST /api/semantic-objects "$(jq -nc --arg source_id "$memory_source_id" --arg run_id "$run_id" '{
  source_id: $source_id,
  object_type: "memory",
  object_key: ("memory:managed-workflow-runtime-proof:" + $run_id),
  title: "Managed workflow runtime proof memory",
  summary: "Runtime proof memory is isolated to the managed-workflow proof partition.",
  content: {workflow_run_id: $run_id, evidence_gate: "managed-workflow-runtime-evidence-gate"},
  semantic_scopes: {
    domain_scope: "managed-workflow-proof",
    workflow_scope: "runtime-proof",
    memory_scope: "operator-evidence",
    share_policy: "isolated"
  },
  trust_level: "human_verified",
  freshness: "current"
}')" api-workflow-runtime-proof-memory-object >/dev/null

memory_summary_file="$(fetch_json GET /api/memory-governance/summary '{}' api-workflow-runtime-proof-memory-governance-summary)"
partition_key="domain=managed-workflow-proof|workflow=runtime-proof|memory=operator-evidence"
partition_query="$(printf '%s' "$partition_key" | urlencode)"
memory_partition_file="$(fetch_json GET "/api/memory-governance/partitions?partition_key=$partition_query" '{}' api-workflow-runtime-proof-memory-governance-partition)"
memory_writebacks_file="$(fetch_json GET /api/memory-governance/writebacks?status=pending '{}' api-workflow-runtime-proof-memory-governance-writebacks)"

summary_file="$EVIDENCE_DIR/summary.json"
jq -n \
  --arg evidence_dir "$EVIDENCE_DIR" \
  --slurpfile run "$final_run_file" \
  --slurpfile steps "$final_steps_file" \
  --slurpfile transitions "$transitions_file" \
  --slurpfile graph "$graph_file" \
  --slurpfile scheduler "$scheduler_run_file" \
  --slurpfile initial_claim "$collect_b_initial_claim_file" \
  --slurpfile reclaim "$collect_b_reclaim_file" \
  --slurpfile artifacts "$artifacts_file" \
  --slurpfile memory "$memory_summary_file" \
  --slurpfile partition "$memory_partition_file" \
  --slurpfile writebacks "$memory_writebacks_file" \
  '{
    status: (if ($run[0].response.status == "completed"
      and ($scheduler[0].response.workflow_scheduled_steps.activated_count // 0) >= 1
      and (($scheduler[0].response.actions // [] | index("workflow_scheduled_steps_activated")) != null)
      and (($transitions[0].response | map(.transition_type) | index("retry")) != null)
      and (($transitions[0].response | map(.transition_type) | index("schedule")) != null)
      and (($transitions[0].response | map(.transition_type) | index("fan_in")) != null)
      and (($transitions[0].response | map(.transition_type) | index("complete")) != null)
      and ($initial_claim[0].response.step.id == $reclaim[0].response.step.id)
      and ($initial_claim[0].response.step.claimed_by_worker == "managed-workflow-lease-drill-a")
      and ($reclaim[0].response.step.claimed_by_worker == "managed-workflow-lease-drill-b")
      and ($initial_claim[0].response.step.lease_expires_at != $reclaim[0].response.step.lease_expires_at)
      and (($artifacts[0].response | length) >= 1)
      and ($graph[0].response.status == "completed")
      and ($partition[0].response.partition.partition_key == "domain=managed-workflow-proof|workflow=runtime-proof|memory=operator-evidence"))
      then "passed" else "failed" end),
    workflow_run_id: $run[0].response.id,
    workflow_status: $run[0].response.status,
    step_count: ($steps[0].response | length),
    transition_types: ($transitions[0].response | map(.transition_type) | unique),
    lease_expiry_reclaim: {
      status: (if ($initial_claim[0].response.step.id == $reclaim[0].response.step.id
        and $initial_claim[0].response.step.claimed_by_worker == "managed-workflow-lease-drill-a"
        and $reclaim[0].response.step.claimed_by_worker == "managed-workflow-lease-drill-b"
        and $initial_claim[0].response.step.lease_expires_at != $reclaim[0].response.step.lease_expires_at)
        then "passed" else "failed" end),
      workflow_step_run_id: $reclaim[0].response.step.id,
      initial_worker: $initial_claim[0].response.step.claimed_by_worker,
      reclaim_worker: $reclaim[0].response.step.claimed_by_worker,
      initial_lease_expires_at: $initial_claim[0].response.step.lease_expires_at,
      reclaim_lease_expires_at: $reclaim[0].response.step.lease_expires_at
    },
    scheduler_workflow_activation: $scheduler[0].response.workflow_scheduled_steps,
    graph: {
      status: $graph[0].response.status,
      node_count: $graph[0].response.node_count,
      edge_count: $graph[0].response.edge_count,
      due_scheduled_count: $graph[0].response.due_scheduled_count,
      status_counts: $graph[0].response.status_counts
    },
    artifact_count: ($artifacts[0].response | length),
    memory_governance: {
      status: $memory[0].response.status,
      partition_count: $memory[0].response.partition_count,
      proof_partition: $partition[0].response.partition,
      pending_writebacks: $writebacks[0].response.pending_count
    },
    evidence_dir: $evidence_dir
  }' >"$summary_file"

cat "$summary_file"

if [[ "$(jq -r '.status' "$summary_file")" != "passed" ]]; then
  echo "managed workflow runtime evidence gate failed" >&2
  [[ "${ALLOW_BLOCKED:-0}" == "1" ]] && exit 0
  exit 1
fi
