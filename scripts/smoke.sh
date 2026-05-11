#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"

curl -fsS "$BASE_URL/healthz" >/dev/null
curl -fsS "$BASE_URL/api/agents" >/tmp/mandoforge-agents.json

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    -H 'content-type: application/json' \
    -d '{"agent_id":"11111111-1111-4111-8111-111111111111","title":"GMV diagnosis smoke","message":"昨天 GMV 为什么下降？请找出主要原因，并生成今天可执行的运营建议。"}' \
    | sed -E 's/.*"id":"([^"]+)".*/\1/'
)"

curl -fsS -X POST "$BASE_URL/api/sessions/$SESSION_ID/run" >/dev/null
curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" | grep -q 'approval.requested'

echo "smoke ok: $SESSION_ID"

