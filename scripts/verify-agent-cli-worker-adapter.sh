#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
PROFILE="${MANDOFORGE_AGENT_CLI_VERIFY_PROFILE:-fake-coder}"

if ! command -v jq >/dev/null 2>&1; then
  echo "agent CLI worker adapter verification requires jq" >&2
  exit 1
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

shim_dir="$(mktemp -d)"
trap 'rm -rf "$shim_dir"' EXIT
shim="$shim_dir/fake-agent-cli"
cat >"$shim" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
echo "profile=$MANDOFORGE_AGENT_CLI_PROFILE"
echo "task=$MANDOFORGE_AGENT_TASK"
echo "argv=$*"
SH
chmod +x "$shim"

export MANDOFORGE_AGENT_CLI_ALLOWED_PROFILES="$PROFILE"
export "MANDOFORGE_AGENT_CLI_$(printf '%s' "$PROFILE" | tr '[:lower:]-' '[:upper:]_')_COMMAND=$shim"
export "MANDOFORGE_AGENT_CLI_$(printf '%s' "$PROFILE" | tr '[:lower:]-' '[:upper:]_')_ARGS=--mode worker"

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
      title: "Agent CLI worker adapter verification",
      message: "Verify approved agent_cli.exec can be run by the execution worker."
    }')" \
    | jq -r '.id'
)"

APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/agent_cli.exec/execute" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" --arg profile "$PROFILE" '{
      session_id: $session_id,
      args: {
        profile: $profile,
        task: "Inspect workspace as coding worker",
        args: ["--json"]
      }
    }')" \
    | jq -r '.approval_id'
)"

curl -fsS -X POST "$BASE_URL/api/approvals/$APPROVAL_ID/approve" \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' >/dev/null

JOB_ID="$(
  curl -fsS "$BASE_URL/api/execution-jobs" \
    -H 'x-mandoforge-subject: admin-1' \
    -H 'x-mandoforge-roles: admin' \
    | jq -r --arg approval_id "$APPROVAL_ID" '
      map(select(.approval_id == $approval_id and .tool_name == "agent_cli.exec"))[0].id // empty
    '
)"
if [[ -z "$JOB_ID" || "$JOB_ID" == "null" ]]; then
  echo "no queued agent_cli.exec execution job found for approval $APPROVAL_ID" >&2
  exit 1
fi

curl -fsS -X POST "$BASE_URL/api/execution-jobs/$JOB_ID/run" \
  -H 'x-mandoforge-worker-id: agent-cli-worker-verify' \
  -H 'x-mandoforge-subject: admin-1' \
  -H 'x-mandoforge-roles: admin' >/dev/null

TOOL_CALL="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/tool-calls" \
    -H 'x-mandoforge-subject: admin-1' \
    -H 'x-mandoforge-roles: admin' \
    | jq -c 'map(select(.tool_name == "agent_cli.exec"))[0]'
)"
echo "$TOOL_CALL" | jq -e --arg profile "$PROFILE" '
  (.status == "completed")
  and (.result.runner == "agent-cli")
  and (.result.profile == $profile)
  and (.result.stdout | contains("profile=" + $profile))
  and (.result.stdout | contains("argv=--mode worker --json Inspect workspace as coding worker"))
' >/dev/null

EVENT_TYPES="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    -H 'x-mandoforge-subject: admin-1' \
    -H 'x-mandoforge-roles: admin' \
    | jq -r '[.[].event_type] | unique | join(",")'
)"
if [[ "$EVENT_TYPES" != *"execution.queued"* || "$EVENT_TYPES" != *"agent_cli.task.completed"* ]]; then
  echo "expected execution.queued and agent_cli.task.completed, got $EVENT_TYPES" >&2
  exit 1
fi

echo "agent CLI worker adapter verification ok"
echo "session_id=$SESSION_ID"
echo "approval_id=$APPROVAL_ID"
echo "execution_job_id=$JOB_ID"
echo "event_types=$EVENT_TYPES"
