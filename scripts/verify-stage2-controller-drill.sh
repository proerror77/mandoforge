#!/usr/bin/env bash
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  echo "node is required to verify the Stage 2 mock controller" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required to verify the Stage 2 mock controller" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to verify the Stage 2 mock controller" >&2
  exit 1
fi

STAGE2_MOCK_CONTROLLER_PORT="${STAGE2_MOCK_CONTROLLER_PORT:-18081}"
STAGE2_MOCK_CONTROLLER_HOST="${STAGE2_MOCK_CONTROLLER_HOST:-127.0.0.1}"
export STAGE2_MOCK_CONTROLLER_PORT
export STAGE2_MOCK_CONTROLLER_HOST
base_url="http://$STAGE2_MOCK_CONTROLLER_HOST:$STAGE2_MOCK_CONTROLLER_PORT"

node scripts/stage2-mock-controller.js &
mock_pid="$!"
cleanup() {
  kill "$mock_pid" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for _ in $(seq 1 50); do
  if curl -fsS "$base_url/healthz" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

curl -fsS "$base_url/healthz" >/dev/null

post_status() {
  local path="$1"
  curl -fsS -X POST "$base_url$path" \
    -H "content-type: application/json" \
    -d '{"type":"stage2.controller.drill"}' \
    | jq -r '.status'
}

[[ "$(post_status /tenant/routing/validate)" == "validated" ]]
[[ "$(post_status /provider/rollout/apply)" == "applied" ]]
[[ "$(post_status /provider/rollout/rollback)" == "rolled_back" ]]
[[ "$(post_status /mcp/rollout/apply)" == "approved" ]]
[[ "$(post_status /agents/releases/rollout/apply)" == "promoted" ]]
[[ "$(post_status /finance/close)" == "closed" ]]
[[ "$(post_status /finance/reconcile)" == "reconciled" ]]

echo "stage2 controller drill verifier ok"
