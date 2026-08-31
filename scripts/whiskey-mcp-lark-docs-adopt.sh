#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
QUERY="${WHISKEY_MCP_LARK_DOCS_QUERY:-README}"
APPLY=0
REFRESH_PROMPT=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    --sync-dir)
      SYNC_DIR="${2:?--sync-dir requires a value}"
      shift 2
      ;;
    --remote-root)
      REMOTE_ROOT="${2:?--remote-root requires a value}"
      shift 2
      ;;
    --query)
      QUERY="${2:?--query requires a value}"
      shift 2
      ;;
    --apply)
      APPLY=1
      shift
      ;;
    --refresh-prompt)
      REFRESH_PROMPT=1
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-mcp-lark-docs-adopt.sh [--host <ssh-host>] [--sync-dir <dir>] [--remote-root <dir>] [--query <search-query>] [--apply] [--refresh-prompt]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey mcp lark docs adopt helper requires $1" >&2
    exit 1
  fi
}

require_cmd jq

resolve_running_image_identity() {
  local compose_file="$REMOTE_ROOT/docker-compose.yml"
  local inspect_json
  inspect_json="$(ssh "$REMOTE_HOST" "container_id=\$(docker compose -p mandoforge-adoption -f '$compose_file' ps -q api); if [[ -n \"\$container_id\" ]]; then image_id=\$(docker inspect --format '{{.Image}}' \"\$container_id\"); docker image inspect \"\$image_id\"; fi" 2>/dev/null || true)"
  [[ -n "$inspect_json" ]] || return 0
  printf '%s\n' "$inspect_json" | jq -r '.[0].Config.Labels // {} | [.["org.opencontainers.image.version"], .["org.opencontainers.image.revision"]] | map(. // "") | @tsv'
}

resolve_env_image_tag() {
  local remote_env="$REMOTE_ROOT/whiskey.env"
  ssh "$REMOTE_HOST" "if [[ -f '$remote_env' ]]; then sed -n 's/^MANDOFORGE_IMAGE_TAG=//p' '$remote_env' | tail -n 1; fi" 2>/dev/null | tr -d '\r'
}

resolve_status_doc_image_tag() {
  sed -n 's/^- Image: `ghcr\.io\/proerror77\/mandoforge\/mandoforge-api:\([^`]*\)`/\1/p' docs/whiskey-adoption-status.md | head -n 1
}

find_latest() {
  local pattern="$1"
  find "$SYNC_DIR" -maxdepth 1 -type f -name "$pattern" -print | sort | tail -n 1
}

scope_args=(--host "$REMOTE_HOST" --output-dir "$SYNC_DIR" --json)
scope_output="$(scripts/whiskey-mcp-lark-docs-scope.sh "${scope_args[@]}")"
scope_status="$(printf '%s\n' "$scope_output" | jq -r '.status')"
scope_json_path="$(find_latest 'whiskey-mcp-lark-docs-scope-*.json')"
prompt_json_path="$(find_latest 'whiskey-mcp-lark-docs-login-prompt-*.json')"
login_url="$(printf '%s\n' "$scope_output" | jq -r '.login_url // empty')"
user_code="$(printf '%s\n' "$scope_output" | jq -r '.user_code // empty')"

if [[ -z "$login_url" && -n "$prompt_json_path" && -f "$prompt_json_path" ]]; then
  login_url="$(jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$prompt_json_path" | grep -Eo 'https://[^[:space:]]+' | head -n 1 || true)"
fi
if [[ -z "$user_code" && -n "$prompt_json_path" && -f "$prompt_json_path" ]]; then
  user_code="$(jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$prompt_json_path" | grep -Eo '[A-Z0-9]{4}-[A-Z0-9]{4}' | head -n 1 || true)"
fi

if [[ "$scope_status" != "ready" && "$REFRESH_PROMPT" == "1" ]]; then
  prompt_output="$(scripts/whiskey-mcp-lark-docs-scope.sh --host "$REMOTE_HOST" --output-dir "$SYNC_DIR" --capture-login-prompt)"
  printf '%s\n' "$prompt_output"
  prompt_json_path="$(printf '%s\n' "$prompt_output" | sed -n 's/^json=//p' | tail -1)"
  login_url="$(jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$prompt_json_path" | grep -Eo 'https://[^[:space:]]+' | head -n 1 || true)"
  user_code="$(jq -r '(.stdout // "") + "\n" + (.stderr // "")' "$prompt_json_path" | grep -Eo '[A-Z0-9]{4}-[A-Z0-9]{4}' | head -n 1 || true)"
  scope_status="missing_scope"
fi

if [[ "$APPLY" == "1" ]]; then
  if [[ "$scope_status" != "ready" ]]; then
    echo "Whiskey Lark docs adoption is blocked: scope status is $scope_status" >&2
    if [[ -n "$login_url" ]]; then
      echo "Complete this device-flow login first:" >&2
      echo "$login_url" >&2
    fi
    exit 1
  fi

  require_cmd ssh

  image_tag="${MANDOFORGE_IMAGE_TAG:-}"
  git_sha="${MANDOFORGE_GIT_SHA:-}"
  if [[ -z "$image_tag" ]]; then
    running_identity="$(resolve_running_image_identity)"
    if [[ -n "$running_identity" ]]; then
      IFS=$'\t' read -r image_tag running_git_sha <<<"$running_identity"
      git_sha="${git_sha:-$running_git_sha}"
    fi
  fi
  if [[ -z "$image_tag" ]]; then
    image_tag="$(resolve_env_image_tag)"
  fi
  if [[ -z "$image_tag" ]]; then
    image_tag="$(resolve_status_doc_image_tag)"
  fi
  if [[ -z "$image_tag" ]]; then
    echo "Whiskey Lark docs adoption could not determine a deploy image tag; set MANDOFORGE_IMAGE_TAG explicitly" >&2
    exit 1
  fi
  if [[ ! "$git_sha" =~ ^([0-9a-fA-F]{40}|[0-9a-fA-F]{64})$ ]]; then
    echo "Whiskey Lark docs adoption requires the selected image's exact Git SHA; set MANDOFORGE_GIT_SHA explicitly" >&2
    exit 1
  fi

  MANDOFORGE_IMAGE_TAG="$image_tag" \
  MANDOFORGE_GIT_SHA="$git_sha" \
  WHISKEY_MCP_UPSTREAM_MODE=lark_docs_search \
  WHISKEY_WORKFLOW_PACK_MCP_QUERY="$QUERY" \
  scripts/whiskey-adoption-deploy.sh

  MANDOFORGE_IMAGE_TAG="$image_tag" \
  WHISKEY_MCP_UPSTREAM_MODE=lark_docs_search \
  WHISKEY_WORKFLOW_PACK_MCP_QUERY="$QUERY" \
  RUN_STAGE2_PRODUCTION_VALIDATIONS=1 \
  scripts/whiskey-adoption-evidence.sh

  exit 0
fi

printf 'Whiskey MCP Lark Docs Adopt\n'
printf 'remote_host=%s\n' "$REMOTE_HOST"
printf 'scope_status=%s\n' "$scope_status"
printf 'query=%s\n' "$QUERY"
printf 'scope_artifact=%s\n' "${scope_json_path:-none}"
printf 'login_prompt_artifact=%s\n' "${prompt_json_path:-none}"
if [[ -n "$login_url" ]]; then
  printf 'login_url=%s\n' "$login_url"
fi
if [[ -n "$user_code" ]]; then
  printf 'user_code=%s\n' "$user_code"
fi
printf '\n'
if [[ "$scope_status" == "ready" ]]; then
  printf 'next_apply_command:\n'
  printf 'scripts/whiskey-mcp-lark-docs-adopt.sh --apply --query %q\n' "$QUERY"
else
  printf 'scope is not ready; complete Feishu device-flow login before apply\n'
  printf 'post_auth_apply_command=scripts/whiskey-mcp-lark-docs-adopt.sh --apply --query %q\n' "$QUERY"
fi
