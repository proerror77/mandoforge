#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
COMPOSE_PROJECT="${WHISKEY_COMPOSE_PROJECT:-mandoforge-adoption}"
LOCAL_COMPOSE="${WHISKEY_COMPOSE_FILE:-deploy/whiskey/docker-compose.adoption.yml}"
REMOTE_COMPOSE="$REMOTE_ROOT/docker-compose.yml"
REMOTE_ENV="$REMOTE_ROOT/whiskey.env"
IMAGE_TAG="${MANDOFORGE_IMAGE_TAG:-latest}"
PULL_IMAGE="${WHISKEY_PULL_IMAGE:-1}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey adoption deploy requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd rsync

if [[ ! -f "$LOCAL_COMPOSE" ]]; then
  echo "missing Whiskey compose file: $LOCAL_COMPOSE" >&2
  exit 1
fi

ssh "$REMOTE_HOST" "mkdir -p '$REMOTE_ROOT/evidence' '$REMOTE_ROOT/archives'"
rsync -az "$LOCAL_COMPOSE" "$REMOTE_HOST:$REMOTE_COMPOSE"

ssh "$REMOTE_HOST" "if [[ ! -f '$REMOTE_ENV' ]]; then cat > '$REMOTE_ENV' <<'ENV'
MANDOFORGE_IMAGE_TAG=$IMAGE_TAG
MANDOFORGE_API_HOST_PORT=18787
MANDOFORGE_POSTGRES_HOST_PORT=15432
MANDOFORGE_SCHEDULER_TOKEN=whiskey-stage2-scheduler-token
ENV
fi"

remote_cmd="cd '$REMOTE_ROOT' && set -a && source '$REMOTE_ENV' && set +a"
if [[ "$PULL_IMAGE" == "1" ]]; then
  remote_cmd="$remote_cmd && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' pull"
fi
remote_cmd="$remote_cmd && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' up -d && docker compose -p '$COMPOSE_PROJECT' -f '$REMOTE_COMPOSE' ps"

ssh "$REMOTE_HOST" "bash -lc $(printf '%q' "$remote_cmd")"

echo "Whiskey MandoForge pilot is deployed on $REMOTE_HOST at http://127.0.0.1:18787"
