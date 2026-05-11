#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"

if [[ "${RUN_PROVIDER_SMOKE:-0}" != "1" ]]; then
  echo "external provider verification skipped; set RUN_PROVIDER_SMOKE=1 with provider env to run"
  exit 0
fi

if [[ -z "${MANDOFORGE_PROVIDER_BASE_URL:-}" || -z "${MANDOFORGE_PROVIDER_API_KEY:-}" ]]; then
  echo "RUN_PROVIDER_SMOKE=1 requires MANDOFORGE_PROVIDER_BASE_URL and MANDOFORGE_PROVIDER_API_KEY" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "external provider verification requires jq" >&2
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
      title: "External provider verification",
      message: "Verify the configured OpenAI-compatible provider transport responds through the runtime harness."
    }')" \
    | jq -r '.id'
)"

curl -fsS -X POST "$BASE_URL/api/sessions/$SESSION_ID/run" >/dev/null

PROVIDER="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    | jq -r 'map(select(.event_type == "llm.response"))[-1].payload.provider // ""'
)"
if [[ "$PROVIDER" != "openai-compatible-http" ]]; then
  echo "expected llm.response provider=openai-compatible-http, got ${PROVIDER:-<empty>}" >&2
  exit 1
fi

EVENT_TYPES="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    | jq -r '[.[].event_type] | unique | join(",")'
)"
if [[ "$EVENT_TYPES" != *"llm.request"* || "$EVENT_TYPES" != *"llm.response"* ]]; then
  echo "expected llm.request and llm.response events, got $EVENT_TYPES" >&2
  exit 1
fi

echo "external provider verification ok"
echo "session_id=$SESSION_ID"
echo "provider=$PROVIDER"
echo "event_types=$EVENT_TYPES"
