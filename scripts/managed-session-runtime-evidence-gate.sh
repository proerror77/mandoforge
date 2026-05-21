#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/managed-session-runtime-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
CONTROLLER_URL="${MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL:-}"
CONTROLLER_TOKEN="${MANAGED_SESSION_RESTART_RESUME_CONTROLLER_TOKEN:-}"
SOURCE_FILE="${MANAGED_SESSION_RESTART_RESUME_EVIDENCE_FILE:-}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "managed-session runtime evidence gate requires $1" >&2
    exit 1
  fi
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

capture_controller_evidence() {
  local target="$1"
  local response_body
  local http_status
  response_body="$(mktemp)"

  local headers=()
  if [[ -n "$CONTROLLER_TOKEN" ]]; then
    headers+=(-H "authorization: Bearer $CONTROLLER_TOKEN")
  fi

  http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X POST "${headers[@]}" \
    -H "content-type: application/json" \
    -d '{}' \
    "$CONTROLLER_URL")"
  if [[ "$http_status" != 2* ]]; then
    echo "managed-session restart/resume controller returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
}

validate_evidence() {
  local artifact="$1"
  local summary_json="$EVIDENCE_DIR/summary.json"
  local summary_txt="$EVIDENCE_DIR/summary.txt"

  local status
  local target_id
  local target_kind
  local enqueue_event_persisted
  local worker_drain_observed
  local api_restarted
  local worker_restarted
  local session_state_resumed
  local processed_event_seq_preserved
  local thread_lineage_preserved
  local finalization_fenced
  local stale_worker_rejected
  local runtime_turn_completed
  local final_message_preserved

  status="$(jq -r '.status // "unknown"' "$artifact")"
  target_id="$(jq -r '.target.id // .target.cluster_id // .target.deployment_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  enqueue_event_persisted="$(jq -r '.session_loop.enqueue_event_persisted // false' "$artifact")"
  worker_drain_observed="$(jq -r '.session_loop.worker_drain_observed // false' "$artifact")"
  api_restarted="$(jq -r '.restart.api_restarted // false' "$artifact")"
  worker_restarted="$(jq -r '.restart.worker_restarted // false' "$artifact")"
  session_state_resumed="$(jq -r '.resume.session_state_resumed // false' "$artifact")"
  processed_event_seq_preserved="$(jq -r '.resume.processed_event_seq_preserved // false' "$artifact")"
  thread_lineage_preserved="$(jq -r '.thread_lineage.preserved // false' "$artifact")"
  finalization_fenced="$(jq -r '.lease_fencing.finalization_fenced // false' "$artifact")"
  stale_worker_rejected="$(jq -r '.lease_fencing.stale_worker_rejected // false' "$artifact")"
  runtime_turn_completed="$(jq -r '.runtime_turn.completed // false' "$artifact")"
  final_message_preserved="$(jq -r '.runtime_turn.final_message_preserved // false' "$artifact")"

  local blocked_count=0
  [[ "$status" == "validated" || "$status" == "completed" || "$status" == "ready" ]] || blocked_count=$((blocked_count + 1))
  is_production_identity "$target_id" || blocked_count=$((blocked_count + 1))
  [[ "$target_kind" == "managed_session_runtime" || "$target_kind" == "production_runtime_cluster" || "$target_kind" == "managed_agent_cluster" ]] || blocked_count=$((blocked_count + 1))
  [[ "$enqueue_event_persisted" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_drain_observed" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$api_restarted" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_restarted" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$session_state_resumed" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$processed_event_seq_preserved" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$thread_lineage_preserved" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$finalization_fenced" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$stale_worker_rejected" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$runtime_turn_completed" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$final_message_preserved" == "true" ]] || blocked_count=$((blocked_count + 1))

  local gate_status="ready"
  if [[ "$blocked_count" != "0" ]]; then
    gate_status="blocked"
  fi

  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg status "$gate_status" \
    --arg artifact "$artifact" \
    --arg target_id "$target_id" \
    --arg target_kind "$target_kind" \
    --argjson blocked_count "$blocked_count" \
    --argjson enqueue_event_persisted "$enqueue_event_persisted" \
    --argjson worker_drain_observed "$worker_drain_observed" \
    --argjson api_restarted "$api_restarted" \
    --argjson worker_restarted "$worker_restarted" \
    --argjson session_state_resumed "$session_state_resumed" \
    --argjson processed_event_seq_preserved "$processed_event_seq_preserved" \
    --argjson thread_lineage_preserved "$thread_lineage_preserved" \
    --argjson finalization_fenced "$finalization_fenced" \
    --argjson stale_worker_rejected "$stale_worker_rejected" \
    --argjson runtime_turn_completed "$runtime_turn_completed" \
    --argjson final_message_preserved "$final_message_preserved" \
    '{
      generated_at: $generated_at,
      status: $status,
      artifact: $artifact,
      target: {
        id: $target_id,
        kind: $target_kind
      },
      blocked_count: $blocked_count,
      checks: {
        enqueue_event_persisted: $enqueue_event_persisted,
        worker_drain_observed: $worker_drain_observed,
        api_restarted: $api_restarted,
        worker_restarted: $worker_restarted,
        session_state_resumed: $session_state_resumed,
        processed_event_seq_preserved: $processed_event_seq_preserved,
        thread_lineage_preserved: $thread_lineage_preserved,
        finalization_fenced: $finalization_fenced,
        stale_worker_rejected: $stale_worker_rejected,
        runtime_turn_completed: $runtime_turn_completed,
        final_message_preserved: $final_message_preserved
      }
    }' >"$summary_json"

  {
    echo "managed_session_runtime_status=$gate_status"
    echo "target_id=$target_id"
    echo "target_kind=$target_kind"
    echo "blocked_count=$blocked_count"
  } >"$summary_txt"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "managed-session runtime restart/resume evidence is blocked: $blocked_count check(s) failed" >&2
    exit 1
  fi
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"

artifact="$EVIDENCE_DIR/managed-session-restart-resume-evidence.json"
if [[ -n "$SOURCE_FILE" ]]; then
  cp "$SOURCE_FILE" "$artifact"
elif [[ -n "$CONTROLLER_URL" ]]; then
  require_cmd curl
  capture_controller_evidence "$artifact"
elif [[ ! -s "$artifact" ]]; then
  echo "set MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL or MANAGED_SESSION_RESTART_RESUME_EVIDENCE_FILE" >&2
  exit 1
fi

validate_evidence "$artifact"
