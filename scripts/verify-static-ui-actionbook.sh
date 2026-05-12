#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8791}"
GATE_ADDR="${GATE_ADDR:-127.0.0.1:8791}"
ACTIONBOOK_CDP_PORT="${ACTIONBOOK_CDP_PORT:-9224}"
ACTIONBOOK_SCREENSHOT="${ACTIONBOOK_SCREENSHOT:-/tmp/mandoforge-actionbook-smoke.png}"
ACTIONBOOK_EVAL_JSON="${ACTIONBOOK_EVAL_JSON:-/tmp/mandoforge-actionbook-eval.json}"
CHROME_PATH="${CHROME_PATH:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
CHROME_USER_DATA_DIR="${CHROME_USER_DATA_DIR:-/tmp/mandoforge-actionbook-chrome}"
API_PID=""
CHROME_PID=""

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
  fi
  if [[ -n "${CHROME_PID:-}" ]]; then
    kill "$CHROME_PID" >/dev/null 2>&1 || true
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

wait_for_url() {
  local url="$1"
  for _ in $(seq 1 60); do
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
    actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser eval "
(() => {
  const text = document.body?.innerText || '';
  const result = {
    title: document.title,
    hasProviderBreakdown: text.includes('PROVIDER COST BREAKDOWN'),
    hasToolBreakdown: text.includes('TOOL RUNTIME BREAKDOWN'),
    hasProviderBudgetForecast: text.includes('PROVIDER BUDGET FORECAST'),
    hasCostAlertRoutes: text.includes('Create Alert Route') && text.includes('No cost alert routes'),
    hasProviderCredentialFields: text.includes('API key env var') && text.includes('API key ref'),
    hasPolicyConsole: text.includes('Policy Console') && text.includes('Simulate Policy') && text.includes('Test Policy') && text.includes('POLICY REVISIONS') && text.includes('Create Policy Revision') && text.includes('Gate cases JSON') && text.includes('Rollout %') && text.includes('Activate after') && text.includes('Activate before'),
    hasPolicyRolloutCancel: text.includes('Cancel Staged Rollout') && text.includes('RUNTIME ROLLOUT'),
    hasVaultHealthAction: text.includes('Check Vault Health') && text.includes('Register Secret Ref'),
    hasApprovalGovernance: text.includes('Approval Governance') && text.includes('Create Approval Group') && text.includes('Create Escalation Rule'),
    hasCodexAppServer: text.includes('Codex App Server') && text.includes('Check Codex Health') && text.includes('Load Codex Runs') && text.includes('Create Codex Thread') && text.includes('Create Codex Turn') && text.includes('Execute Codex Command') && text.includes('Interrupt Codex Turn') && text.includes('Sync Codex Artifacts') && text.includes('Codex steering'),
    hasMcpLifecycle: text.includes('MCP Servers') && text.includes('Config JSON') && text.includes('Load Team Servers') && text.includes('Run Team Health') && text.includes('Run Due Health'),
    hasTenantGovernance: text.includes('Tenant Governance'),
    hasEvalGateAction: text.includes('Gate 100') || text.includes('No eval runs'),
    hasEvalDriftAction: text.includes('Check Drift') || text.includes('No eval runs'),
    hasAgentReleases: text.includes('AGENT RELEASES') && Boolean(document.querySelector('#agent-releases')),
    hasWorkerDashboard: text.includes('Worker Dashboard') && text.includes('Attempts'),
    hasProviderHealthAction: text.includes('Check Health') || text.includes('No stored providers'),
    metricCards: document.querySelectorAll('.metric').length,
    hasUsageRoot: Boolean(document.querySelector('#usage-summary')),
    hasAdminConsole: text.includes('Admin Console')
  };
  result.ok = result.title === 'MandoForge Agent OS Kernel'
    && result.hasProviderBreakdown
    && result.hasToolBreakdown
    && result.hasProviderBudgetForecast
    && result.hasCostAlertRoutes
    && result.hasProviderCredentialFields
    && result.hasPolicyConsole
    && result.hasPolicyRolloutCancel
    && result.hasVaultHealthAction
    && result.hasApprovalGovernance
    && result.hasCodexAppServer
    && result.hasMcpLifecycle
    && result.hasTenantGovernance
    && result.hasEvalGateAction
    && result.hasEvalDriftAction
    && result.hasAgentReleases
    && result.hasWorkerDashboard
    && result.hasProviderHealthAction
    && result.metricCards >= 4
    && result.hasUsageRoot
    && result.hasAdminConsole;
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
  actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser text --json >&2 || true
  exit 1
}

require_command actionbook
require_command curl

if ! curl -fsS "$BASE_URL/healthz" >/dev/null 2>&1; then
  MANDOFORGE_ADDR="$GATE_ADDR" cargo run -p mandoforge-api >/tmp/mandoforge-actionbook-api.log 2>&1 &
  API_PID="$!"
  wait_for_url "$BASE_URL/healthz"
fi

curl -fsS "$BASE_URL/" >/tmp/mandoforge-actionbook-index.html
curl -fsS "$BASE_URL/app.js" >/tmp/mandoforge-actionbook-app.js
grep -q "checkMcpHealth" /tmp/mandoforge-actionbook-app.js
grep -q "data-health-mcp" /tmp/mandoforge-actionbook-app.js
grep -q "runMcpHealth" /tmp/mandoforge-actionbook-app.js
grep -q "runDueMcpHealth" /tmp/mandoforge-actionbook-app.js
grep -q "renderPolicyDiffSummary" /tmp/mandoforge-actionbook-app.js
grep -q "policy-diff-table" /tmp/mandoforge-actionbook-app.js
grep -q "cancelPolicyRollout" /tmp/mandoforge-actionbook-app.js
grep -q "pollCodexRun" /tmp/mandoforge-actionbook-app.js
grep -q "data-poll-codex-run" /tmp/mandoforge-actionbook-app.js
curl -fsS "$BASE_URL/api/usage" \
  -H 'x-mandoforge-subject: actionbook-smoke' \
  -H 'x-mandoforge-roles: admin' \
  >/tmp/mandoforge-actionbook-usage.json

if ! curl -fsS "http://127.0.0.1:$ACTIONBOOK_CDP_PORT/json/version" >/dev/null 2>&1; then
  if [[ ! -x "$CHROME_PATH" ]]; then
    echo "Chrome executable not found at $CHROME_PATH" >&2
    exit 1
  fi
  "$CHROME_PATH" \
    --headless=new \
    "--remote-debugging-port=$ACTIONBOOK_CDP_PORT" \
    "--user-data-dir=$CHROME_USER_DATA_DIR" \
    --disable-gpu \
    --no-first-run \
    --no-default-browser-check \
    about:blank \
    >/tmp/mandoforge-actionbook-chrome.log 2>&1 &
  CHROME_PID="$!"
  wait_for_url "http://127.0.0.1:$ACTIONBOOK_CDP_PORT/json/version"
fi

actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser connect "$ACTIONBOOK_CDP_PORT" --json >/tmp/mandoforge-actionbook-connect.json
actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser open "$BASE_URL" --json >/tmp/mandoforge-actionbook-open.json

wait_for_static_ui

actionbook --cdp "$ACTIONBOOK_CDP_PORT" browser screenshot "$ACTIONBOOK_SCREENSHOT" --json >/tmp/mandoforge-actionbook-screenshot.json

echo "static UI actionbook smoke ok"
echo "base_url=$BASE_URL"
echo "screenshot=$ACTIONBOOK_SCREENSHOT"
