#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/managed-session-runtime-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
CONTROLLER_URL="${MANAGED_SESSION_RESTART_RESUME_CONTROLLER_URL:-}"
CONTROLLER_TOKEN="${MANAGED_SESSION_RESTART_RESUME_CONTROLLER_TOKEN:-}"
SOURCE_FILE="${MANAGED_SESSION_RESTART_RESUME_EVIDENCE_FILE:-}"
EXPECTED_TARGET_ID="${MANDOFORGE_STAGE2_MANAGED_SESSION_RUNTIME_TARGET_ID:-}"

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

is_nonnegative_integer() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

is_positive_integer() {
  [[ "$1" =~ ^[0-9]+$ && "$1" -gt 0 ]]
}

managed_session_detail_issue() {
  local artifact="$1"
  local pending_start
  local pending_end
  local processed_before
  local processed_after
  local original_thread_id
  local resumed_thread_id
  local active_worker_lease_id
  local stale_worker_lease_id
  local stale_rejection_reason
  local runtime_turn_id
  local final_message_evidence

  pending_start="$(jq -r '.session_loop.pending_event_seq_start // .session_loop.event_window.start // .session_loop.sequence_range.start // ""' "$artifact")"
  pending_end="$(jq -r '.session_loop.pending_event_seq_end // .session_loop.event_window.end // .session_loop.sequence_range.end // ""' "$artifact")"
  processed_before="$(jq -r '.resume.processed_event_seq_before_restart // .resume.processed_event_seq_before // ""' "$artifact")"
  processed_after="$(jq -r '.resume.processed_event_seq_after_resume // .resume.processed_event_seq_after // .resume.processed_event_seq // ""' "$artifact")"
  original_thread_id="$(jq -r '.thread_lineage.original_thread_id // .thread_lineage.before_restart_thread_id // ""' "$artifact")"
  resumed_thread_id="$(jq -r '.thread_lineage.resumed_thread_id // .thread_lineage.after_restart_thread_id // ""' "$artifact")"
  active_worker_lease_id="$(jq -r '.lease_fencing.active_worker_lease_id // .lease_fencing.valid_worker_lease_id // ""' "$artifact")"
  stale_worker_lease_id="$(jq -r '.lease_fencing.stale_worker_lease_id // .lease_fencing.rejected_worker_lease_id // ""' "$artifact")"
  stale_rejection_reason="$(jq -r '.lease_fencing.stale_rejection_reason // .lease_fencing.rejection_reason // ""' "$artifact")"
  runtime_turn_id="$(jq -r '.runtime_turn.turn_id // .runtime_turn.id // ""' "$artifact")"
  final_message_evidence="$(jq -r '.runtime_turn.final_message // .runtime_turn.final_message_text // .runtime_turn.final_message_artifact_id // .runtime_turn.final_artifact_id // ""' "$artifact")"

  if ! is_positive_integer "$pending_start" || ! is_positive_integer "$pending_end" || [[ "$pending_start" -gt "$pending_end" ]]; then
    printf 'session-loop event cursor window evidence incomplete'
    return 0
  fi
  if ! is_nonnegative_integer "$processed_before" || ! is_nonnegative_integer "$processed_after" || [[ "$processed_after" != "$processed_before" || "$processed_after" -lt "$pending_end" ]]; then
    printf 'processed event cursor sequence evidence incomplete'
    return 0
  fi
  if [[ -z "$original_thread_id" || -z "$resumed_thread_id" || "$original_thread_id" != "$resumed_thread_id" ]]; then
    printf 'thread lineage id evidence incomplete'
    return 0
  fi
  if [[ -z "$active_worker_lease_id" || -z "$stale_worker_lease_id" || "$active_worker_lease_id" == "$stale_worker_lease_id" || -z "$stale_rejection_reason" ]]; then
    printf 'lease fencing id evidence incomplete'
    return 0
  fi
  if [[ -z "$runtime_turn_id" || -z "$final_message_evidence" ]]; then
    printf 'runtime turn final message detail evidence incomplete'
    return 0
  fi

  return 1
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
  local managed_session_detail_status
  local managed_session_detail_issue_text

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
  managed_session_detail_status="complete"
  managed_session_detail_issue_text=""
  if managed_session_detail_issue_text="$(managed_session_detail_issue "$artifact")"; then
    managed_session_detail_status="blocked"
  fi

  local blocked_count=0
  [[ "$status" == "validated" || "$status" == "completed" || "$status" == "ready" ]] || blocked_count=$((blocked_count + 1))
  is_production_identity "$target_id" || blocked_count=$((blocked_count + 1))
  [[ -z "$EXPECTED_TARGET_ID" || "$EXPECTED_TARGET_ID" == "$target_id" ]] || blocked_count=$((blocked_count + 1))
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
  [[ "$managed_session_detail_status" == "complete" ]] || blocked_count=$((blocked_count + 1))

  local gate_status="ready"
  if [[ "$blocked_count" != "0" ]]; then
    gate_status="blocked"
  fi

  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg status "$gate_status" \
    --arg artifact "$artifact" \
    --arg target_id "$target_id" \
    --arg expected_target_id "$EXPECTED_TARGET_ID" \
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
    --arg managed_session_detail_status "$managed_session_detail_status" \
    --arg managed_session_detail_issue "$managed_session_detail_issue_text" \
    '{
      generated_at: $generated_at,
      status: $status,
      artifact: $artifact,
      target: {
        id: $target_id,
        expected_id: (if $expected_target_id == "" then null else $expected_target_id end),
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
        final_message_preserved: $final_message_preserved,
        structured_restart_resume_details: $managed_session_detail_status,
        structured_restart_resume_issue: (if $managed_session_detail_issue == "" then null else $managed_session_detail_issue end)
      }
    }' >"$summary_json"

  {
    echo "managed_session_runtime_status=$gate_status"
    echo "target_id=$target_id"
    echo "target_kind=$target_kind"
    echo "blocked_count=$blocked_count"
    echo "structured_restart_resume_details=$managed_session_detail_status"
    if [[ -n "$managed_session_detail_issue_text" ]]; then
      echo "structured_restart_resume_issue=$managed_session_detail_issue_text"
    fi
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
