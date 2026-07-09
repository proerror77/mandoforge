#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_RUNTIME_ADAPTER_VERIFY_SUBJECT:-admin-1}"
ROLES="${MANDOFORGE_RUNTIME_ADAPTER_VERIFY_ROLES:-admin}"

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "runtime adapter turn metadata verification requires $1" >&2
    exit 1
  fi
}

run_queued_job_if_present() {
  local approval_id="$1"
  local job_id

  job_id="$(
    curl -fsS "$BASE_URL/api/execution-jobs" \
      "${auth_headers[@]}" \
      | jq -r --arg approval_id "$approval_id" '
        map(select(.approval_id == $approval_id and .tool_name == "agent_cli.exec" and .status == "queued"))[0].id // empty
      '
  )"
  if [[ -n "$job_id" && "$job_id" != "null" ]]; then
    curl -fsS -X POST "$BASE_URL/api/execution-jobs/$job_id/run" \
      "${auth_headers[@]}" \
      -H 'x-mandoforge-worker-id: runtime-adapter-turn-metadata-gate' \
      >/dev/null
  fi
}

wait_for_completed_tool_call() {
  local session_id="$1"
  local approval_id="$2"
  local tool_call

  for _ in $(seq 1 20); do
    tool_call="$(
      curl -fsS "$BASE_URL/api/sessions/$session_id/tool-calls" \
        "${auth_headers[@]}" \
        | jq -c 'map(select(.tool_name == "agent_cli.exec"))[0] // empty'
    )"
    if [[ -n "$tool_call" && "$tool_call" != "null" ]]; then
      if jq -e '.status == "completed"' <<<"$tool_call" >/dev/null; then
        printf '%s\n' "$tool_call"
        return 0
      fi
      if jq -e '.status == "failed"' <<<"$tool_call" >/dev/null; then
        echo "agent_cli.exec failed during runtime adapter metadata verification" >&2
        jq . <<<"$tool_call" >&2
        exit 1
      fi
    fi
    run_queued_job_if_present "$approval_id"
    sleep 0.2
  done

  echo "agent_cli.exec did not complete for session $session_id" >&2
  exit 1
}

require_cmd jq
require_cmd curl

curl -fsS "$BASE_URL/healthz" >/dev/null

shim_dir="$(mktemp -d -t mandoforge-runtime-adapter-turn.XXXXXX)"
trap 'rm -rf "$shim_dir"' EXIT
schema_path="$shim_dir/decision.schema.json"
shim="$shim_dir/codex"
profile="codex-cli-evidence-$$"
agent_name="Codex CLI Evidence Agent $$"
environment_name="Codex CLI Evidence Environment $$"

printf '%s\n' '{"type":"object","properties":{"decision":{"type":"string"}},"required":["decision"]}' >"$schema_path"
cat >"$shim" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' '{"type":"turn.started","turn_id":"turn-gate-1","resume_handle":{"session_id":"codex-session-gate","command":"codex exec resume codex-session-gate"},"api_key":"secret"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item-gate-1","kind":"message","text":"Collected runtime evidence"}}'
printf '%s\n' '{"type":"tool_call.started","call_id":"call-gate-1","tool":"shell.exec","args":{"cmd":"pwd"}}'
printf '%s\n' '{"type":"usage","usage":{"input_tokens":13,"output_tokens":8,"total_tokens":21}}'
printf '%s\n' '{"type":"turn.completed","turn_id":"turn-gate-1","status":"completed","duration_ms":1500,"usage":{"input_tokens":13,"output_tokens":8,"total_tokens":21},"output_schema_validation":{"status":"passed"},"final_message":"Runtime adapter structured final"}'
printf 'profile=%s\n' "$MANDOFORGE_AGENT_CLI_PROFILE"
printf 'task=%s\n' "$MANDOFORGE_AGENT_TASK"
printf 'argv=%s\n' "$*"
SH
chmod +x "$shim"

PROFILE_ID="$(
  curl -fsS -X POST "$BASE_URL/api/agent-runtime-profiles" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg name "$profile" --arg command "$shim" '{
      name: $name,
      runtime_type: "codex_cli",
      command: $command,
      default_args: ["exec", "--json"],
      timeout_seconds: 30,
      remote_computer_required: false
    }')" \
    | jq -r '.id'
)"

ENVIRONMENT_ID="$(
  curl -fsS -X POST "$BASE_URL/api/environments" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg name "$environment_name" --arg profile_id "$PROFILE_ID" '{
      name: $name,
      environment_type: "self_hosted",
      runtime_profile_id: $profile_id,
      release_state: "active",
      status: "enabled"
    }')" \
    | jq -r '.id'
)"

AGENT_ID="$(
  curl -fsS -X POST "$BASE_URL/api/agents" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg name "$agent_name" --arg profile_id "$PROFILE_ID" '{
      name: $name,
      kind: "specialist",
      agent_role: "specialist",
      provider: "openai-compatible",
      model: "gpt-5.5-mini",
      runtime_profile_id: $profile_id,
      tools: ["agent_cli.exec"]
    }')" \
    | jq -r '.id'
)"

SESSION_ID="$(
  curl -fsS -X POST "$BASE_URL/api/sessions" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg agent_id "$AGENT_ID" --arg environment_id "$ENVIRONMENT_ID" '{
      agent_id: $agent_id,
      environment_id: $environment_id,
      title: "Runtime adapter turn metadata evidence"
    }')" \
    | jq -r '.id'
)"

APPROVAL_ID="$(
  curl -fsS -X POST "$BASE_URL/api/tools/agent_cli.exec/execute" \
    "${auth_headers[@]}" \
    -H 'content-type: application/json' \
    -d "$(jq -nc --arg session_id "$SESSION_ID" --arg profile "$profile" --arg schema_path "$schema_path" '{
      session_id: $session_id,
      args: {
        profile: $profile,
        task: "Inspect workspace with Codex CLI runtime metadata.",
        args: ["resume", "codex-session-gate", "--output-schema", $schema_path, "--sandbox", "workspace-write"]
      }
    }')" \
    | jq -r '.approval_id'
)"

curl -fsS -X POST "$BASE_URL/api/approvals/$APPROVAL_ID/approve" \
  "${auth_headers[@]}" \
  >/dev/null

TOOL_CALL="$(wait_for_completed_tool_call "$SESSION_ID" "$APPROVAL_ID")"
echo "$TOOL_CALL" | jq -e --arg profile "$profile" '
  (.status == "completed")
  and (.result.runner == "agent-cli")
  and (.result.profile == $profile)
  and (.result.profile_source == "managed")
  and (.result.runtime_type == "codex_cli")
  and (.result.runtime_adapter_event_count == 5)
  and (.result.runtime_turn_event_count == 6)
  and (.result.runtime_final_artifact_count == 1)
' >/dev/null

EVENTS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/events" \
    "${auth_headers[@]}"
)"
echo "$EVENTS" | jq -e --arg schema_path "$schema_path" '
  any(.[]; .event_type == "runtime_adapter.event"
    and .payload.runtime_type == "codex_cli"
    and .payload.log_mode == "codex_jsonl"
    and .payload.adapter_event_type == "turn.started"
    and .payload.event.api_key == "[REDACTED]")
  and any(.[]; .event_type == "runtime.turn.started"
    and .payload.runtime_type == "codex_cli"
    and .payload.turn_id == "turn-gate-1"
    and .payload.resume_handle.session_id == "codex-session-gate"
    and .payload.output_schema.path == $schema_path
    and .payload.output_schema.source == "cli_args")
  and any(.[]; .event_type == "runtime.item"
    and .payload.turn_id == "turn-gate-1"
    and .payload.item.id == "item-gate-1")
  and any(.[]; .event_type == "runtime.tool_call"
    and .payload.turn_id == "turn-gate-1"
    and .payload.tool_call.call_id == "call-gate-1"
    and .payload.tool_call.tool == "shell.exec")
  and any(.[]; .event_type == "runtime.usage"
    and .payload.turn_id == "turn-gate-1"
    and .payload.usage.total_tokens == 21)
  and any(.[]; .event_type == "runtime.final"
    and .payload.turn_id == "turn-gate-1"
    and .payload.final_message == "Runtime adapter structured final"
    and (.payload.artifact_id | type == "string"))
  and any(.[]; .event_type == "runtime.turn.completed"
    and .payload.turn_id == "turn-gate-1"
    and .payload.status == "completed"
    and .payload.duration_ms == 1500
    and .payload.usage.total_tokens == 21
    and .payload.output_schema_validation.status == "passed"
    and (.payload.final_artifact_id | type == "string"))
' >/dev/null

ARTIFACTS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/artifacts" \
    "${auth_headers[@]}"
)"
echo "$ARTIFACTS" | jq -e '
  any(.[]; .name == "runtime-final-message.md"
    and .artifact_type == "markdown"
    and .content.markdown == "Runtime adapter structured final")
' >/dev/null

AUDIT_LOGS="$(
  curl -fsS "$BASE_URL/api/sessions/$SESSION_ID/audit-logs" \
    "${auth_headers[@]}"
)"
echo "$AUDIT_LOGS" | jq -e --arg profile "$profile" '
  any(.[]; .action == "tool.completed"
    and .resource_type == "tool_call"
    and .details.tool == "agent_cli.exec"
    and .details.profile == $profile
    and .details.profile_source == "managed"
    and .details.runtime_type == "codex_cli"
    and .details.runner == "agent-cli"
    and .details.runtime_adapter_event_count == 5
    and .details.runtime_turn_event_count == 6
    and .details.runtime_final_artifact_count == 1)
' >/dev/null

echo "runtime adapter turn metadata verification ok"
echo "session_id=$SESSION_ID"
echo "profile_id=$PROFILE_ID"
echo "environment_id=$ENVIRONMENT_ID"
echo "agent_id=$AGENT_ID"
echo "approval_id=$APPROVAL_ID"
