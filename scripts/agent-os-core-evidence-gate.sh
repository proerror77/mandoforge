#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
WORKSPACE_ROOT="${MANDOFORGE_WORKSPACE_ROOT:-.mandoforge/agent-os-core-evidence-workspaces}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "agent os core evidence gate requires $1" >&2
    exit 1
  fi
}

field_value() {
  local key="$1"
  local file="$2"
  awk -F= -v key="$key" '$1 == key { sub(/^[^=]*=/, ""); print }' "$file" | tail -n 1
}

csv_contains() {
  local csv="$1"
  local expected="$2"
  IFS=',' read -r -a items <<<"$csv"
  local item
  for item in "${items[@]}"; do
    if [[ "$item" == "$expected" ]]; then
      return 0
    fi
  done
  return 1
}

require_csv_item() {
  local label="$1"
  local csv="$2"
  local expected="$3"
  if ! csv_contains "$csv" "$expected"; then
    echo "missing $label evidence: $expected" >&2
    echo "$label=$csv" >&2
    exit 1
  fi
}

require_cmd jq
require_cmd curl

output_file="$(mktemp -t mandoforge-agent-os-core-evidence.XXXXXX)"
trap 'rm -f "$output_file"' EXIT

BASE_URL="$BASE_URL" MANDOFORGE_WORKSPACE_ROOT="$WORKSPACE_ROOT" \
  ./scripts/stage1-demo.sh | tee "$output_file"
BASE_URL="$BASE_URL" ./scripts/verify-runtime-adapter-turn-metadata.sh

event_types="$(field_value event_types "$output_file")"
tool_calls="$(field_value tool_calls "$output_file")"
audit_actions="$(field_value audit_actions "$output_file")"
workspace_file="$(field_value workspace_file "$output_file")"

for event_type in \
  user.message \
  agent.plan \
  llm.request \
  llm.response \
  tool.call \
  tool.result \
  policy.allowed \
  policy.requires_approval \
  approval.requested \
  approval.approved \
  artifact.created \
  agent.final \
  session.status_idle; do
  require_csv_item event_types "$event_types" "$event_type"
done

for tool_call in \
  file.read:completed \
  sql.get_schema:completed \
  sql.query:completed \
  shell.exec:completed \
  file.write:completed; do
  require_csv_item tool_calls "$tool_calls" "$tool_call"
done

for audit_action in \
  session.started \
  policy.requires_approval \
  approval.requested \
  approval.approved \
  tool.completed \
  artifact.created; do
  require_csv_item audit_actions "$audit_actions" "$audit_action"
done

if [[ -z "$workspace_file" || ! -f "$workspace_file" ]]; then
  echo "missing workspace artifact evidence: ${workspace_file:-<empty>}" >&2
  exit 1
fi

echo "agent os core evidence gate ok"
echo "event_log_evidence=session_events"
echo "tool_action_evidence=tool_calls"
echo "audit_evidence=audit_logs"
echo "runtime_adapter_evidence=runtime_adapter.event,runtime.turn.*"
