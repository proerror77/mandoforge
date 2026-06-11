#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SMOKE_EXIT_MS="${MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS:-1800}"
START_API="${START_API:-0}"
EMBEDDED_API="${EMBEDDED_API:-0}"
LOG_DIR="${LOG_DIR:-.mandoforge/desktop-smoke}"
API_PID=""

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "desktop runtime smoke requires $1" >&2
    exit 1
  fi
}

cleanup() {
  if [[ -n "$API_PID" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_cmd cargo
require_cmd curl

mkdir -p "$LOG_DIR"
base_url="${BASE_URL%/}"
health_url="$base_url/healthz"
api_log="$LOG_DIR/api.log"
desktop_log="$LOG_DIR/desktop.log"

if [[ "$START_API" == "1" ]]; then
  if [[ "$EMBEDDED_API" == "1" ]]; then
    echo "desktop runtime smoke cannot use START_API=1 with EMBEDDED_API=1" >&2
    exit 1
  fi
  MANDOFORGE_INSECURE_DEV_AUTH=1 \
    MANDOFORGE_STORE_BACKEND=memory \
    MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
    cargo run -p mandoforge-api >"$api_log" 2>&1 &
  API_PID="$!"
fi

if [[ "$EMBEDDED_API" == "1" ]]; then
  cargo build -p mandoforge-api >/dev/null
  if ! MANDOFORGE_DESKTOP_EMBEDDED_API=1 \
    MANDOFORGE_DESKTOP_API_COMMAND="target/debug/mandoforge-api" \
    MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS="$SMOKE_EXIT_MS" \
    MANDOFORGE_INSECURE_DEV_AUTH=1 \
    MANDOFORGE_STORE_BACKEND=memory \
    MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
    cargo run -p mandoforge-desktop >"$desktop_log" 2>&1; then
    echo "desktop runtime smoke failed to launch embedded API desktop shell" >&2
    tail -120 "$desktop_log" >&2 || true
    exit 1
  fi
  echo "desktop runtime smoke ok: embedded API"
else
  ready=0
  for _ in {1..120}; do
    if curl -fsS "$health_url" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.5
  done

  if [[ "$ready" != "1" ]]; then
    echo "desktop runtime smoke could not reach $health_url" >&2
    if [[ -s "$api_log" ]]; then
      tail -80 "$api_log" >&2 || true
    fi
    exit 1
  fi

  if ! MANDOFORGE_API_BASE_URL="$base_url" \
    MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS="$SMOKE_EXIT_MS" \
    cargo run -p mandoforge-desktop >"$desktop_log" 2>&1; then
    echo "desktop runtime smoke failed to launch desktop shell" >&2
    tail -120 "$desktop_log" >&2 || true
    exit 1
  fi

  echo "desktop runtime smoke ok: $base_url"
fi
