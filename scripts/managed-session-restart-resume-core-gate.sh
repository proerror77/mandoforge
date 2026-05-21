#!/usr/bin/env bash
set -euo pipefail

GATE_ADDR="${GATE_ADDR:-127.0.0.1:8790}"
BASE_URL="${BASE_URL:-http://$GATE_ADDR}"
DATABASE_URL="${DATABASE_URL:-postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge}"
WORKSPACE_ROOT="${MANDOFORGE_WORKSPACE_ROOT:-.mandoforge/restart-resume-core-workspaces}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/managed-session-restart-resume-core-evidence}"
START_POSTGRES="${START_POSTGRES:-0}"
SUBJECT="${MANDOFORGE_RESTART_RESUME_SUBJECT:-admin-1}"
ROLES="${MANDOFORGE_RESTART_RESUME_ROLES:-admin}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
QUEUE="restart-resume-core-$RUN_ID"
TARGET_ID="agent-os-core-local-$RUN_ID"
API_PID=""
STARTED_POSTGRES=0

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "managed-session restart/resume core gate requires $1" >&2
    exit 1
  fi
}

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
  fi
  if [[ "$STARTED_POSTGRES" == "1" ]]; then
    docker compose stop postgres >/dev/null 2>&1 || true
  fi
}

api_request() {
  local method="$1"
  local path="$2"
  local payload=""
  if [[ $# -ge 3 ]]; then
    payload="$3"
  fi
  local response_file
  local status
  response_file="$(mktemp)"
  if [[ "$method" == "POST" ]]; then
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" -X POST "$BASE_URL$path" \
        "${auth_headers[@]}" \
        -H 'content-type: application/json' \
        -d "$payload"
    )"
  else
    status="$(
      curl -sS -o "$response_file" -w "%{http_code}" "$BASE_URL$path" \
        "${auth_headers[@]}"
    )"
  fi
  if [[ "$status" != 2* ]]; then
    echo "managed-session restart/resume API request failed: $method $path returned HTTP $status" >&2
    sed -n '1,80p' "$response_file" >&2
    rm -f "$response_file"
    exit 1
  fi
  cat "$response_file"
  rm -f "$response_file"
}

api_get() {
  local path="$1"
  api_request GET "$path"
}

api_post() {
  local path="$1"
  local payload="{}"
  if [[ $# -ge 2 ]]; then
    payload="$2"
  fi
  api_request POST "$path" "$payload"
}

wait_for_api() {
  local log_file="$1"
  for _ in $(seq 1 120); do
    if curl -fsS "$BASE_URL/healthz" >/dev/null 2>&1; then
      return 0
    fi
    if [[ -n "${API_PID:-}" ]] && ! kill -0 "$API_PID" >/dev/null 2>&1; then
      echo "managed-session restart/resume API exited early; log follows:" >&2
      cat "$log_file" >&2
      exit 1
    fi
    sleep 0.5
  done
  echo "managed-session restart/resume API did not become healthy; log follows:" >&2
  cat "$log_file" >&2
  exit 1
}

start_api() {
  local log_file="$1"
  env \
    "MANDOFORGE_ADDR=$GATE_ADDR" \
    "MANDOFORGE_WORKSPACE_ROOT=$WORKSPACE_ROOT" \
    "MANDOFORGE_INSECURE_DEV_AUTH=1" \
    "MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1" \
    "MANDOFORGE_EXECUTION_WORKER=queue" \
    "DATABASE_URL=$DATABASE_URL" \
    cargo run -p mandoforge-api >"$log_file" 2>&1 &
  API_PID="$!"
  wait_for_api "$log_file"
}

stop_api() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
    API_PID=""
  fi
}

ensure_postgres() {
  if [[ "$START_POSTGRES" != "1" ]]; then
    return 0
  fi
  require_cmd docker
  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not available; start Docker or provide DATABASE_URL" >&2
    exit 1
  fi
  if [[ -z "$(docker compose ps -q --status running postgres 2>/dev/null)" ]]; then
    STARTED_POSTGRES=1
  fi
  docker compose up -d postgres >/dev/null
  for _ in $(seq 1 60); do
    if docker compose exec -T postgres pg_isready -U mandoforge -d mandoforge >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  docker compose exec -T postgres pg_isready -U mandoforge -d mandoforge >/dev/null
}

primary_thread_id() {
  local session_id="$1"
  api_get "/api/sessions/$session_id/threads" \
    | jq -r 'map(select(.thread_kind == "primary"))[0].id // empty'
}

session_loop_job_by_id() {
  local job_id="$1"
  api_get /api/session-loop-jobs \
    | jq -c --arg job_id "$job_id" 'map(select(.id == $job_id))[0] // empty'
}

session_loop_job_for_session() {
  local session_id="$1"
  local status="$2"
  api_get /api/session-loop-jobs \
    | jq -c --arg session_id "$session_id" --arg status "$status" '
        map(select(.session_id == $session_id and .status == $status)) | last // empty
      '
}

execution_job_for_approval() {
  local approval_id="$1"
  local status="$2"
  api_get /api/execution-jobs \
    | jq -c --arg approval_id "$approval_id" --arg status "$status" '
        map(select(.approval_id == $approval_id and .status == $status)) | last // empty
      '
}

pending_approval_id() {
  local session_id="$1"
  api_get /api/approvals \
    | jq -r --arg session_id "$session_id" '
        map(select(.session_id == $session_id and .status == "pending"))[0].id // empty
      '
}

run_worker_once() {
  local worker_id="$1"
  local log_file="$2"
  env \
    "BASE_URL=$BASE_URL" \
    "WORKER_ID=$worker_id" \
    "WORKER_QUEUE=$QUEUE" \
    "WORKER_SUBJECT=mandoforge-restart-resume-worker" \
    "WORKER_ROLES=admin" \
    "MANDOFORGE_INSECURE_DEV_AUTH=1" \
    "RUN_ONCE=1" \
    "MAX_JOBS=1" \
    "POLL_INTERVAL_SECONDS=0" \
    cargo run -p mandoforge-api --bin mandoforge-worker >"$log_file" 2>&1
}

queued_work_count() {
  local session_id="$1"
  local loop_count
  local execution_count
  loop_count="$(
    api_get /api/session-loop-jobs \
      | jq --arg session_id "$session_id" '[.[] | select(.session_id == $session_id and (.status == "queued" or .status == "running"))] | length'
  )"
  execution_count="$(
    api_get /api/execution-jobs \
      | jq --arg session_id "$session_id" '[.[] | select(.session_id == $session_id and (.status == "queued" or .status == "running"))] | length'
  )"
  printf '%s\n' $((loop_count + execution_count))
}

drain_session_work() {
  local session_id="$1"
  local worker_prefix="$2"
  local index
  for index in $(seq 1 6); do
    if [[ "$(queued_work_count "$session_id")" == "0" ]]; then
      return 0
    fi
    run_worker_once "$worker_prefix-$index" "$EVIDENCE_DIR/$worker_prefix-$index.log"
  done
  if [[ "$(queued_work_count "$session_id")" != "0" ]]; then
    echo "queued work remained for session $session_id after worker drain" >&2
    exit 1
  fi
}

wait_for_pending_approval() {
  local session_id="$1"
  local approval_id
  for _ in $(seq 1 30); do
    approval_id="$(pending_approval_id "$session_id")"
    if [[ -n "$approval_id" && "$approval_id" != "null" ]]; then
      printf '%s\n' "$approval_id"
      return 0
    fi
    sleep 0.2
  done
  echo "expected pending approval for session $session_id" >&2
  exit 1
}

require_event_type() {
  local events_file="$1"
  local event_type="$2"
  if ! jq -e --arg event_type "$event_type" 'any(.[]; .event_type == $event_type)' "$events_file" >/dev/null; then
    echo "missing event type evidence: $event_type" >&2
    exit 1
  fi
}

require_cmd jq
require_cmd curl
require_cmd cargo

mkdir -p "$EVIDENCE_DIR" "$WORKSPACE_ROOT"
trap cleanup EXIT

ensure_postgres

api_log_before="$EVIDENCE_DIR/api-before-restart.log"
api_log_after="$EVIDENCE_DIR/api-after-restart.log"
worker_before_id="restart-resume-worker-before-$RUN_ID"
worker_after_id="restart-resume-worker-after-$RUN_ID"
worker_stale_id="restart-resume-worker-stale-$RUN_ID"

start_api "$api_log_before"
run_worker_once "$worker_before_id" "$EVIDENCE_DIR/worker-before-restart.log"

agent_id="$(
  api_get /api/agents \
    | jq -r 'map(select(.name == "Generic Orchestrator Agent"))[0].id // empty'
)"
if [[ -z "$agent_id" || "$agent_id" == "null" ]]; then
  echo "no Generic Orchestrator Agent returned by /api/agents" >&2
  exit 1
fi

environment_id="$(
  api_post /api/environments "$(
    jq -nc --arg name "Restart Resume Core $RUN_ID" --arg queue "$QUEUE" '{
      name: $name,
      environment_type: "local",
      worker_queue_binding: {queue: $queue},
      state_mounts: {workspace: "local"}
    }'
  )" | jq -r '.id'
)"

session_id="$(
  api_post /api/sessions "$(
    jq -nc --arg agent_id "$agent_id" --arg environment_id "$environment_id" '{
      agent_id: $agent_id,
      environment_id: $environment_id,
      title: "Managed session restart/resume core gate"
    }'
  )" | jq -r '.id'
)"
thread_before_restart="$(primary_thread_id "$session_id")"

stored_event="$(
  api_post "/api/sessions/$session_id/events" "$(
    jq -nc '{
      events: [{
        type: "user.message",
        payload: {
          message: "Run the managed session restart/resume core evidence flow."
        }
      }]
    }'
  )" | jq -c '.[0]'
)"
trigger_event_id="$(jq -r '.id' <<<"$stored_event")"
trigger_event_seq="$(jq -r '.seq' <<<"$stored_event")"
queued_job="$(session_loop_job_for_session "$session_id" queued)"
if [[ -z "$queued_job" || "$queued_job" == "null" ]]; then
  echo "expected queued session-loop job before restart" >&2
  exit 1
fi

session_loop_job_id="$(jq -r '.id' <<<"$queued_job")"
pending_event_seq_start="$(jq -r '.pending_event_seq_start' <<<"$queued_job")"
pending_event_seq_end="$(jq -r '.pending_event_seq_end' <<<"$queued_job")"
processed_event_seq_before_restart="$(jq -r '.processed_event_seq // 0' <<<"$queued_job")"

stop_api
start_api "$api_log_after"

session_after_restart="$(api_get "/api/sessions/$session_id")"
thread_after_restart="$(primary_thread_id "$session_id")"
job_after_restart="$(session_loop_job_by_id "$session_loop_job_id")"
if [[ -z "$job_after_restart" || "$job_after_restart" == "null" ]]; then
  echo "session-loop job $session_loop_job_id was not durable after API restart" >&2
  exit 1
fi

run_worker_once "$worker_after_id" "$EVIDENCE_DIR/worker-after-restart.log"

completed_job="$(session_loop_job_by_id "$session_loop_job_id")"
processed_event_seq_after_resume="$(jq -r '.processed_event_seq // 0' <<<"$completed_job")"
if [[ "$(jq -r '.status' <<<"$completed_job")" != "completed" ]]; then
  echo "expected resumed session-loop job completed" >&2
  jq . <<<"$completed_job" >&2
  exit 1
fi

stale_status="$(
  curl -sS -o "$EVIDENCE_DIR/stale-worker-response.json" -w "%{http_code}" \
    -X POST "$BASE_URL/api/session-loop-jobs/$session_loop_job_id/run" \
    "${auth_headers[@]}" \
    -H "x-mandoforge-worker-id: $worker_stale_id"
)"
stale_worker_rejected=false
if [[ "$stale_status" == "404" || "$stale_status" == "400" ]]; then
  stale_worker_rejected=true
fi

approval_id="$(wait_for_pending_approval "$session_id")"
api_post "/api/approvals/$approval_id/approve" '{}' >/dev/null

execution_job="$(execution_job_for_approval "$approval_id" queued)"
if [[ -z "$execution_job" || "$execution_job" == "null" ]]; then
  echo "expected queued execution job after approval" >&2
  exit 1
fi
execution_job_id="$(jq -r '.id' <<<"$execution_job")"

drain_session_work "$session_id" "restart-resume-worker-continuation"

execution_job_after="$(api_get /api/execution-jobs | jq -c --arg id "$execution_job_id" 'map(select(.id == $id))[0] // empty')"
session_after_resume="$(api_get "/api/sessions/$session_id")"
thread_after_resume="$(primary_thread_id "$session_id")"
events_file="$EVIDENCE_DIR/session-events.json"
tool_calls_file="$EVIDENCE_DIR/tool-calls.json"
audit_logs_file="$EVIDENCE_DIR/audit-logs.json"
api_get "/api/sessions/$session_id/events" >"$events_file"
api_get "/api/sessions/$session_id/tool-calls" >"$tool_calls_file"
api_get "/api/sessions/$session_id/audit-logs" >"$audit_logs_file"

for event_type in \
  user.message \
  session.loop.queued \
  session.loop.started \
  session.loop.completed \
  approval.requested \
  approval.approved \
  execution.completed \
  tool.result; do
  require_event_type "$events_file" "$event_type"
done

runtime_output="$EVIDENCE_DIR/runtime-adapter-turn-metadata.log"
BASE_URL="$BASE_URL" ./scripts/verify-runtime-adapter-turn-metadata.sh | tee "$runtime_output" >/dev/null
runtime_session_id="$(awk -F= '$1 == "session_id" {print $2}' "$runtime_output" | tail -n 1)"
runtime_events_file="$EVIDENCE_DIR/runtime-session-events.json"
runtime_artifacts_file="$EVIDENCE_DIR/runtime-session-artifacts.json"
api_get "/api/sessions/$runtime_session_id/events" >"$runtime_events_file"
api_get "/api/sessions/$runtime_session_id/artifacts" >"$runtime_artifacts_file"
runtime_final_artifact_id="$(
  jq -r 'map(select(.event_type == "runtime.final"))[0].payload.artifact_id // empty' "$runtime_events_file"
)"

api_restarted=true
worker_restarted=false
if [[ "$worker_before_id" != "$worker_after_id" ]] && grep -q "mandoforge worker processed" "$EVIDENCE_DIR/worker-after-restart.log"; then
  worker_restarted=true
fi
session_state_resumed=false
if jq -e '.status == "idle" or .status == "requires_action"' <<<"$session_after_restart" >/dev/null; then
  session_state_resumed=true
fi
processed_event_seq_preserved=false
if [[ "$processed_event_seq_after_resume" == "$pending_event_seq_end" && "$processed_event_seq_after_resume" -ge "$trigger_event_seq" ]]; then
  processed_event_seq_preserved=true
fi
thread_lineage_preserved=false
if [[ -n "$thread_before_restart" && "$thread_before_restart" == "$thread_after_restart" && "$thread_before_restart" == "$thread_after_resume" ]]; then
  thread_lineage_preserved=true
fi
worker_drain_observed=false
if [[ "$(jq -r '.status' <<<"$completed_job")" == "completed" && "$(jq -r '.status' <<<"$execution_job_after")" == "completed" ]]; then
  worker_drain_observed=true
fi
approval_loopback_persisted=false
if jq -e 'any(.[]; .event_type == "approval.approved") and any(.[]; .event_type == "execution.completed")' "$events_file" >/dev/null; then
  approval_loopback_persisted=true
fi
runtime_turn_completed=false
runtime_final_message_preserved=false
if jq -e 'any(.[]; .event_type == "runtime.turn.completed" and .payload.turn_id == "turn-gate-1")' "$runtime_events_file" >/dev/null; then
  runtime_turn_completed=true
fi
if [[ -n "$runtime_final_artifact_id" ]] && jq -e 'any(.[]; .name == "runtime-final-message.md" and .content.markdown == "Runtime adapter structured final")' "$runtime_artifacts_file" >/dev/null; then
  runtime_final_message_preserved=true
fi

evidence_file="$EVIDENCE_DIR/managed-session-restart-resume-core-evidence.json"
jq -n \
  --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg target_id "$TARGET_ID" \
  --arg run_id "$RUN_ID" \
  --arg queue "$QUEUE" \
  --arg session_id "$session_id" \
  --arg environment_id "$environment_id" \
  --arg trigger_event_id "$trigger_event_id" \
  --argjson trigger_event_seq "$trigger_event_seq" \
  --arg session_loop_job_id "$session_loop_job_id" \
  --argjson pending_event_seq_start "$pending_event_seq_start" \
  --argjson pending_event_seq_end "$pending_event_seq_end" \
  --argjson processed_event_seq_before_restart "$processed_event_seq_before_restart" \
  --argjson processed_event_seq_after_resume "$processed_event_seq_after_resume" \
  --arg thread_before_restart "$thread_before_restart" \
  --arg thread_after_restart "$thread_after_restart" \
  --arg thread_after_resume "$thread_after_resume" \
  --arg worker_before_id "$worker_before_id" \
  --arg worker_after_id "$worker_after_id" \
  --arg worker_stale_id "$worker_stale_id" \
  --arg stale_status "$stale_status" \
  --arg approval_id "$approval_id" \
  --arg execution_job_id "$execution_job_id" \
  --arg runtime_session_id "$runtime_session_id" \
  --arg runtime_final_artifact_id "$runtime_final_artifact_id" \
  --argjson api_restarted "$api_restarted" \
  --argjson worker_restarted "$worker_restarted" \
  --argjson session_state_resumed "$session_state_resumed" \
  --argjson processed_event_seq_preserved "$processed_event_seq_preserved" \
  --argjson thread_lineage_preserved "$thread_lineage_preserved" \
  --argjson worker_drain_observed "$worker_drain_observed" \
  --argjson stale_worker_rejected "$stale_worker_rejected" \
  --argjson approval_loopback_persisted "$approval_loopback_persisted" \
  --argjson runtime_turn_completed "$runtime_turn_completed" \
  --argjson runtime_final_message_preserved "$runtime_final_message_preserved" \
  --arg session_status_after_resume "$(jq -r '.status' <<<"$session_after_resume")" \
  --argjson event_count "$(jq 'length' "$events_file")" \
  --argjson tool_call_count "$(jq 'length' "$tool_calls_file")" \
  --argjson audit_log_count "$(jq 'length' "$audit_logs_file")" \
  '{
    status: "validated",
    generated_at: $generated_at,
    target: {id: $target_id, kind: "agent_os_core_runtime", run_id: $run_id, worker_queue: $queue},
    session: {id: $session_id, environment_id: $environment_id, status_after_resume: $session_status_after_resume},
    session_loop: {
      enqueue_event_persisted: true,
      worker_drain_observed: $worker_drain_observed,
      trigger_event_id: $trigger_event_id,
      trigger_event_seq: $trigger_event_seq,
      job_id: $session_loop_job_id,
      pending_event_seq_start: $pending_event_seq_start,
      pending_event_seq_end: $pending_event_seq_end
    },
    restart: {
      api_restarted: $api_restarted,
      worker_restarted: $worker_restarted,
      worker_before_restart_id: $worker_before_id,
      worker_after_restart_id: $worker_after_id
    },
    resume: {
      session_state_resumed: $session_state_resumed,
      processed_event_seq_preserved: $processed_event_seq_preserved,
      processed_event_seq_before_restart: $processed_event_seq_before_restart,
      processed_event_seq_after_resume: $processed_event_seq_after_resume
    },
    thread_lineage: {
      preserved: $thread_lineage_preserved,
      original_thread_id: $thread_before_restart,
      after_restart_thread_id: $thread_after_restart,
      resumed_thread_id: $thread_after_resume
    },
    lease_fencing: {
      finalization_fenced: $stale_worker_rejected,
      stale_worker_rejected: $stale_worker_rejected,
      active_worker_lease_id: $worker_after_id,
      stale_worker_lease_id: $worker_stale_id,
      stale_rejection_reason: ("HTTP " + $stale_status)
    },
    approval_loopback: {
      approval_id: $approval_id,
      execution_job_id: $execution_job_id,
      persisted: $approval_loopback_persisted
    },
    runtime_turn: {
      session_id: $runtime_session_id,
      turn_id: "turn-gate-1",
      completed: $runtime_turn_completed,
      final_message_preserved: $runtime_final_message_preserved,
      final_artifact_id: $runtime_final_artifact_id
    },
    durable_records: {
      session_event_count: $event_count,
      tool_call_count: $tool_call_count,
      audit_log_count: $audit_log_count,
      session_events_file: "session-events.json",
      tool_calls_file: "tool-calls.json",
      audit_logs_file: "audit-logs.json"
    }
  }' >"$evidence_file"

jq -e '
  .status == "validated"
  and .session_loop.enqueue_event_persisted == true
  and .session_loop.worker_drain_observed == true
  and .restart.api_restarted == true
  and .restart.worker_restarted == true
  and .resume.session_state_resumed == true
  and .resume.processed_event_seq_preserved == true
  and .thread_lineage.preserved == true
  and .lease_fencing.finalization_fenced == true
  and .lease_fencing.stale_worker_rejected == true
  and .approval_loopback.persisted == true
  and .runtime_turn.completed == true
  and .runtime_turn.final_message_preserved == true
  and .durable_records.session_event_count > 0
  and .durable_records.tool_call_count > 0
  and .durable_records.audit_log_count > 0
' "$evidence_file" >/dev/null

summary_file="$EVIDENCE_DIR/summary.txt"
{
  echo "managed_session_restart_resume_core_status=validated"
  echo "session_id=$session_id"
  echo "environment_id=$environment_id"
  echo "session_loop_job_id=$session_loop_job_id"
  echo "pending_event_seq_start=$pending_event_seq_start"
  echo "pending_event_seq_end=$pending_event_seq_end"
  echo "processed_event_seq_before_restart=$processed_event_seq_before_restart"
  echo "processed_event_seq_after_resume=$processed_event_seq_after_resume"
  echo "thread_id=$thread_after_resume"
  echo "approval_id=$approval_id"
  echo "execution_job_id=$execution_job_id"
  echo "runtime_session_id=$runtime_session_id"
  echo "evidence_file=$evidence_file"
} | tee "$summary_file"

echo "managed-session restart/resume core gate ok"
