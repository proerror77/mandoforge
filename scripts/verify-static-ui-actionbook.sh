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
  const result = {
    title: document.title,
    hasProviderBreakdown: text.includes('PROVIDER COST BREAKDOWN'),
    hasToolBreakdown: text.includes('TOOL RUNTIME BREAKDOWN'),
    hasProviderBudgetForecast: text.includes('PROVIDER BUDGET FORECAST'),
    hasFinanceOperations: text.includes('Run Finance Ops') && text.includes('FINANCE OPERATIONS') && text.includes('OPERATIONS STATUS') && text.includes('RUNBOOK ACTIONS'),
    hasObservabilityDashboard: text.includes('Observability') && text.includes('Telemetry / Backpressure / Error Events') && text.includes('Run Remediation') && text.includes('Validate Collector Cluster') && Boolean(document.querySelector('#observability-summary')),
    hasCollectorReadiness: text.includes('COLLECTOR READINESS') && text.includes('COLLECTOR ENDPOINT') && text.includes('HEALTH MESSAGE') && (text.includes('CLUSTER ROLLOUT') || text.includes('Cluster rollout')) && Boolean(document.querySelector('#validate-observability-collector-cluster')),
    hasCostAlertRoutes: text.includes('Create Alert Route') && Array.from(document.querySelectorAll('#cost-alert-route-form input')).some((input) => input.placeholder.includes('SMTP recipient email')) && text.includes('No cost alert routes'),
    hasProviderCredentialFields: text.includes('API key env var') && text.includes('API key ref') && text.includes('Rotate Provider API Key Ref') && typeof rotateProviderApiKeyRefFromForm === 'function' && Boolean(document.querySelector('#provider-api-key-ref-rotate-form')),
    hasProviderUpdateWorkflow: text.includes('Update Provider') && typeof updateProvider === 'function' && Boolean(document.querySelector('#provider-update-form')),
    hasProviderAccessWorkflow: text.includes('Provider Access') && text.includes('Create Provider Access') && text.includes('Update Provider Access') && text.includes('Archive Provider Access') && typeof createProviderAccess === 'function' && typeof updateProviderAccess === 'function' && typeof archiveProviderAccess === 'function' && Boolean(document.querySelector('#provider-access-form')) && Boolean(document.querySelector('#provider-access-update-form')),
    hasProviderApprovalWorkflow: text.includes('Request Provider Approval') && text.includes('Approver subject'),
    hasProviderGateRuns: text.includes('Run Provider Gate') && text.includes('PROVIDER GATE RUNS') && text.includes('PRODUCTION ENFORCEMENT') && text.includes('Run Production Rollout') && text.includes('PRODUCTION ROLLOUT'),
    hasPolicyConsole: text.includes('Policy Console') && text.includes('Simulate Policy') && text.includes('Test Policy') && text.includes('POLICY REVISIONS') && text.includes('Create Policy Revision') && text.includes('Gate cases JSON') && text.includes('Rollout %') && text.includes('Activate after') && text.includes('Activate before'),
    hasPolicyRolloutCancel: text.includes('Cancel Staged Rollout') && text.includes('Run Due Policy Rollouts') && text.includes('Rollback Active Policy') && text.includes('RUNTIME ROLLOUT'),
    hasVaultHealthAction: text.includes('Check Vault Health') && text.includes('Register Secret Ref'),
    hasVaultReadiness: Boolean(document.querySelector('#vault-readiness')) && text.includes('Secret provider:') && text.includes('STALE ROTATIONS') && (text.includes('endpoint missing') || text.includes('endpoint configured')),
    hasApprovalGovernance: text.includes('Approval Governance') && text.includes('Create Approval Group') && text.includes('Create Escalation Rule') && text.includes('Run Due Escalations'),
    hasApprovalNotificationRouting: Boolean(document.querySelector('#approval-notification-routing')) && Boolean(document.querySelector('#approval-notification-runs')) && text.includes('ROUTING') && text.includes('UNROUTABLE') && text.includes('Run Notifications') && text.includes('NOTIFICATION RUNS') && text.includes('Production ops:'),
    hasCodexAppServer: text.includes('Codex App Server') && text.includes('Check Codex Health') && text.includes('Load Codex Runs') && text.includes('Create Codex Thread') && text.includes('Create Codex Turn') && text.includes('Execute Codex Command') && text.includes('Interrupt Codex Turn') && text.includes('Sync Codex Artifacts') && text.includes('Codex steering') && text.includes('LONG-RUNNING STEERING') && typeof renderCountTable === 'function',
    hasMcpLifecycle: text.includes('MCP Servers') && text.includes('Config JSON') && Array.from(document.querySelectorAll('#mcp-form textarea')).some((textarea) => textarea.value.includes('vault:mcp/docs#api_key')) && text.includes('Load Team Servers') && text.includes('Run Team Health') && text.includes('Run Due Health') && text.includes('Update MCP Server') && text.includes('Request MCP Rollout') && typeof updateMcpServerFromForm === 'function' && typeof requestMcpRolloutFromForm === 'function' && Boolean(document.querySelector('#mcp-update-form')) && Boolean(document.querySelector('#mcp-rollout-form')) && text.includes('MCP ROLLOUT RUNS'),
    hasTenantGovernance: text.includes('Tenant Governance') && text.includes('Archive Organization') && text.includes('Delete Organization') && text.includes('Archive Team') && text.includes('Delete Team') && text.includes('Archive Project') && text.includes('Delete Project') && text.includes('Bootstrap Tenant') && text.includes('Transfer Ownership') && text.includes('Update Organization') && text.includes('Update Team') && text.includes('Update Project') && text.includes('Create Membership') && text.includes('Delete Membership') && text.includes('Create Invitation') && text.includes('Accept Invitation') && (text.includes('No tenant invitations') || text.includes('Select an organization to manage invitations')),
    hasTenantScopeUpdate: typeof updateOrganization === 'function' && typeof updateTeam === 'function' && typeof updateProject === 'function' && Boolean(document.querySelector('#organization-update-form')) && Boolean(document.querySelector('#team-update-form')) && Boolean(document.querySelector('#project-update-form')),
    hasTenantScopeDelete: typeof deleteTeam === 'function' && typeof deleteProject === 'function' && text.includes('Delete Team') && text.includes('Delete Project'),
    hasEvalGateAction: text.includes('Gate 100') || text.includes('No eval runs'),
    hasEvalDriftAction: text.includes('Check Drift') || text.includes('No eval runs'),
    hasEvalJudgeProfiles: text.includes('Create Judge Profile') && text.includes('Judge profile') && Array.from(document.querySelectorAll('#eval-judge-profile-form input')).some((input) => input.value.includes('vault:eval/judges/default#api_key')) && Boolean(document.querySelector('#eval-judge-profiles')),
    hasEvalSuiteBootstrap: text.includes('Bootstrap Stage 2 Suite') && Boolean(document.querySelector('#eval-suite-bootstrap')),
    hasReleasePromotionWorkflow: (text.includes('Request Prod Approval') || text.includes('No eval runs')) && Boolean(document.querySelector('#agent-releases')) && text.includes('RELEASE AUTOMATION RUNS'),
    hasAgentReleases: text.includes('AGENT RELEASES') && Boolean(document.querySelector('#agent-releases')),
    hasWorkerDashboard: text.includes('Worker Dashboard') && text.includes('Queue readiness') && text.includes('QUEUE DURABILITY') && text.includes('AUTOSCALING SKELETON') && text.includes('WORKER LOAD VALIDATION') && text.includes('WORKER RUNBOOK ACTIONS') && Boolean(document.querySelector('#worker-readiness')),
    hasWorkerRemoteLeaseControls: text.includes('Assign Remote Lease') && typeof assignExecutionJobRemoteLease === 'function' && typeof assignExecutionJobRemoteLeaseFromForm === 'function' && typeof cancelExecutionJob === 'function' && Boolean(document.querySelector('#execution-job-remote-lease-form')),
    hasRemoteComputerReadiness: text.includes('Remote Computers') && text.includes('REMOTE COMPUTER READINESS') && text.includes('STATE FILESYSTEM') && text.includes('Production profile:') && text.includes('RUNNER BOUNDARY') && text.includes('REMOTE COMPUTER LEASE STORE') && text.includes('REMOTE COMPUTER ATTACHMENTS') && text.includes('REMOTE COMPUTER STATE LOCKS') && text.includes('REMOTE ARTIFACT DISCOVERY') && text.includes('REMOTE COMPUTER SIDECAR HEARTBEATS') && text.includes('Supervision:') && text.includes('Artifact discovery sidecar') && text.includes('Sidecar API URL') && text.includes('Discover Remote Artifacts') && text.includes('Acquire State Lock') && text.includes('Validate State Sync') && text.includes('REMOTE COMPUTER RUNBOOK') && Boolean(document.querySelector('#remote-computer-readiness')) && Boolean(document.querySelector('#validate-remote-state-sync')),
    hasRemoteComputerLifecycle: text.includes('Register Remote Computer') && text.includes('Create Remote Lease') && text.includes('Reclaim Stale Remote Computers') && typeof createRemoteComputer === 'function' && typeof createRemoteComputerLease === 'function' && typeof updateRemoteComputerLease === 'function' && typeof reclaimStaleRemoteComputers === 'function' && Boolean(document.querySelector('#remote-computer-form')) && Boolean(document.querySelector('#remote-computer-lease-form')) && Boolean(document.querySelector('#reclaim-remote-computers')),
    hasRemoteComputerAttachmentControls: text.includes('Attach Remote Lease') && typeof attachRemoteComputerLease === 'function' && typeof releaseRemoteComputerAttachment === 'function' && Boolean(document.querySelector('#remote-computer-attachment-form')),
    hasRemoteComputerSidecarHeartbeatControls: text.includes('Record Sidecar Heartbeat') && typeof recordRemoteSidecarHeartbeat === 'function' && Boolean(document.querySelector('#remote-sidecar-heartbeat-form')),
    hasRemoteComputerRunnerOps: text.includes('Dry-run Runner') && text.includes('Mutate Runner') && text.includes('Supported operations:') && typeof dryRunRemoteRunner === 'function' && typeof mutateRemoteRunner === 'function' && typeof remoteRunnerPayload === 'function' && Boolean(document.querySelector('#remote-runner-form')) && Boolean(document.querySelector('#dry-run-remote-runner')) && Boolean(document.querySelector('#mutate-remote-runner')),
    hasProviderHealthAction: text.includes('Check Health') || text.includes('No stored providers'),
    metricCards: document.querySelectorAll('.metric').length,
    hasUsageRoot: Boolean(document.querySelector('#usage-summary')),
    hasStage2ReadinessGate: text.toLowerCase().includes('stage 2 completion gate') && text.toLowerCase().includes('stage 2 open gaps') && text.toLowerCase().includes('stage 2 evidence checklist') && text.includes('/api/tenant-isolation/routing/validate') && Boolean(document.querySelector('#governance-status')),
    hasAdminConsole: text.includes('Admin Console')
  };
  result.ok = result.title === 'MandoForge Agent OS Kernel'
    && result.hasProviderBreakdown
    && result.hasToolBreakdown
    && result.hasProviderBudgetForecast
    && result.hasFinanceOperations
    && result.hasObservabilityDashboard
    && result.hasCollectorReadiness
    && result.hasCostAlertRoutes
    && result.hasProviderCredentialFields
    && result.hasProviderUpdateWorkflow
    && result.hasProviderAccessWorkflow
    && result.hasProviderApprovalWorkflow
    && result.hasProviderGateRuns
    && result.hasPolicyConsole
    && result.hasPolicyRolloutCancel
    && result.hasVaultHealthAction
    && result.hasVaultReadiness
    && result.hasApprovalGovernance
    && result.hasApprovalNotificationRouting
    && result.hasCodexAppServer
    && result.hasMcpLifecycle
    && result.hasTenantGovernance
    && result.hasTenantScopeUpdate
    && result.hasTenantScopeDelete
    && result.hasEvalGateAction
    && result.hasEvalDriftAction
    && result.hasEvalJudgeProfiles
    && result.hasEvalSuiteBootstrap
    && result.hasReleasePromotionWorkflow
    && result.hasAgentReleases
    && result.hasWorkerDashboard
    && result.hasWorkerRemoteLeaseControls
    && result.hasRemoteComputerReadiness
    && result.hasRemoteComputerLifecycle
    && result.hasRemoteComputerAttachmentControls
    && result.hasRemoteComputerSidecarHeartbeatControls
    && result.hasRemoteComputerRunnerOps
    && result.hasProviderHealthAction
    && result.hasStage2ReadinessGate
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
  actionbook_browser_text >&2 || true
  exit 1
}

require_command actionbook
require_command curl
require_command jq

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
grep -q "mcpRolloutRuns" /tmp/mandoforge-actionbook-app.js
grep -q "updateMcpServerFromForm" /tmp/mandoforge-actionbook-app.js
grep -q "requestMcpRolloutFromForm" /tmp/mandoforge-actionbook-app.js
grep -q "mcp-update-form" /tmp/mandoforge-actionbook-index.html
grep -q "mcp-rollout-form" /tmp/mandoforge-actionbook-index.html
if grep -q "window.prompt" /tmp/mandoforge-actionbook-app.js; then
  echo "MCP lifecycle UI must use explicit forms instead of window.prompt" >&2
  exit 1
fi
grep -q "renderPolicyDiffSummary" /tmp/mandoforge-actionbook-app.js
grep -q "policy-diff-table" /tmp/mandoforge-actionbook-app.js
grep -q "cancelPolicyRollout" /tmp/mandoforge-actionbook-app.js
grep -q "Archive Provider" /tmp/mandoforge-actionbook-index.html
grep -q "provider-api-key-ref-rotate-form" /tmp/mandoforge-actionbook-index.html
grep -q "rotateProviderApiKeyRefFromForm" /tmp/mandoforge-actionbook-app.js
grep -q "pollCodexRun" /tmp/mandoforge-actionbook-app.js
grep -q "data-poll-codex-run" /tmp/mandoforge-actionbook-app.js
grep -q "Trace Status Breakdown" /tmp/mandoforge-actionbook-app.js
grep -q "Trace Operation Breakdown" /tmp/mandoforge-actionbook-app.js
grep -q "Trace Detail Operations" /tmp/mandoforge-actionbook-app.js
grep -q "renderCountTable" /tmp/mandoforge-actionbook-app.js
grep -q "createEvalJudgeProfile" /tmp/mandoforge-actionbook-app.js
grep -q "bootstrapEvalSuite" /tmp/mandoforge-actionbook-app.js
grep -q "requestEvalRunPromotion" /tmp/mandoforge-actionbook-app.js
grep -q "agentReleaseAutomationRuns" /tmp/mandoforge-actionbook-app.js
grep -q "runObservabilityRemediation" /tmp/mandoforge-actionbook-app.js
grep -q "validateObservabilityCollectorCluster" /tmp/mandoforge-actionbook-app.js
grep -q "validate-observability-collector-cluster" /tmp/mandoforge-actionbook-index.html
grep -q "stage2Readiness" /tmp/mandoforge-actionbook-app.js
grep -q "evidence_requirements" /tmp/mandoforge-actionbook-app.js
grep -q "evidence_scripts" /tmp/mandoforge-actionbook-app.js
grep -q "evidence_job_manifests" /tmp/mandoforge-actionbook-app.js
grep -q "required_flags" /tmp/mandoforge-actionbook-app.js
grep -q "required_artifacts" /tmp/mandoforge-actionbook-app.js
grep -q "Stage 2 Evidence Checklist" /tmp/mandoforge-actionbook-app.js
grep -q "/api/stage2/readiness" /tmp/mandoforge-actionbook-app.js
grep -q "Create Judge Profile" /tmp/mandoforge-actionbook-index.html
grep -q "Bootstrap Stage 2 Suite" /tmp/mandoforge-actionbook-index.html
grep -q "vaultReadiness" /tmp/mandoforge-actionbook-app.js
grep -q "vaultKmsRotationRun" /tmp/mandoforge-actionbook-app.js
grep -q "vault-readiness" /tmp/mandoforge-actionbook-index.html
grep -q "run-vault-kms-rotation" /tmp/mandoforge-actionbook-index.html
grep -q "approvalNotificationRouting" /tmp/mandoforge-actionbook-app.js
grep -q "approval-notification-routing" /tmp/mandoforge-actionbook-index.html
grep -q "approvalNotificationRuns" /tmp/mandoforge-actionbook-app.js
grep -q "approval-notification-runs" /tmp/mandoforge-actionbook-index.html
grep -q "workerReadiness" /tmp/mandoforge-actionbook-app.js
grep -q "workerLoadValidationRun" /tmp/mandoforge-actionbook-app.js
grep -q "worker-readiness" /tmp/mandoforge-actionbook-index.html
grep -q "run-worker-load-validation" /tmp/mandoforge-actionbook-index.html
grep -q "execution-job-remote-lease-form" /tmp/mandoforge-actionbook-index.html
grep -q "Assign Remote Lease" /tmp/mandoforge-actionbook-index.html
grep -q "assignExecutionJobRemoteLease" /tmp/mandoforge-actionbook-app.js
grep -q "cancelExecutionJob" /tmp/mandoforge-actionbook-app.js
grep -q "/api/execution-jobs/" /tmp/mandoforge-actionbook-app.js
grep -q "/remote-computer-lease" /tmp/mandoforge-actionbook-app.js
grep -q "/cancel" /tmp/mandoforge-actionbook-app.js
grep -q "remote-computer-readiness" /tmp/mandoforge-actionbook-index.html
grep -q "remote-computer-form" /tmp/mandoforge-actionbook-index.html
grep -q "remote-computer-lease-form" /tmp/mandoforge-actionbook-index.html
grep -q "remote-computer-attachment-form" /tmp/mandoforge-actionbook-index.html
grep -q "Attach Remote Lease" /tmp/mandoforge-actionbook-index.html
grep -q "remote-sidecar-heartbeat-form" /tmp/mandoforge-actionbook-index.html
grep -q "Record Sidecar Heartbeat" /tmp/mandoforge-actionbook-index.html
grep -q "remote-runner-form" /tmp/mandoforge-actionbook-index.html
grep -q "dry-run-remote-runner" /tmp/mandoforge-actionbook-index.html
grep -q "mutate-remote-runner" /tmp/mandoforge-actionbook-index.html
grep -q "reclaim-remote-computers" /tmp/mandoforge-actionbook-index.html
grep -q "dryRunRemoteRunner" /tmp/mandoforge-actionbook-app.js
grep -q "mutateRemoteRunner" /tmp/mandoforge-actionbook-app.js
grep -q "remoteRunnerPayload" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computers/runner/dry-run" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computers/runner/mutate" /tmp/mandoforge-actionbook-app.js
grep -q "createRemoteComputer" /tmp/mandoforge-actionbook-app.js
grep -q "createRemoteComputerLease" /tmp/mandoforge-actionbook-app.js
grep -q "updateRemoteComputerLease" /tmp/mandoforge-actionbook-app.js
grep -q "attachRemoteComputerLease" /tmp/mandoforge-actionbook-app.js
grep -q "releaseRemoteComputerAttachment" /tmp/mandoforge-actionbook-app.js
grep -q "recordRemoteSidecarHeartbeat" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computers/sidecars/heartbeats" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computer-leases/" /tmp/mandoforge-actionbook-app.js
grep -q "/attach" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computer-attachments/" /tmp/mandoforge-actionbook-app.js
grep -q "reclaimStaleRemoteComputers" /tmp/mandoforge-actionbook-app.js
grep -q "/api/remote-computers/reclaim-stale" /tmp/mandoforge-actionbook-app.js
grep -q "remoteComputerStateLocks" /tmp/mandoforge-actionbook-app.js
grep -q "validateRemoteStateSync" /tmp/mandoforge-actionbook-app.js
grep -q "validate-remote-state-sync" /tmp/mandoforge-actionbook-index.html
grep -q "remoteComputerSidecarHeartbeats" /tmp/mandoforge-actionbook-app.js
grep -q "remote_computer_sidecar_supervision" /tmp/mandoforge-actionbook-app.js
grep -q "discoverRemoteArtifacts" /tmp/mandoforge-actionbook-app.js
grep -q "Artifact discovery sidecar" /tmp/mandoforge-actionbook-app.js
grep -q "Acquire State Lock" /tmp/mandoforge-actionbook-index.html
grep -q "Discover Remote Artifacts" /tmp/mandoforge-actionbook-index.html
grep -q "acceptTenantInvitation" /tmp/mandoforge-actionbook-app.js
grep -q "data-accept-invitation" /tmp/mandoforge-actionbook-app.js
grep -q "/api/invitations/accept" /tmp/mandoforge-actionbook-app.js
grep -q "deleteMembership" /tmp/mandoforge-actionbook-app.js
grep -q "data-delete-membership" /tmp/mandoforge-actionbook-app.js
grep -q "/api/memberships/" /tmp/mandoforge-actionbook-app.js
curl -fsS "$BASE_URL/api/usage" \
  -H 'x-mandoforge-subject: actionbook-smoke' \
  -H 'x-mandoforge-roles: admin' \
  >/tmp/mandoforge-actionbook-usage.json

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
