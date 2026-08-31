#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
CONCURRENCY="${CONCURRENCY:-24}"
SUBJECT="${MANDOFORGE_SESSION_EVENT_CONCURRENCY_SUBJECT:-session-event-concurrency-verifier}"
ROLES="${MANDOFORGE_SESSION_EVENT_CONCURRENCY_ROLES:-admin}"
AUTH_TOKEN="${MANDOFORGE_SESSION_EVENT_CONCURRENCY_TOKEN:-${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-}}}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "session event concurrency verifier requires $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

auth_headers=()
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

curl -fsS "$BASE_URL/healthz" >/dev/null

agents_file="$tmpdir/agents.json"
curl -fsS "${auth_headers[@]}" "$BASE_URL/api/agents" >"$agents_file"
agent_id="$(jq -r '.[] | select(.release_state == "active") | .id' "$agents_file" | head -1)"
if [[ -z "$agent_id" ]]; then
  agent_id="$(jq -r '.[0].id // empty' "$agents_file")"
fi
if [[ -z "$agent_id" ]]; then
  echo "session event concurrency verifier could not resolve an agent" >&2
  exit 1
fi

session_file="$tmpdir/session.json"
curl -fsS -X POST "${auth_headers[@]}" \
  -H "content-type: application/json" \
  -d "$(jq -nc --arg agent_id "$agent_id" '{agent_id: $agent_id, title: "Session event append concurrency verifier"}')" \
  "$BASE_URL/api/sessions" >"$session_file"
session_id="$(jq -r '.id // empty' "$session_file")"
if [[ -z "$session_id" ]]; then
  echo "session event concurrency verifier could not create a session" >&2
  exit 1
fi

for i in $(seq 1 "$CONCURRENCY"); do
  (
    payload="$(jq -nc --arg i "$i" '{
      events: [
        {
          type: "session.goal.updated",
          payload: {
            objective: ("append concurrency verifier event " + $i)
          }
        }
      ]
    }')"
    http_status="$(curl -sS -o "$tmpdir/event-$i.body" -w "%{http_code}" -X POST \
      "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d "$payload" \
      "$BASE_URL/api/sessions/$session_id/events" || true)"
    printf '%s\n' "$http_status" >"$tmpdir/event-$i.status"
  ) &
done
wait

failure_count=0
for i in $(seq 1 "$CONCURRENCY"); do
  status="$(cat "$tmpdir/event-$i.status")"
  if [[ "$status" != 2* ]]; then
    failure_count=$((failure_count + 1))
    echo "event append $i failed with HTTP $status" >&2
    sed -n '1,20p' "$tmpdir/event-$i.body" >&2 || true
  fi
done

events_file="$tmpdir/events.json"
curl -fsS "${auth_headers[@]}" "$BASE_URL/api/sessions/$session_id/events" >"$events_file"
event_count="$(jq 'length' "$events_file")"
unique_seq_count="$(jq '[.[].seq] | unique | length' "$events_file")"
max_seq="$(jq '[.[].seq] | max // 0' "$events_file")"
target_event_count="$(jq '[.[] | select(.event_type == "session.goal.updated" and (.payload.objective // "" | startswith("append concurrency verifier event ")))] | length' "$events_file")"
expected_count="$CONCURRENCY"

if [[ "$failure_count" != "0" ]]; then
  echo "session event append concurrency verifier failed: $failure_count request(s) failed" >&2
  exit 1
fi
if [[ "$target_event_count" != "$expected_count" || "$unique_seq_count" != "$event_count" ]]; then
  echo "session event append concurrency verifier failed: target_event_count=$target_event_count event_count=$event_count unique_seq_count=$unique_seq_count max_seq=$max_seq expected=$expected_count" >&2
  exit 1
fi

jq -n \
  --arg session_id "$session_id" \
  --argjson concurrency "$CONCURRENCY" \
  --argjson event_count "$event_count" \
  --argjson target_event_count "$target_event_count" \
  --argjson unique_seq_count "$unique_seq_count" \
  --argjson max_seq "$max_seq" \
  '{
    status: "passed",
    session_id: $session_id,
    concurrency: $concurrency,
    event_count: $event_count,
    target_event_count: $target_event_count,
    unique_seq_count: $unique_seq_count,
    max_seq: $max_seq
  }'
