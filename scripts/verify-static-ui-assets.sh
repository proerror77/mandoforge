#!/usr/bin/env bash
set -euo pipefail

INDEX_FILE="${INDEX_FILE:-web/index.html}"
APP_FILE="${APP_FILE:-web/app.js}"

require_file() {
  if [[ ! -s "$1" ]]; then
    echo "missing static UI asset: $1" >&2
    exit 1
  fi
}

require_text() {
  local file="$1"
  local pattern="$2"
  if ! grep -q "$pattern" "$file"; then
    echo "static UI asset check failed: $file missing pattern: $pattern" >&2
    exit 1
  fi
}

for command in node grep; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "missing required command: $command" >&2
    exit 1
  fi
done

require_file "$INDEX_FILE"
require_file "$APP_FILE"

node --check "$APP_FILE" >/dev/null

index_patterns=(
  "Tenant Governance"
  "Providers"
  "Vault"
  "Approval Governance"
  "Policy Console"
  "Eval Runs"
  "MCP Servers"
  "Worker Dashboard"
  "Remote Computers"
  "Codex App Server"
  "Governance"
  "Stage 2"
  "Runtime Profile ID"
  "Skill IDs"
  "MCP Server IDs"
  "Remote Computer Profile JSON"
  "Semantic Scopes JSON"
  "Managed Agent Console"
  "Runtime Profiles"
  "Register Remote Computer"
  "Create Remote Lease"
  "Attach Remote Lease"
  "Record Sidecar Heartbeat"
  "Assign Remote Lease"
  "Dry-run Runner"
  "Mutate Runner"
  "Reclaim Stale Remote Computers"
)

app_patterns=(
  "stage2Readiness"
  "evidence_requirements"
  "evidence_scripts"
  "evidence_job_manifests"
  "required_flags"
  "required_artifacts"
  "controller_evidence_fresh"
  "latest_controller_age_hours"
  "createRemoteComputer"
  "createRemoteComputerLease"
  "attachRemoteComputerLease"
  "recordRemoteSidecarHeartbeat"
  "artifact_discovery_sidecar_config"
  "dryRunRemoteRunner"
  "mutateRemoteRunner"
  "assignExecutionJobRemoteLease"
  "cancelExecutionJob"
  "releaseRemoteComputerAttachment"
  "agentRuntimeProfiles"
  "agentRuntimeProfileReleaseGates"
  "renderManagedAgentConsole"
  "/api/agent-runtime-profiles"
  "/api/agent-runtime-profile-release-gates"
  "/api/stage2/readiness"
  "/api/remote-computers/runner/dry-run"
  "/api/remote-computers/runner/mutate"
  "/api/remote-computers/sidecars/heartbeats"
  "/api/execution-jobs/"
)

for pattern in "${index_patterns[@]}"; do
  require_text "$INDEX_FILE" "$pattern"
done

for pattern in "${app_patterns[@]}"; do
  require_text "$APP_FILE" "$pattern"
done

if grep -q "window.prompt" "$APP_FILE"; then
  echo "static UI must use explicit forms instead of window.prompt" >&2
  exit 1
fi

echo "static UI asset verification ok"
