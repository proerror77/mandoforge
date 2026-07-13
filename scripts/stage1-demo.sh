#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
WORKSPACE_ROOT="${MANDOFORGE_WORKSPACE_ROOT:-.mandoforge/workspaces}"
SUBJECT="${MANDOFORGE_STAGE1_DEMO_SUBJECT:-stage1-demo-principal}"
ROLES="${MANDOFORGE_STAGE1_DEMO_ROLES:-admin}"
SESSION_LOOP_POLL_ATTEMPTS="${MANDOFORGE_STAGE1_DEMO_POLL_ATTEMPTS:-100}"
SESSION_LOOP_POLL_INTERVAL="${MANDOFORGE_STAGE1_DEMO_POLL_INTERVAL:-0.2}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

if ! command -v jq >/dev/null 2>&1; then
  echo "stage1 demo requires jq" >&2
  exit 1
fi

drain_session_loop() {
  local session_id="$1"
  local reason="$2"
  local job_id=""

  for _ in $(seq 1 "$SESSION_LOOP_POLL_ATTEMPTS"); do
    job_id="$(
      curl -fsS "$BASE_URL/api/session-loop-jobs" \
        "${auth_headers[@]}" \
        | jq -r --arg session_id "$session_id" '
          map(select(.session_id == $session_id and .status == "queued")) | last.id // empty
        '
    )"
    if [[ -n "$job_id" && "$job_id" != "null" ]]; then
      break
    fi
    sleep "$SESSION_LOOP_POLL_INTERVAL"
  done

  if [[ -z "$job_id" || "$job_id" == "null" ]]; then
    echo "no queued session loop job for $session_id after $reason" >&2
    curl -fsS "$BASE_URL/api/session-loop-jobs" "${auth_headers[@]}" \
      | jq --arg session_id "$session_id" 'map(select(.session_id == $session_id))' >&2
    curl -fsS "$BASE_URL/api/sessions/$session_id/events" "${auth_headers[@]}" \
      | jq 'map({event_type, created_at, payload})' >&2
    curl -fsS "$BASE_URL/api/sessions/$session_id/tool-calls" "${auth_headers[@]}" \
      | jq 'map({tool_name, status, error})' >&2
    exit 1
  fi

  curl -fsS -X POST "$BASE_URL/api/session-loop-jobs/$job_id/run" \
    "${auth_headers[@]}" \
    >/dev/null
}

curl -fsS "$BASE_URL/healthz" >/dev/null

AGENT_ID="$(
  curl -fsS "$BASE_URL/api/agents" \
    "${auth_headers[@]}" \
    | jq -r 'map(select(.name == "Generic Orchestrator Agent"))[0].id // empty'
)"
if [[ -z "$AGENT_ID" || "$AGENT_ID" == "null" ]]; then
  echo "no Generic Orchestrator Agent returned by $BASE_URL/api/agents" >&2
  exit 1
fi

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg agent_id "$AGENT_ID" '{
      agent_id: $agent_id,
      title: "Stage 1 demo",
      message: "Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md."
    }')" \
    | jq -r '.id'
)"

curl -fsS -X POST "$BASE_URL/api/sessions/$SESSION_ID/run" "${auth_headers[@]}" >/dev/null
drain_session_loop "$SESSION_ID" "session run"

SHELL_APPROVAL_ID="$(
  curl -fsS "$BASE_URL/api/approvals" \
    "${auth_headers[@]}" \
    | jq -r --arg session_id "$SESSION_ID" '
      map(select(.session_id == $session_id and .action == "shell.exec" and .status == "pending"))[0].id
    '
)"
curl -fsS -X POST "$BASE_URL/api/approvals/$SHELL_APPROVAL_ID/approve" "${auth_headers[@]}" >/dev/null
drain_session_loop "$SESSION_ID" "shell approval"

WRITE_APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/file.write/execute" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" '{
      session_id: $session_id,
      args: {
        path: "diagnostics.md",
        content: "# Stage 1 Diagnostics\n\nApproved file.write created this workspace artifact."
      }
    }')" \
    | jq -r '.approval_id'
)"
curl -fsS -X POST "$BASE_URL/api/approvals/$WRITE_APPROVAL_ID/approve" "${auth_headers[@]}" >/dev/null

EVENT_TYPES="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    "${auth_headers[@]}" \
    | jq -r '[.[].event_type] | unique | join(",")'
)"
TOOL_CALLS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    "${auth_headers[@]}" \
    | jq -r '[.[] | "\(.tool_name):\(.status)"] | join(",")'
)"
ARTIFACTS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/artifacts" \
    "${auth_headers[@]}" \
    | jq -r '[.[].name] | join(",")'
)"
AUDIT_ACTIONS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/audit-logs" \
    "${auth_headers[@]}" \
    | jq -r '[.[].action] | unique | join(",")'
)"

WORKSPACE_FILE="$WORKSPACE_ROOT/$SESSION_ID/diagnostics.md"
if [[ ! -f "$WORKSPACE_FILE" ]]; then
  echo "expected workspace artifact missing: $WORKSPACE_FILE" >&2
  exit 1
fi

echo "stage1 demo ok"
echo "session_id=$SESSION_ID"
echo "event_types=$EVENT_TYPES"
echo "tool_calls=$TOOL_CALLS"
echo "artifacts=$ARTIFACTS"
echo "audit_actions=$AUDIT_ACTIONS"
echo "workspace_file=$WORKSPACE_FILE"
