#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
WORKSPACE_ROOT="${MANDOFORGE_WORKSPACE_ROOT:-.mandoforge/workspaces}"

if ! command -v jq >/dev/null 2>&1; then
  echo "stage1 demo requires jq" >&2
  exit 1
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

AGENT_ID="$(
  curl -fsS "$BASE_URL/api/agents" \
    | jq -r 'map(select(.name == "Generic Orchestrator Agent"))[0].id // empty'
)"
if [[ -z "$AGENT_ID" || "$AGENT_ID" == "null" ]]; then
  echo "no Generic Orchestrator Agent returned by $BASE_URL/api/agents" >&2
  exit 1
fi

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg agent_id "$AGENT_ID" '{
      agent_id: $agent_id,
      title: "Stage 1 demo",
      message: "Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md."
    }')" \
    | jq -r '.id'
)"

curl -fsS -X POST "$BASE_URL/api/sessions/$SESSION_ID/run" >/dev/null

SHELL_APPROVAL_ID="$(
  curl -fsS "$BASE_URL/api/approvals" \
    | jq -r --arg session_id "$SESSION_ID" '
      map(select(.session_id == $session_id and .action == "shell.exec" and .status == "pending"))[0].id
    '
)"
curl -fsS -X POST "$BASE_URL/api/approvals/$SHELL_APPROVAL_ID/approve" >/dev/null

WRITE_APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/file.write/execute" \
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
curl -fsS -X POST "$BASE_URL/api/approvals/$WRITE_APPROVAL_ID/approve" >/dev/null

EVENT_TYPES="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    | jq -r '[.[].event_type] | unique | join(",")'
)"
TOOL_CALLS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    | jq -r '[.[] | "\(.tool_name):\(.status)"] | join(",")'
)"
ARTIFACTS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/artifacts" \
    | jq -r '[.[].name] | join(",")'
)"
AUDIT_ACTIONS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/audit-logs" \
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
