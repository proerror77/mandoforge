#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"

if ! command -v jq >/dev/null 2>&1; then
  echo "docker shell runner verification requires jq" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not available; start Docker before running this verification" >&2
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
      title: "Docker shell runner verification",
      message: "Verify approved shell.exec runs through the Docker sandbox runner."
    }')" \
    | jq -r '.id'
)"

APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/shell.exec/execute" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" '{
      session_id: $session_id,
      args: {
        command: "printf sandbox-ok: && pwd"
      }
    }')" \
    | jq -r '.approval_id'
)"

curl -fsS -X POST "$BASE_URL/api/approvals/$APPROVAL_ID/approve" >/dev/null

TOOL_CALL="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    | jq -c 'map(select(.tool_name == "shell.exec"))[0]'
)"

echo "$TOOL_CALL" | jq -e '
  (.status == "completed")
  and (.result.runner == "docker")
  and (.result.exit_code == 0)
  and (.result.stdout | contains("sandbox-ok:/workspace"))
' >/dev/null

echo "docker shell runner verification ok"
echo "session_id=$SESSION_ID"
echo "approval_id=$APPROVAL_ID"
echo "runner=$(echo "$TOOL_CALL" | jq -r '.result.runner')"
echo "stdout=$(echo "$TOOL_CALL" | jq -r '.result.stdout')"
