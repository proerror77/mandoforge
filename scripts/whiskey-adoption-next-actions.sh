#!/usr/bin/env bash
set -euo pipefail

SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
OUTPUT_MODE="text"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sync-dir)
      SYNC_DIR="${2:?--sync-dir requires a value}"
      shift 2
      ;;
    --json)
      OUTPUT_MODE="json"
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-adoption-next-actions.sh [--sync-dir <dir>] [--json]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey adoption next-actions requires $1" >&2
    exit 1
  fi
}

require_cmd jq

find_latest() {
  local pattern="$1"
  find "$SYNC_DIR" -maxdepth 1 -type f -name "$pattern" -print | sort | tail -n 1
}

latest_full_archive="$(find_latest 'mandoforge-whiskey-pilot-*.tar.gz')"
latest_strict_archive="$(find_latest 'stage2-production-whiskey-*.tar.gz')"
latest_k3s_plan="$(find_latest 'remote-computer-k3s-constrained-pilot-*.json')"
latest_lark_scope="$(find_latest 'whiskey-mcp-lark-docs-scope-*.json')"
latest_lark_login_prompt="$(find_latest 'whiskey-mcp-lark-docs-scope-*.json')"

# Prefer the explicit login-prompt artifact when present.
explicit_login_prompt="$(find "$SYNC_DIR" -maxdepth 1 -type f -name 'whiskey-mcp-lark-docs-scope-*.json' -o -name 'whiskey-mcp-lark-docs-login-prompt-*.json' 2>/dev/null | sort | tail -n 1)"
if [[ -n "$explicit_login_prompt" && "$explicit_login_prompt" == *"login-prompt"* ]]; then
  latest_lark_login_prompt="$explicit_login_prompt"
else
  latest_lark_login_prompt="$(find "$SYNC_DIR" -maxdepth 1 -type f -name 'whiskey-mcp-lark-docs-login-prompt-*.json' -print | sort | tail -n 1)"
fi

if [[ -z "$latest_full_archive" || -z "$latest_strict_archive" ]]; then
  echo "missing Whiskey adoption archives under $SYNC_DIR" >&2
  exit 1
fi

extract_login_url() {
  local json_file="$1"
  [[ -f "$json_file" ]] || return 0
  jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$json_file" | grep -Eo 'https://[^[:space:]]+' | head -n 1 || true
}

extract_user_code() {
  local json_file="$1"
  [[ -f "$json_file" ]] || return 0
  jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$json_file" | grep -Eo '[A-Z0-9]{4}-[A-Z0-9]{4}' | head -n 1 || true
}

login_url=""
user_code=""
if [[ -n "$latest_lark_login_prompt" && -f "$latest_lark_login_prompt" ]]; then
  login_url="$(extract_login_url "$latest_lark_login_prompt")"
  user_code="$(extract_user_code "$latest_lark_login_prompt")"
fi

if [[ "$OUTPUT_MODE" == "json" ]]; then
  jq -n \
    --arg sync_dir "$SYNC_DIR" \
    --arg latest_full_archive "$latest_full_archive" \
    --arg latest_strict_archive "$latest_strict_archive" \
    --arg latest_k3s_plan "${latest_k3s_plan:-}" \
    --arg latest_lark_scope "${latest_lark_scope:-}" \
    --arg latest_lark_login_prompt "${latest_lark_login_prompt:-}" \
    --arg login_url "$login_url" \
    --arg user_code "$user_code" \
    '{
      sync_dir: $sync_dir,
      latest_full_archive: $latest_full_archive,
      latest_strict_archive: $latest_strict_archive,
      latest_k3s_plan: (if $latest_k3s_plan == "" then null else $latest_k3s_plan end),
      latest_lark_scope: (if $latest_lark_scope == "" then null else $latest_lark_scope end),
      latest_lark_login_prompt: (if $latest_lark_login_prompt == "" then null else $latest_lark_login_prompt end),
      login_url: (if $login_url == "" then null else $login_url end),
      user_code: (if $user_code == "" then null else $user_code end),
      next_steps: [
        "Approve the constrained Whiskey k3s pilot and then run the repo-native pilot wrapper with apply flags.",
        "Complete the captured Feishu device-flow login so Whiskey gains search:docs:read."
      ]
    }'
  exit 0
fi

printf 'Whiskey Adoption Next Actions\n'
printf 'latest_full_archive=%s\n' "$latest_full_archive"
printf 'latest_strict_archive=%s\n' "$latest_strict_archive"
printf 'latest_k3s_plan=%s\n' "${latest_k3s_plan:-none}"
printf 'latest_lark_scope=%s\n' "${latest_lark_scope:-none}"
printf 'latest_lark_login_prompt=%s\n' "${latest_lark_login_prompt:-none}"
printf '\n'
printf 'Action 1: Remote Computer constrained pilot\n'
if [[ -n "${latest_k3s_plan:-}" ]]; then
  jq -r '
    "remote_host=" + (.remote_host // "unknown"),
    "inventory_status=" + (.inventory.status // "unknown"),
    "prepare_status=" + (.prepare.status // "unknown"),
    "install_status=" + (.install.status // "unknown"),
    "verify_status=" + (.verify.status // "unknown"),
    "next_apply_command=" + (.next_apply_command // "none"),
    "next_evidence_command=" + (.next_evidence_command // "none")
  ' "$latest_k3s_plan"
else
  printf 'no constrained k3s pilot plan artifact found\n'
fi
printf '\n'
printf 'Action 2: Lark docs scope\n'
if [[ -n "$login_url" ]]; then
  printf 'login_url=%s\n' "$login_url"
else
  printf 'login_url=none\n'
fi
if [[ -n "$user_code" ]]; then
  printf 'user_code=%s\n' "$user_code"
else
  printf 'user_code=none\n'
fi
printf 'scope_check_command=scripts/whiskey-mcp-lark-docs-scope.sh\n'
printf 'scope_login_command=scripts/whiskey-mcp-lark-docs-scope.sh --start-login\n'
