#!/usr/bin/env bash
set -euo pipefail

INDEX_FILE="${INDEX_FILE:-web/index.html}"
APP_FILE="${APP_FILE:-web/app.js}"
STYLE_FILE="${STYLE_FILE:-web/styles.css}"

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
require_file "$STYLE_FILE"

node --check "$APP_FILE" >/dev/null

index_patterns=(
  "demo-overview"
  "Primary Agent OS navigation"
  "开始一个任务"
  "查看运行记录"
  "Session Threads"
  "managed-session-workspace"
  "Managed Session Workspace"
  "Blocking Actions"
  "Event Stream"
  "检查系统状态"
  "orchestrator-form"
  "workspace-tabs"
  "开始任务"
  "系统状态"
  "运行记录"
  "高级设置"
  "检查 Whiskey runtime 状态"
  "agent-builder-section"
  "panel-advanced"
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
  "Environments"
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
  "renderDemoOverview"
  "renderInfraOverview"
  "runOrchestrator"
  "renderSessionThreads"
  "renderManagedSessionWorkspace"
  "/api/sessions/\${state.session.id}/threads"
  "Blocking Actions"
  "Event Stream"
  "applyTaskTemplate"
  "setWorkspaceTab"
  "Whiskey Demo Entry"
  "不是聊天框"
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
  "environments"
  "/api/environments"
  "renderManagedAgentConsole"
  "confirmDestructiveAction"
  "/api/agent-runtime-profiles"
  "/api/agent-runtime-profile-release-gates"
  "/api/stage2/readiness"
  "/api/remote-computers/runner/dry-run"
  "/api/remote-computers/runner/mutate"
  "/api/remote-computers/sidecars/heartbeats"
  "/api/execution-jobs/"
)

style_patterns=(
  "overflow-wrap: anywhere"
  ".demo-overview"
  ".thread-list"
  ".thread-card"
  ".managed-session-workspace"
  ".managed-session-grid"
  ".managed-session-card"
  ".managed-session-columns"
  ".side-nav"
  ".entry-map"
  ".current-run"
  ".workspace-tabs"
  ".orchestrator-form"
  ".run-guide"
  ".task-examples"
  ".run-aftercare"
  ".workspace-panel.is-hidden"
  ".demo-status-grid"
  ".demo-flow"
  ".agent-builder-section"
  ".workspace-panel.is-hidden"
  ".compact-agent"
  ".compact-approval"
  "details"
  "overflow-x: auto"
  "grid-template-columns: 1fr"
)

for pattern in "${index_patterns[@]}"; do
  require_text "$INDEX_FILE" "$pattern"
done

for pattern in "${app_patterns[@]}"; do
  require_text "$APP_FILE" "$pattern"
done

for pattern in "${style_patterns[@]}"; do
  require_text "$STYLE_FILE" "$pattern"
done

if grep -q "window.prompt" "$APP_FILE"; then
  echo "static UI must use explicit forms instead of window.prompt" >&2
  exit 1
fi

destructive_confirm_patterns=(
  "confirmDestructiveAction(\"Delete membership?\""
  "confirmDestructiveAction(\"Archive organization?\""
  "confirmDestructiveAction(\"Delete organization?\""
  "confirmDestructiveAction(\"Archive team?\""
  "confirmDestructiveAction(\"Delete team?\""
  "confirmDestructiveAction(\"Archive project?\""
  "confirmDestructiveAction(\"Delete project?\""
  "confirmDestructiveAction(\"Archive approval notification policy?\""
  "confirmDestructiveAction(\"Archive provider access?\""
  "confirmDestructiveAction(\"Rollback agent release?\""
  "confirmDestructiveAction(\"Rollback provider production rollout?\""
  "confirmDestructiveAction(\"Cancel staged policy rollout?\""
  "confirmDestructiveAction(\"Rollback active policy?\""
  "confirmDestructiveAction(\"Apply MCP rollout?\""
  "confirmDestructiveAction(\"Rollback MCP rollout?\""
  "confirmDestructiveAction(\"Cancel execution job?\""
  "confirmDestructiveAction(\"Mutate remote runner?\""
  "confirmDestructiveAction(\"Release remote attachment?\""
  "confirmDestructiveAction(\`Set remote lease to"
  "confirmDestructiveAction(\"Reclaim stale remote computers?\""
  "confirmDestructiveAction(\"Release remote state lock?\""
  "confirmDestructiveAction(\`Set MCP server status to"
  "confirmDestructiveAction(\`Set provider status to"
)

for pattern in "${destructive_confirm_patterns[@]}"; do
  require_text "$APP_FILE" "$pattern"
done

echo "static UI asset verification ok"
