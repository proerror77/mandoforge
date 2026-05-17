#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"
MODE="check"
SCOPE="${WHISKEY_LARK_DOCS_SCOPE:-search:docs:read}"

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
    --scope)
      SCOPE="${2:?--scope requires a value}"
      shift 2
      ;;
    *)
      echo "usage: scripts/whiskey-mcp-lark-docs-scope.sh [--host <ssh-host>] [--output-dir <dir>] [--scope <scope>] [--start-login]" >&2
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

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
