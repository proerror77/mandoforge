#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"

curl -fsS "$BASE_URL/healthz" >/dev/null
curl -fsS "$BASE_URL/api/agents" >/tmp/mandoforge-agents.json

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    -H 'content-type: application/json' \
    -d '{"agent_id":"11111111-1111-4111-8111-111111111111","title":"Generic runtime diagnostics smoke","message":"Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md."}' \
    | sed -E 's/.*"id":"([^"]+)".*/\1/'
)"

curl -fsS -X POST "$BASE_URL/api/sessions/$SESSION_ID/run" >/dev/null
curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" | grep -q 'approval.requested'
curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" | grep -q 'shell.exec'
curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/audit-logs" | grep -q 'policy.requires_approval'

echo "smoke ok: $SESSION_ID"
