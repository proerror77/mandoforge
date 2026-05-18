#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"
MODE="check"
SCOPE="${WHISKEY_LARK_DOCS_SCOPE:-search:docs:read}"
LOGIN_PROMPT_TIMEOUT_SECONDS="${WHISKEY_LARK_DOCS_LOGIN_PROMPT_TIMEOUT_SECONDS:-10}"
OUTPUT_MODE="text"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:?--output-dir requires a value}"
      shift 2
      ;;
    --start-login)
      MODE="start_login"
      shift
      ;;
    --capture-login-prompt)
      MODE="capture_login_prompt"
      shift
      ;;
    --scope)
      SCOPE="${2:?--scope requires a value}"
      shift 2
      ;;
    --timeout-seconds)
      LOGIN_PROMPT_TIMEOUT_SECONDS="${2:?--timeout-seconds requires a value}"
      shift 2
      ;;
    --json)
      OUTPUT_MODE="json"
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-mcp-lark-docs-scope.sh [--host <ssh-host>] [--output-dir <dir>] [--scope <scope>] [--timeout-seconds <n>] [--json] [--start-login | --capture-login-prompt]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey mcp lark docs scope helper requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq

mkdir -p "$OUTPUT_DIR"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/whiskey-mcp-lark-docs-scope-$stamp.json"
text_file="$OUTPUT_DIR/whiskey-mcp-lark-docs-scope-$stamp.txt"

if [[ "$MODE" == "start_login" ]]; then
  echo "Starting remote Lark login flow on $REMOTE_HOST for scope $SCOPE"
  echo "The remote command will block until the device flow is completed or cancelled."
  exec ssh -t "$REMOTE_HOST" "lark-cli auth login --scope '$SCOPE'"
fi

if [[ "$MODE" == "capture_login_prompt" ]]; then
  remote_payload="$(ssh "$REMOTE_HOST" "LARK_SCOPE='$SCOPE' LOGIN_TIMEOUT='$LOGIN_PROMPT_TIMEOUT_SECONDS' bash -s" <<'REMOTE'
set -euo pipefail

scope="${LARK_SCOPE:-search:docs:read}"
login_timeout="${LOGIN_TIMEOUT:-10}"

if ! command -v lark-cli >/dev/null 2>&1; then
  jq -n \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg hostname "$(hostname)" \
    --arg scope "$scope" \
    '{
      generated_at: $generated_at,
      hostname: $hostname,
      scope: $scope,
      status: "missing_cli",
      message: "lark-cli is not installed on the remote host"
    }'
  exit 0
fi

python3 - <<'PY'
import json
import os
import subprocess
from datetime import datetime, timezone

scope = os.environ.get("LARK_SCOPE", "search:docs:read")
timeout_seconds = int(os.environ.get("LOGIN_TIMEOUT", "10"))

payload = {
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "hostname": subprocess.check_output(["hostname"], text=True).strip(),
    "scope": scope,
    "status": "unknown",
}

def normalize_stream(value):
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value

try:
    completed = subprocess.run(
        ["lark-cli", "auth", "login", "--scope", scope],
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
    payload["status"] = "completed"
    payload["returncode"] = completed.returncode
    payload["stdout"] = normalize_stream(completed.stdout)
    payload["stderr"] = normalize_stream(completed.stderr)
except subprocess.TimeoutExpired as exc:
    payload["status"] = "prompt_captured"
    payload["message"] = "captured initial device-flow prompt before timeout"
    payload["stdout"] = normalize_stream(exc.stdout)
    payload["stderr"] = normalize_stream(exc.stderr)

print(json.dumps(payload))
PY
REMOTE
)"

  printf '%s\n' "$remote_payload" >"$json_file"

  jq -r '
    [
      "Whiskey MCP Lark Docs Login Prompt",
      "generated_at=" + .generated_at,
      "hostname=" + .hostname,
      "scope=" + .scope,
      "status=" + .status,
      "message=" + (.message // "none"),
      "returncode=" + ((.returncode // "none") | tostring),
      "",
      "stdout:",
      (.stdout // ""),
      "",
      "stderr:",
      (.stderr // "")
    ] | .[]
  ' "$json_file" >"$text_file"

  cp "$json_file" "$OUTPUT_DIR/whiskey-mcp-lark-docs-login-prompt-latest.json"
  cp "$text_file" "$OUTPUT_DIR/whiskey-mcp-lark-docs-login-prompt-latest.txt"

  if [[ "$OUTPUT_MODE" == "json" ]]; then
    cat "$json_file"
    exit 0
  fi

  cat "$text_file"
  printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
  exit 0
fi

remote_payload="$(ssh "$REMOTE_HOST" "LARK_SCOPE='$SCOPE' bash -s" <<'REMOTE'
set -euo pipefail

scope="${LARK_SCOPE:-search:docs:read}"
if ! command -v lark-cli >/dev/null 2>&1; then
  jq -n \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg hostname "$(hostname)" \
    --arg scope "$scope" \
    '{
      generated_at: $generated_at,
      hostname: $hostname,
      scope: $scope,
      status: "missing_cli",
      message: "lark-cli is not installed on the remote host"
    }'
  exit 0
fi

response_file="$(mktemp)"
status="ready"
message="docs search scope is available"
hint=""

if ! lark-cli docs +search --as user --page-size 1 --format json >"$response_file" 2>&1; then
  status="missing_scope"
  message="docs search scope is not currently granted on the remote host"
  hint="run lark-cli auth login --scope \"$scope\" on the remote host and complete the device flow"
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg hostname "$(hostname)" \
  --arg scope "$scope" \
  --arg status "$status" \
  --arg message "$message" \
  --arg hint "$hint" \
  --rawfile raw "$response_file" \
  '{
    generated_at: $generated_at,
    hostname: $hostname,
    scope: $scope,
    status: $status,
    message: $message,
    hint: (if $hint == "" then null else $hint end),
    raw_response: $raw
  }'
rm -f "$response_file"
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq -r '
  [
    "Whiskey MCP Lark Docs Scope",
    "generated_at=" + .generated_at,
    "hostname=" + .hostname,
    "scope=" + .scope,
    "status=" + .status,
    "message=" + .message,
    "hint=" + (.hint // "none"),
    "",
    "raw_response:",
    .raw_response
  ] | .[]
' "$json_file" >"$text_file"

cp "$json_file" "$OUTPUT_DIR/whiskey-mcp-lark-docs-scope-latest.json"
cp "$text_file" "$OUTPUT_DIR/whiskey-mcp-lark-docs-scope-latest.txt"

if [[ "$OUTPUT_MODE" == "json" ]]; then
  cat "$json_file"
  exit 0
fi

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
