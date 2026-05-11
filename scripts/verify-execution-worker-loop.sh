#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
WORKSPACE_ROOT="${MANDOFORGE_WORKSPACE_ROOT:-.mandoforge/workspaces}"

if ! command -v jq >/dev/null 2>&1; then
  echo "execution worker loop verification requires jq" >&2
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

RELATIVE_PATH="worker-loop-$RANDOM.md"
CONTENT="execution worker loop verification ok"

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg agent_id "$AGENT_ID" '{
      agent_id: $agent_id,
      title: "Execution worker loop verification",
      message: "Verify approved file.write waits for the external worker loop."
    }')" \
    | jq -r '.id'
)"

APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/file.write/execute" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" --arg path "$RELATIVE_PATH" --arg content "$CONTENT" '{
      session_id: $session_id,
      args: {path: $path, content: $content}
    }')" \
    | jq -r '.approval_id'
)"

curl -fsS -X POST "$BASE_URL/api/approvals/$APPROVAL_ID/approve" >/dev/null

JOB_ID="$(
  curl -fsS "$BASE_URL/api/execution-jobs" \
    | jq -r --arg approval_id "$APPROVAL_ID" 'map(select(.approval_id == $approval_id and .status == "queued"))[0].id // empty'
)"
if [[ -z "$JOB_ID" || "$JOB_ID" == "null" ]]; then
  echo "expected queued execution job for approval $APPROVAL_ID" >&2
  exit 1
fi

WORKSPACE_FILE="$WORKSPACE_ROOT/$SESSION_ID/$RELATIVE_PATH"
if [[ -e "$WORKSPACE_FILE" ]]; then
  echo "queued worker mode wrote $WORKSPACE_FILE before external worker loop ran" >&2
  exit 1
fi

BASE_URL="$BASE_URL" MAX_JOBS=1 POLL_INTERVAL_SECONDS=0 ./scripts/execution-worker-loop.sh >/tmp/mandoforge-execution-worker-loop.log

if [[ ! -f "$WORKSPACE_FILE" ]]; then
  echo "expected external worker loop to write $WORKSPACE_FILE" >&2
  cat /tmp/mandoforge-execution-worker-loop.log >&2
  exit 1
fi
if [[ "$(cat "$WORKSPACE_FILE")" != "$CONTENT" ]]; then
  echo "unexpected worker output in $WORKSPACE_FILE" >&2
  exit 1
fi

COMPLETED_JOB_STATUS="$(
  curl -fsS "$BASE_URL/api/execution-jobs" \
    | jq -r --arg job_id "$JOB_ID" 'map(select(.id == $job_id))[0].status // empty'
)"
if [[ "$COMPLETED_JOB_STATUS" != "completed" ]]; then
  echo "expected execution job $JOB_ID completed, got $COMPLETED_JOB_STATUS" >&2
  exit 1
fi

echo "execution worker loop verification ok"
echo "session_id=$SESSION_ID"
echo "approval_id=$APPROVAL_ID"
echo "execution_job_id=$JOB_ID"
echo "workspace_file=$WORKSPACE_FILE"
