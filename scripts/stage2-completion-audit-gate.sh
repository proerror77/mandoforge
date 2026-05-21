#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-stage2-completion-audit-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-${STAGE2_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}}"
AUDIT_DIR="${AUDIT_DIR:-.mandoforge/stage2-completion-audit}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
TEAM_ID="${MANDOFORGE_STAGE2_TEAM_ID:-}"
CONTROLLER_ENV_TEMPLATE="${CONTROLLER_ENV_TEMPLATE:-deploy/stage2-evidence/stage2-production-controllers.env.example}"
CONTROLLER_SECRET_TEMPLATE="${CONTROLLER_SECRET_TEMPLATE:-deploy/stage2-evidence/stage2-controller-env-secret.example.yaml}"
MAX_EVIDENCE_AGE_HOURS="${STAGE2_EVIDENCE_MAX_AGE_HOURS:-24}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "stage2 completion audit gate requires $1" >&2
    exit 1
  fi
}

auth_headers=(
  -H "x-mandoforge-subject: $SUBJECT"
  -H "x-mandoforge-roles: $ROLES"
)

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

discover_team_id() {
  if [[ -n "$TEAM_ID" ]]; then
    return 0
  fi

  local discovery_file="$SOURCE_EVIDENCE_DIR/team-discovery.json"
  if [[ -s "$discovery_file" ]]; then
    TEAM_ID="$(jq -r '.team_id // empty' "$discovery_file")"
    if [[ -n "$TEAM_ID" ]]; then
      return 0
    fi
  fi

  local organizations_body="$tmp_dir/organizations.json"
  local http_status
  http_status="$(curl -sS -o "$organizations_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL/api/organizations")"
  if [[ "$http_status" != 2* ]]; then
    echo "stage2 completion audit gate could not auto-discover teams: /api/organizations returned HTTP $http_status" >&2
    return 0
  fi

  local organization_id
  while IFS= read -r organization_id; do
    [[ -z "$organization_id" ]] && continue
    local teams_body="$tmp_dir/teams-$organization_id.json"
    http_status="$(curl -sS -o "$teams_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL/api/organizations/$organization_id/teams")"
    if [[ "$http_status" != 2* ]]; then
      continue
    fi
    TEAM_ID="$(jq -r 'map(select((.archived_at // null) == null)) | .[0].id // empty' "$teams_body")"
    if [[ -n "$TEAM_ID" ]]; then
      return 0
    fi
  done < <(jq -r 'map(select((.archived_at // null) == null)) | .[].id' "$organizations_body")
}

resolve_endpoint() {
  local endpoint="$1"
  if [[ "$endpoint" == ./* ]]; then
    return 1
  fi
  if [[ "$endpoint" == *"{team_id}"* ]]; then
    if [[ -z "$TEAM_ID" ]]; then
      return 1
    fi
    endpoint="${endpoint//\{team_id\}/$TEAM_ID}"
  fi
  printf '%s\n' "$endpoint"
}

local_script_artifact_path() {
  local endpoint="$1"
  endpoint="${endpoint#./}"
  printf '%s/local-script-%s.json\n' "$SOURCE_EVIDENCE_DIR" "$(slugify "$endpoint")"
}

required_evidence_artifacts_for_requirement() {
  local req_id="$1"
  case "$req_id" in
    tenant-routing)
      echo "$SOURCE_EVIDENCE_DIR/production-evidence-run.json"
      echo "$SOURCE_EVIDENCE_DIR/api-tenant-isolation-routing-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/tenant-routing-validation-evidence.json"
      ;;
    policy-rollout)
      echo "$SOURCE_EVIDENCE_DIR/production-evidence-run.json"
      echo "$SOURCE_EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/policy-rollout-due-run-evidence.json"
      ;;
    provider-rollout)
      echo "$SOURCE_EVIDENCE_DIR/api-providers-policy-gate-run.json"
      echo "$SOURCE_EVIDENCE_DIR/api-providers-deployment-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/provider-production-rollout-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/provider-production-rollback-evidence.json"
      ;;
    vault-kms)
      echo "$SOURCE_EVIDENCE_DIR/production-evidence-run.json"
      echo "$SOURCE_EVIDENCE_DIR/vault-kms-recovery-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/vault-kms-rotation-evidence.json"
      ;;
    worker-remote-computer)
      echo "$SOURCE_EVIDENCE_DIR/production-evidence-run.json"
      echo "$SOURCE_EVIDENCE_DIR/worker-load-validation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/remote-computer-state-sync-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"
      ;;
    approval-notifications)
      echo "$SOURCE_EVIDENCE_DIR/api-approvals-notifications-deployment-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/api-approvals-notifications-ops-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/approval-notification-delivery-evidence.json"
      ;;
    mcp-rollout)
      echo "$SOURCE_EVIDENCE_DIR/team-discovery.json"
      echo "$SOURCE_EVIDENCE_DIR/mcp-deployment-validation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/mcp-rollout-due-run-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/mcp-rollback-evidence.json"
      ;;
    codex-app-server)
      echo "$SOURCE_EVIDENCE_DIR/api-codex-app-server-deployment-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/api-codex-app-server-ops-validate.json"
      echo "$SOURCE_EVIDENCE_DIR/codex-app-server-stale-poll-evidence.json"
      ;;
    eval-release)
      echo "$SOURCE_EVIDENCE_DIR/eval-release-deployment-validation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/eval-release-orchestration-validation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/eval-release-stage2-regression-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/eval-release-due-run-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/eval-release-rollback-evidence.json"
      ;;
    observability-collector)
      echo "$SOURCE_EVIDENCE_DIR/observability-collector-deployment-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/observability-collector-cluster-rollout-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/observability-collector-remediation-evidence.json"
      ;;
    finance-close)
      echo "$SOURCE_EVIDENCE_DIR/production-evidence-run.json"
      echo "$SOURCE_EVIDENCE_DIR/finance-close-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/finance-reconciliation-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/usage-export-csv-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/finance-export-delivery-evidence.json"
      echo "$SOURCE_EVIDENCE_DIR/finance-export-delivery-observer.json"
      ;;
    ui-production-crud)
      echo "$SOURCE_EVIDENCE_DIR/local-script-scripts-verify-static-ui-actionbook.sh.json"
      echo "$SOURCE_EVIDENCE_DIR/local-script-scripts-verify-static-ui-assets.sh.json"
      ;;
  esac
}

json_array_from_file() {
  local path="$1"
  jq -R -s 'split("\n") | map(select(length > 0))' "$path"
}

file_mtime_epoch() {
  local path="$1"
  if stat -f %m "$path" >/dev/null 2>&1; then
    stat -f %m "$path"
    return 0
  fi
  stat -c %Y "$path"
}

artifact_is_fresh() {
  local path="$1"
  local now_epoch
  local mtime_epoch
  local max_age_seconds

  [[ -s "$path" ]] || return 1
  if [[ "$MAX_EVIDENCE_AGE_HOURS" == "0" ]]; then
    return 0
  fi

  now_epoch="$(date -u +%s)"
  mtime_epoch="$(file_mtime_epoch "$path")"
  max_age_seconds=$((MAX_EVIDENCE_AGE_HOURS * 3600))
  [[ $((now_epoch - mtime_epoch)) -le "$max_age_seconds" ]]
}

is_production_policy_controller_kind() {
  local value="$1"
  case "$value" in
    production_policy_controller|enterprise_policy_controller|external_policy_controller|policy_controller_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_policy_environment() {
  local value="$1"
  case "$value" in
    production|prod)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_policy_rollout_scope() {
  local value="$1"
  case "$value" in
    production|global|enterprise|multi_tenant)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

normalize_kms_kind() {
  printf '%s' "$1" | tr '[:upper:]-' '[:lower:]_'
}

is_production_kms_backend_kind() {
  local value
  value="$(normalize_kms_kind "$1")"
  case "$value" in
    external_kms|aws_kms|gcp_kms|azure_key_vault|hashicorp_vault_transit|vault_transit|hsm|cloudhsm|pkcs11_hsm)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_kms_environment() {
  local value="$1"
  case "$value" in
    production|prod)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

is_finance_system_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  is_production_identity "$value" || return 1
  [[ ! "$value" =~ (^|[./:_-])(feishu|lark|drive|file|artifact)([./:_-]|$) ]] || return 1
}

is_real_cluster_kind() {
  local value="$1"
  case "$value" in
    k8s_cluster|kubernetes_cluster|production_cluster|real_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_distributed_state_backend() {
  local value="$1"
  case "$value" in
    juicefs|cephfs|longhorn-rwx)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

artifact_contract_issue() {
  local req_id="$1"
  local artifact="$2"
  local artifact_name="${artifact##*/}"

  if [[ "$artifact_name" == "production-evidence-run.json" ]]; then
    local expected_cluster_id
    local expected_tenant_deployment_id
    local expected_policy_controller_id
    local expected_kms_backend_id
    local expected_kms_key_id
    local expected_finance_system_id

    expected_cluster_id="$(jq -r '.expected_targets.worker_remote_computer.cluster_id // ""' "$artifact" 2>/dev/null || echo "")"
    expected_tenant_deployment_id="$(jq -r '.expected_targets.tenant_routing.deployment_id // ""' "$artifact" 2>/dev/null || echo "")"
    expected_policy_controller_id="$(jq -r '.expected_targets.policy_rollout.controller_id // ""' "$artifact" 2>/dev/null || echo "")"
    expected_kms_backend_id="$(jq -r '.expected_targets.vault_kms.backend_id // ""' "$artifact" 2>/dev/null || echo "")"
    expected_kms_key_id="$(jq -r '.expected_targets.vault_kms.key_id // ""' "$artifact" 2>/dev/null || echo "")"
    expected_finance_system_id="$(jq -r '.expected_targets.finance.system_id // ""' "$artifact" 2>/dev/null || echo "")"

    if [[ "$req_id" == "worker-remote-computer" ]] && ! is_production_identity "$expected_cluster_id"; then
      printf 'expected worker/Remote Computer cluster id=%s is pilot/mock/local' "${expected_cluster_id:-<empty>}"
      return 0
    fi
    if [[ "$req_id" == "tenant-routing" ]] && ! is_production_identity "$expected_tenant_deployment_id"; then
      printf 'expected tenant deployment id=%s is pilot/mock/local' "${expected_tenant_deployment_id:-<empty>}"
      return 0
    fi
    if [[ "$req_id" == "policy-rollout" ]] && ! is_production_identity "$expected_policy_controller_id"; then
      printf 'expected policy controller id=%s is pilot/mock/local' "${expected_policy_controller_id:-<empty>}"
      return 0
    fi
    if [[ "$req_id" == "vault-kms" ]] && { ! is_production_identity "$expected_kms_backend_id" || ! is_production_identity "$expected_kms_key_id"; }; then
      printf 'expected KMS backend or key id is pilot/mock/local'
      return 0
    fi
    if [[ "$req_id" == "finance-close" ]] && ! is_finance_system_identity "$expected_finance_system_id"; then
      printf 'expected finance system id=%s is not a true ERP/accounting system identity' "${expected_finance_system_id:-<empty>}"
      return 0
    fi
  fi

  if [[ "$req_id" == "finance-close" && "$artifact_name" == "finance-export-delivery-observer.json" ]]; then
    local observer_status
    local delivery_mode
    local delivery_count
    local system_id

    observer_status="$(jq -r '.status // "unknown"' "$artifact" 2>/dev/null || echo "invalid_json")"
    delivery_mode="$(jq -r '.export_state.delivery_mode // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    delivery_count="$(jq -r '.export_state.delivery_count // 0' "$artifact" 2>/dev/null || echo "0")"
    system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$artifact" 2>/dev/null || echo "")"

    if [[ "$observer_status" != "ok" ]]; then
      printf 'observer_status=%s' "$observer_status"
      return 0
    fi
    if [[ ! "$delivery_count" =~ ^[0-9]+$ || "$delivery_count" == "0" ]]; then
      printf 'delivery_count=%s' "$delivery_count"
      return 0
    fi
    case "$delivery_mode" in
      accounting*|erp*|netsuite|quickbooks|xero|sap|oracle_erp)
        ;;
      *)
        printf 'delivery_mode=%s is not accounting/ERP' "$delivery_mode"
        return 0
        ;;
    esac
    if [[ -z "$system_id" ]]; then
      printf 'system_id is missing'
      return 0
    fi
    if ! is_finance_system_identity "$system_id"; then
      printf 'system_id=%s is not a true ERP/accounting system identity' "$system_id"
      return 0
    fi
  fi

  if [[ "$req_id" == "managed-session-restart-resume" && "$artifact_name" == "managed-session-restart-resume-evidence.json" ]]; then
    local status
    local target_id
    local target_kind
    local enqueue_event_persisted
    local worker_drain_observed
    local api_restarted
    local worker_restarted
    local session_state_resumed
    local processed_event_seq_preserved
    local thread_lineage_preserved
    local finalization_fenced
    local stale_worker_rejected
    local runtime_turn_completed
    local final_message_preserved

    status="$(jq -r '.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    target_id="$(jq -r '.target.id // .target.cluster_id // .target.deployment_id // ""' "$artifact" 2>/dev/null || echo "")"
    target_kind="$(jq -r '.target.kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    enqueue_event_persisted="$(jq -r '.session_loop.enqueue_event_persisted // false' "$artifact" 2>/dev/null || echo "false")"
    worker_drain_observed="$(jq -r '.session_loop.worker_drain_observed // false' "$artifact" 2>/dev/null || echo "false")"
    api_restarted="$(jq -r '.restart.api_restarted // false' "$artifact" 2>/dev/null || echo "false")"
    worker_restarted="$(jq -r '.restart.worker_restarted // false' "$artifact" 2>/dev/null || echo "false")"
    session_state_resumed="$(jq -r '.resume.session_state_resumed // false' "$artifact" 2>/dev/null || echo "false")"
    processed_event_seq_preserved="$(jq -r '.resume.processed_event_seq_preserved // false' "$artifact" 2>/dev/null || echo "false")"
    thread_lineage_preserved="$(jq -r '.thread_lineage.preserved // false' "$artifact" 2>/dev/null || echo "false")"
    finalization_fenced="$(jq -r '.lease_fencing.finalization_fenced // false' "$artifact" 2>/dev/null || echo "false")"
    stale_worker_rejected="$(jq -r '.lease_fencing.stale_worker_rejected // false' "$artifact" 2>/dev/null || echo "false")"
    runtime_turn_completed="$(jq -r '.runtime_turn.completed // false' "$artifact" 2>/dev/null || echo "false")"
    final_message_preserved="$(jq -r '.runtime_turn.final_message_preserved // false' "$artifact" 2>/dev/null || echo "false")"

    if ! [[ "$status" == "validated" || "$status" == "completed" || "$status" == "ready" ]]; then
      printf 'status=%s' "$status"
      return 0
    fi
    if ! is_production_identity "$target_id"; then
      printf 'target_id=%s is pilot/mock/local' "${target_id:-<empty>}"
      return 0
    fi
    case "$target_kind" in
      managed_session_runtime|production_runtime_cluster|managed_agent_cluster)
        ;;
      *)
        printf 'target_kind=%s is not managed-session production runtime' "$target_kind"
        return 0
        ;;
    esac
    if [[ "$enqueue_event_persisted" != "true" || "$worker_drain_observed" != "true" ]]; then
      printf 'session-loop enqueue/drain evidence incomplete'
      return 0
    fi
    if [[ "$api_restarted" != "true" || "$worker_restarted" != "true" ]]; then
      printf 'API/worker restart evidence incomplete'
      return 0
    fi
    if [[ "$session_state_resumed" != "true" || "$processed_event_seq_preserved" != "true" ]]; then
      printf 'session resume or processed cursor evidence incomplete'
      return 0
    fi
    if [[ "$thread_lineage_preserved" != "true" ]]; then
      printf 'thread lineage evidence incomplete'
      return 0
    fi
    if [[ "$finalization_fenced" != "true" || "$stale_worker_rejected" != "true" ]]; then
      printf 'lease fencing evidence incomplete'
      return 0
    fi
    if [[ "$runtime_turn_completed" != "true" || "$final_message_preserved" != "true" ]]; then
      printf 'runtime turn finalization evidence incomplete'
      return 0
    fi
  fi

  if [[ "$req_id" == "worker-remote-computer" && "$artifact_name" == "worker-load-validation-evidence.json" ]]; then
    local target_kind
    local node_count
    local cluster_id

    target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    node_count="$(jq -r '.response.controller_execution.node_count // 0' "$artifact" 2>/dev/null || echo "0")"
    cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$artifact" 2>/dev/null || echo "")"

    if ! is_real_cluster_kind "$target_kind"; then
      printf 'target_kind=%s is not a real cluster' "$target_kind"
      return 0
    fi
    if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
      printf 'node_count=%s is not multi-node' "$node_count"
      return 0
    fi
    if ! is_production_identity "$cluster_id"; then
      printf 'cluster_id=%s is pilot/mock/local' "${cluster_id:-<empty>}"
      return 0
    fi
  fi

  if [[ "$req_id" == "worker-remote-computer" && "$artifact_name" == "remote-computer-state-sync-evidence.json" ]]; then
    local target_kind
    local node_count
    local state_backend
    local cluster_id

    target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    node_count="$(jq -r '.response.controller_execution.node_count // 0' "$artifact" 2>/dev/null || echo "0")"
    state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$artifact" 2>/dev/null || echo "")"

    if ! is_real_cluster_kind "$target_kind"; then
      printf 'target_kind=%s is not a real cluster' "$target_kind"
      return 0
    fi
    if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
      printf 'node_count=%s is not multi-node' "$node_count"
      return 0
    fi
    if ! is_distributed_state_backend "$state_backend"; then
      printf 'state_backend=%s is not distributed' "$state_backend"
      return 0
    fi
    if ! is_production_identity "$cluster_id"; then
      printf 'cluster_id=%s is pilot/mock/local' "${cluster_id:-<empty>}"
      return 0
    fi
  fi

  if [[ "$req_id" == "worker-remote-computer" && "$artifact_name" == "remote-computer-sidecar-recovery-evidence.json" ]]; then
    local validation_status
    local target_kind
    local node_count
    local replacement_scope
    local cluster_id

    validation_status="$(jq -r '.response.validation_result.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    node_count="$(jq -r '.response.validation_result.node_count // 0' "$artifact" 2>/dev/null || echo "0")"
    replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$artifact" 2>/dev/null || echo "")"

    if [[ "$validation_status" != "validated" ]]; then
      printf 'validation_status=%s' "$validation_status"
      return 0
    fi
    if ! is_real_cluster_kind "$target_kind"; then
      printf 'target_kind=%s is not a real cluster' "$target_kind"
      return 0
    fi
    if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
      printf 'node_count=%s is not multi-node' "$node_count"
      return 0
    fi
    if [[ "$replacement_scope" != "cluster" ]]; then
      printf 'replacement_scope=%s is not cluster' "$replacement_scope"
      return 0
    fi
    if ! is_production_identity "$cluster_id"; then
      printf 'cluster_id=%s is pilot/mock/local' "${cluster_id:-<empty>}"
      return 0
    fi
  fi

  if [[ "$req_id" == "policy-rollout" && "$artifact_name" == "policy-rollout-orchestration-validation-evidence.json" ]]; then
    local validation_status
    local controller_status
    local target_kind
    local environment
    local controller_id
    local rollout_scope
    local production_policy_store
    local rollback_supported

    validation_status="$(jq -r '.response.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$artifact" 2>/dev/null || echo "")"
    rollout_scope="$(jq -r '.response.controller_execution.rollout_scope // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    production_policy_store="$(jq -r '.response.controller_execution.production_policy_store // false' "$artifact" 2>/dev/null || echo "false")"
    rollback_supported="$(jq -r '.response.controller_execution.rollback_supported // false' "$artifact" 2>/dev/null || echo "false")"

    if [[ "$validation_status" != "validated" ]]; then
      printf 'validation_status=%s' "$validation_status"
      return 0
    fi
    if [[ "$controller_status" != "validated" ]]; then
      printf 'controller_status=%s' "$controller_status"
      return 0
    fi
    if ! is_production_policy_controller_kind "$target_kind"; then
      printf 'target_kind=%s is not production policy controller' "$target_kind"
      return 0
    fi
    if ! is_production_policy_environment "$environment"; then
      printf 'environment=%s is not production' "$environment"
      return 0
    fi
    if [[ -z "$controller_id" ]]; then
      printf 'controller_id is missing'
      return 0
    fi
    if ! is_production_identity "$controller_id"; then
      printf 'controller_id=%s is pilot/mock/local' "$controller_id"
      return 0
    fi
    if ! is_production_policy_rollout_scope "$rollout_scope"; then
      printf 'rollout_scope=%s is not production-grade' "$rollout_scope"
      return 0
    fi
    if [[ "$production_policy_store" != "true" ]]; then
      printf 'production_policy_store=%s' "$production_policy_store"
      return 0
    fi
    if [[ "$rollback_supported" != "true" ]]; then
      printf 'rollback_supported=%s' "$rollback_supported"
      return 0
    fi
  fi

  if [[ "$req_id" == "tenant-routing" && "$artifact_name" == "tenant-routing-validation-evidence.json" ]]; then
    local target_kind
    local tenant_count
    local rls_enforced
    local tenant_context_validated
    local cross_tenant_negative_tests
    local deployment_id

    target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    tenant_count="$(jq -r '.response.controller_execution.tenant_count // 0' "$artifact" 2>/dev/null || echo "0")"
    rls_enforced="$(jq -r '.response.controller_execution.rls_enforced // false' "$artifact" 2>/dev/null || echo "false")"
    tenant_context_validated="$(jq -r '.response.controller_execution.tenant_context_validated // false' "$artifact" 2>/dev/null || echo "false")"
    cross_tenant_negative_tests="$(jq -r '.response.controller_execution.cross_tenant_negative_tests // false' "$artifact" 2>/dev/null || echo "false")"
    deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$artifact" 2>/dev/null || echo "")"

    case "$target_kind" in
      multi_tenant_deployment|enterprise_multi_tenant|production_multi_tenant)
        ;;
      *)
        printf 'target_kind=%s is not broader multi-tenant' "$target_kind"
        return 0
        ;;
    esac
    if [[ ! "$tenant_count" =~ ^[0-9]+$ || "$tenant_count" -lt 2 ]]; then
      printf 'tenant_count=%s is not multi-tenant' "$tenant_count"
      return 0
    fi
    if [[ "$rls_enforced" != "true" ]]; then
      printf 'rls_enforced=%s' "$rls_enforced"
      return 0
    fi
    if [[ "$tenant_context_validated" != "true" ]]; then
      printf 'tenant_context_validated=%s' "$tenant_context_validated"
      return 0
    fi
    if [[ "$cross_tenant_negative_tests" != "true" ]]; then
      printf 'cross_tenant_negative_tests=%s' "$cross_tenant_negative_tests"
      return 0
    fi
    if ! is_production_identity "$deployment_id"; then
      printf 'deployment_id=%s is pilot/mock/local' "${deployment_id:-<empty>}"
      return 0
    fi
  fi

  if [[ "$req_id" == "vault-kms" && "$artifact_name" == "vault-kms-rotation-evidence.json" ]]; then
    local rotation_status
    local execution_status
    local production_backend
    local backend_kind
    local environment
    local backend_id
    local key_id

    rotation_status="$(jq -r '.response.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    execution_status="$(jq -r '.response.external_execution.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    production_backend="$(jq -r '.response.external_execution.production_backend // false' "$artifact" 2>/dev/null || echo "false")"
    backend_kind="$(jq -r '.response.external_execution.backend_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    environment="$(jq -r '.response.external_execution.environment // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$artifact" 2>/dev/null || echo "")"
    key_id="$(jq -r '.response.external_execution.key_id // ""' "$artifact" 2>/dev/null || echo "")"

    if [[ "$rotation_status" != "validated" ]]; then
      printf 'rotation_status=%s' "$rotation_status"
      return 0
    fi
    if [[ "$execution_status" != "validated" ]]; then
      printf 'external_execution_status=%s' "$execution_status"
      return 0
    fi
    if [[ "$production_backend" != "true" ]]; then
      printf 'production_backend=%s' "$production_backend"
      return 0
    fi
    if ! is_production_kms_backend_kind "$backend_kind"; then
      printf 'backend_kind=%s is not production KMS/HSM' "$backend_kind"
      return 0
    fi
    if ! is_production_kms_environment "$environment"; then
      printf 'environment=%s is not production' "$environment"
      return 0
    fi
    if [[ -z "$backend_id" || -z "$key_id" ]]; then
      printf 'backend_id or key_id is missing'
      return 0
    fi
    if ! is_production_identity "$backend_id" || ! is_production_identity "$key_id"; then
      printf 'backend_id or key_id is pilot/mock/local'
      return 0
    fi
  fi

  if [[ "$req_id" == "vault-kms" && "$artifact_name" == "vault-kms-recovery-evidence.json" ]]; then
    local recovery_status
    local controller_status
    local backend_kind
    local environment
    local backend_id
    local key_id

    recovery_status="$(jq -r '.response.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    backend_kind="$(jq -r '.response.controller_execution.backend_kind // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$artifact" 2>/dev/null || echo "unknown")"
    backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$artifact" 2>/dev/null || echo "")"
    key_id="$(jq -r '.response.controller_execution.key_id // ""' "$artifact" 2>/dev/null || echo "")"

    if [[ "$recovery_status" != "validated" ]]; then
      printf 'recovery_status=%s' "$recovery_status"
      return 0
    fi
    if [[ "$controller_status" != "validated" ]]; then
      printf 'controller_status=%s' "$controller_status"
      return 0
    fi
    if ! is_production_kms_backend_kind "$backend_kind"; then
      printf 'backend_kind=%s is not production KMS/HSM' "$backend_kind"
      return 0
    fi
    if ! is_production_kms_environment "$environment"; then
      printf 'environment=%s is not production' "$environment"
      return 0
    fi
    if [[ -z "$backend_id" || -z "$key_id" ]]; then
      printf 'backend_id or key_id is missing'
      return 0
    fi
    if ! is_production_identity "$backend_id" || ! is_production_identity "$key_id"; then
      printf 'backend_id or key_id is pilot/mock/local'
      return 0
    fi
  fi
}

requirement_cross_artifact_issue() {
  local req_id="$1"
  local run_manifest="$SOURCE_EVIDENCE_DIR/production-evidence-run.json"

  if [[ "$req_id" == "worker-remote-computer" ]]; then
    local worker_artifact="$SOURCE_EVIDENCE_DIR/worker-load-validation-evidence.json"
    local state_artifact="$SOURCE_EVIDENCE_DIR/remote-computer-state-sync-evidence.json"
    local sidecar_artifact="$SOURCE_EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"
    local expected_cluster_id
    local worker_cluster_id
    local state_cluster_id
    local sidecar_cluster_id

    expected_cluster_id="$(jq -r '.expected_targets.worker_remote_computer.cluster_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$worker_artifact" 2>/dev/null || echo "")"
    state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$state_artifact" 2>/dev/null || echo "")"
    sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$sidecar_artifact" 2>/dev/null || echo "")"

    if [[ -n "$expected_cluster_id" && -n "$worker_cluster_id" && "$expected_cluster_id" != "$worker_cluster_id" ]]; then
      printf 'worker cluster id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$expected_cluster_id" && -n "$state_cluster_id" && "$expected_cluster_id" != "$state_cluster_id" ]]; then
      printf 'state-sync cluster id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$expected_cluster_id" && -n "$sidecar_cluster_id" && "$expected_cluster_id" != "$sidecar_cluster_id" ]]; then
      printf 'sidecar cluster id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$worker_cluster_id" && -n "$state_cluster_id" && -n "$sidecar_cluster_id" ]]; then
      if ! [[ "$worker_cluster_id" == "$state_cluster_id" && "$worker_cluster_id" == "$sidecar_cluster_id" ]]; then
        printf 'worker, state-sync, and sidecar evidence do not share one cluster id'
        return 0
      fi
    fi
  fi

  if [[ "$req_id" == "tenant-routing" ]]; then
    local expected_deployment_id
    local actual_deployment_id
    expected_deployment_id="$(jq -r '.expected_targets.tenant_routing.deployment_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    actual_deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$SOURCE_EVIDENCE_DIR/tenant-routing-validation-evidence.json" 2>/dev/null || echo "")"
    if [[ -n "$expected_deployment_id" && -n "$actual_deployment_id" && "$expected_deployment_id" != "$actual_deployment_id" ]]; then
      printf 'tenant routing deployment id does not match production-evidence-run.json'
      return 0
    fi
  fi

  if [[ "$req_id" == "policy-rollout" ]]; then
    local expected_controller_id
    local actual_controller_id
    expected_controller_id="$(jq -r '.expected_targets.policy_rollout.controller_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    actual_controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$SOURCE_EVIDENCE_DIR/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
    if [[ -n "$expected_controller_id" && -n "$actual_controller_id" && "$expected_controller_id" != "$actual_controller_id" ]]; then
      printf 'policy rollout controller id does not match production-evidence-run.json'
      return 0
    fi
  fi

  if [[ "$req_id" == "vault-kms" ]]; then
    local expected_backend_id
    local expected_key_id
    local rotation_backend_id
    local rotation_key_id
    local recovery_backend_id
    local recovery_key_id
    expected_backend_id="$(jq -r '.expected_targets.vault_kms.backend_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    expected_key_id="$(jq -r '.expected_targets.vault_kms.key_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    rotation_backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$SOURCE_EVIDENCE_DIR/vault-kms-rotation-evidence.json" 2>/dev/null || echo "")"
    rotation_key_id="$(jq -r '.response.external_execution.key_id // ""' "$SOURCE_EVIDENCE_DIR/vault-kms-rotation-evidence.json" 2>/dev/null || echo "")"
    recovery_backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$SOURCE_EVIDENCE_DIR/vault-kms-recovery-evidence.json" 2>/dev/null || echo "")"
    recovery_key_id="$(jq -r '.response.controller_execution.key_id // ""' "$SOURCE_EVIDENCE_DIR/vault-kms-recovery-evidence.json" 2>/dev/null || echo "")"
    if [[ -n "$expected_backend_id" && -n "$rotation_backend_id" && "$expected_backend_id" != "$rotation_backend_id" ]]; then
      printf 'KMS rotation backend id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$expected_key_id" && -n "$rotation_key_id" && "$expected_key_id" != "$rotation_key_id" ]]; then
      printf 'KMS rotation key id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$expected_backend_id" && -n "$recovery_backend_id" && "$expected_backend_id" != "$recovery_backend_id" ]]; then
      printf 'KMS recovery backend id does not match production-evidence-run.json'
      return 0
    fi
    if [[ -n "$expected_key_id" && -n "$recovery_key_id" && "$expected_key_id" != "$recovery_key_id" ]]; then
      printf 'KMS recovery key id does not match production-evidence-run.json'
      return 0
    fi
  fi

  if [[ "$req_id" == "finance-close" ]]; then
    local expected_finance_system_id
    local actual_finance_system_id
    expected_finance_system_id="$(jq -r '.expected_targets.finance.system_id // ""' "$run_manifest" 2>/dev/null || echo "")"
    actual_finance_system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$SOURCE_EVIDENCE_DIR/finance-export-delivery-observer.json" 2>/dev/null || echo "")"
    if [[ -n "$expected_finance_system_id" && -n "$actual_finance_system_id" && "$expected_finance_system_id" != "$actual_finance_system_id" ]]; then
      printf 'finance ERP system id does not match production-evidence-run.json'
      return 0
    fi
  fi
}

require_cmd curl
require_cmd jq
require_cmd base64

mkdir -p "$AUDIT_DIR"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

health_body="$(mktemp)"
if ! health_status="$(curl -sS -o "$health_body" -w "%{http_code}" "$BASE_URL/healthz")"; then
  echo "stage2 completion audit gate could not reach BASE_URL health endpoint: $BASE_URL/healthz" >&2
  rm -f "$health_body"
  exit 1
fi
if [[ "$health_status" != 2* ]]; then
  echo "stage2 completion audit gate could not verify MandoForge API health at $BASE_URL/healthz: HTTP $health_status" >&2
  sed -n '1,40p' "$health_body" >&2
  rm -f "$health_body"
  exit 1
fi
rm -f "$health_body"

readiness_file="$AUDIT_DIR/api-stage2-readiness.json"
response_body="$(mktemp)"
http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL/api/stage2/readiness")"
if [[ "$http_status" != 2* ]]; then
  echo "stage2 completion audit gate could not fetch $BASE_URL/api/stage2/readiness: HTTP $http_status" >&2
  sed -n '1,40p' "$response_body" >&2
  rm -f "$response_body"
  exit 1
fi
tee "$readiness_file" <"$response_body" >/dev/null
rm -f "$response_body"

discover_team_id

status="$(jq -r '.status // "unknown"' "$readiness_file")"
objective="$(jq -r '.objective // ""' "$readiness_file")"
completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$readiness_file")"
open_gap_count="$(jq -r '.open_gap_count // 0' "$readiness_file")"
requirement_count="$(jq -r '.evidence_requirements | length' "$readiness_file")"

requirements_jsonl="$tmp_dir/requirements.jsonl"
: >"$requirements_jsonl"

total_missing_readiness=0
total_missing_validation=0
total_missing_required_evidence=0
total_stale_readiness=0
total_stale_validation=0
total_stale_required_evidence=0
total_unresolved=0
total_missing_evidence_scripts=0
total_missing_evidence_job_manifests=0
total_missing_required_flags=0

while IFS= read -r encoded; do
  req_json="$(printf '%s' "$encoded" | base64 -d)"
  req_id="$(jq -r '.id' <<<"$req_json")"
  req_title="$(jq -r '.title' <<<"$req_json")"
  req_gap="$(jq -r '.gap' <<<"$req_json")"
  production_target="$(jq -r '.production_target' <<<"$req_json")"

  readiness_declared="$tmp_dir/$req_id.readiness-declared"
  validation_declared="$tmp_dir/$req_id.validation-declared"
  readiness_artifacts="$tmp_dir/$req_id.readiness-artifacts"
  validation_artifacts="$tmp_dir/$req_id.validation-artifacts"
  stale_readiness_artifacts="$tmp_dir/$req_id.stale-readiness-artifacts"
  stale_validation_artifacts="$tmp_dir/$req_id.stale-validation-artifacts"
  missing_readiness="$tmp_dir/$req_id.missing-readiness"
  missing_validation="$tmp_dir/$req_id.missing-validation"
  unresolved_endpoints="$tmp_dir/$req_id.unresolved"
  required_evidence="$tmp_dir/$req_id.required-evidence"
  evidence_scripts="$tmp_dir/$req_id.evidence-scripts"
  evidence_job_manifests="$tmp_dir/$req_id.evidence-job-manifests"
  required_flags="$tmp_dir/$req_id.required-flags"
  missing_evidence_scripts="$tmp_dir/$req_id.missing-evidence-scripts"
  missing_evidence_job_manifests="$tmp_dir/$req_id.missing-evidence-job-manifests"
  missing_required_flags="$tmp_dir/$req_id.missing-required-flags"
  required_evidence_artifacts="$tmp_dir/$req_id.required-evidence-artifacts"
  present_required_evidence_artifacts="$tmp_dir/$req_id.present-required-evidence-artifacts"
  missing_required_evidence_artifacts="$tmp_dir/$req_id.missing-required-evidence-artifacts"
  stale_required_evidence_artifacts="$tmp_dir/$req_id.stale-required-evidence-artifacts"

  : >"$readiness_declared"
  : >"$validation_declared"
  : >"$readiness_artifacts"
  : >"$validation_artifacts"
  : >"$stale_readiness_artifacts"
  : >"$stale_validation_artifacts"
  : >"$missing_readiness"
  : >"$missing_validation"
  : >"$unresolved_endpoints"
  : >"$evidence_scripts"
  : >"$evidence_job_manifests"
  : >"$required_flags"
  : >"$missing_evidence_scripts"
  : >"$missing_evidence_job_manifests"
  : >"$missing_required_flags"
  : >"$required_evidence_artifacts"
  : >"$present_required_evidence_artifacts"
  : >"$missing_required_evidence_artifacts"
  : >"$stale_required_evidence_artifacts"

  jq -r '.required_evidence[]?' <<<"$req_json" >"$required_evidence"
  jq -r '.evidence_scripts[]?' <<<"$req_json" >"$evidence_scripts"
  jq -r '.evidence_job_manifests[]?' <<<"$req_json" >"$evidence_job_manifests"
  jq -r '.required_flags[]?' <<<"$req_json" >"$required_flags"
  if jq -e '(.required_artifacts // []) | length > 0' <<<"$req_json" >/dev/null; then
    jq -r --arg evidence_dir "$SOURCE_EVIDENCE_DIR" \
      '(.required_artifacts // [])[] | $evidence_dir + "/" + .' \
      <<<"$req_json" >"$required_evidence_artifacts"
  else
    required_evidence_artifacts_for_requirement "$req_id" >"$required_evidence_artifacts"
  fi

  while IFS= read -r endpoint; do
    [[ -z "$endpoint" ]] && continue
    if [[ "$endpoint" == ./* ]]; then
      echo "$endpoint" >>"$readiness_declared"
      artifact="$(local_script_artifact_path "$endpoint")"
      if artifact_is_fresh "$artifact"; then
        echo "$artifact" >>"$readiness_artifacts"
      elif [[ -s "$artifact" ]]; then
        echo "$artifact" >>"$stale_readiness_artifacts"
        echo "$endpoint" >>"$missing_readiness"
      else
        echo "$endpoint" >>"$missing_readiness"
      fi
      continue
    fi
    if ! resolved="$(resolve_endpoint "$endpoint")"; then
      echo "$endpoint" >>"$unresolved_endpoints"
      echo "$endpoint" >>"$missing_readiness"
      continue
    fi
    echo "$resolved" >>"$readiness_declared"
    artifact="$SOURCE_EVIDENCE_DIR/$(slugify "$resolved").json"
    if artifact_is_fresh "$artifact"; then
      echo "$artifact" >>"$readiness_artifacts"
    elif [[ -s "$artifact" ]]; then
      echo "$artifact" >>"$stale_readiness_artifacts"
      echo "$resolved" >>"$missing_readiness"
    else
      echo "$resolved" >>"$missing_readiness"
    fi
  done < <(jq -r '.readiness_endpoints[]?' <<<"$req_json")

  while IFS= read -r endpoint; do
    [[ -z "$endpoint" ]] && continue
    if [[ "$endpoint" == ./* ]]; then
      echo "$endpoint" >>"$validation_declared"
      artifact="$(local_script_artifact_path "$endpoint")"
      if artifact_is_fresh "$artifact"; then
        echo "$artifact" >>"$validation_artifacts"
      elif [[ -s "$artifact" ]]; then
        echo "$artifact" >>"$stale_validation_artifacts"
        echo "$endpoint" >>"$missing_validation"
      else
        echo "$endpoint" >>"$missing_validation"
      fi
      continue
    fi
    if ! resolved="$(resolve_endpoint "$endpoint")"; then
      echo "$endpoint" >>"$unresolved_endpoints"
      echo "$endpoint" >>"$missing_validation"
      continue
    fi
    echo "$resolved" >>"$validation_declared"
    artifact="$SOURCE_EVIDENCE_DIR/$(slugify "$resolved").json"
    if artifact_is_fresh "$artifact"; then
      echo "$artifact" >>"$validation_artifacts"
    elif [[ -s "$artifact" ]]; then
      echo "$artifact" >>"$stale_validation_artifacts"
      echo "$resolved" >>"$missing_validation"
    else
      echo "$resolved" >>"$missing_validation"
    fi
  done < <(jq -r '.validation_endpoints[]?' <<<"$req_json")

  while IFS= read -r artifact; do
    [[ -z "$artifact" ]] && continue
    artifact_issue=""
    if [[ -s "$artifact" ]]; then
      artifact_issue="$(artifact_contract_issue "$req_id" "$artifact")"
    fi
    if artifact_is_fresh "$artifact" && [[ -z "$artifact_issue" ]]; then
      echo "$artifact" >>"$present_required_evidence_artifacts"
    elif [[ -s "$artifact" && -n "$artifact_issue" ]]; then
      echo "$artifact ($artifact_issue)" >>"$missing_required_evidence_artifacts"
    elif [[ -s "$artifact" ]]; then
      echo "$artifact" >>"$stale_required_evidence_artifacts"
      echo "$artifact" >>"$missing_required_evidence_artifacts"
    else
      echo "$artifact" >>"$missing_required_evidence_artifacts"
    fi
  done <"$required_evidence_artifacts"

  cross_artifact_issue="$(requirement_cross_artifact_issue "$req_id")"
  if [[ -n "$cross_artifact_issue" ]]; then
    echo "$req_id cross-artifact contract ($cross_artifact_issue)" >>"$missing_required_evidence_artifacts"
  fi

  while IFS= read -r script_path; do
    [[ -z "$script_path" ]] && continue
    if [[ ! -x "$script_path" ]]; then
      echo "$script_path" >>"$missing_evidence_scripts"
    fi
  done <"$evidence_scripts"

  while IFS= read -r manifest_path; do
    [[ -z "$manifest_path" ]] && continue
    if [[ ! -s "$manifest_path" ]]; then
      echo "$manifest_path" >>"$missing_evidence_job_manifests"
    fi
  done <"$evidence_job_manifests"

  while IFS= read -r required_flag; do
    [[ -z "$required_flag" ]] && continue
    flag_name="${required_flag%%=*}"
    missing_reasons=()
    if [[ ! -s "$CONTROLLER_ENV_TEMPLATE" ]] || ! grep -q "^${flag_name}=" "$CONTROLLER_ENV_TEMPLATE"; then
      missing_reasons+=("env-template")
    fi
    if [[ ! -s "$CONTROLLER_SECRET_TEMPLATE" ]] || ! grep -q "^[[:space:]]*${flag_name}:" "$CONTROLLER_SECRET_TEMPLATE"; then
      missing_reasons+=("secret-template")
    fi
    if [[ "${#missing_reasons[@]}" -gt 0 ]]; then
      printf '%s missing:%s\n' "$required_flag" "$(IFS=,; echo "${missing_reasons[*]}")" >>"$missing_required_flags"
    fi
  done <"$required_flags"

  missing_readiness_count="$(grep -c . "$missing_readiness" || true)"
  missing_validation_count="$(grep -c . "$missing_validation" || true)"
  missing_required_evidence_count="$(grep -c . "$missing_required_evidence_artifacts" || true)"
  stale_readiness_count="$(grep -c . "$stale_readiness_artifacts" || true)"
  stale_validation_count="$(grep -c . "$stale_validation_artifacts" || true)"
  stale_required_evidence_count="$(grep -c . "$stale_required_evidence_artifacts" || true)"
  unresolved_count="$(grep -c . "$unresolved_endpoints" || true)"
  missing_evidence_script_count="$(grep -c . "$missing_evidence_scripts" || true)"
  missing_evidence_job_manifest_count="$(grep -c . "$missing_evidence_job_manifests" || true)"
  missing_required_flag_count="$(grep -c . "$missing_required_flags" || true)"
  readiness_artifact_count="$(grep -c . "$readiness_artifacts" || true)"
  validation_artifact_count="$(grep -c . "$validation_artifacts" || true)"
  required_evidence_artifact_count="$(grep -c . "$present_required_evidence_artifacts" || true)"

  total_missing_readiness=$((total_missing_readiness + missing_readiness_count))
  total_missing_validation=$((total_missing_validation + missing_validation_count))
  total_missing_required_evidence=$((total_missing_required_evidence + missing_required_evidence_count))
  total_stale_readiness=$((total_stale_readiness + stale_readiness_count))
  total_stale_validation=$((total_stale_validation + stale_validation_count))
  total_stale_required_evidence=$((total_stale_required_evidence + stale_required_evidence_count))
  total_unresolved=$((total_unresolved + unresolved_count))
  total_missing_evidence_scripts=$((total_missing_evidence_scripts + missing_evidence_script_count))
  total_missing_evidence_job_manifests=$((total_missing_evidence_job_manifests + missing_evidence_job_manifest_count))
  total_missing_required_flags=$((total_missing_required_flags + missing_required_flag_count))

  req_status="blocked"
  if [[ "$completion_blocked" != "true" && "$missing_readiness_count" == "0" && "$missing_validation_count" == "0" && "$missing_required_evidence_count" == "0" && "$missing_evidence_script_count" == "0" && "$missing_evidence_job_manifest_count" == "0" && "$missing_required_flag_count" == "0" ]]; then
    req_status="ready"
  fi

  jq -n \
    --arg id "$req_id" \
    --arg title "$req_title" \
    --arg gap "$req_gap" \
    --arg production_target "$production_target" \
    --arg status "$req_status" \
    --argjson evidence_scripts "$(json_array_from_file "$evidence_scripts")" \
    --argjson evidence_job_manifests "$(json_array_from_file "$evidence_job_manifests")" \
    --argjson required_flags "$(json_array_from_file "$required_flags")" \
    --argjson readiness_endpoints "$(json_array_from_file "$readiness_declared")" \
    --argjson validation_endpoints "$(json_array_from_file "$validation_declared")" \
    --argjson required_evidence "$(json_array_from_file "$required_evidence")" \
    --argjson missing_evidence_scripts "$(json_array_from_file "$missing_evidence_scripts")" \
    --argjson missing_evidence_job_manifests "$(json_array_from_file "$missing_evidence_job_manifests")" \
    --argjson missing_required_flags "$(json_array_from_file "$missing_required_flags")" \
    --argjson readiness_artifacts "$(json_array_from_file "$readiness_artifacts")" \
    --argjson validation_artifacts "$(json_array_from_file "$validation_artifacts")" \
    --argjson required_evidence_artifacts "$(json_array_from_file "$required_evidence_artifacts")" \
    --argjson present_required_evidence_artifacts "$(json_array_from_file "$present_required_evidence_artifacts")" \
    --argjson missing_required_evidence_artifacts "$(json_array_from_file "$missing_required_evidence_artifacts")" \
    --argjson stale_readiness_artifacts "$(json_array_from_file "$stale_readiness_artifacts")" \
    --argjson stale_validation_artifacts "$(json_array_from_file "$stale_validation_artifacts")" \
    --argjson stale_required_evidence_artifacts "$(json_array_from_file "$stale_required_evidence_artifacts")" \
    --argjson missing_readiness_endpoints "$(json_array_from_file "$missing_readiness")" \
    --argjson missing_validation_endpoints "$(json_array_from_file "$missing_validation")" \
    --argjson unresolved_endpoints "$(json_array_from_file "$unresolved_endpoints")" \
    --argjson readiness_artifact_count "$readiness_artifact_count" \
    --argjson validation_artifact_count "$validation_artifact_count" \
    --argjson required_evidence_artifact_count "$required_evidence_artifact_count" \
    --argjson missing_evidence_script_count "$missing_evidence_script_count" \
    --argjson missing_evidence_job_manifest_count "$missing_evidence_job_manifest_count" \
    --argjson missing_required_flag_count "$missing_required_flag_count" \
    --argjson missing_readiness_count "$missing_readiness_count" \
    --argjson missing_validation_count "$missing_validation_count" \
    --argjson missing_required_evidence_count "$missing_required_evidence_count" \
    --argjson stale_readiness_count "$stale_readiness_count" \
    --argjson stale_validation_count "$stale_validation_count" \
    --argjson stale_required_evidence_count "$stale_required_evidence_count" \
    '{
      id: $id,
      title: $title,
      gap: $gap,
      production_target: $production_target,
      status: $status,
      evidence_scripts: $evidence_scripts,
      evidence_job_manifests: $evidence_job_manifests,
      required_flags: $required_flags,
      readiness_endpoints: $readiness_endpoints,
      validation_endpoints: $validation_endpoints,
      required_evidence: $required_evidence,
      missing_evidence_scripts: $missing_evidence_scripts,
      missing_evidence_job_manifests: $missing_evidence_job_manifests,
      missing_required_flags: $missing_required_flags,
      readiness_artifacts: $readiness_artifacts,
      validation_artifacts: $validation_artifacts,
      required_evidence_artifacts: $required_evidence_artifacts,
      present_required_evidence_artifacts: $present_required_evidence_artifacts,
      missing_required_evidence_artifacts: $missing_required_evidence_artifacts,
      stale_readiness_artifacts: $stale_readiness_artifacts,
      stale_validation_artifacts: $stale_validation_artifacts,
      stale_required_evidence_artifacts: $stale_required_evidence_artifacts,
      missing_readiness_endpoints: $missing_readiness_endpoints,
      missing_validation_endpoints: $missing_validation_endpoints,
      unresolved_endpoints: $unresolved_endpoints,
      readiness_artifact_count: $readiness_artifact_count,
      validation_artifact_count: $validation_artifact_count,
      required_evidence_artifact_count: $required_evidence_artifact_count,
      missing_evidence_script_count: $missing_evidence_script_count,
      missing_evidence_job_manifest_count: $missing_evidence_job_manifest_count,
      missing_required_flag_count: $missing_required_flag_count,
      missing_readiness_count: $missing_readiness_count,
      missing_validation_count: $missing_validation_count,
      missing_required_evidence_count: $missing_required_evidence_count,
      stale_readiness_count: $stale_readiness_count,
      stale_validation_count: $stale_validation_count,
      stale_required_evidence_count: $stale_required_evidence_count
    }' >>"$requirements_jsonl"
done < <(jq -r '.evidence_requirements[]? | @base64' "$readiness_file")

checklist_json="$AUDIT_DIR/checklist.json"
jq -s \
  --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg base_url "$BASE_URL" \
  --arg source_evidence_dir "$SOURCE_EVIDENCE_DIR" \
  --arg audit_dir "$AUDIT_DIR" \
  --arg controller_env_template "$CONTROLLER_ENV_TEMPLATE" \
  --arg controller_secret_template "$CONTROLLER_SECRET_TEMPLATE" \
  --arg max_evidence_age_hours "$MAX_EVIDENCE_AGE_HOURS" \
  --arg status "$status" \
  --arg objective "$objective" \
  --arg completion_blocked "$completion_blocked" \
  --argjson open_gap_count "$open_gap_count" \
  --argjson requirement_count "$requirement_count" \
  --argjson missing_readiness_endpoint_count "$total_missing_readiness" \
  --argjson missing_validation_endpoint_count "$total_missing_validation" \
  --argjson missing_required_evidence_artifact_count "$total_missing_required_evidence" \
  --argjson stale_readiness_artifact_count "$total_stale_readiness" \
  --argjson stale_validation_artifact_count "$total_stale_validation" \
  --argjson stale_required_evidence_artifact_count "$total_stale_required_evidence" \
  --argjson unresolved_endpoint_count "$total_unresolved" \
  --argjson missing_evidence_script_count "$total_missing_evidence_scripts" \
  --argjson missing_evidence_job_manifest_count "$total_missing_evidence_job_manifests" \
  --argjson missing_required_flag_count "$total_missing_required_flags" \
  '{
    generated_at: $generated_at,
    base_url: $base_url,
    source_evidence_dir: $source_evidence_dir,
    audit_dir: $audit_dir,
    controller_env_template: $controller_env_template,
    controller_secret_template: $controller_secret_template,
    max_evidence_age_hours: ($max_evidence_age_hours | tonumber),
    stage2_status: $status,
    objective: $objective,
    completion_blocked: ($completion_blocked == "true"),
    open_gap_count: $open_gap_count,
    evidence_requirement_count: $requirement_count,
    missing_readiness_endpoint_count: $missing_readiness_endpoint_count,
    missing_validation_endpoint_count: $missing_validation_endpoint_count,
    missing_required_evidence_artifact_count: $missing_required_evidence_artifact_count,
    stale_readiness_artifact_count: $stale_readiness_artifact_count,
    stale_validation_artifact_count: $stale_validation_artifact_count,
    stale_required_evidence_artifact_count: $stale_required_evidence_artifact_count,
    unresolved_endpoint_count: $unresolved_endpoint_count,
    missing_evidence_script_count: $missing_evidence_script_count,
    missing_evidence_job_manifest_count: $missing_evidence_job_manifest_count,
    missing_required_flag_count: $missing_required_flag_count,
    requirements: .
  }' "$requirements_jsonl" >"$checklist_json"

checklist_md="$AUDIT_DIR/checklist.md"
{
  echo "# Stage 2 Completion Evidence Checklist"
  echo
  echo "- generated_at: $(jq -r '.generated_at' "$checklist_json")"
  echo "- base_url: $BASE_URL"
  echo "- source_evidence_dir: $SOURCE_EVIDENCE_DIR"
  echo "- controller_env_template: $CONTROLLER_ENV_TEMPLATE"
  echo "- controller_secret_template: $CONTROLLER_SECRET_TEMPLATE"
  echo "- max_evidence_age_hours: $MAX_EVIDENCE_AGE_HOURS"
  echo "- stage2_status: $status"
  echo "- completion_blocked: $completion_blocked"
  echo "- open_gap_count: $open_gap_count"
  echo "- evidence_requirement_count: $requirement_count"
  echo "- missing_readiness_endpoint_count: $total_missing_readiness"
  echo "- missing_validation_endpoint_count: $total_missing_validation"
  echo "- missing_required_evidence_artifact_count: $total_missing_required_evidence"
  echo "- stale_readiness_artifact_count: $total_stale_readiness"
  echo "- stale_validation_artifact_count: $total_stale_validation"
  echo "- stale_required_evidence_artifact_count: $total_stale_required_evidence"
  echo "- unresolved_endpoint_count: $total_unresolved"
  echo "- missing_evidence_script_count: $total_missing_evidence_scripts"
  echo "- missing_evidence_job_manifest_count: $total_missing_evidence_job_manifests"
  echo "- missing_required_flag_count: $total_missing_required_flags"
  echo
  echo "## Requirements"
  jq -r '
    .requirements[]
    | "### " + .id + "\n"
      + "- title: " + .title + "\n"
      + "- status: " + .status + "\n"
      + "- production_target: " + .production_target + "\n"
      + "- missing_readiness_count: " + (.missing_readiness_count | tostring) + "\n"
      + "- missing_validation_count: " + (.missing_validation_count | tostring) + "\n"
      + "- missing_required_evidence_count: " + (.missing_required_evidence_count | tostring) + "\n"
      + "- stale_readiness_count: " + (.stale_readiness_count | tostring) + "\n"
      + "- stale_validation_count: " + (.stale_validation_count | tostring) + "\n"
      + "- stale_required_evidence_count: " + (.stale_required_evidence_count | tostring) + "\n"
      + "- missing_evidence_script_count: " + (.missing_evidence_script_count | tostring) + "\n"
      + "- missing_evidence_job_manifest_count: " + (.missing_evidence_job_manifest_count | tostring) + "\n"
      + "- missing_required_flag_count: " + (.missing_required_flag_count | tostring) + "\n"
      + "- readiness_artifacts: " + (.readiness_artifact_count | tostring) + "\n"
      + "- validation_artifacts: " + (.validation_artifact_count | tostring) + "\n"
      + "- required_evidence_artifacts: " + (.required_evidence_artifact_count | tostring) + "\n"
      + "- gap: " + .gap + "\n"
      + "- required_evidence:\n"
      + ((.required_evidence // []) | map("  - " + .) | join("\n")) + "\n"
      + "- evidence_scripts:\n"
      + ((.evidence_scripts // []) | map("  - " + .) | join("\n")) + "\n"
      + "- evidence_job_manifests:\n"
      + ((.evidence_job_manifests // []) | map("  - " + .) | join("\n")) + "\n"
      + "- required_flags:\n"
      + ((.required_flags // []) | map("  - " + .) | join("\n")) + "\n"
      + "- missing_readiness_endpoints:\n"
      + (if (.missing_readiness_endpoints | length) == 0 then "  - <none>" else ((.missing_readiness_endpoints // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_validation_endpoints:\n"
      + (if (.missing_validation_endpoints | length) == 0 then "  - <none>" else ((.missing_validation_endpoints // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_required_evidence_artifacts:\n"
      + (if (.missing_required_evidence_artifacts | length) == 0 then "  - <none>" else ((.missing_required_evidence_artifacts // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- stale_readiness_artifacts:\n"
      + (if (.stale_readiness_artifacts | length) == 0 then "  - <none>" else ((.stale_readiness_artifacts // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- stale_validation_artifacts:\n"
      + (if (.stale_validation_artifacts | length) == 0 then "  - <none>" else ((.stale_validation_artifacts // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- stale_required_evidence_artifacts:\n"
      + (if (.stale_required_evidence_artifacts | length) == 0 then "  - <none>" else ((.stale_required_evidence_artifacts // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_evidence_scripts:\n"
      + (if (.missing_evidence_scripts | length) == 0 then "  - <none>" else ((.missing_evidence_scripts // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_evidence_job_manifests:\n"
      + (if (.missing_evidence_job_manifests | length) == 0 then "  - <none>" else ((.missing_evidence_job_manifests // []) | map("  - " + .) | join("\n")) end) + "\n"
      + "- missing_required_flags:\n"
      + (if (.missing_required_flags | length) == 0 then "  - <none>" else ((.missing_required_flags // []) | map("  - " + .) | join("\n")) end) + "\n"
  ' "$checklist_json"
} >"$checklist_md"

cat "$checklist_md"

if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
  echo "Stage 2 completion audit gate failed closed because readiness reports completion_blocked=true." >&2
  exit 1
fi

if [[ "$total_missing_readiness" != "0" || "$total_missing_validation" != "0" || "$total_missing_required_evidence" != "0" || "$total_stale_readiness" != "0" || "$total_stale_validation" != "0" || "$total_stale_required_evidence" != "0" || "$total_unresolved" != "0" || "$total_missing_evidence_scripts" != "0" || "$total_missing_evidence_job_manifests" != "0" || "$total_missing_required_flags" != "0" ]]; then
  if [[ "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 completion audit gate failed closed because required evidence metadata or fresh artifacts are missing." >&2
    exit 1
  fi
fi
