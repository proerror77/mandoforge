#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8791}"
GATE_ADDR="${GATE_ADDR:-127.0.0.1:8791}"
ACTIONBOOK_CDP_PORT="${ACTIONBOOK_CDP_PORT:-9324}"
ACTIONBOOK_SCREENSHOT="${ACTIONBOOK_SCREENSHOT:-/tmp/mandoforge-actionbook-smoke.png}"
ACTIONBOOK_EVAL_JSON="${ACTIONBOOK_EVAL_JSON:-/tmp/mandoforge-actionbook-eval.json}"
ACTIONBOOK_SMOKE_URL="${ACTIONBOOK_SMOKE_URL:-$BASE_URL/?actionbook_smoke=$(date +%s)}"
CHROME_PATH="${CHROME_PATH:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
CHROME_USER_DATA_DIR="${CHROME_USER_DATA_DIR:-/tmp/mandoforge-actionbook-chrome}"
ACTIONBOOK_SESSION_ID=""
ACTIONBOOK_TAB_ID=""
ACTIONBOOK_DRIVER_MODE=""
API_PID=""
CHROME_PID=""

cleanup() {
  if [[ -n "${ACTIONBOOK_SESSION_ID:-}" && "${ACTIONBOOK_DRIVER_MODE:-}" == "browser-start" ]]; then
    actionbook browser close --session "$ACTIONBOOK_SESSION_ID" --json >/tmp/mandoforge-actionbook-close.json 2>/dev/null || true
  fi
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
  fi
  if [[ -n "${CHROME_PID:-}" ]]; then
    kill "$CHROME_PID" >/dev/null 2>&1 || true
    for _ in $(seq 1 20); do
      if ! kill -0 "$CHROME_PID" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    kill -9 "$CHROME_PID" >/dev/null 2>&1 || true
    wait "$CHROME_PID" 2>/dev/null || true
  fi
}

trap cleanup EXIT

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local pattern="$2"
  if ! grep -q "$pattern" "$file"; then
    echo "static UI actionbook check failed: $file missing pattern: $pattern" >&2
    exit 1
  fi
}

actionbook_uses_browser_start() {
  actionbook browser start --help 2>/dev/null | grep -q -- "--cdp-endpoint"
}

actionbook_browser_eval() {
  local expression="$1"
  if [[ "$ACTIONBOOK_DRIVER_MODE" == "browser-start" ]]; then
    actionbook browser eval "$expression" --session "$ACTIONBOOK_SESSION_ID" --tab "$ACTIONBOOK_TAB_ID" --json
  else
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser eval "$expression" --json
  fi
}

actionbook_browser_text() {
  if [[ "$ACTIONBOOK_DRIVER_MODE" == "browser-start" ]]; then
    actionbook browser text --session "$ACTIONBOOK_SESSION_ID" --tab "$ACTIONBOOK_TAB_ID" --json
  else
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser text --json
  fi
}

actionbook_browser_screenshot() {
  local path="$1"
  if [[ "$ACTIONBOOK_DRIVER_MODE" == "browser-start" ]]; then
    actionbook browser screenshot "$path" --session "$ACTIONBOOK_SESSION_ID" --tab "$ACTIONBOOK_TAB_ID" --json
  else
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser screenshot "$path" --json
  fi
}

actionbook_open_smoke_url() {
  if [[ "$ACTIONBOOK_DRIVER_MODE" == "browser-start" ]]; then
    actionbook browser goto "$ACTIONBOOK_SMOKE_URL" --session "$ACTIONBOOK_SESSION_ID" --tab "$ACTIONBOOK_TAB_ID" --json >/tmp/mandoforge-actionbook-open.json
  else
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser connect "$ACTIONBOOK_CDP_PORT" --json >/tmp/mandoforge-actionbook-connect.json
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser open "$ACTIONBOOK_SMOKE_URL" --json >/tmp/mandoforge-actionbook-open.json
  fi
}

wait_for_url() {
  local url="$1"
  for _ in $(seq 1 240); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.5
  done
  curl -fsS "$url" >/dev/null
}

wait_for_static_ui() {
  local attempt
  for attempt in $(seq 1 40); do
    actionbook_browser_eval "
(() => {
  const text = document.body?.innerText || '';
  const normalizedText = text.toLowerCase();
  const selectors = ['.console-shell', '.topbar', '.language-toggle', '.notification-center', '.agent-os-cockpit', '.overview-signals', '.live-log'];
  const result = {
    title: document.title,
    hasMountedShell: selectors.every((selector) => Boolean(document.querySelector(selector))),
    hasHeader: normalizedText.includes('mandoforge co-work') && normalizedText.includes('managed agents'),
    hasMetrics: normalizedText.includes('running agents') && normalizedText.includes('queue') && normalizedText.includes('approvals') && normalizedText.includes('errors'),
    hasLiveLog: normalizedText.includes('live operation log'),
    hasWorkflowNavigation: normalizedText.includes('managed agents') && normalizedText.includes('runs & tasks') && normalizedText.includes('ontology') && normalizedText.includes('capabilities') && normalizedText.includes('system ops'),
    hasExecutionPanels: normalizedText.includes('runtime pressure') && normalizedText.includes('enterprise readiness') && normalizedText.includes('connector and ontology gates') && normalizedText.includes('evidence endpoints'),
    hasVisualizations: Boolean(document.querySelector('.agent-os-cockpit')),
    hasTaskLauncher: normalizedText.includes('managed agents') && normalizedText.includes('runs & tasks') && normalizedText.includes('ontology') && normalizedText.includes('system ops'),
    metricCards: document.querySelectorAll('.metric').length,
  };
  result.ok = result.title === 'MandoForge Agent OS Console'
    && result.hasMountedShell
    && result.hasHeader
    && result.hasMetrics
    && result.hasLiveLog
    && result.hasWorkflowNavigation
    && result.hasExecutionPanels
    && result.hasVisualizations
    && result.hasTaskLauncher
    && result.metricCards >= 4
    && !text.includes('Uncaught');
  return (result.ok ? 'MANDOFORGE_ACTIONBOOK_OK ' : 'MANDOFORGE_ACTIONBOOK_PENDING ') + JSON.stringify(result);
})()
" --json >"$ACTIONBOOK_EVAL_JSON" || true

    if grep -q 'MANDOFORGE_ACTIONBOOK_OK' "$ACTIONBOOK_EVAL_JSON" \
      && ! grep -q '"className": "Error"' "$ACTIONBOOK_EVAL_JSON"; then
      return 0
    fi
    sleep 0.5
  done

  echo "static UI actionbook smoke failed after waiting for UI readiness" >&2
  echo "last eval:" >&2
  cat "$ACTIONBOOK_EVAL_JSON" >&2 || true
  echo >&2
  echo "visible text:" >&2
  actionbook_browser_text >&2 || true
  exit 1
}

require_command actionbook
require_command curl
require_command grep
require_command jq
require_command sed

if ! curl -fsS "$BASE_URL/healthz" >/dev/null 2>&1; then
  MANDOFORGE_ADDR="$GATE_ADDR" \
    MANDOFORGE_ALLOW_IN_MEMORY_STORE=1 \
    cargo run -p mandoforge-api >/tmp/mandoforge-actionbook-api.log 2>&1 &
  API_PID="$!"
  wait_for_url "$BASE_URL/healthz"
fi

curl -fsS "$BASE_URL/" >/tmp/mandoforge-actionbook-index.html
require_text /tmp/mandoforge-actionbook-index.html "MandoForge Agent OS Console"
require_text /tmp/mandoforge-actionbook-index.html "id=\"root\""

asset_refs=()
while IFS= read -r asset_ref; do
  asset_refs+=("$asset_ref")
done < <(grep -Eo '/[^"]+\.(js|css|wasm)' /tmp/mandoforge-actionbook-index.html | sort -u)
if [[ "${#asset_refs[@]}" -eq 0 ]]; then
  echo "static UI actionbook check failed: index does not reference Trunk JS/CSS/WASM assets" >&2
  exit 1
fi
rm -rf /tmp/mandoforge-actionbook-assets
mkdir -p /tmp/mandoforge-actionbook-assets
for asset_ref in "${asset_refs[@]}"; do
  curl -fsS "$BASE_URL$asset_ref" >"/tmp/mandoforge-actionbook-assets/${asset_ref##*/}"
done

js_asset_count="$(find /tmp/mandoforge-actionbook-assets -maxdepth 1 -type f -name '*.js' | wc -l | tr -d ' ')"
css_asset_count="$(find /tmp/mandoforge-actionbook-assets -maxdepth 1 -type f -name '*.css' | wc -l | tr -d ' ')"
wasm_asset_count="$(find /tmp/mandoforge-actionbook-assets -maxdepth 1 -type f -name '*.wasm' | wc -l | tr -d ' ')"
if [[ "$js_asset_count" -lt 1 || "$css_asset_count" -lt 1 || "$wasm_asset_count" -lt 1 ]]; then
  echo "static UI actionbook check failed: expected Trunk JS, CSS, and WASM assets" >&2
  exit 1
fi

asset_patterns=(
  "Agent OS Kernel"
  "Managed Agents"
  "Live operation log"
  "Runs & Tasks"
  "Ontology Builder"
  "Ontology onboarding journey"
  "Preview proposal"
  "Workflow"
  "System Ops"
  "Enterprise completion"
  "/api/enterprise-product/readiness"
  "Connector production readiness"
  "/api/native-connectors/production-readiness"
  "Ontology engine readiness"
  "/api/ontology/engine-readiness"
  "Transitions"
  "Start task"
  "/api/workflow-runs"
  "/transitions"
  "/task-grants"
)

for pattern in "${asset_patterns[@]}"; do
  if ! grep -aR -q "$pattern" /tmp/mandoforge-actionbook-assets; then
    echo "static UI actionbook check failed: Trunk assets missing pattern: $pattern" >&2
    exit 1
  fi
done

if grep -aR -q "window.prompt" /tmp/mandoforge-actionbook-assets; then
  echo "static UI must not use window.prompt" >&2
  exit 1
fi

if ! curl -fsS "http://127.0.0.1:$ACTIONBOOK_CDP_PORT/json/version" >/dev/null 2>&1; then
  if [[ ! -x "$CHROME_PATH" ]]; then
    echo "Chrome executable not found at $CHROME_PATH" >&2
    exit 1
  fi
  chrome_args=(
    --headless=new
    "--remote-debugging-port=$ACTIONBOOK_CDP_PORT"
    "--user-data-dir=$CHROME_USER_DATA_DIR"
    --disable-gpu
    --no-first-run
    --no-default-browser-check
  )
  if [[ "$(id -u)" == "0" ]]; then
    chrome_args+=(--no-sandbox --disable-dev-shm-usage)
  fi
  "$CHROME_PATH" \
    "${chrome_args[@]}" \
    about:blank \
    >/tmp/mandoforge-actionbook-chrome.log 2>&1 &
  CHROME_PID="$!"
  wait_for_url "http://127.0.0.1:$ACTIONBOOK_CDP_PORT/json/version"
fi

if actionbook_uses_browser_start; then
  ACTIONBOOK_DRIVER_MODE="browser-start"
  ACTIONBOOK_SESSION_ID="actionbook-smoke-$$"
  ACTIONBOOK_TAB_ID="t1"
  ACTIONBOOK_CDP_ENDPOINT="$(curl -fsS "http://127.0.0.1:$ACTIONBOOK_CDP_PORT/json/version" | jq -r '.webSocketDebuggerUrl')"
  actionbook browser start \
    --set-session-id "$ACTIONBOOK_SESSION_ID" \
    --profile "$ACTIONBOOK_SESSION_ID" \
    --cdp-endpoint "$ACTIONBOOK_CDP_ENDPOINT" \
    --json >/tmp/mandoforge-actionbook-connect.json
else
  ACTIONBOOK_DRIVER_MODE="legacy-cdp"
fi

actionbook_open_smoke_url
wait_for_static_ui
actionbook_browser_screenshot "$ACTIONBOOK_SCREENSHOT" >/tmp/mandoforge-actionbook-screenshot.json

echo "static UI actionbook smoke ok"
echo "base_url=$BASE_URL"
echo "screenshot=$ACTIONBOOK_SCREENSHOT"
