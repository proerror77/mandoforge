#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_CODEX_ADAPTER_VERIFY_SUBJECT:-admin-1}"
ROLES="${MANDOFORGE_CODEX_ADAPTER_VERIFY_ROLES:-admin}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

if ! command -v jq >/dev/null 2>&1; then
  echo "codex adapter verification requires jq" >&2
  exit 1
fi

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
      title: "Codex adapter verification",
      message: "Verify approved codex.exec ingests JSONL events and captures final message artifact."
    }')" \
    | jq -r '.id'
)"

APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/codex.exec/execute" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" '{
      session_id: $session_id,
      args: {
        task: "Return a short adapter verification message.",
        sandbox_mode: "read-only"
      }
    }')" \
    | jq -r '.approval_id'
)"

curl -fsS -X POST "$BASE_URL/api/approvals/$APPROVAL_ID/approve" \
  "${auth_headers[@]}" \
  >/dev/null

TOOL_CALL="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    "${auth_headers[@]}" \
    | jq -c 'map(select(.tool_name == "codex.exec"))[0]'
)"
echo "$TOOL_CALL" | jq -e '
  (.status == "completed")
  and (.result.status == 0)
  and (.result.final_message | contains("Fake Codex adapter final message"))
' >/dev/null

EVENT_TYPES="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    "${auth_headers[@]}" \
    | jq -r '[.[].event_type] | unique | join(",")'
)"
if [[ "$EVENT_TYPES" != *"codex.event"* || "$EVENT_TYPES" != *"codex.task.completed"* ]]; then
  echo "expected codex.event and codex.task.completed, got $EVENT_TYPES" >&2
  exit 1
fi

ARTIFACTS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/artifacts" \
    "${auth_headers[@]}" \
    | jq -r '[.[].name] | join(",")'
)"
if [[ "$ARTIFACTS" != *"codex-final-message.md"* ]]; then
  echo "expected codex-final-message.md artifact, got $ARTIFACTS" >&2
  exit 1
fi

echo "codex adapter verification ok"
echo "session_id=$SESSION_ID"
echo "approval_id=$APPROVAL_ID"
echo "event_types=$EVENT_TYPES"
echo "artifacts=$ARTIFACTS"
