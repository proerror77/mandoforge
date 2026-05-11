#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
POLL_INTERVAL_SECONDS="${POLL_INTERVAL_SECONDS:-2}"
MAX_JOBS="${MAX_JOBS:-0}"
RUN_ONCE="${RUN_ONCE:-0}"
WORKER_ID="${WORKER_ID:-mandoforge-worker-$$}"

if ! command -v jq >/dev/null 2>&1; then
  echo "execution worker loop requires jq" >&2
  exit 1
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

processed=0

while true; do
  JOB_IDS="$(
    curl -fsS "$BASE_URL/api/execution-jobs" \
      | jq -r '.[] | select(.status == "queued" or .status == "running") | .id'
  )"

  while IFS= read -r job_id; do
    if [[ -z "$job_id" ]]; then
      continue
    fi
    if ! curl -fsS -X POST "$BASE_URL/api/execution-jobs/$job_id/run" \
      -H "x-mandoforge-worker-id: $WORKER_ID" \
      >/dev/null; then
      echo "execution job not claimable: $job_id" >&2
      continue
    fi
    processed=$((processed + 1))
    echo "execution job completed: $job_id"
    if [[ "$MAX_JOBS" != "0" && "$processed" -ge "$MAX_JOBS" ]]; then
      echo "execution worker processed $processed job(s)"
      exit 0
    fi
  done <<<"$JOB_IDS"

  if [[ "$RUN_ONCE" == "1" ]]; then
    echo "execution worker processed $processed job(s)"
    exit 0
  fi

  sleep "$POLL_INTERVAL_SECONDS"
done
