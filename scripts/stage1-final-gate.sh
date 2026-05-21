#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
GATE_ADDR="${GATE_ADDR:-127.0.0.1:8789}"
GATE_BASE_URL="http://$GATE_ADDR"
GATE_WORKSPACE_ROOT="${GATE_WORKSPACE_ROOT:-.mandoforge/final-gate-workspaces}"
RUN_DEMO="${RUN_DEMO:-1}"
RUN_LIVE="${RUN_LIVE:-0}"
START_LIVE_STACK="${START_LIVE_STACK:-0}"
DATABASE_URL="${DATABASE_URL:-postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge}"
API_PID=""
FAKE_CODEX_DIR=""
STARTED_COMPOSE_POSTGRES=0

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
  fi
  if [[ -n "${FAKE_CODEX_DIR:-}" ]]; then
    rm -rf "$FAKE_CODEX_DIR"
  fi
  if [[ "$STARTED_COMPOSE_POSTGRES" == "1" ]]; then
    docker compose stop postgres >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

cargo fmt --all -- --check
cargo check -p mandoforge-api --bins
cargo test -p mandoforge-api -- --test-threads=1

bash -n scripts/smoke.sh
bash -n scripts/stage1-demo.sh
bash -n scripts/agent-os-core-evidence-gate.sh
bash -n scripts/managed-session-restart-resume-core-gate.sh
bash -n scripts/work-item-collaboration-evidence-gate.sh
bash -n scripts/verify-runtime-adapter-turn-metadata.sh
bash -n scripts/seed-platform-events.sh
bash -n scripts/verify-postgres-sql-query.sh
bash -n scripts/verify-docker-shell-runner.sh
bash -n scripts/verify-codex-exec-adapter.sh
bash -n scripts/execution-worker-loop.sh
bash -n scripts/verify-execution-worker-loop.sh
bash -n scripts/verify-external-provider.sh

prepare_fake_codex() {
  FAKE_CODEX_DIR="$(mktemp -d -t mandoforge-fake-codex.XXXXXX)"
  cat >"$FAKE_CODEX_DIR/codex" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

LAST_MESSAGE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-last-message)
      shift
      LAST_MESSAGE="${1:-}"
      ;;
  esac
  shift || true
done

printf '%s\n' '{"type":"session.started","id":"fake-codex"}'
printf '%s\n' '{"msg":"agent.message","text":"fake adapter ok"}'
if [[ -n "$LAST_MESSAGE" ]]; then
  mkdir -p "$(dirname "$LAST_MESSAGE")"
  printf '%s\n' 'Fake Codex adapter final message.' >"$LAST_MESSAGE"
fi
SH
  chmod +x "$FAKE_CODEX_DIR/codex"
}

start_gate_api() {
  local log_file="$1"
  shift
  env "$@" cargo run -p mandoforge-api >"$log_file" 2>&1 &
  API_PID="$!"

  for _ in $(seq 1 60); do
    if curl -fsS "$GATE_BASE_URL/healthz" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$API_PID" >/dev/null 2>&1; then
      echo "stage1 gate API exited early; log follows:" >&2
      cat "$log_file" >&2
      exit 1
    fi
    sleep 0.5
  done
  curl -fsS "$GATE_BASE_URL/healthz" >/dev/null
}

if [[ "$RUN_LIVE" != "1" ]]; then
  if [[ "$RUN_DEMO" == "1" ]]; then
    prepare_fake_codex
    LOG_FILE="$(mktemp -t mandoforge-stage1-gate.XXXXXX.log)"
    start_gate_api \
      "$LOG_FILE" \
      "MANDOFORGE_ADDR=$GATE_ADDR" \
      "MANDOFORGE_WORKSPACE_ROOT=$GATE_WORKSPACE_ROOT" \
      "MANDOFORGE_INSECURE_DEV_AUTH=1" \
      "MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1" \
      "PATH=$FAKE_CODEX_DIR:$PATH"

    BASE_URL="$GATE_BASE_URL" MANDOFORGE_WORKSPACE_ROOT="$GATE_WORKSPACE_ROOT" ./scripts/agent-os-core-evidence-gate.sh
    BASE_URL="$GATE_BASE_URL" EVIDENCE_DIR="$GATE_WORKSPACE_ROOT/work-item-collaboration-evidence" ./scripts/work-item-collaboration-evidence-gate.sh
    BASE_URL="$GATE_BASE_URL" ./scripts/verify-codex-exec-adapter.sh
  fi

  echo "stage1 static+demo gate ok"
  echo "set RUN_LIVE=1 with a running API, Postgres, and Docker to execute live gates"
  exit 0
fi

if [[ "$START_LIVE_STACK" == "1" ]]; then
  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not available; start Docker before running START_LIVE_STACK=1" >&2
    exit 1
  fi
  docker compose up -d postgres
  STARTED_COMPOSE_POSTGRES=1
  for _ in $(seq 1 60); do
    if docker compose exec -T postgres pg_isready -U mandoforge -d mandoforge >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  docker compose exec -T postgres pg_isready -U mandoforge -d mandoforge >/dev/null
  prepare_fake_codex
  LOG_FILE="$(mktemp -t mandoforge-stage1-live-gate.XXXXXX.log)"
  start_gate_api \
    "$LOG_FILE" \
    "MANDOFORGE_ADDR=$GATE_ADDR" \
    "MANDOFORGE_WORKSPACE_ROOT=$GATE_WORKSPACE_ROOT" \
    "MANDOFORGE_INSECURE_DEV_AUTH=1" \
    "MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1" \
    "DATABASE_URL=$DATABASE_URL" \
    "MANDOFORGE_SHELL_RUNNER=docker" \
    "MANDOFORGE_SHELL_DOCKER_IMAGE=${MANDOFORGE_SHELL_DOCKER_IMAGE:-alpine:3.20}" \
    "PATH=$FAKE_CODEX_DIR:$PATH"
  BASE_URL="$GATE_BASE_URL"
fi

curl -fsS "$BASE_URL/healthz" >/dev/null

BASE_URL="$BASE_URL" MANDOFORGE_WORKSPACE_ROOT="$GATE_WORKSPACE_ROOT" ./scripts/agent-os-core-evidence-gate.sh
BASE_URL="$BASE_URL" EVIDENCE_DIR="$GATE_WORKSPACE_ROOT/work-item-collaboration-evidence" ./scripts/work-item-collaboration-evidence-gate.sh
if [[ "$START_LIVE_STACK" == "1" || "${RUN_CODEX_VERIFY:-0}" == "1" ]]; then
  BASE_URL="$BASE_URL" ./scripts/verify-codex-exec-adapter.sh
fi
BASE_URL="$BASE_URL" DATABASE_URL="$DATABASE_URL" ./scripts/verify-postgres-sql-query.sh
BASE_URL="$BASE_URL" ./scripts/verify-docker-shell-runner.sh
if [[ "${RUN_PROVIDER_SMOKE:-0}" == "1" ]]; then
  BASE_URL="$BASE_URL" ./scripts/verify-external-provider.sh
fi

echo "stage1 final gate ok"
