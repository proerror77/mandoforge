#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
DATABASE_URL="${DATABASE_URL:-postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge}"

if ! command -v jq >/dev/null 2>&1; then
  echo "postgres sql.query verification requires jq" >&2
  exit 1
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

DATABASE_URL="$DATABASE_URL" COUNT="${COUNT:-96}" ./scripts/seed-platform-events.sh >/dev/null

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
      title: "Postgres sql.query verification",
      message: "Verify sql.query executes against live Postgres generic_demo.platform_events."
    }')" \
    | jq -r '.id'
)"

QUERY_RESULT="$(
  curl -fsS -X POST "$BASE_URL/api/tools/sql.query/execute" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" '{
      session_id: $session_id,
      args: {
        sql: "select event_type, status, count(*)::int as count from generic_demo.platform_events group by event_type, status order by event_type, status"
      }
    }')"
)"

echo "$QUERY_RESULT" | jq -e '
  (.row_count > 0)
  and (.rows | type == "array")
  and (.[ "rows" ][0] | has("event_type") and has("status") and has("count"))
' >/dev/null

TOOL_CALL_STATUS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    | jq -r 'map(select(.tool_name == "sql.query"))[0].status'
)"
if [[ "$TOOL_CALL_STATUS" != "completed" ]]; then
  echo "expected completed sql.query tool call, got $TOOL_CALL_STATUS" >&2
  exit 1
fi

echo "postgres sql.query verification ok"
echo "session_id=$SESSION_ID"
echo "row_count=$(echo "$QUERY_RESULT" | jq -r '.row_count')"
echo "first_row=$(echo "$QUERY_RESULT" | jq -c '.rows[0]')"
echo "tool_call_status=$TOOL_CALL_STATUS"
