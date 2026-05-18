#!/usr/bin/env bash
set -euo pipefail

SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
OUTPUT_MODE="text"
LARK_DOCS_QUERY="${WHISKEY_MCP_LARK_DOCS_QUERY:-README}"

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
latest_lark_login_prompt=""

# Prefer the explicit login-prompt artifact when present.
explicit_login_prompt="$(find "$SYNC_DIR" -maxdepth 1 \( -type f -name 'whiskey-mcp-lark-docs-scope-*.json' -o -type f -name 'whiskey-mcp-lark-docs-login-prompt-*.json' \) -print 2>/dev/null | sort | tail -n 1)"
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

remote_computer_remote_host="unknown"
remote_computer_inventory_status="unknown"
remote_computer_prepare_status="unknown"
remote_computer_install_status="unknown"
remote_computer_verify_status="unknown"
remote_computer_next_command="scripts/whiskey-remote-computer-k3s-constrained-pilot.sh"
remote_computer_stage_command="scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence"
remote_computer_note="generate or refresh the constrained pilot plan before approval"

if [[ -n "${latest_k3s_plan:-}" && -f "$latest_k3s_plan" ]]; then
  remote_computer_remote_host="$(jq -r '.remote_host // "unknown"' "$latest_k3s_plan")"
  remote_computer_inventory_status="$(jq -r '.inventory.status // "unknown"' "$latest_k3s_plan")"
  remote_computer_prepare_status="$(jq -r '.prepare.status // "unknown"' "$latest_k3s_plan")"
  remote_computer_install_status="$(jq -r '.install.status // "unknown"' "$latest_k3s_plan")"
  remote_computer_verify_status="$(jq -r '.verify.status // "unknown"' "$latest_k3s_plan")"

  if [[ "$remote_computer_verify_status" == "ready" ]]; then
    remote_computer_next_command="$remote_computer_stage_command"
    remote_computer_note="k3s is verified; apply manifests and rerun strict evidence"
  elif [[ "$remote_computer_install_status" == "applied" ]]; then
    remote_computer_next_command="scripts/whiskey-remote-computer-k3s-verify.sh"
    remote_computer_note="k3s install has run; verify cluster readiness before staging manifests"
  elif [[ "$remote_computer_prepare_status" == "applied" ]]; then
    remote_computer_next_command="scripts/whiskey-remote-computer-k3s-constrained-pilot.sh --install-k3s"
    remote_computer_note="host prerequisites are applied; installation approval is the next mutating step"
  elif [[ "$remote_computer_verify_status" == "not_installed" ]]; then
    remote_computer_next_command="scripts/whiskey-remote-computer-k3s-constrained-pilot.sh --apply-host-prereqs --install-k3s"
    remote_computer_note="approval-required constrained single-node k3s pilot"
  fi
fi

lark_scope_status="missing_scope_artifact"
lark_scope_message="no local Lark docs scope artifact found"
lark_docs_next_command="scripts/whiskey-mcp-lark-docs-scope.sh"
lark_docs_post_auth_command="scripts/whiskey-mcp-lark-docs-adopt.sh --apply --query $LARK_DOCS_QUERY"

if [[ -n "${latest_lark_scope:-}" && -f "$latest_lark_scope" ]]; then
  lark_scope_status="$(jq -r '.status // "unknown"' "$latest_lark_scope")"
  lark_scope_message="$(jq -r '.message // "unknown"' "$latest_lark_scope")"

  if [[ "$lark_scope_status" == "ready" ]]; then
    lark_docs_next_command="$lark_docs_post_auth_command"
  elif [[ -n "$login_url" ]]; then
    lark_docs_next_command="scripts/whiskey-mcp-lark-docs-scope.sh --start-login"
  else
    lark_docs_next_command="scripts/whiskey-mcp-lark-docs-adopt.sh --refresh-prompt --query $LARK_DOCS_QUERY"
  fi
fi

if [[ "$OUTPUT_MODE" == "json" ]]; then
  jq -n \
    --arg sync_dir "$SYNC_DIR" \
    --arg lark_docs_query "$LARK_DOCS_QUERY" \
    --arg latest_full_archive "$latest_full_archive" \
    --arg latest_strict_archive "$latest_strict_archive" \
    --arg latest_k3s_plan "${latest_k3s_plan:-}" \
    --arg latest_lark_scope "${latest_lark_scope:-}" \
    --arg latest_lark_login_prompt "${latest_lark_login_prompt:-}" \
    --arg login_url "$login_url" \
    --arg user_code "$user_code" \
    --arg remote_computer_remote_host "$remote_computer_remote_host" \
    --arg remote_computer_inventory_status "$remote_computer_inventory_status" \
    --arg remote_computer_prepare_status "$remote_computer_prepare_status" \
    --arg remote_computer_install_status "$remote_computer_install_status" \
    --arg remote_computer_verify_status "$remote_computer_verify_status" \
    --arg remote_computer_next_command "$remote_computer_next_command" \
    --arg remote_computer_stage_command "$remote_computer_stage_command" \
    --arg remote_computer_note "$remote_computer_note" \
    --arg lark_scope_status "$lark_scope_status" \
    --arg lark_scope_message "$lark_scope_message" \
    --arg lark_docs_next_command "$lark_docs_next_command" \
    --arg lark_docs_post_auth_command "$lark_docs_post_auth_command" \
    '{
      sync_dir: $sync_dir,
      lark_docs_query: $lark_docs_query,
      latest_full_archive: $latest_full_archive,
      latest_strict_archive: $latest_strict_archive,
      latest_k3s_plan: (if $latest_k3s_plan == "" then null else $latest_k3s_plan end),
      latest_lark_scope: (if $latest_lark_scope == "" then null else $latest_lark_scope end),
      latest_lark_login_prompt: (if $latest_lark_login_prompt == "" then null else $latest_lark_login_prompt end),
      login_url: (if $login_url == "" then null else $login_url end),
      user_code: (if $user_code == "" then null else $user_code end),
      remote_computer: {
        remote_host: $remote_computer_remote_host,
        inventory_status: $remote_computer_inventory_status,
        prepare_status: $remote_computer_prepare_status,
        install_status: $remote_computer_install_status,
        verify_status: $remote_computer_verify_status,
        next_command: $remote_computer_next_command,
        post_install_stage_command: $remote_computer_stage_command,
        note: $remote_computer_note
      },
      lark_docs: {
        scope_status: $lark_scope_status,
        scope_message: $lark_scope_message,
        next_command: $lark_docs_next_command,
        post_auth_command: $lark_docs_post_auth_command
      }
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
printf 'remote_host=%s\n' "$remote_computer_remote_host"
printf 'inventory_status=%s\n' "$remote_computer_inventory_status"
printf 'prepare_status=%s\n' "$remote_computer_prepare_status"
printf 'install_status=%s\n' "$remote_computer_install_status"
printf 'verify_status=%s\n' "$remote_computer_verify_status"
printf 'next_command=%s\n' "$remote_computer_next_command"
printf 'post_install_stage_command=%s\n' "$remote_computer_stage_command"
printf 'note=%s\n' "$remote_computer_note"
printf '\n'
printf 'Action 2: Lark docs scope\n'
printf 'scope_status=%s\n' "$lark_scope_status"
printf 'scope_message=%s\n' "$lark_scope_message"
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
printf 'next_command=%s\n' "$lark_docs_next_command"
printf 'post_auth_command=%s\n' "$lark_docs_post_auth_command"
